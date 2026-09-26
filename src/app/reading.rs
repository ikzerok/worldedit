//! 完整对象的聚合资料页，只消费核心快照和来源导航。
use super::catalog::kind_label;
use super::WorldeditApp;
use crate::theme;
use egui::RichText;
use worldline_core::ast::PropertyValue;
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
        if self.map_navigation_blocked() {
            return;
        }
        let layer_id = self.snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .map_index
                .maps
                .get(map_id)?
                .placements
                .get(placement_id)
                .map(|placement| placement.layer_id.clone())
        });
        let Some(layer_id) = layer_id else {
            self.message = Some("该地图标记已不存在，请刷新资料后重试".into());
            return;
        };
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
                                            .size(16.0)
                                            .color(theme::ACCENT)
                                            .underline(),
                                    )
                                    .on_hover_text("阅读关联对象")
                                    .clicked()
                                {
                                    self.open_reading(target);
                                }
                            } else {
                                self.wiki_inline(ui, &part.text, 16.0);
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
        self.pinned_reading_windows(ctx);
    }

    fn transient_reading_window(&mut self, ctx: &egui::Context) {
        let Some(target) = self.reading_target.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new("Wiki · 注释索引")
            .id(egui::Id::new("object-reading"))
            .order(egui::Order::Foreground)
            .open(&mut open)
            .default_width(720.0)
            .default_height(660.0)
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                if ui
                    .add_enabled(
                        self.reading_panels.ids().len() < super::reading_state::PANEL_LIMIT,
                        egui::Button::new("钉住旁查"),
                    )
                    .clicked()
                {
                    self.selected_reading_panel = self.reading_panels.pin(target.clone());
                    self.close_transient_reading();
                }
                if !self.reading_history.is_empty() && ui.button("← 返回上一词条").clicked()
                {
                    self.reading_target = self.reading_history.pop();
                    self.alias_input.clear();
                }
                self.reading_content(ui, target);
            });
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
                    if ui
                        .add_enabled(can_back, egui::Button::new("← 返回"))
                        .clicked()
                    {
                        self.reading_panels.back(id);
                    }
                    if ui.button("关闭旁查").clicked() {
                        self.reading_panels.close(id);
                    }
                    if ui.button("返回源码编辑").clicked() {
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
                        ui.memory_mut(|memory| {
                            memory.request_focus(egui::Id::new((domain, &self.active_file)))
                        });
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

    pub(super) fn reading_content(&mut self, ui: &mut egui::Ui, target: TargetRef) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let catalog = snapshot.result.analysis.catalog.clone();
        let character = snapshot
            .result
            .analysis
            .symbols
            .characters
            .get(&target.id)
            .filter(|_| target.kind == "character")
            .cloned();
        let world = snapshot
            .result
            .analysis
            .world
            .clone()
            .filter(|_| target.kind == "world");
        let Some(object) = catalog.object(&target).cloned() else {
            ui.label(format!(
                "资料已失效：{}:{}。可能已被删除或更改 ID。",
                target.kind, target.id
            ));
            return;
        };
        ui.push_id((&target.kind, &target.id), |ui| {
            ui.heading(&object.display);
            ui.label(theme::muted(format!(
                "{} · {}",
                kind_label(&target.kind),
                target.id
            )));
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.active_reading_panel.is_none(),
                        egui::Button::new("编辑此对象"),
                    )
                    .on_hover_text("钉住面板用于只读旁查；从临时阅读页进入编辑")
                    .clicked()
                {
                    if target.kind == "tag"
                        && catalog.tags.get(&target.id).is_some_and(|t| t.declared)
                    {
                        self.edit_wiki_entry(Some(&target.id));
                    } else {
                        self.navigate_object(&object);
                    }
                    self.close_transient_reading();
                }
                if ui.button("在 Wiki 中查看").clicked() {
                    self.wiki_target = Some(target.clone());
                    self.tab = super::Tab::Wiki;
                    self.close_transient_reading();
                }
                if ui.button("查看关联").clicked() {
                    self.open_network(target.clone());
                    self.close_transient_reading();
                }
                if ui.button("定位源文件").clicked() {
                    self.jump_to_file(&object.file, object.line, 1);
                    self.close_transient_reading();
                }
            });
            ui.label(theme::muted(
                "按当前工程内容汇总；表单修改应用后会更新此页。",
            ));
            let placements = self
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.map_index.placements_for(&target))
                .unwrap_or_default();
            ui.collapsing("地图中的位置", |ui| {
                if placements.is_empty() {
                    ui.label(theme::muted("未放置在地图上"));
                }
                for placement in placements {
                    if ui
                        .button(format!(
                            "定位 {} / {}",
                            placement.map_id, placement.placement_id
                        ))
                        .clicked()
                    {
                        self.locate_reference(&placement.map_id, &placement.placement_id);
                    }
                }
            });
            ui.separator();
            ui.label(RichText::new("别名").strong());
            let names = catalog.aliases_for(&target);
            ui.add_enabled_ui(self.active_reading_panel.is_none(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    for name in &names {
                        if ui
                            .button(format!("{name} ×"))
                            .on_hover_text("移除此别名")
                            .clicked()
                        {
                            let remaining: Vec<_> =
                                names.iter().filter(|n| *n != name).cloned().collect();
                            self.commit("别名已移除", |p| p.set_aliases(&target, &remaining));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.alias_input)
                            .hint_text("例如：昵称、旧称或简称"),
                    );
                    if ui
                        .add_enabled(
                            !self.alias_input.trim().is_empty(),
                            egui::Button::new("添加别名"),
                        )
                        .clicked()
                    {
                        let mut updated = names.clone();
                        updated.push(self.alias_input.trim().into());
                        if self.commit("别名已添加", |p| p.set_aliases(&target, &updated)) {
                            self.alias_input.clear();
                        }
                    }
                });
            });
            let tag = catalog
                .tags
                .get(&target.id)
                .filter(|_| target.kind == "tag");
            let entity = catalog
                .entities
                .get(&target.id)
                .filter(|_| target.kind == "entity");
            if let Some(entity) = entity {
                ui.label(theme::muted(format!("分类：{}", entity.entity_type)));
            }
            let relation = catalog
                .relations
                .get(&target.id)
                .filter(|_| target.kind == "relation");
            if let Some(relation) = relation {
                let relation_type = catalog.relation_types.get(&relation.relation_type);
                let display = relation_type
                    .map(|kind| kind.display.as_str())
                    .unwrap_or(&relation.relation_type);
                ui.label(format!("关系类型：{display}"));
                if let Some(kind) = relation_type {
                    ui.label(match kind.direction {
                        worldline_core::RelationDirection::Directed => "方向：有向",
                        worldline_core::RelationDirection::Undirected => "方向：无向",
                    });
                    if let Some(inverse) = &kind.inverse_display {
                        ui.label(theme::muted(format!("反向读取名称：{inverse}")));
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label("起点：");
                    self.reading_link(ui, &catalog, &relation.from_ref);
                    ui.label("终点：");
                    self.reading_link(ui, &catalog, &relation.to_ref);
                });
                if let Some(source) = &relation.source_note {
                    ui.label(RichText::new("来源").strong());
                    self.wiki_text(ui, source);
                }
                if !relation.scope_refs.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("作者范围：");
                        for scope in &relation.scope_refs {
                            self.reading_link(ui, &catalog, scope);
                        }
                    });
                }
            }
            let properties = character
                .as_ref()
                .map(|c| &c.properties)
                .or_else(|| world.as_ref().map(|w| &w.properties))
                .or_else(|| tag.map(|t| &t.properties))
                .or_else(|| entity.map(|entity| &entity.properties))
                .or_else(|| relation.map(|relation| &relation.properties));
            let description = world
                .as_ref()
                .map(|w| w.description.as_str())
                .or_else(|| tag.map(|t| t.description.as_str()))
                .or_else(|| entity.map(|entity| entity.description.as_str()))
                .or_else(|| relation.map(|relation| relation.description.as_str()))
                .or_else(|| {
                    catalog
                        .anchors
                        .get(&target.id)
                        .filter(|_| target.kind == "anchor")
                        .map(|a| a.description.as_str())
                });
            if let Some(description) = description.filter(|s| !s.is_empty()) {
                ui.separator();
                self.wiki_text(ui, description);
            }
            if let Some(properties) = properties {
                for (key, value) in properties {
                    ui.add_space(10.0);
                    ui.label(RichText::new(property_label(key)).strong());
                    let text = match value {
                        PropertyValue::Str(s) => s.clone(),
                        PropertyValue::Num(n) => n.to_string(),
                        PropertyValue::Bool(b) => {
                            if *b {
                                "是".into()
                            } else {
                                "否".into()
                            }
                        }
                    };
                    self.wiki_text(
                        ui,
                        if text.is_empty() {
                            "尚未填写"
                        } else {
                            &text
                        },
                    );
                }
            }
            self.wiki_occurrences(ui, &target);
            if target.kind == "event" || target.kind == "scene" {
                ui.separator();
                ui.label(RichText::new("正文与条件 · 不执行分支").strong());
                if let Ok(source) = self.project.object_source(&object.file, object.line) {
                    self.linked_source(ui, &source, &object.file);
                }
            }
            if let Some(character) = &character {
                ui.separator();
                ui.label(RichText::new("人物关系").strong());
                for relation in &character.relations {
                    ui.horizontal_wrapped(|ui| {
                        self.wiki_inline(ui, &relation.label, 14.0);
                        self.reading_link(
                            ui,
                            &catalog,
                            &TargetRef::new("character", &relation.target),
                        );
                    });
                }
            }
            ui.separator();
            ui.label(RichText::new("关联标签").strong());
            for tag in catalog.tags_for(&target) {
                self.reading_link(ui, &catalog, &TargetRef::new("tag", &tag));
            }
            if target.kind == "tag" {
                ui.collapsing("此标签直接及递归关联的对象", |ui| {
                    for object in catalog.query(&target.id, true) {
                        self.reading_link(ui, &catalog, &object.target);
                    }
                });
            }
            ui.separator();
            ui.label(RichText::new("状态 · 初始定义与源码变更").strong());
            ui.label(theme::muted(
                "以下变化可能属于不同分支，尚未合成为唯一当前状态。",
            ));
            for state in catalog
                .states
                .values()
                .filter(|s| s.target == target || (target.kind == "state" && target.id == s.id))
            {
                ui.collapsing(&state.display, |ui| {
                    self.reading_link(ui, &catalog, &TargetRef::new("state", &state.id));
                    ui.label(format!(
                        "初始标签：{}",
                        if state.tags.is_empty() {
                            "空集合".into()
                        } else {
                            state.tags.join("、")
                        }
                    ));
                    for (index, change) in state.changes.iter().enumerate() {
                        ui.push_id(index, |ui| {
                            ui.separator();
                            self.reading_link(
                                ui,
                                &catalog,
                                &TargetRef::new("event", &change.event),
                            );
                            ui.label(format!(
                                "{} · {}：{}",
                                change.timing,
                                change.kind.label(),
                                if change.tags.is_empty() {
                                    "空集合".into()
                                } else {
                                    change.tags.join("、")
                                }
                            ));
                            for context in &change.contexts {
                                ui.label(format!("条件：{context}"));
                            }
                            if let Some(note) = &change.note {
                                self.wiki_text(ui, note);
                            }
                            if ui
                                .small_button(format!("出处 {}:{}", change.file, change.line))
                                .clicked()
                            {
                                self.jump_to_file(&change.file, change.line, 1);
                                self.close_transient_reading();
                            }
                        });
                    }
                });
            }
            ui.separator();
            ui.label(RichText::new("独立叙事锚点").strong());
            for anchor in catalog.anchors_for(&target) {
                self.reading_link(ui, &catalog, &TargetRef::new("anchor", &anchor.id));
            }
            ui.separator();
            ui.label(RichText::new("关联事件").strong());
            let mut events = std::collections::BTreeSet::new();
            if let Some(character) = &character {
                events.extend(character.events.iter().cloned());
            }
            for reference in catalog.references_to(&target) {
                if reference.source.kind == "event" {
                    events.insert(reference.source.id);
                } else if reference.source.kind == "scene" {
                    if let Some(event) = reference.source.id.split('.').next() {
                        events.insert(event.into());
                    }
                }
            }
            if target.kind == "tag" {
                events.extend(
                    catalog
                        .query(&target.id, true)
                        .into_iter()
                        .filter(|o| o.target.kind == "event")
                        .map(|o| o.target.id),
                );
            }
            for event in events {
                self.reading_link(ui, &catalog, &TargetRef::new("event", &event));
            }
            ui.separator();
            ui.label(RichText::new("引用文件").strong());
            let mut assets = catalog.assets_for(&target);
            if target.kind == "asset" {
                if let Some(asset) = catalog.assets.get(&target.id) {
                    assets.push(asset.clone());
                }
            }
            for asset in assets {
                ui.label(&asset.display);
                ui.label(&asset.path);
                ui.label(theme::muted(&asset.resolved_path));
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            asset.available,
                            egui::Button::new(crate::media::OPEN_REFERENCE_LABEL),
                        )
                        .clicked()
                    {
                        if let Err(error) = crate::media::open_reference(
                            &self.project.root,
                            std::path::Path::new(&asset.resolved_path),
                        ) {
                            self.io_error = Some(error);
                        }
                    }
                    if ui.button("复制路径").clicked() {
                        ui.ctx().copy_text(asset.resolved_path.clone());
                    }
                    if !asset.available {
                        ui.label("文件缺失或格式不可用");
                    }
                });
            }
            ui.collapsing("引用此对象的来源", |ui| {
                for reference in catalog.references_to(&target) {
                    self.reading_link(ui, &catalog, &reference.source);
                    if ui
                        .small_button(format!(
                            "{} · {}:{}",
                            reference.kind, reference.file, reference.line
                        ))
                        .clicked()
                    {
                        self.jump_to_file(&reference.file, reference.line, 1);
                        self.close_transient_reading();
                    }
                }
            });
            ui.collapsing("此对象指向的资料", |ui| {
                for reference in catalog.references.iter().filter(|r| r.source == target) {
                    self.reading_link(ui, &catalog, &reference.target);
                }
            });
        });
    }
}
