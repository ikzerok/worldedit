use super::*;
impl ViewState {
    pub(in crate::app::writing_workspace) fn dialogue_retained_for(&self, path: &Path) -> bool {
        self.dialogue
            .forms
            .iter()
            .any(|(key, form)| key.path == path && form.protected())
    }
    pub(in crate::app::writing_workspace) fn discard_dialogue_for(&mut self, path: &Path) {
        self.dialogue.forms.retain(|key, _| key.path != path);
    }
    pub(in crate::app::writing_workspace) fn dialogue_runtime_drafts(
        &self,
        root: &Path,
    ) -> BTreeMap<String, String> {
        self.dialogue
            .forms
            .iter()
            .filter(|(_, form)| form.protected())
            .map(|(key, form)| {
                (
                    format!(
                        "未插入对白 · {}:{} · {}",
                        key.target.kind,
                        key.target.id,
                        key.path.strip_prefix(root).unwrap_or(&key.path).display()
                    ),
                    serde_json::to_string(&form.request).expect("typed输入可序列化"),
                )
            })
            .collect()
    }
    pub(in crate::app::writing_workspace) fn dialogue_receiver_key(
        &self,
        ctx: &egui::Context,
    ) -> Option<Key> {
        let receiver = self.composition_receiver(ctx)?;
        self.dialogue
            .forms
            .iter()
            .find(|(_, form)| form.input_ids.contains(&receiver))
            .map(|(key, _)| key.clone())
    }
    pub(in crate::app::writing_workspace) fn dialogue_selection_focused(
        &self,
        ctx: &egui::Context,
    ) -> bool {
        ctx.memory(|memory| memory.focused()).is_some_and(|id| {
            self.dialogue
                .forms
                .values()
                .any(|form| form.input_ids.contains(&id))
        })
    }
}
pub(in crate::app::writing_workspace) fn notices(ui: &mut egui::Ui, view: &mut ViewState) {
    notices_except(ui, view, None);
}
pub(super) fn notices_except(ui: &mut egui::Ui, view: &mut ViewState, skip: Option<&Key>) {
    for (key, form) in &view.dialogue.forms {
        if !form.protected() || skip == Some(key) {
            continue;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::muted(format!(
                "保留的对白输入 · {}:{}",
                key.target.kind, key.target.id
            )));
            if ui.small_button("复制这份对白输入").clicked() {
                ui.ctx()
                    .copy_text(serde_json::to_string_pretty(&form.request).unwrap_or_default());
            }
        });
    }
}
pub(in crate::app::writing_workspace) fn rescue(
    ui: &mut egui::Ui,
    view: &mut ViewState,
    typography: Typography,
) {
    let Some(key) = view.dialogue_receiver_key(ui.ctx()) else {
        return;
    };
    let Some(mut form) = view.dialogue.forms.remove(&key) else {
        return;
    };
    let receiver = view.composition_receiver(ui.ctx());
    if receiver.is_some_and(|id| super::super::input_registry::drawn_this_frame(ui.ctx(), id)) {
        view.dialogue.forms.insert(key, form);
        return;
    }
    ui.colored_label(
        theme::WARNING(),
        "来源、权限或入口已变化；请完成对白组合输入。完整字段保留，不写入来源。",
    );
    ui.ctx().input_mut(|input| {
        input.events.retain(|event| {
            !matches!(
                event,
                egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::Cut
                    | egui::Event::Key { .. }
            )
        });
    });
    form.batch_owner = None;
    form::fields(
        ui,
        &mut form,
        &key,
        typography,
        false,
        false,
        receiver,
        &mut Action::default(),
    );
    form.plan = None;
    form.error = Some("输入期间来源或权限变化；请核对新来源，完整字段已保留。".into());
    view.dialogue.forms.insert(key, form);
}

