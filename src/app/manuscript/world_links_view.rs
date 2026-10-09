use super::world_links::{Kind, State};
use super::world_links_layout::{FocusReveal, WindowLayout};
use crate::app::{object_picker, WorldeditApp};
use crate::theme;

impl WorldeditApp {
    pub(super) fn draw_manuscript_world_links(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.manuscript.world_links.take() else {
            return;
        };
        if !state.open {
            self.manuscript.world_links = Some(state);
            return;
        }
        self.refresh_world_link_catalog(&mut state);
        let blocked = self.world_links_input_blocked(ctx);
        let focus = FocusReveal::for_frame(ctx, blocked);
        let layout = WindowLayout::new(ctx);
        let mut open = true;
        let mut close = false;
        let mut refresh_selection = false;
        let mut preview = false;
        let mut apply = false;
        let mut peek = false;
        let mut clear = false;
        if !blocked
            && self
                .command_palette
                .focus_stack
                .last()
                .is_none_or(|(layer, _)| *layer == "world-links")
            && !egui::Popup::is_any_open(ctx)
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            close = true;
        }
        egui::Window::new("关联世界资料")
            .id(egui::Id::new("manuscript-world-links"))
            .open(&mut open)
            .default_width(560.0)
            .frame(layout.frame)
            .max_width(layout.max_inner.x)
            .max_height(layout.max_inner.y)
            .constrain_to(ctx.screen_rect().shrink(8.0))
            .collapsible(false)
            .show(ctx, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.add(
                    egui::Label::new(egui::RichText::new(&state.selection.expected_text).strong())
                        .truncate(),
                )
                .on_hover_text(&state.selection.expected_text);
                if !layout.compact {
                    draw_origin(ui, &self.project.root, &state);
                    ui.horizontal_wrapped(|ui| {
                        close |= return_button(ui, blocked).clicked();
                        refresh_selection |= crate::theme::add_enabled(
                            ui,
                            !blocked,
                            egui::Button::new("改用当前选区"),
                        )
                        .clicked();
                    });
                    ui.separator();
                }
                ui.horizontal_wrapped(|ui| {
                    if layout.compact {
                        close |= return_button(ui, blocked).clicked();
                    }
                    preview =
                        crate::theme::add_enabled(ui, !blocked, egui::Button::new("预览关联计划"))
                            .clicked();
                    if let Some(plan) = &state.plan {
                        let label = if plan.creates_object() {
                            "应用这组关联草稿"
                        } else {
                            "插入引用到正文草稿"
                        };
                        apply = crate::theme::add_enabled(
                            ui,
                            !blocked && plan.can_apply,
                            theme::primary(label),
                        )
                        .clicked();
                    }
                    if layout.compact {
                        ui.menu_button("更多", |ui| {
                            refresh_selection |= crate::theme::add_enabled(
                                ui,
                                !blocked,
                                egui::Button::new("改用当前选区"),
                            )
                            .clicked();
                            if refresh_selection {
                                ui.close();
                            }
                            clear |= clear_input(ui, state.touched, blocked);
                        });
                    } else {
                        clear |= clear_input(ui, state.touched, blocked);
                    }
                });
                if blocked {
                    ui.label(theme::muted("输入法组合中 · 确认与关闭暂不可用"));
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("world-link-form")
                    .max_height(super::world_links_layout::scroll_height(ui))
                    .min_scrolled_height(0.0)
                    .animated(false)
                    .show(ui, |ui| {
                        if layout.compact {
                            let origin = egui::CollapsingHeader::new("固定选区与来源")
                                .id_salt("world-link-origin")
                                .show(ui, |ui| draw_origin(ui, &self.project.root, &state));
                            focus.reveal(ui, &origin.header_response);
                        }
                        let kind_before = state.kind;
                        crate::theme::add_enabled_ui(ui, !blocked, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                let response = ui.selectable_value(
                                    &mut state.kind,
                                    Kind::Existing,
                                    "关联已有资料",
                                );
                                focus.reveal(ui, &response);
                                let response = ui.selectable_value(
                                    &mut state.kind,
                                    Kind::Character,
                                    "新建人物",
                                );
                                focus.reveal(ui, &response);
                                let response = ui.selectable_value(
                                    &mut state.kind,
                                    Kind::Entity,
                                    "新建实体资料",
                                );
                                focus.reveal(ui, &response);
                            });
                        });
                        if kind_before != state.kind {
                            state.plan = None;
                            state.touched |= state.kind != Kind::Existing;
                        }
                        match state.kind {
                            Kind::Existing => {
                                peek = draw_existing(
                                    ui,
                                    &self.project.root,
                                    &mut state,
                                    blocked,
                                    focus,
                                );
                            }
                            _ => {
                                self.draw_new_world_link(ui, &mut state, blocked, focus);
                            }
                        }
                        let focus_result = std::mem::take(&mut state.focus_result);
                        if let Some(error) = &state.error {
                            let response = ui.colored_label(theme::ERROR(), error);
                            if focus_result {
                                ui.scroll_to_rect(response.rect, Some(egui::Align::Min));
                            }
                        }
                        if let Some(plan) = &state.plan {
                            ui.separator();
                            super::world_links_plan::draw(
                                ui,
                                &self.project.root,
                                plan,
                                focus_result,
                                focus,
                            );
                        }
                    });
            });
        if refresh_selection {
            match self.capture_world_link_selection(ctx) {
                Ok((source, selection, generation, baseline, origin)) => {
                    state.source = source;
                    state.selection = selection;
                    state.generation = generation;
                    state.baseline = baseline;
                    state.origin = origin;
                    state.plan = None;
                    state.error = None;
                }
                Err(error) => state.error = Some(error),
            }
        }
        if preview {
            self.preview_manuscript_world_link(&mut state);
        }
        if apply && self.apply_manuscript_world_link(ctx, &mut state) {
            return;
        }
        if peek {
            if let Some(target) = &state.chosen {
                if self
                    .project
                    .compile_object_search_snapshot()
                    .analysis
                    .catalog
                    .object(target)
                    .is_some()
                {
                    state.previous_reading = self.reading_target.clone();
                    self.open_reading(target.clone());
                    state.peek = Some(target.clone());
                    state.open = false;
                } else {
                    state.error = Some("此资料尚在未应用稿中；请先预览关联，不能旁查旧对象".into());
                }
            }
        }
        if (close || !open) && !blocked {
            state.open = false;
            self.return_world_link_origin(ctx, &state);
        }
        if clear {
            self.return_world_link_origin(ctx, &state);
            return;
        }
        self.manuscript.world_links = Some(state);
    }

    fn refresh_world_link_catalog(&self, state: &mut State) {
        let source = self.manuscript.writing_buffers.get(&state.selection.path);
        let basis = format!(
            "{}:{}:{:?}",
            self.project.content_baseline(),
            self.version,
            source.map(|buffer| (
                buffer.generation(),
                crate::app::writing_workspace::fingerprint(buffer.source())
            ))
        );
        if state.catalog_basis == basis {
            return;
        }
        if !state.catalog_basis.is_empty() {
            state.plan = None;
            state.error = Some("当前稿或资料已变化；请核对选区并重新预览".into());
        }
        state.catalog_basis = basis;
        let result = source
            .ok_or_else(|| "原正文缓冲已关闭".to_string())
            .and_then(|buffer| {
                self.project
                    .compile_writing_drafts(std::slice::from_ref(buffer))
            });
        match result {
            Ok(content) => {
                state.catalog = Some(content.analysis.catalog);
                state.catalog_warning = None;
            }
            Err(error) => {
                state.catalog = Some(
                    self.project
                        .compile_object_search_snapshot()
                        .analysis
                        .catalog,
                );
                state.catalog_warning =
                    Some(format!("当前稿暂不可解析，以下为已应用资料目录：{error}"));
            }
        }
    }

    fn draw_new_world_link(
        &self,
        ui: &mut egui::Ui,
        state: &mut State,
        blocked: bool,
        focus: FocusReveal,
    ) {
        ui.add_space(theme::SPACE_SM);
        ui.label(if state.kind == Kind::Character {
            "创建正式人物 character"
        } else {
            "地点、组织、物品等通用 entity 资料"
        });
        let mut changed = false;
        for (label, value) in [("稳定 ID", &mut state.id), ("显示名称", &mut state.display)] {
            ui.label(label);
            let response = ui.add(
                egui::TextEdit::singleline(value).id(egui::Id::new(("world-link-field", label))),
            );
            changed |= response.changed();
            super::navigation_input::register(&response);
            focus.reveal(ui, &response);
        }
        if state.kind == Kind::Entity {
            ui.label("实体分类（如 place / organization / item）");
            let response = ui.add(
                egui::TextEdit::singleline(&mut state.entity_type)
                    .id(egui::Id::new(("world-link-field", "实体分类"))),
            );
            changed |= response.changed();
            super::navigation_input::register(&response);
            focus.reveal(ui, &response);
            ui.label("资料说明");
            let response = ui.add(
                egui::TextEdit::multiline(&mut state.description)
                    .id(egui::Id::new(("world-link-field", "资料说明")))
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            changed |= response.changed();
            super::navigation_input::register(&response);
            focus.reveal(ui, &response);
        }
        crate::theme::add_enabled_ui(ui, !blocked, |ui| {
            ui.label("资料写入文件");
            let before = state.destination.clone();
            let destination = egui::ComboBox::from_id_salt("world-link-destination")
                .selected_text(theme::relative_source(
                    &self.project.root,
                    &state.destination,
                ))
                .width(ui.available_width().min(360.0))
                .show_ui(ui, |ui| {
                    for path in self.project.sources().keys() {
                        let response = ui.selectable_value(
                            &mut state.destination,
                            path.clone(),
                            theme::relative_source(&self.project.root, path),
                        );
                        focus.reveal(ui, &response);
                    }
                });
            focus.reveal(ui, &destination.response);
            changed |= before != state.destination;
            if state.kind == Kind::Entity
                && !self.project.language_version_kind().supports_entities()
            {
                ui.colored_label(theme::WARNING(), "此工程为1.9，实体需要显式语言迁移。");
                let response = ui.checkbox(
                    &mut state.enable_entities,
                    "预览启用语言1.10（最终与关联一起确认）",
                );
                changed |= response.changed();
                focus.reveal(ui, &response);
            }
        });
        if changed {
            state.touched = true;
            state.plan = None;
        }
    }
}

