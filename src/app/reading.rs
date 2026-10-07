//! 完整对象的聚合资料页，只消费核心快照和来源导航。
mod content;
mod context;
pub(super) use context::ContextCache;
mod compact;
mod dock;
use super::catalog::kind_label;
use super::WorldeditApp;
use crate::theme;
use egui::RichText;
use worldline_core::catalog::{Catalog, TargetRef};

pub(super) fn property_label(key: &str) -> &str {
    match key {
        "appearance" => "外貌与识别特征",
        "background" => "背景经历",
        "personality" => "性格与行为习惯",
        "motivation" => "欲望与动机",
        "boundaries" => "底线与恐惧",
        "voice" => "说话方式",
        "dialogue_examples" => "口吻例句",
        _ => key,
    }
}

impl WorldeditApp {
    pub(super) fn close_transient_reading(&mut self) {
        if self.active_reading_panel.is_none() {
            self.reading_target = None;
            self.reading_history.clear();
        }
    }

    /// 所有资料入口共用的地图引用定位；显隐确认由地图视图处理。
    pub(super) fn locate_reference(&mut self, map_id: &str, placement_id: &str) {
        self.locate_reference_from(map_id, placement_id, self.author_location(None));
    }

    pub(super) fn locate_reference_from(
        &mut self,
        map_id: &str,
        placement_id: &str,
        origin: super::personal::Location,
    ) {
        if self.map_navigation_blocked() {
            return;
        }
        if let Err(error) = self.project.verify_review_navigation() {
            self.message = Some(format!("地图来源尚未确认，未离开当前位置：{error}"));
            return;
        }
        let layer_id = self.snapshot.as_ref().and_then(|snapshot| {
            let map = snapshot.map_index.maps.get(map_id)?;
            map.placements
                .get(placement_id)
                .map(|placement| placement.layer_id.clone())
                .or_else(|| {
                    map.scene
                        .as_ref()?
                        .nodes
                        .get(placement_id)
                        .map(|node| node.layer_id.clone())
                })
        });
        let Some(layer_id) = layer_id else {
            self.message = Some("该地图标记已不存在，请刷新资料后重试".into());
            return;
        };
        self.remember_author_location(origin);
        self.map_selection = Some(map_id.into());
        self.map_locate_request = Some(super::maps::LocateRequest {
            map_id: map_id.into(),
            placement_id: placement_id.into(),
            layer_id,
        });
        self.tab = super::Tab::Map;
        self.close_transient_reading();
    }

    pub(super) fn open_reading(&mut self, target: TargetRef) {
        if let Some(id) = self.active_reading_panel {
            self.reading_panels.navigate(id, target);
            return;
        }
        if let Some(previous) = &self.reading_target {
            if *previous != target {
                self.reading_history.push(previous.clone());
                if self.reading_history.len() > 64 {
                    self.reading_history.remove(0);
                }
            }
        } else {
            self.reading_history.clear();
        }
        self.reading_target = Some(target);
        self.alias_input.clear();
    }

