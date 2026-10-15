use super::fields::remember_focus;
use super::{fields, Session};
use crate::app::WorldeditApp;
use crate::theme;
use worldline_core::schemas::SchemaAuthoringSource;

impl WorldeditApp {
    pub(in crate::app) fn schema_authoring_window(&mut self, ctx: &egui::Context) {
        let top = self.edit_layer_is_top("schema-authoring") || self.schema_authoring.focus_pending;
        let (local_ime, ime_focus, pending_preedit) = self.schema_authoring.ime_guard(ctx, top);
        let ime = if self.schema_authoring.open && top {
            local_ime || self.ime_composing
        } else {
            // 来源编辑与上层命令仍沿用其既有输入法守卫。
            self.auxiliary_ime_active(ctx)
        };
        if self.schema_authoring.source_return && !self.schema_authoring.open {
            egui::Window::new("资料来源导航")
                .id(egui::Id::new("schema-source-return"))
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("资料字段输入已保留");
                    if crate::theme::add_enabled(ui, !ime, egui::Button::new("返回资料表单"))
                        .clicked()
                    {
                        // 来源导航可能已离开原人物/实体页，只交还给真实绘制的源码输入。
                        let source_id = egui::Id::new(("source", &self.active_file));
                        if self.tab == crate::app::Tab::Edit
                            && ctx.read_response(source_id).is_some_and(|response| {
                                response.enabled() && response.interact_rect.is_positive()
                            })
                        {
                            self.schema_authoring.return_focus = Some(source_id);
                        }
                        self.schema_authoring.resume();
                    }
                });
        }
        if !self.schema_authoring.open {
            // 上一帧 modal 尚会使底层控件 disabled；待底层绘制后再交还一次焦点。
            if let Some(focus) = self.schema_authoring.restore_focus.take() {
                ctx.memory_mut(|memory| memory.request_focus(focus));
            }
            return;
        }
        let mut state = std::mem::take(&mut self.schema_authoring);
        let mut open = true;
        let target = state.active.clone();
        if let Some(target) = &target {
            let replace = state.sessions.get(target).is_none_or(|session| {
                !session.dirty() && session.base.baseline != self.project.content_baseline()
            });
            if replace {
                match self.current_schema_authoring(target) {
                    Ok(projection) => {
                        state
                            .sessions
                            .insert(target.clone(), Session::new(projection));
                        state.error = None;
                    }
                    Err(error) => state.error = Some(error),
                }
            }
        }
        let mut source = None;
        let mut preview = false;
        let mut apply = false;
        let mut binding = false;
        let mut reset = false;
        let mut close = false;
        // 工作表覆盖完整视口；主页面的侧栏/顶栏不能挤掉浮窗标题和可滚动正文。
        let viewport = ctx.screen_rect().shrink(12.0);
        let viewport_changed = state.last_viewport != Some(viewport);
        state.last_viewport = Some(viewport);
        let style = ctx.style();
        let frame = egui::Frame::window(&style);
        let title_height = ctx
            .fonts(|fonts| fonts.row_height(&style.text_styles[&egui::TextStyle::Heading]))
            .max(style.spacing.interact_size.y);
        let margins = frame.total_margin().sum();
        let chrome_height =
            margins.y + title_height + frame.inner_margin.sum().y + frame.stroke.width;
        let content_size = egui::vec2(
            (viewport.width() - margins.x).max(160.0),
            (viewport.height() - chrome_height).max(100.0),
        );
        if let Some(focus) = ime_focus {
            crate::app::object_picker::consume_candidate_ime_keys(ctx);
            ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
                input.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
            });
            ctx.memory_mut(|memory| memory.request_focus(focus));
        }
        let baseline = self.project.content_baseline();
        let other_dirty = target
            .as_ref()
            .is_some_and(|target| self.schema_target_draft_dirty(target));
        egui::Window::new("资料 · 约束与属性")
            .id(egui::Id::new("schema-authoring-form"))
            .open(&mut open).default_size(egui::vec2(820.0, 680.0).min(content_size))
            .min_size(egui::vec2(220.0, 100.0).min(content_size)).max_size(content_size)
            .constrain_to(viewport).resizable(true).vscroll(false).show(ctx, |ui| {
                if top { ui.memory_mut(|memory| memory.set_modal_layer(ui.layer_id())); }
                if state.focus_pending && !ime { ctx.move_to_top(ui.layer_id()); }
                if let Some(error) = &state.error { ui.colored_label(theme::ERROR(), error); }
                let Some(session) = target.as_ref().and_then(|target| state.sessions.get_mut(target)) else {
                    ui.label("投影尚不可用；工程更新后可从原资料入口重试");
                    let cancel = crate::theme::add_enabled(ui, !ime, egui::Button::new("取消（保留输入）"));
                    if state.focus_pending && !ime { cancel.request_focus(); state.focus_pending = false; }
                    close = cancel.clicked();
                    return;
                };
                let current = session.base.baseline == baseline;
                let writable = current && session.projection.writable && !other_dirty;
                let actions = writable && !ime;
                let selection_ready = session.selected_schema == session.projection.binding.as_ref().map(|binding| binding.schema_id.clone());
                ui.add(egui::Label::new(egui::RichText::new(&session.base.object.display).heading()).truncate()).on_hover_text(&session.base.object.display);
                let identity = format!("{}:{}", session.base.target.kind, session.base.target.id);
                ui.add(egui::Label::new(theme::muted(&identity)).truncate()).on_hover_text(identity);
                // 主动作位于内容滚动区外；窄窗与放大字号自动换行。
                ui.horizontal_wrapped(|ui| {
                    let preview_button = crate::theme::add_enabled(ui, actions && selection_ready, egui::Button::new("预览字段修改"));
                    preview = preview_button.clicked();
                    let ready = actions && selection_ready && session.preview.as_ref().is_some_and(|plan| {
                        plan.changed && (!plan.has_errors || session.allow_errors)
                            && (!plan.runtime_changed || session.allow_runtime_change)
                    });
                    let apply_button = crate::theme::add_enabled(ui, ready, theme::primary("应用到工程"));
                    apply = apply_button.clicked();
                    let cancel = crate::theme::add_enabled(ui, !ime, egui::Button::new("取消（保留输入）"));
                    close = cancel.clicked();
                    if state.focus_pending && !ime {
                        if preview_button.enabled() { preview_button.request_focus(); } else { cancel.request_focus(); }
                        state.focus_pending = false;
                    }
                });
                if !current {
                    ui.colored_label(theme::WARNING(), "投影已过期：工程已变化，输入保留；重新载入前不能应用");
                } else if other_dirty {
                    ui.colored_label(theme::WARNING(), "此对象还有另一份未应用资料输入，请先处理；这里的输入保留");
                } else if !session.projection.writable {
                    ui.colored_label(theme::WARNING(), "当前只读：来源或能力检查未通过");
                } else { ui.label(theme::muted("应用只改工程内存；之后仍需保存全部")); }
                if session.preview.as_ref().is_some_and(|plan| plan.runtime_changed) {
                    ui.colored_label(theme::WARNING(), "运行内容指纹改变，已有运行记录需重新验证或重新开始");
                    ui.checkbox(&mut session.allow_runtime_change, "确认应用这次运行指纹变更");
                }
                if session.preview.as_ref().is_some_and(|plan| plan.has_errors) {
                    ui.checkbox(&mut session.allow_errors, "已知仍有错误，允许保留作者稿（运行与发布仍受检查）");
                }
                if let Some(error) = &session.error { ui.colored_label(theme::ERROR(), error); }
                ui.separator();
                let body = egui::ScrollArea::vertical().id_salt(("schema-authoring-body", &target))
                    .auto_shrink([false, false]).show(ui, |ui| {
                        let mut body_focus = None;
                        binding = binding_selector(ui, session, actions, &mut body_focus);
                        if !selection_ready { remember_focus(&mut body_focus, ui.label("请先预览所选绑定，再填写字段")); }
                        for reason in &session.projection.read_only_reasons { remember_focus(&mut body_focus, ui.label(reason)); }
                        if !session.projection.complete {
                            remember_focus(&mut body_focus, ui.colored_label(theme::WARNING(), "投影不完整，不能据此结构化写入"));
                            for reason in &session.projection.incomplete_reasons { remember_focus(&mut body_focus, ui.label(format!("{reason:?}"))); }
                        }
                        diagnostics_summary(ui, session, &mut source, &mut body_focus);
                        if session.diagnostics_stale { remember_focus(&mut body_focus, ui.label(theme::muted("输入已变化；以下诊断为上次检查，重新预览后更新"))); }
                        if session.projection.fields.is_empty() {
                            remember_focus(&mut body_focus, ui.label("未显示约束字段。选择相容的既有 schema 后预览绑定；不会自动填必填值。"));
                        }
                        let (field_source, field_focus) = fields::render(ui, session, writable && selection_ready, !ime);
                        source = field_source.or(source.take());
                        body_focus = field_focus.or(body_focus);
                        if let Some(plan) = &session.preview {
                            remember_focus(&mut body_focus, ui.strong(if plan.changed { "待应用修改" } else { "预览完成：没有源码变化" }));
                            for file in &plan.changed_files { remember_focus(&mut body_focus, ui.label(file.display().to_string())); }
                            remember_focus(&mut body_focus, ui.label(if plan.runtime_changed { "core 判定：运行内容指纹改变" } else { "core 判定：运行内容指纹保持不变" }));
                            let header = egui::CollapsingHeader::new("前后运行身份").show(ui, |ui| {
                                remember_focus(&mut body_focus, ui.label(format!("原指纹：{}", plan.before_fingerprint)));
                                remember_focus(&mut body_focus, ui.label(format!("候选指纹：{}", plan.after_fingerprint)));
                            });
                            remember_focus(&mut body_focus, header.header_response);
                            remember_focus(&mut body_focus, ui.label(format!("候选工程诊断：{} 项", plan.diagnostics.len())));
                            for diagnostic in &plan.diagnostics {
                                remember_focus(&mut body_focus, ui.label(format!("{} · {} · {}", diagnostic.severity.label(), diagnostic.code, diagnostic.message)));
                            }
                        }
                        let header = egui::CollapsingHeader::new("对象来源与未应用输入").show(ui, |ui| {
                            remember_focus(&mut body_focus, ui.label(theme::muted(&session.base.object.file)));
                            if remember_focus(&mut body_focus, ui.link("查看对象源码")).clicked() {
                                source = Some(SchemaAuthoringSource { file: session.base.object.file.clone(), line: session.base.object.line, column: 1 });
                            }
                            remember_focus(&mut body_focus, ui.label("关闭、取消和换对象会保留此工作表。未知属性继续使用原资料表单或源码编辑。"));
                            if remember_focus(&mut body_focus, ui.button("重新载入已应用值…")).clicked() { session.discard_confirm = true; }
                            if session.discard_confirm {
                                remember_focus(&mut body_focus, ui.label("仅丢弃这个对象的约束字段输入，重新读取当前工程；其他对象草稿保留。"));
                                ui.horizontal_wrapped(|ui| {
                                    reset = remember_focus(&mut body_focus, crate::theme::add_enabled(ui, !ime, egui::Button::new("确认重新载入"))).clicked();
                                    if remember_focus(&mut body_focus, ui.button("保留输入")).clicked() { session.discard_confirm = false; }
                                });
                            }
                        });
                        remember_focus(&mut body_focus, header.header_response);
                        if let Some(response) = body_focus {
                            if response.has_focus() && response.layer_id == ui.layer_id()
                                && (response.gained_focus() || viewport_changed) {
                                response.scroll_to_me(None);
                            }
                        }
                    });
                #[cfg(test)]
                { state.body_scroll = Some((body.id, body.inner_rect, body.content_size)); }
                let _ = body;
            });
        if let Some(focus) = ime_focus {
            ctx.memory_mut(|memory| {
                memory.request_focus(focus);
                memory.set_focus_lock_filter(
                    focus,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                );
            });
        }
        if top {
            state.last_focus = ctx.memory(|memory| memory.focused());
            if pending_preedit && state.ime_owner.is_none() {
                // 不预先抢焦点；只在真实 render 后记录本帧已获焦的 TextEdit。
                if let (Some(target), Some(id)) = (
                    state.active.as_ref(),
                    state
                        .last_focus
                        .filter(|id| egui::TextEdit::load_state(ctx, *id).is_some()),
                ) {
                    state.ime_owner = Some((target.clone(), id));
                }
            }
        }
        if let Some(target) = &target {
            if reset {
                match self.current_schema_authoring(target) {
                    Ok(projection) => {
                        state
                            .sessions
                            .insert(target.clone(), Session::new(projection));
                        state.error = None;
                    }
                    Err(error) => state.error = Some(error),
                }
            } else if let Some(session) = state.sessions.get_mut(target) {
                if binding {
                    let mut request = session.request();
                    request.binding = session.binding_request();
                    let result = if request.fields.is_empty() {
                        self.project.preview_schema_authoring(&request)
                    } else {
                        Err("绑定预览只处理绑定；字段输入已保留，请先应用字段或明确重新载入".into())
                    };
                    match result {
                        Ok(plan) => {
                            session.binding = request.binding;
                            session.projection = plan.after.clone();
                            // 此分支已证明没有字段编辑：core 的 bind 只追加 EOF，change/unbind
                            // 原位替换且保留换行，既有属性位置不变。新覆盖的属性可安全保留 after 来源。
                            for field in &mut session.projection.fields {
                                field.property_source = session
                                    .base
                                    .fields
                                    .iter()
                                    .find(|old| old.key == field.key)
                                    .and_then(|old| old.property_source.clone())
                                    .or_else(|| field.property_source.clone());
                                if let Some(schema) =
                                    session.base.compatible_schemas.iter().find(|schema| {
                                        Some(&schema.id) == session.selected_schema.as_ref()
                                    })
                                {
                                    if let Some(declaration) =
                                        schema.fields.iter().find(|item| item.id == field.id)
                                    {
                                        field.constraint_source = SchemaAuthoringSource {
                                            file: schema.file.clone(),
                                            line: declaration.loc.line,
                                            column: declaration.loc.column,
                                        };
                                    }
                                }
                            }
                            session.values = plan
                                .after
                                .fields
                                .iter()
                                .map(|field| (field.id.clone(), field.value.clone()))
                                .collect();
                            session.preview = Some(plan);
                            session.diagnostics_stale = false;
                            session.allow_errors = false;
                            session.allow_runtime_change = false;
                            session.error = None;
                        }
                        Err(error) => session.error = Some(error),
                    }
                } else if preview {
                    match self.project.preview_schema_authoring(&session.request()) {
                        Ok(plan) => {
                            for field in &mut session.projection.fields {
                                if let Some(after) =
                                    plan.after.fields.iter().find(|after| after.id == field.id)
                                {
                                    field.diagnostics = after.diagnostics.clone();
                                }
                            }
                            session.preview = Some(plan);
                            session.diagnostics_stale = false;
                            session.allow_errors = false;
                            session.allow_runtime_change = false;
                            session.error = None;
                        }
                        Err(error) => {
                            session.preview = None;
                            session.error = Some(error);
                        }
                    }
                } else if apply {
                    if let Some(plan) = &session.preview {
                        let before = self.project.clone();
                        match self
                            .project
                            .apply_schema_authoring(&session.request(), &plan.plan_digest)
                        {
                            Ok(_) => {
                                self.remember(before);
                                self.recompile();
                                self.refresh_schema_target_editor(target);
                                match self.current_schema_authoring(target) {
                                    Ok(projection) => *session = Session::new(projection),
                                    Err(error) => session.error = Some(error),
                                }
                                self.message =
                                    Some("约束与字段已一次应用，可撤销；尚需保存全部".into());
                            }
                            Err(error) => session.error = Some(error),
                        }
                    }
                }
            }
        }
        if ime {
            open = true;
            close = false;
            source = None;
        }
        state.open = open && !close;
        if !state.open {
            state.close(ctx);
        }
        if let Some(source) = source {
            // 过期来源不猜测新位置，保留所有输入。
            let current = target
                .as_ref()
                .and_then(|target| state.sessions.get(target))
                .is_some_and(|s| s.base.baseline == self.project.content_baseline());
            if current {
                self.jump_to_file(&source.file, source.line, source.column);
                state.open = false;
                state.source_return = true;
            } else {
                state.error = Some("来源已过期；请先重新载入或保留输入后核对源码".into());
            }
        }
        self.schema_authoring = state;
    }
}
fn binding_selector(
    ui: &mut egui::Ui,
    session: &mut Session,
    writable: bool,
    focused: &mut Option<egui::Response>,
) -> bool {
    remember_focus(focused, ui.strong("持续资料约束"));
    let mut preview = false;
    let can_change = writable && session.field_changes().is_empty();
    ui.add_enabled_ui(can_change, |ui| {
        let old = session.selected_schema.clone();
        let choice = egui::ComboBox::from_id_salt("schema-binding-choice")
            .width(ui.available_width().min(380.0))
            .truncate()
            .selected_text(session.selected_schema.as_deref().unwrap_or("未绑定"))
            .show_ui(ui, |ui| {
                let mut chosen = ui
                    .selectable_value(&mut session.selected_schema, None, "不绑定（保留已有属性）")
                    .clicked();
                for schema in &session.base.compatible_schemas {
                    chosen |= ui
                        .selectable_value(
                            &mut session.selected_schema,
                            Some(schema.id.clone()),
                            &schema.id,
                        )
                        .clicked();
                }
                if chosen {
                    ui.close();
                }
                chosen
            });
        if choice.inner == Some(true) {
            choice.response.request_focus();
        }
        remember_focus(focused, choice.response);
        if old != session.selected_schema {
            session.invalidate_preview();
        }
        preview = remember_focus(focused, ui.button("预览所选绑定")).clicked();
    });
    if !session.field_changes().is_empty() {
        remember_focus(
            focused,
            ui.label(theme::muted(
                "已有字段输入；应用或明确重新载入后，再切换绑定",
            )),
        );
    }
    if let Some(schema) = &session.projection.schema {
        remember_focus(
            focused,
            ui.label(theme::muted(format!(
                "当前填写：{} · {}",
                schema.id,
                if schema.closed {
                    "封闭字段集"
                } else {
                    "开放字段集"
                }
            ))),
        );
    }
    ui.separator();
    preview
}
fn diagnostics_summary(
    ui: &mut egui::Ui,
    session: &mut Session,
    source: &mut Option<SchemaAuthoringSource>,
    focused: &mut Option<egui::Response>,
) {
    let diagnostics = session
        .preview
        .as_ref()
        .map(|plan| &plan.after.diagnostics)
        .unwrap_or(&session.projection.diagnostics);
    ui.horizontal_wrapped(|ui| {
        remember_focus(
            focused,
            ui.label(format!(
                "{}项对象诊断 · {}",
                diagnostics.len(),
                if session.diagnostics_stale {
                    "上次检查（待重新预览）"
                } else if session.preview.is_some() {
                    "本次预览"
                } else {
                    "已应用工程"
                }
            )),
        );
        let problems: Vec<_> = session
            .projection
            .fields
            .iter()
            .filter(|field| !field.diagnostics.is_empty())
            .map(|field| field.id.clone())
            .collect();
        if !problems.is_empty() && remember_focus(focused, ui.button("下一个字段问题")).clicked()
        {
            session.scroll_field = Some(problems[session.next_problem % problems.len()].clone());
            session.next_problem += 1;
        }
    });
    let header = egui::CollapsingHeader::new("对象诊断与关联来源").show(ui, |ui| {
        if !diagnostic_sources_current(session) {
            remember_focus(
                focused,
                ui.label(theme::muted(
                    "候选位置，应用后可定位；字段的原始来源仍可查看",
                )),
            );
        }
        for diagnostic in diagnostics {
            remember_focus(
                focused,
                ui.label(format!(
                    "{} · {} · {}",
                    diagnostic.severity.label(),
                    diagnostic.code,
                    diagnostic.message
                )),
            );
            if remember_focus(
                focused,
                crate::theme::add_enabled(
                    ui,
                    diagnostic_sources_current(session),
                    egui::Button::new(format!("来源 {}:{}", diagnostic.file, diagnostic.span.line)),
                ),
            )
            .clicked()
            {
                *source = Some(SchemaAuthoringSource {
                    file: diagnostic.file.clone(),
                    line: diagnostic.span.line,
                    column: diagnostic.span.column,
                });
            }
        }
    });
    remember_focus(focused, header.header_response);
}

pub(super) fn diagnostic_sources_current(session: &Session) -> bool {
    session
        .preview
        .as_ref()
        .is_none_or(|preview| !preview.changed)
        && matches!(
            session.binding,
            worldline_core::schemas::SchemaBindingMutation::Keep
        )
}