/// 只读恢复原输入；无 Project/Buffer 参数，不能应用、合并或推断新位置。
pub(super) fn unavailable(
    ui: &mut egui::Ui,
    view: &mut ViewState,
    key: &Key,
    typography: Typography,
) -> bool {
    if let Some(form) = view.dialogue.forms.get_mut(key) {
        form.navigation = Default::default();
    }
    if view.dialogue_receiver_key(ui.ctx()).as_ref() == Some(key) {
        // Commit 可没有此前 Preedit；先让原接收者完成，再判断是否产生受保护输入。
        // 下面只有无交互标签，不重复绘制 TextEdit。
        rescue(ui, view, typography);
    }
    if !view.dialogue.forms.get(key).is_some_and(Form::protected) {
        return false;
    }
    let Some(mut form) = view.dialogue.forms.remove(key) else {
        return false;
    };
    let busy = view.input_blocked(ui.ctx());
    let viewport = view.dialogue.viewport.unwrap_or(ui.clip_rect());
    // Only this actually drawn readonly Form owns geometry. The cached plan is
    // deliberately excluded: an unavailable source grants no apply/withdraw right.
    let navigation = preview_keyboard::Scope::new(ui, key, viewport, !busy, None);
    navigation.sync(None);
    let mut closed = false;
    ui.push_id(("dialogue-unavailable-recovery", key), |ui| {
        ui.set_clip_rect(viewport);
        ui.strong("恢复用原输入，含作者私密备注；非台本交付");
        ui.label(format!(
            "{}:{} · 来源暂不可验证；未自动重绑或写回正文",
            key.target.kind, key.target.id
        ));
        let mut reader_response = None;
        let mut return_to_copy = false;
        // In a parent ScrollArea, max_height is only a ceiling on the remaining
        // layout space. Reserve the same bounded height so this nested reader
        // cannot collapse below its real control while the outer view is scrolled.
        let reading_height = (viewport.height() - 80.0)
            .clamp(48.0, 320.0)
            .min(viewport.height().max(0.0));
        let output = egui::ScrollArea::vertical()
            .id_salt("retained-fields")
            .max_height(reading_height)
            .min_scrolled_height(reading_height)
            .animated(false)
            .show_viewport(ui, |ui, relative| {
                use super::super::preview_navigation as shared;
                let domain = shared::viewport(ui, relative);
                // Real inset leaves room for the control's normal focus outline;
                // the reader frame remains bound to the actual complete viewport.
                egui::Frame::NONE.inner_margin(2).show(ui, |ui| {
                    let response = shared::reading(
                        ui,
                        shared::Reader {
                            identity: egui::Id::new(("retained-input-reader", key)),
                            label: "原输入恢复阅读区 · ↑↓滚动",
                            viewport: domain,
                            enabled: !busy
                                && ui.is_enabled()
                                && viewport.width() > 16.0
                                && viewport.height() > 16.0,
                            horizontal: false,
                            vertical: true,
                            step: egui::vec2(0.0, (typography.size * typography.spacing).max(16.0)),
                            escape: true,
                        },
                    );
                    // This nested viewport may still be clipped by its parent on
                    // entry. Centering in that temporary clip can leave the real
                    // control clipped after the parent is revealed. Reveal its
                    // complete frame minimally, only on actual focus acquisition.
                    if response.enabled()
                        && response.has_focus()
                        && response.gained_focus()
                        && shared::available(ui.ctx())
                    {
                        ui.scroll_to_rect(response.rect.expand(2.0), None);
                    }
                    return_to_copy = response.enabled()
                        && response.has_focus()
                        && !busy
                        && shared::available(ui.ctx())
                        && ui.input(|input| input.modifiers == egui::Modifiers::NONE)
                        && ui.input_mut(|input| {
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                        });
                    reader_response = Some(response);
                    recovery_fields(ui, &form.request.operation);
                });
            });
        if let Some(response) = reader_response {
            super::super::preview_navigation::reveal_domain(ui, &response, output.inner_rect);
        }
        ui.horizontal_wrapped(|ui| {
            let copy = preview_keyboard::add(ui, !busy, egui::Button::new("复制这份对白输入"));
            if return_to_copy {
                copy.request_focus();
                ui.scroll_to_rect(copy.rect.expand(2.0), None);
                ui.ctx().request_repaint();
            }
            if copy.clicked() {
                ui.ctx()
                    .copy_text(serde_json::to_string_pretty(&form.request).unwrap_or_default());
            }
            closed |= form::cancel_button(ui, &mut form, busy);
        });
        closed |= form::cancel_confirmation(ui, &mut form, busy);
        ui.label(theme::muted(
            "先核对并恢复可验证的来源，再明确选择当前语句或插入位置；这里不能应用原输入。",
        ));
    });
    if closed {
        if let Some(id) = form.return_focus {
            ui.ctx().memory_mut(|memory| memory.request_focus(id));
        }
    } else {
        view.dialogue.forms.insert(key.clone(), form);
    }
    true
}
fn recovery_fields(ui: &mut egui::Ui, operation: &DialogueOperation) {
    match operation {
        DialogueOperation::Update { draft, .. } | DialogueOperation::Insert { draft, .. } => {
            ui.label(format!("原语句类型：{:?}", draft.kind));
            ui.label(format!(
                "原角色：{}",
                draft
                    .speaker
                    .as_ref()
                    .map(|target| format!("{}:{}", target.kind, target.id))
                    .unwrap_or_else(|| "无正式角色".into())
            ));
            for (index, part) in draft.parts.iter().enumerate() {
                match part {
                    DialoguePart::Literal { text } => {
                        ui.label(format!("文字 {}", index + 1));
                        ui.add(egui::Label::new(text).wrap());
                    }
                    DialoguePart::Expression { source } => {
                        ui.label(format!("表达式 {} · 未求值", index + 1));
                        ui.add(egui::Label::new(source).wrap());
                    }
                    DialoguePart::Link { target, label } => {
                        ui.label(format!(
                            "强链接 {} · {}:{}",
                            index + 1,
                            target.kind,
                            target.id
                        ));
                        ui.add(egui::Label::new(label).wrap());
                    }
                }
            }
            if let Some(direction) = &draft.direction {
                ui.label("演出备注 · 作者私密");
                ui.add(egui::Label::new(direction).wrap());
            }
        }
        DialogueOperation::Convert {
            to,
            speaker,
            allow_direction_loss,
            ..
        } => {
            ui.label(format!(
                "原转换请求：{to:?}；角色：{speaker:?}；允许演出备注损失：{allow_direction_loss}"
            ));
        }
        DialogueOperation::Delete { statement_id } => {
            ui.label(format!("原删除请求 · 不透明语句锚：{statement_id}"));
        }
    }
}
