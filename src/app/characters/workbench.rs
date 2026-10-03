use super::*;
use worldline_core::{world_context::WorldContextOptions, TargetRef};

impl WorldeditApp {
    fn refresh_character_focus(&mut self) {
        let Some(id) = self
            .character_editor
            .as_ref()
            .and_then(|e| e.original.clone())
        else {
            self.character_focus.result = None;
            self.character_focus.error = None;
            self.character_focus.cache = None;
            return;
        };
        let key = (self.version, id.clone(), self.character_focus.mentions);
        if self.character_focus.cache.as_ref() == Some(&key) {
            return;
        }
        let changed_target = self
            .character_focus
            .cache
            .as_ref()
            .is_none_or(|(_, old, _)| old != &id);
        self.character_focus.cache = Some(key);
        self.character_focus.result = None;
        self.character_focus.error = None;
        self.character_focus.source_status = None;
        self.character_focus.source_error = None;
        if changed_target {
            self.character_focus.positions.clear();
            self.character_focus.layout_manual = false;
            self.character_focus.auto_fit = true;
            self.character_focus.fit = true;
            self.character_focus.full_page = 0;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let options = WorldContextOptions {
            include_text_mentions: self.character_focus.mentions,
            ..Default::default()
        };
        match snapshot
            .result
            .query_world_context(&TargetRef::new("character", &id), options)
        {
            Ok(mut result) => {
                let recovery = !self.project.recovery_conflicts().is_empty();
                match self.project.conflict_snapshots() {
                    Ok(conflicts) if recovery || !conflicts.is_empty() => {
                        result.mark_source_conflict();
                        self.character_focus.source_status = Some(
                            "当前展示缓冲快照 · 有未解决外部冲突；请先保留并核对两个版本".into(),
                        );
                    }
                    Err(error) => {
                        self.character_focus.source_error = Some(error);
                        self.character_focus.source_status = Some(
                            "无法核对磁盘冲突，当前仅展示缓冲快照；请检查工作区访问与错误详情"
                                .into(),
                        );
                    }
                    _ => {}
                }
                self.character_focus.result = Some(result);
                self.character_focus.fit = true;
            }
            Err(error) => self.character_focus.error = Some(error.to_string()),
        }
    }
    pub(in crate::app) fn characters_tab(&mut self, ctx: &egui::Context) {
        self.refresh_character_focus();
        let width = ctx.available_rect().width();
        let dock_index = state::dock_index(width);
        if dock_index {
            self.character_collection(ctx);
            self.refresh_character_focus();
        }
        if self.character_focus.inspector_open
            && self.character_editor.is_some()
            && state::dock_inspector(width)
        {
            egui::SidePanel::right("character-inspector")
                .default_width(280.0)
                .width_range(260.0..=300.0)
                .frame(theme::panel())
                .show(ctx, |ui| self.character_inspector_content(ui));
        }
        egui::CentralPanel::default()
            .frame(theme::panel().fill(theme::canvas_background()))
            .show(ctx, |ui| {
                let heading = self
                    .character_editor
                    .as_ref()
                    .map(|e| e.draft.display.as_str())
                    .unwrap_or("人物");
                ui.add(egui::Label::new(RichText::new(heading).strong().size(22.0)).truncate())
                    .on_hover_text(heading);
                if let Some(status) = &self.character_focus.source_status {
                    ui.colored_label(WARNING(), status);
                }
                if let Some(error) = &self.character_focus.source_error {
                    ui.collapsing("磁盘核对详情", |ui| {
                        ui.label(error);
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    if !dock_index && ui.button("人物索引").clicked() {
                        self.character_focus.index_open = true;
                        self.character_focus.focus_zone = Some(0);
                    }
                    if ui.add(theme::primary("＋ 新建人物")).clicked() {
                        self.new_character();
                    }
                    if self.character_editor.is_some() && ui.button("编辑档案 / 旧式关系").clicked()
                    {
                        self.character_focus.inspector_open = true;
                        self.character_focus.focus_zone = Some(3);
                    }
                    if ui.button("时间问题与比较").clicked() {
                        self.open_temporal_issues(ctx);
                    }
                    if ui
                        .add_enabled(
                            !self.personal.history.is_empty(),
                            egui::Button::new("返回作者位置"),
                        )
                        .clicked()
                    {
                        self.author_back(ctx);
                    }
                });
                if let Some(error) = &self.character_focus.error {
                    ui.colored_label(ERROR(), "当前人物上下文无法读取，不能据此判断没有关系");
                    ui.collapsing("技术详情", |ui| {
                        ui.label(error);
                    });
                    if ui.button("重新查询").clicked() {
                        self.character_focus.cache = None;
                    }
                }
                self.character_context_toolbar(ui);
                self.character_context_graph(ui);
                if self.character_focus.show_results && !self.character_focus.full {
                    self.character_context_results(ui);
                }
            });
        if !dock_index && self.character_focus.index_open {
            let mut open = true;
            egui::Window::new("人物索引")
                .id(egui::Id::new("character-index-drawer"))
                .open(&mut open)
                .collapsible(false)
                .default_width(310.0)
                .max_height((ctx.screen_rect().height() - 130.0).max(240.0))
                .show(ctx, |ui| self.character_collection_content(ui));
            self.character_focus.index_open &= open;
        }
        if !state::dock_inspector(width)
            && self.character_focus.inspector_open
            && self.character_editor.is_some()
        {
            let mut open = true;
            egui::Window::new("人物档案")
                .id(egui::Id::new("character-details-drawer"))
                .open(&mut open)
                .collapsible(false)
                .default_width(430.0)
                .max_width((ctx.screen_rect().width() - 60.0).min(560.0))
                .max_height((ctx.screen_rect().height() - 110.0).max(240.0))
                .constrain_to(ctx.screen_rect().shrink(12.0))
                .show(ctx, |ui| self.character_inspector_content(ui));
            self.character_focus.inspector_open &= open;
        }
        if let Some(zone) = self.character_focus.focus_zone.take() {
            let id = [
                "character-index-query-input",
                "character-context-canvas",
                "character-context-results-entry",
                "character-name-input",
            ][zone];
            ctx.memory_mut(|m| m.request_focus(egui::Id::new(id)));
        }
    }
    pub(in crate::app) fn character_region_shortcut(&mut self, ctx: &egui::Context) {
        let upper_layer = self
            .command_palette
            .focus_stack
            .last()
            .is_some_and(|(kind, _)| !matches!(*kind, "character-index" | "character-details"));
        if self.tab != Tab::Characters
            || upper_layer
            || self.ime_composing
            || self.command_palette.ime
            || self.command_palette.ime_frame
        {
            return;
        }
        let backward = ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::F6));
        let forward = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F6));
        if forward || backward {
            let zone = (self.character_focus.zone + if backward { 3 } else { 1 }) % 4;
            self.character_focus.zone = zone;
            self.character_focus.focus_zone = Some(zone);
            if zone == 0 {
                self.character_focus.index_open = true;
            }
            if zone == 2 {
                self.character_focus.show_results = true;
            }
            if zone == 3 {
                self.character_focus.inspector_open = true;
            }
        }
    }
}
