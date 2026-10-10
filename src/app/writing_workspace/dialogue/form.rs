use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    ui: &mut egui::Ui,
    project: &Project,
    buffer: &WritingBuffer,
    view: &mut ViewState,
    typography: Typography,
    action: &mut Action,
    projection: &DialogueProjection,
    key: &Key,
) {
    let Some(mut form) = view.dialogue.forms.remove(key) else {
        return;
    };
    let busy = view.ime_active;
    let viewport = view.dialogue.viewport.unwrap_or(ui.clip_rect());
    let navigation = preview_keyboard::Scope::new(ui, key, viewport, !busy);
    navigation.sync(
        form.plan
            .as_ref()
            .filter(|_| !needs_rebind(&form, projection, &view.dialogue.statement_indices)),
    );
    let previous = form.request.clone();
    let mut closed = false;
    let focused_id = ui
        .ctx()
        .memory(|memory| memory.focused())
        .filter(|id| form.input_ids.contains(id));
    let focused = focused_id.is_some();
    form.batch_owner = if ui.ctx().input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Text(_) | egui::Event::Paste(_)))
    }) {
        focused_id
    } else {
        None
    };
    let next_key = focused
        && ui
            .ctx()
            .input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
    let mut next = None;
    let mut preview_id = None;
    let mut result_requested = false;
    ui.push_id(("dialogue-form", key), |ui| {
        if form.plan.is_some() {
            ui.set_clip_rect(viewport);
        }
        ui.strong(&form.label);
        fields(
            ui,
            &mut form,
            key,
            typography,
            !busy,
            true,
            view.composition_receiver(ui.ctx()),
            action,
        );
        if matches!(project.language_version(), "1.9" | "1.10") {
            theme::add_enabled_ui(ui, !busy, |ui| {
                preview_keyboard::checkbox(
                    ui,
                    &mut form.request.enable_language_1_11,
                    "明确预览启用语言 1.11；确认后与全文草稿一次应用",
                );
            });
        }
        if previous != form.request {
            form.plan = None;
            form.error = None;
            form.migration_confirmed = false;
        }
        if focused {
            view.selection_mode = None;
        }
        if next_key && !busy {
            // Text/Paste 与快捷键可能在同一帧；必须先收齐字段，再判定是否可直接续句。
            if !form.protected() {
                if let DialogueOperation::Update { statement_id, .. } = &form.request.operation {
                    next = projection
                        .statements
                        .iter()
                        .find(|statement| {
                            &statement.id == statement_id && statement.after_anchor_id.is_some()
                        })
                        .cloned();
                }
            }
            if next.is_none() {
                form.continue_after = true;
                match project.preview_dialogue_edit(buffer, &form.request) {
                    Ok(plan) => {
                        form.plan = Some(plan);
                        form.error = None;
                        form.migration_confirmed = false;
                        result_requested = true;
                    }
                    Err(error) => {
                        form.plan = None;
                        form.error = Some(error.to_string());
                    }
                }
            }
        }
        ui.horizontal_wrapped(|ui| {
            let preview = preview_keyboard::add(ui, !busy, theme::primary("预览语句变更"));
            preview_id = Some(preview.id);
            if form.navigation.return_to_preview && !busy && ui.is_enabled() {
                preview.request_focus();
                preview.scroll_to_me(Some(egui::Align::Center));
                form.navigation.return_to_preview = false;
            }
            if preview.clicked() {
                match project.preview_dialogue_edit(buffer, &form.request) {
                    Ok(plan) => {
                        form.plan = Some(plan);
                        form.error = None;
                        form.migration_confirmed = false;
                        result_requested = true;
                    }
                    Err(error) => {
                        form.plan = None;
                        form.error = Some(format!("{}：{}", error.code, error.message));
                    }
                }
            }
            closed |= cancel_button(ui, &mut form, busy);
            if preview_keyboard::button(ui, "复制保留的 typed 输入").clicked() {
                ui.ctx()
                    .copy_text(serde_json::to_string_pretty(&form.request).unwrap_or_default());
            }
        });
        navigation.sync(
            form.plan
                .as_ref()
                .filter(|_| !needs_rebind(&form, projection, &view.dialogue.statement_indices)),
        );
        if let Some(plan) = &form.plan {
            ui.set_clip_rect(viewport);
            preview_keyboard::reading(ui, typography.size * typography.spacing);
            let result = plan_preview(ui, plan, typography);
            if result_requested && ui.is_enabled() && preview_keyboard::available(ui.ctx()) {
                // An explicit preview can be requested from a button near the
                // viewport edge. Reveal its actual result without changing focus.
                ui.scroll_to_rect(result.rect.expand(2.0), None);
            }
            if plan.migration.is_some() {
                theme::add_enabled_ui(ui, !busy, |ui| {
                    preview_keyboard::checkbox(
                        ui,
                        &mut form.migration_confirmed,
                        "已核对全稿解释变化、诊断与完整文件；确认一次应用",
                    );
                });
            }
            if !matches!(form.request.operation, DialogueOperation::Delete { .. }) {
                theme::add_enabled_ui(ui, !busy, |ui| {
                    preview_keyboard::checkbox(
                        ui,
                        &mut form.continue_after,
                        "纳入此句后继续写下一句",
                    );
                });
            }
            let label = match (plan.migration.is_some(), form.continue_after) {
                (true, true) => "确认迁移并写下一句",
                (true, false) => "确认迁移并应用所列完整稿",
                (false, true) => "纳入并写下一句",
                (false, false) => "纳入正文草稿",
            };
            if preview_keyboard::add(
                ui,
                !busy && plan.can_apply && (plan.migration.is_none() || form.migration_confirmed),
                theme::primary(label),
            )
            .clicked()
            {
                action.dialogue_plan = Some(plan.clone());
                action.dialogue_continue = form.continue_after;
            }
        }
        if let Some(error) = &form.error {
            ui.add(egui::Label::new(egui::RichText::new(error).color(theme::ERROR())).wrap());
        }
        if needs_rebind(&form, projection, &view.dialogue.statement_indices) {
            rebind(ui, &mut form, projection, !busy);
        }
        closed |= cancel_confirmation(ui, &mut form, busy);
    });
    navigation.sync(
        form.plan
            .as_ref()
            .filter(|_| !needs_rebind(&form, projection, &view.dialogue.statement_indices)),
    );
    navigation.finish(&mut form.navigation);
    if form.refocus && !busy && !closed {
        focus::restore(ui, &mut form, preview_id);
    }
    if let Some(statement) = next {
        // 原字段收齐后仍先放回；render 末尾统一确认 replacement。
        view.dialogue.forms.insert(key.clone(), form);
        render::next(ui, buffer, &mut view.dialogue, projection, &statement);
    } else if closed {
        if let Some(id) = form.return_focus {
            ui.ctx().memory_mut(|memory| memory.request_focus(id));
        }
    } else {
        view.dialogue.forms.insert(key.clone(), form);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fields(
    ui: &mut egui::Ui,
    form: &mut Form,
    key: &Key,
    typography: Typography,
    actions_enabled: bool,
    editable: bool,
    receiver: Option<egui::Id>,
    action: &mut Action,
) {
    // 请求 clone 只是 UI 瞬态值；不存在第二份可保存台词。
    form.drawn_inputs.clear();
    // 一旦首字段登记，本帧 registry 会替换上一帧集合；须在遍历前保留原接收资格。
    form.ime_owner = receiver
        .or_else(|| {
            ui.ctx().memory(|memory| memory.focused()).filter(|id| {
                form.input_ids.contains(id) && super::super::input_registry::owns_focus(ui.ctx())
            })
        })
        .filter(|id| form.input_ids.contains(id));
    let mut operation = form.request.operation.clone();
    match &mut operation {
        DialogueOperation::Update { draft, .. } | DialogueOperation::Insert { draft, .. } => {
            fields::draft(
                ui,
                form,
                draft,
                key,
                typography,
                actions_enabled,
                editable,
                receiver,
                action,
            );
        }
        DialogueOperation::Convert {
            to,
            speaker,
            allow_direction_loss,
            ..
        } => {
            ui.label(if *to == DialogueKind::Say {
                "转换为正式角色台词：正文、稳定 ID 和注释由 core 保留。"
            } else {
                "转换为普通旁白：正式 speaker 将移除；演出备注损失须单独确认。"
            });
            if *to == DialogueKind::Say {
                fields::speaker(ui, speaker, &form.speakers, actions_enabled, action);
            }
            if form
                .plan
                .as_ref()
                .is_some_and(|plan| !plan.metadata_losses.is_empty())
                || *allow_direction_loss
            {
                theme::add_enabled_ui(ui, actions_enabled, |ui| {
                    preview_keyboard::checkbox(
                        ui,
                        allow_direction_loss,
                        "我确认丢弃预览列出的演出备注，并重新预览转换",
                    );
                });
            }
        }
        DialogueOperation::Delete { .. } => {
            ui.colored_label(
                theme::WARNING(),
                "删除只作用于 core 确认的单一语句。请先核对前后预览；独立和行尾注释保留。",
            );
        }
    }
    form.request.operation = operation;
}
fn plan_preview(
    ui: &mut egui::Ui,
    plan: &DialogueEditPlan,
    typography: Typography,
) -> egui::Response {
    ui.separator();
    let result = ui.strong(if plan.no_change {
        "无内容变化 · 保持原字节和代次"
    } else {
        "正式语句变更预览"
    });
    ui.label(format!(
        "{} · UTF-8 {}..{}",
        plan.source_path.display(),
        plan.range.start,
        plan.range.end
    ));
    ui.label(format!(
        "说话者：{} → {}",
        identity(plan.old_speaker.as_ref()),
        identity(plan.new_speaker.as_ref())
    ));
    for loss in &plan.metadata_losses {
        ui.colored_label(theme::WARNING(), loss);
    }
    if plan.migration.is_some() {
        ui.colored_label(
            theme::WARNING(),
            "此动作将同时启用语言 1.11，并把此文件完整未应用稿应用到工程；一次撤销，磁盘另行保存。",
        );
        preview::migration(ui, plan, typography);
    } else {
        ui.label(theme::muted(
            "只纳入唯一文件正文缓冲；之后另行应用与保存工程。",
        ));
    }
    ui.label("变更前");
    preview::text(
        ui,
        &plan.before,
        ("dialogue-plan-before", &plan.plan_digest),
        typography,
    );
    ui.label("变更后");
    preview::text(
        ui,
        &plan.after,
        ("dialogue-plan-after", &plan.plan_digest),
        typography,
    );
    result
}
fn identity(target: Option<&TargetRef>) -> String {
    target
        .map(|target| format!("{}:{}", target.kind, target.id))
        .unwrap_or_else(|| "无正式角色".into())
}
fn rebind(ui: &mut egui::Ui, form: &mut Form, projection: &DialogueProjection, enabled: bool) {
    ui.colored_label(theme::WARNING(), "来源基线已变化；不会推断原语句的新位置。请明确选择当前接收语句或插入锚，再看完整前后预览。");
    theme::add_enabled_ui(ui, enabled, |ui| {
        let insert = matches!(form.request.operation, DialogueOperation::Insert { .. });
        egui::ComboBox::from_id_salt("dialogue-explicit-rebind")
            .selected_text(form.rebind.as_deref().unwrap_or("明确选择当前位置"))
            .show_ui(ui, |ui| {
                if insert {
                    for anchor in &projection.anchors {
                        ui.selectable_value(
                            &mut form.rebind,
                            Some(anchor.id.clone()),
                            format!("{} · 第{}行", anchor.label, anchor.line),
                        );
                    }
                } else {
                    for statement in &projection.statements {
                        ui.selectable_value(
                            &mut form.rebind,
                            Some(statement.id.clone()),
                            format!(
                                "第{}行 · {:?} · {}",
                                statement.source.line,
                                statement.kind,
                                statement.localization_id.as_deref().unwrap_or("无持久 ID")
                            ),
                        );
                    }
                }
            });
        if ui.button("绑定到所选当前位置并重新核对").clicked() {
            if let Some(id) = form.rebind.take() {
                match &mut form.request.operation {
                    DialogueOperation::Insert { anchor_id, .. } => *anchor_id = id,
                    DialogueOperation::Update { statement_id, .. }
                    | DialogueOperation::Convert { statement_id, .. }
                    | DialogueOperation::Delete { statement_id } => *statement_id = id,
                }
                form.request.expected_baseline = projection.baseline.clone();
                form.request.generation = projection.generation;
                form.speakers = projection.speakers.clone();
                form.plan = None;
                form.error = None;
            }
        }
    });
}

/// 观察变化可以重发不透明锚，而不改变正文 baseline/generation。
pub(super) fn needs_rebind(
    form: &Form,
    projection: &DialogueProjection,
    indices: &BTreeMap<String, usize>,
) -> bool {
    form.request.generation != projection.generation
        || form.request.expected_baseline != projection.baseline
        || match &form.request.operation {
            DialogueOperation::Insert { anchor_id, .. } => !projection
                .anchors
                .iter()
                .any(|anchor| &anchor.id == anchor_id),
            DialogueOperation::Update { statement_id, .. }
            | DialogueOperation::Convert { statement_id, .. }
            | DialogueOperation::Delete { statement_id } => !indices.contains_key(statement_id),
        }
}

// 正常表单和来源不可用救援共用同一取消确认，不提供旁路丢弃。
pub(super) fn cancel_button(ui: &mut egui::Ui, form: &mut Form, busy: bool) -> bool {
    if preview_keyboard::add(ui, !busy, egui::Button::new("取消此句输入")).clicked() {
        if form.protected() {
            form.discard_confirm = true;
        } else {
            return true;
        }
    }
    false
}
pub(super) fn cancel_confirmation(ui: &mut egui::Ui, form: &mut Form, busy: bool) -> bool {
    let mut closed = false;
    if form.discard_confirm {
        ui.label("仅丢弃这份尚未纳入正文的输入；其他文件和章节草稿仍保留。");
        ui.horizontal_wrapped(|ui| {
            if preview_keyboard::add(ui, !busy, egui::Button::new("确认取消此句输入")).clicked()
            {
                closed = true;
            }
            if preview_keyboard::add(ui, !busy, egui::Button::new("继续保留此句")).clicked() {
                form.discard_confirm = false;
            }
        });
    }
    closed
}
