//! 标签指针、素材引用和对象之间的来源导航。
use super::inspector::{field, properties};
use super::{Tab, WorldeditApp};
use crate::theme::{self, *};
use egui::RichText;
use worldline_core::authoring::WorldDraft;
use worldline_core::catalog::{CatalogObject, TargetRef};
use worldline_core::catalog_edit::AssetDraft;

pub(super) fn kind_label(kind: &str) -> &str {
    match kind {
        "state" => "状态",
        "anchor" => "锚点",
        "event" => "事件",
        "scene" => "场景",
        "character" => "人物",
        "entity" => "实体",
        "relation" => "关系",
        "world" => "世界观",
        "storyline" => "故事线",
        "period" => "时段",
        "variable" => "变量",
        "tag" => "标签",
        "asset" => "素材",
        "file" => "文件",
        _ => kind,
    }
}

impl WorldeditApp {
    pub(super) fn navigate_object(&mut self, object: &CatalogObject) {
        match object.target.kind.as_str() {
            "event" => {
                self.select_event(&object.target.id);
                self.tab = Tab::Timeline;
            }
            "character" => {
                self.select_character(&object.target.id);
                self.tab = Tab::Characters;
            }
            "world" => self.tab = Tab::World,
            "entity" => self.edit_entity(Some(&object.target.id)),
            "relation" => self.edit_relation(Some(&object.target.id), None),
            "tag" | "asset" | "state" | "anchor" => {
                self.catalog_target = Some(object.target.clone());
                self.tag_editor = None;
                self.state_editor = None;
                self.anchor_editor = None;
                self.tab = Tab::Catalog;
            }
            _ => self.jump_to_file(&object.file, object.line, 1),
        }
    }

