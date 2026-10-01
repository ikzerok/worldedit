use super::WorldeditApp;
use crate::app::Tab;
use crate::theme::{self, *};
use egui::RichText;
use worldline_core::authoring::WorldDraft;
use worldline_core::catalog::TargetRef;
impl WorldeditApp {
    pub(in crate::app) fn object_links(&mut self, ui: &mut egui::Ui, target: &TargetRef) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let catalog = snapshot.result.analysis.catalog.clone();
        let mut impact = worldline_core::reference_impact::deletion_impact_with_collaboration(
            &snapshot.result,
            &snapshot.map_index,
            &snapshot.graph_index,
            &snapshot.comment_index,
            target,
        );
        impact.template_references = snapshot.template_index.references_to(target);
        impact
            .diagnostics
            .extend(snapshot.template_index.diagnostics.iter().cloned());
        impact.complete &= !snapshot
            .template_index
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == worldline_core::Severity::Error);
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
        ui.horizontal_wrapped(|ui| {
            if ui.button("阅读完整资料 / 管理别名").clicked() {
                self.open_reading(target.clone());
            }
            if ui.button("批注此对象…").clicked() {
                self.new_comment_for_anchor(worldline_core::collaboration::CommentAnchor::Object {
                    target: target.clone(),
                });
            }
        });
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
                            .small_button(RichText::new(format!("# {display}")).color(ACCENT()))
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
                if super::super::tags::picker(ui, &catalog, &mut values, &locked) {
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
                let color = if asset.available { BLUE() } else { GOLD() };
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
            references.len()
                + map_references.len()
                + impact.map_rasters.len()
                + impact.graph_views.len()
                + impact.comments.len()
                + impact.template_references.len()
        ))
        .id_salt(("references", target))
        .default_open(target.kind == "event")
        .show(ui, |ui| {
            if !impact.complete {
                ui.colored_label(ERROR(), "引用检查不完整，修复诊断前不能删除资料。");
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
            for reference in &impact.comments {
                if ui
                    .button(format!(
                        "批注引用 · {} · 打开批注文档",
                        reference.comment_id
                    ))
                    .clicked()
                {
                    self.jump_to_file(&reference.file, 1, 1);
                }
            }
            for reference in &impact.template_references {
                if ui
                    .button(format!(
                        "模板默认值引用 · {}:{} · 打开模板",
                        reference.file, reference.line
                    ))
                    .clicked()
                {
                    self.jump_to_file(&reference.file, reference.line, 1);
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
                && impact.comments.is_empty()
                && impact.template_references.is_empty()
            {
                ui.label(theme::muted("尚无直接引用"));
            }
        });
    }
}

#[cfg(test)]
mod tests;