fn draw_existing(
    ui: &mut egui::Ui,
    root: &std::path::Path,
    state: &mut State,
    blocked: bool,
    focus: FocusReveal,
) -> bool {
    ui.label("按名称、稳定 ID、别名或来源检索");
    let response =
        ui.add(egui::TextEdit::singleline(&mut state.query).id(egui::Id::new("world-link-query")));
    super::navigation_input::register(&response);
    focus.reveal(ui, &response);
    if let Some(warning) = &state.catalog_warning {
        ui.colored_label(theme::WARNING(), warning);
    }
    if let Some(catalog) = &state.catalog {
        state.page.refresh(
            catalog,
            &state.query,
            &object_picker::filter(&[], None),
            &state.catalog_basis,
        );
    }
    crate::theme::add_enabled_ui(ui, !blocked, |ui| {
        state.page.controls(ui);
        if let Some(Ok(page)) = state.page.result.clone() {
            egui::ScrollArea::vertical()
                .id_salt("world-link-candidates")
                .max_height(180.0)
                .show(ui, |ui| {
                    for object in page.items {
                        let response = object_picker::candidate_row_at_revision(
                            ui,
                            &object,
                            Some(root),
                            state.chosen.as_ref() == Some(&object.target),
                            state.page.serial,
                        );
                        focus.reveal(ui, &response);
                        if response.clicked() {
                            state.chosen = Some(object.target);
                            state.plan = None;
                        }
                    }
                });
        }
    });
    let mut peek = false;
    if let Some(target) = &state.chosen {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("已选择 {}:{}", target.kind, target.id)).strong());
            let response = crate::theme::add_enabled(ui, !blocked, egui::Button::new("旁查资料"));
            focus.reveal(ui, &response);
            peek = response.clicked();
        });
    }
    peek
}

fn draw_origin(ui: &mut egui::Ui, root: &std::path::Path, state: &State) {
    theme::source_caption(ui, root, &state.selection.path);
    ui.label(theme::muted(format!(
        "来源 {}:{} · 选区已固定",
        state.source.kind, state.source.id
    )));
}

fn return_button(ui: &mut egui::Ui, blocked: bool) -> egui::Response {
    crate::theme::add_enabled(ui, !blocked, egui::Button::new("返回正文，保留输入"))
}

fn clear_input(ui: &mut egui::Ui, touched: bool, blocked: bool) -> bool {
    let mut clear = false;
    if touched {
        ui.menu_button("清除输入…", |ui| {
            ui.label("只清除尚未应用的新建资料输入，正文不改。");
            clear = crate::theme::add_enabled(ui, !blocked, egui::Button::new("确认清除关联输入"))
                .clicked();
        });
    }
    clear
}