    pub(super) fn object_links(&mut self, ui: &mut egui::Ui, target: &TargetRef) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let catalog = snapshot.result.analysis.catalog.clone();
        let impact = worldline_core::reference_impact::deletion_impact_with_views(
            &snapshot.result,
            &snapshot.map_index,
            &snapshot.graph_index,
            target,
        );
        let map_references = impact
            .map_placements
            .into_iter()
            .map(|reference| (reference, "标记"))
            .chain(
                impact
                    .map_scopes
                    .into_iter()
                    .map(|reference| (reference, "作用域")),
            )
            .map(|(reference, kind)| {
                let title = snapshot
                    .map_index
                    .map(&reference.map_id)
                    .map(|map| map.title.clone())
                    .unwrap_or_else(|| reference.map_id.clone());
                (reference, title, kind)
            })
            .collect::<Vec<_>>();
        if matches!(target.kind.as_str(), "entity" | "relation") {
            ui.horizontal_wrapped(|ui| {
                if ui.button("编辑这份资料").clicked() {
                    if let Some(object) = catalog.object(target) {
                        self.navigate_object(object);
                    }
                }
                if ui.button("更改稳定 ID…").clicked() {
                    self.plan_target_rename(target.clone());
                }
            });
        }
        if ui.button("阅读完整资料 / 管理别名").clicked() {
            self.open_reading(target.clone());
        }
        ui.separator();
        ui.label(
            RichText::new(if target.kind == "tag" {
                "给此标签添加标签与引用文件"
            } else {
                "标签与引用文件"
            })
            .strong(),
        );
        ui.horizontal_wrapped(|ui| {
            for state in catalog.states.values().filter(|s| &s.target == target) {
                if ui.button(format!("状态 · {}", state.display)).clicked() {
                    self.catalog_target = Some(TargetRef::new("state", &state.id));
                    self.tag_editor = None;
                    self.state_editor = None;
                    self.anchor_editor = None;
                    self.tab = Tab::Catalog;
                }
            }
            if ui.small_button("＋ 为对象添加状态").clicked() {
                self.new_state(target.clone());
            }
        });
        for anchor in catalog.anchors_for(target) {
            if ui
                .button(format!("叙事锚点 · {}", anchor.display))
                .clicked()
            {
                self.catalog_target = Some(TargetRef::new("anchor", &anchor.id));
                self.tag_editor = None;
                self.state_editor = None;
                self.anchor_editor = None;
                self.tab = Tab::Catalog;
            }
        }
        let tags = catalog.tags_for(target);
        let mut changed_tags = None;
        ui.horizontal_wrapped(|ui| {
            for id in &tags {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let display = catalog
                            .tags
                            .get(id)
                            .map(|t| t.display.as_str())
                            .unwrap_or(id);
                        if ui
                            .small_button(RichText::new(format!("# {display}")).color(ACCENT))
                            .clicked()
                        {
                            self.catalog_target = Some(TargetRef::new("tag", id));
                            self.tab = Tab::Catalog;
                            self.tag_editor = None;
                            self.state_editor = None;
                            self.anchor_editor = None;
                        }
                        let inline = catalog
                            .marks
                            .iter()
                            .any(|m| m.inline && &m.target == target && m.values.contains(id));
                        if ui
                            .add_enabled(!inline, egui::Button::new("×").small())
                            .on_hover_text("移除此标签的直接引用")
                            .clicked()
                        {
                            changed_tags = Some(
                                tags.iter()
                                    .filter(|t| *t != id)
                                    .cloned()
                                    .collect::<Vec<_>>(),
                            );
                        }
                    });
                });
            }
            ui.menu_button("搜索 / 多选标签", |ui| {
                ui.set_min_width(240.0);
                let mut values = tags.clone();
                let locked: Vec<_> = catalog
                    .marks
                    .iter()
                    .filter(|m| m.inline && &m.target == target)
                    .flat_map(|m| m.values.clone())
                    .collect();
                if super::tags::picker(ui, &catalog, &mut values, &locked) {
                    changed_tags = Some(values);
                }
            });
        });
        if let Some(mut values) = changed_tags {
            values.retain(|id| catalog.tags.get(id).is_some_and(|t| t.declared));
            self.commit("标签引用已更新", |p| {
                p.set_catalog_links(target, &values, false)
            });
        }
        ui.push_id(("quick-tag", &target.kind, &target.id), |ui| {
            egui::CollapsingHeader::new("＋ 创建并添加标签").show(ui, |ui| {
                let key = ui.id().with("new-tag-name");
                let mut name = ui
                    .data_mut(|d| d.get_temp::<String>(key))
                    .unwrap_or_default();
                ui.add(
                    egui::TextEdit::singleline(&mut name)
                        .hint_text("标签名称，例如：伏笔")
                        .desired_width(f32::INFINITY),
                );
                if ui
                    .add_enabled(!name.trim().is_empty(), egui::Button::new("创建并添加"))
                    .clicked()
                {
                    let display = name.trim();
                    let existing = catalog
                        .tags
                        .values()
                        .find(|t| t.declared && (t.display == display || t.id == display));
                    let mut i = 1;
                    while catalog.tags.contains_key(&format!("tag_{i}")) {
                        i += 1;
                    }
                    let id = existing
                        .map(|t| t.id.clone())
                        .unwrap_or_else(|| format!("tag_{i}"));
                    let draft = WorldDraft {
                        id: id.clone(),
                        display: display.into(),
                        ..Default::default()
                    };
                    let mut values = tags.clone();
                    if !values.contains(&id) {
                        values.push(id);
                    }
                    if self.commit("标签已添加", |p| {
                        if existing.is_none() {
                            p.write_tag(None, &draft)?;
                        }
                        p.set_catalog_links(target, &values, false)
                    }) {
                        name.clear();
                    }
                }
                ui.label(theme::muted(
                    "同名标签直接复用；新标签自动生成 ID，可在资料页补充说明。",
                ));
                ui.data_mut(|d| d.insert_temp(key, name));
            });
        });
        let assets = catalog.assets_for(target);
        let mut remove = None;
        for asset in &assets {
            ui.horizontal_wrapped(|ui| {
                let color = if asset.available { BLUE } else { GOLD };
                if ui
                    .button(
                        RichText::new(format!(
                            "{}  {}",
                            match asset.kind.as_str() {
                                "image" => "▧",
                                "audio" => "♪",
                                _ => "▤",
                            },
                            asset.display
                        ))
                        .color(color),
                    )
                    .clicked()
                {
                    self.catalog_target = Some(TargetRef::new("asset", &asset.id));
                    self.tab = Tab::Catalog;
                }
                if ui
                    .small_button("×")
                    .on_hover_text("移除此处引用,保留文件与素材资料")
                    .clicked()
                {
                    remove = Some(asset.id.clone());
                }
            });
            ui.label(theme::muted(&asset.path));
        }
        if let Some(id) = remove {
            let ids: Vec<_> = assets
                .iter()
                .filter(|a| a.id != id)
                .map(|a| a.id.clone())
                .collect();
            self.commit("素材引用已移除", |p| {
                p.set_catalog_links(target, &ids, true)
            });
        }
        ui.horizontal_wrapped(|ui| {
            let paths = crate::media::pick_reference_files(ui, target);
            if !paths.is_empty() {
                self.commit("文件已关联到对象", |p| {
                    for path in paths {
                        p.add_asset_reference(target, &path)?;
                    }
                    Ok(())
                });
            }
            ui.menu_button("已有素材", |ui| {
                for asset in catalog
                    .assets
                    .values()
                    .filter(|a| !assets.iter().any(|old| old.id == a.id))
                {
                    if ui.button(&asset.display).clicked() {
                        let mut ids: Vec<_> = assets.iter().map(|a| a.id.clone()).collect();
                        ids.push(asset.id.clone());
                        self.commit("素材已关联", |p| {
                            p.set_catalog_links(target, &ids, true)
                        });
                        ui.close();
                    }
                }
            });
        });
        let references = impact.content_references;
        egui::CollapsingHeader::new(format!(
            "引用来源 · {} 处",
            references.len() + map_references.len() + impact.map_rasters.len()
        ))
        .id_salt(("references", target))
        .default_open(target.kind == "event")
        .show(ui, |ui| {
            if !impact.complete {
                ui.colored_label(ERROR, "引用检查不完整，修复诊断前不能删除资料。");
                for diagnostic in impact
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.severity == worldline_core::Severity::Error)
                {
                    ui.label(theme::muted(&diagnostic.message));
                }
            }
            for (i, reference) in references.iter().enumerate() {
                ui.push_id(i, |ui| {
                    let source = catalog.object(&reference.source);
                    let display = source
                        .map(|o| o.display.as_str())
                        .unwrap_or(&reference.source.id);
                    if ui
                        .button(format!("{} · {display}", reference.kind))
                        .clicked()
                    {
                        self.jump_to_file(&reference.file, reference.line, 1);
                    }
                    ui.label(theme::muted(format!(
                        "{}:{}",
                        std::path::Path::new(&reference.file)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy(),
                        reference.line
                    )));
                });
            }
            for (reference, title, kind) in &map_references {
                if ui
                    .button(format!("地图{kind} · {title} · {}", reference.placement_id))
                    .clicked()
                {
                    self.locate_reference(&reference.map_id, &reference.placement_id);
                }
            }
            for reference in &impact.graph_views {
                if ui
                    .button(format!(
                        "共享布局 · {} · {} · 打开引用文档",
                        reference.view_id, reference.field
                    ))
                    .clicked()
                {
                    self.jump_to_file(&reference.file, 1, 1);
                }
            }
            for reference in &impact.map_rasters {
                if ui
                    .button(format!(
                        "地图底图 · {} · {} · 打开引用文档",
                        reference.map_id, reference.raster_layer_id
                    ))
                    .clicked()
                {
                    match worldline_core::presentation_commands::map_document_path(
                        &self.project,
                        &reference.map_id,
                    ) {
                        Ok(path) => self.jump_to_file(&path.to_string_lossy(), 1, 1),
                        Err(error) => self.io_error = Some(error.to_string()),
                    }
                }
            }
            if !map_references.is_empty() || !impact.map_rasters.is_empty() {
                ui.label(theme::muted(
                    "删除资料前须先明确解除或重新绑定这些地图引用；删除标记不会删除资料。",
                ));
            }
            if impact.complete
                && references.is_empty()
                && map_references.is_empty()
                && impact.map_rasters.is_empty()
                && impact.graph_views.is_empty()
            {
                ui.label(theme::muted("尚无直接引用"));
            }
        });
    }

    pub(super) fn catalog_tab(&mut self, ctx: &egui::Context) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let catalog = snapshot.result.analysis.catalog.clone();
        egui::SidePanel::right("catalog-index")
            .default_width(270.0)
            .width_range(230.0..=380.0)
            .frame(theme::panel())
            .show(ctx, |ui| {
                ui.label(RichText::new("世界资料索引").strong().size(17.0));
                ui.horizontal_wrapped(|ui| {
                    for (id, name) in [
                        ("entity", "通用资料"),
                        ("relation", "独立关系"),
                        ("tag", "标签"),
                        ("state", "状态"),
                        ("anchor", "锚点"),
                        ("asset", "素材"),
                        ("", "全部"),
                    ] {
                        ui.selectable_value(&mut self.catalog_filter, id.into(), name);
                    }
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.catalog_query)
                        .hint_text("名称、ID 或别名")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .id_salt("catalog-items")
                    .show(ui, |ui| {
                        for object in &catalog.search_objects(&self.catalog_query) {
                            if !self.catalog_filter.is_empty()
                                && object.target.kind != self.catalog_filter
                            {
                                continue;
                            }
                            let selected = self.catalog_target.as_ref() == Some(&object.target);
                            if ui
                                .add_sized(
                                    [ui.available_width(), 48.0],
                                    egui::Button::selectable(
                                        selected,
                                        format!(
                                            "{}\n{} · {}",
                                            object.display,
                                            kind_label(&object.target.kind),
                                            object.target.id
                                        ),
                                    ),
                                )
                                .on_hover_text(format!(
                                    "{} · {}",
                                    kind_label(&object.target.kind),
                                    object.target.id
                                ))
                                .clicked()
                            {
                                self.catalog_target = Some(object.target.clone());
                                self.tag_editor = None;
                                self.state_editor = None;
                                self.anchor_editor = None;
                            }
                        }
                    });
            });
        egui::CentralPanel::default().frame(theme::panel().fill(BG)).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.vertical(|ui| { ui.heading("资料与状态"); ui.label(theme::muted(format!("{} 个标签 · {} 个状态 · {} 份素材", catalog.tags.len(), catalog.states.len(), catalog.assets.len()))); });
                if ui.button("全部标签").clicked() {
                    self.catalog_filter = "tag".into(); self.catalog_query.clear(); self.catalog_target = None;
                    self.tag_editor = None; self.state_editor = None; self.anchor_editor = None;
                }
                if ui.button("＋ 通用资料").clicked() { self.edit_entity(None); }
                if ui.button("＋ 独立关系").clicked() { self.edit_relation(None, self.catalog_target.clone()); }
                ui.menu_button("关系类型", |ui| {
                    if ui.button("＋ 新建关系类型").clicked() { self.edit_relation_type(None); ui.close(); }
                    for kind in catalog.relation_types.values() {
                        if ui.button(format!("{} · {}", kind.display, kind.id)).clicked() {
                            self.edit_relation_type(Some(&kind.id)); ui.close();
                        }
                    }
                });
                if ui.button("＋ 新建锚点").clicked() { self.new_anchor(); }
                if ui.button("＋ 新建状态").clicked() {
                    let target = self.catalog_target.clone().or_else(|| catalog.objects.first().map(|o| o.target.clone()));
                    if let Some(target) = target { self.new_state(target); }
                }
                if ui.add(theme::primary("＋ 新建标签")).clicked() {
                    let mut i = 1; while catalog.tags.contains_key(&format!("tag_{i}")) { i += 1; }
                    self.tag_editor = Some((None, WorldDraft { id: format!("tag_{i}"), display: "新的标签".into(), ..Default::default() }));
                    self.catalog_target = None;
                    self.state_editor = None;
                self.anchor_editor = None;
                }
            });
            ui.add_space(18.0);
            if let Some(target) = &self.catalog_target {
                if target.kind == "tag" && self.tag_editor.is_none() {
                    if let Some(tag) = catalog.tags.get(&target.id).filter(|t| t.declared) {
                        self.tag_editor = Some((Some(tag.id.clone()), WorldDraft { id: tag.id.clone(), display: tag.display.clone(), description: tag.description.clone(), properties: tag.properties.clone().into_iter().collect() }));
                    }
                }
            }
            let mut editor = self.tag_editor.take();
            egui::ScrollArea::vertical().id_salt("catalog-detail").show(ui, |ui| {
                if self.catalog_filter == "tag" && self.catalog_target.is_none() && editor.is_none() {
                    ui.heading("全部标签");
                    ui.label(theme::muted("点击标签即可编辑、给它添加标签，并查看关联对象。右侧搜索按名称、ID 或别名筛选。"));
                    for object in catalog.search_objects(&self.catalog_query).into_iter().filter(|o| o.target.kind == "tag") {
                        let count = catalog.query(&object.target.id, false).len();
                        if ui.button(format!("# {} · {} · {} 个关联对象", object.display, object.target.id, count)).clicked() {
                            self.catalog_target = Some(object.target.clone());
                        }
                    }
                    ui.separator();
                }
                self.anchor_form(ui, &catalog);
                self.state_form(ui, &catalog);
                if let Some((original, draft)) = &mut editor {
                    theme::card().show(ui, |ui| {
                        ui.label(theme::muted("标签 ID · 引用身份"));
                        ui.add_enabled(original.is_none(), egui::TextEdit::singleline(&mut draft.id).desired_width(f32::INFINITY));
                        field(ui, "标签名称", &mut draft.display);
                        ui.label(theme::muted("说明"));
                        ui.add(egui::TextEdit::multiline(&mut draft.description).desired_rows(2).desired_width(f32::INFINITY));
                        egui::CollapsingHeader::new("标签属性").show(ui, |ui| { properties(ui, &mut draft.properties); });
                        if ui.add(theme::primary(if original.is_some() { "应用标签资料" } else { "创建标签" })).clicked() && self.commit("标签资料已更新", |p| p.write_tag(original.as_deref(), draft)) {
                            *original = Some(draft.id.clone()); self.catalog_target = Some(TargetRef::new("tag", &draft.id));
                        }
                    });
                }
                if let Some(target) = self.catalog_target.clone() {
                    if let Some(object) = catalog.object(&target) {
                        if target.kind != "tag" { ui.heading(&object.display); ui.label(theme::muted(format!("{} · {}", kind_label(&target.kind), target.id))); }
                        if let Some(asset) = catalog.assets.get(&target.id).filter(|_| target.kind == "asset") {
                            ui.label(RichText::new(if asset.available { "文件可用" } else { "文件缺失或格式不可用" }).color(if asset.available { ACCENT } else { GOLD }));
                            ui.label(theme::muted("引用路径")); ui.label(&asset.path);
                            ui.label(theme::muted("本机文件")); ui.label(&asset.resolved_path);
                            ui.horizontal_wrapped(|ui| {
                                if ui.add_enabled(asset.available, egui::Button::new(crate::media::OPEN_REFERENCE_LABEL)).clicked() {
                                    if let Err(error) = crate::media::open_reference(&self.project.root, std::path::Path::new(&asset.resolved_path)) { self.io_error = Some(error); }
                                }
                                if ui.button("复制路径").clicked() { ui.ctx().copy_text(asset.resolved_path.clone()); }
                                if ui.button("更换文件").clicked() {
                                    #[cfg(target_arch = "wasm32")]
                                    crate::web::select_files(ui.ctx(), false, "", crate::web::FileAction::Replace(AssetDraft { id: asset.id.clone(), kind: asset.kind.clone(), path: String::new(), display: asset.display.clone() }));
                                    #[cfg(not(target_arch = "wasm32"))]
                                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                                        let draft = AssetDraft { id: asset.id.clone(), kind: asset.kind.clone(), path: path.to_string_lossy().into_owned(), display: asset.display.clone() };
                                        self.commit("素材引用已更新", |p| p.write_asset(&draft));
                                    }
                                }
                                if ui.button("重新检查").clicked() { self.recompile(); }
                            });
                        }
                        self.object_links(ui, &target);
                        if target.kind == "tag" {
                            let direct = catalog.query(&target.id, false);
                            ui.add_space(14.0);
                            ui.label(RichText::new(format!("使用此标签的 {} 个对象 · 其中 {} 个标签", direct.len(), direct.iter().filter(|o| o.target.kind == "tag").count())).strong());
                            ui.checkbox(&mut self.catalog_recursive, "包含下级标签标记的对象");
                            let matches = catalog.query(&target.id, self.catalog_recursive);
                            ui.label(theme::muted(format!("去重后 {} 个对象 · {} 个事件", matches.len(), matches.iter().filter(|o| o.target.kind == "event").count())));
                            for hit in matches {
                                if ui.button(format!("{}  {}  ·  {}", kind_label(&hit.target.kind), hit.display, hit.target.id)).clicked() { self.navigate_object(&hit); }
                            }
                        }
                        if ui.button("定位源文件").clicked() { self.jump_to_file(&object.file, object.line, 1); }
                    }
                } else if editor.is_none() && self.state_editor.is_none() && self.anchor_editor.is_none() {
                    theme::card().show(ui, |ui| {
                        ui.label(RichText::new("让资料通过引用连接起来").strong().size(20.0));
                        ui.label(theme::muted("创建“坐标”和“雾港码头”两个标签,用坐标标记雾港码头,再用雾港码头标记事件。从坐标就可以逐层找到地点与事件。"));
                        ui.label(theme::muted("在右侧切换到“全部”,可以为世界、人物、时段、文件等完整对象添加标签与引用文件。"));
                    });
                }
            });
            // 切换标签时不把旧表单带到新标签。
            if editor.as_ref().is_none_or(|(id, _)| id.is_none() || self.catalog_target.as_ref().is_some_and(|t| t.kind == "tag" && Some(&t.id) == id.as_ref())) { self.tag_editor = editor; }
        });
    }
}