    fn return_to_source_edit(&mut self, ctx: &egui::Context) {
        let saved = self.reading_return.take();
        if let Some((path, cursor)) = saved.as_ref() {
            self.active_file = path.clone();
            let id = egui::Id::new(("source", path));
            let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(*cursor),
                )));
            egui::TextEdit::store_state(ctx, id, state);
        }
        self.tab = super::Tab::Edit;
        let domain = if self
            .project
            .authoring_documents
            .contains_key(&self.active_file)
        {
            "authoring-source"
        } else {
            "source"
        };
        ctx.memory_mut(|memory| memory.request_focus(egui::Id::new((domain, &self.active_file))));
    }

    pub(super) fn linked_source(&mut self, ui: &mut egui::Ui, source: &str, file: &str) {
        for (index, parts) in worldline_core::navigation::reading_lines_with_options(
            source,
            file,
            self.project.compile_options(),
        )
        .into_iter()
        .enumerate()
        {
            ui.push_id(index, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (part_index, part) in parts.into_iter().enumerate() {
                        ui.push_id(part_index, |ui| {
                            if let Some(target) = part.target {
                                if ui
                                    .link(
                                        RichText::new(&part.text)
                                            .font(theme::body_font(
                                                self.personal.appearance().body_size,
                                            ))
                                            .color(theme::ACCENT())
                                            .underline(),
                                    )
                                    .on_hover_text("阅读关联对象")
                                    .clicked()
                                {
                                    self.open_reading(target);
                                }
                            } else {
                                self.wiki_inline(
                                    ui,
                                    &part.text,
                                    self.personal.appearance().body_size,
                                );
                            }
                        });
                    }
                });
            });
        }
    }

    fn reading_link(&mut self, ui: &mut egui::Ui, catalog: &Catalog, target: &TargetRef) {
        let display = catalog
            .object(target)
            .map(|o| o.display.as_str())
            .unwrap_or(&target.id);
        if ui
            .link(format!(
                "{} · {} · {}",
                kind_label(&target.kind),
                display,
                target.id
            ))
            .clicked()
        {
            self.open_reading(target.clone());
        }
    }

    pub(super) fn reading_window(&mut self, ctx: &egui::Context) {
        self.transient_reading_window(ctx);
        self.compact_reference_window(ctx);
        if self.personal.settings.references_visible
            && !self.personal.settings.dock_references
            && !self.focus_style()
        {
            self.pinned_reading_windows(ctx);
        }
    }

    fn transient_reading_window(&mut self, ctx: &egui::Context) {
        let Some(target) = self.reading_target.clone() else {
            return;
        };
        let mut open = true;
        let mut keyboard_return = false;
        egui::Window::new("Wiki · 注释索引")
            .id(egui::Id::new("object-reading"))
            .order(egui::Order::Foreground)
            .open(&mut open)
            .default_width(720.0)
            .default_height(660.0)
            .max_width((ctx.screen_rect().width() - 24.0).max(240.0))
            .max_height((ctx.screen_rect().height() - 48.0).max(160.0))
            .constrain_to(ctx.screen_rect().shrink(8.0))
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                if self.reading_return.is_some()
                    && ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    })
                {
                    keyboard_return = true;
                    return;
                }
                if crate::theme::add_enabled(
                    ui,
                    self.reading_panels.ids().len() < super::reading_state::PANEL_LIMIT,
                    egui::Button::new("钉住旁查"),
                )
                .clicked()
                {
                    self.selected_reading_panel = self.reading_panels.pin(target.clone());
                    self.personal.settings.references_visible = true;
                    self.close_transient_reading();
                }
                if self.reading_return.is_some()
                    && ui
                        .button("返回源码编辑")
                        .on_hover_text("Esc 也可返回")
                        .clicked()
                {
                    self.return_to_source_edit(ctx);
                    self.close_transient_reading();
                }
                if !self.reading_history.is_empty() && ui.button("← 返回上一词条").clicked()
                {
                    self.reading_target = self.reading_history.pop();
                    self.alias_input.clear();
                }
                self.reading_content(ui, target);
            });
        if keyboard_return {
            self.return_to_source_edit(ctx);
            self.close_transient_reading();
            return;
        }
        if !open {
            self.close_transient_reading();
        }
    }

    fn pinned_reading_windows(&mut self, ctx: &egui::Context) {
        let ids = self.reading_panels.ids();
        let narrow = ctx.screen_rect().width() < 1300.0;
        if !ids.contains(&self.selected_reading_panel.unwrap_or(u64::MAX)) {
            self.selected_reading_panel = ids.first().copied();
        }
        for (index, id) in ids.iter().copied().enumerate() {
            if narrow && self.selected_reading_panel != Some(id) {
                continue;
            }
            let panel = self.reading_panels.get(id).unwrap();
            let target = panel.target.clone();
            let can_back = !panel.history.is_empty();
            let mut open = true;
            self.active_reading_panel = Some(id);
            egui::Window::new(format!(
                "旁查 {} · {}:{}",
                index + 1,
                target.kind,
                target.id
            ))
            .id(egui::Id::new(("pinned-reading", id)))
            .open(&mut open)
            .default_pos(egui::pos2(40.0 + index as f32 * 440.0, 90.0))
            .default_width(420.0)
            .default_height(460.0)
            .max_width((ctx.screen_rect().width() - 24.0).max(240.0))
            .constrain_to(ctx.screen_rect().shrink(8.0))
            .max_height((ctx.screen_rect().height() - 140.0).max(200.0))
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if narrow {
                        for (other_index, other) in ids.iter().copied().enumerate() {
                            ui.selectable_value(
                                &mut self.selected_reading_panel,
                                Some(other),
                                format!("旁查 {}", other_index + 1),
                            );
                        }
                    }
                    if crate::theme::add_enabled(ui, can_back, egui::Button::new("← 返回"))
                        .clicked()
                    {
                        self.reading_panels.back(id);
                    }
                    if ui.button("关闭旁查").clicked() {
                        self.reading_panels.close(id);
                    }
                    if ui.button("返回源码编辑").clicked() {
                        self.return_to_source_edit(ctx);
                    }
                });
                self.reading_content(ui, target);
            });
            self.active_reading_panel = None;
            if !open {
                self.reading_panels.close(id);
            }
        }
    }
}
