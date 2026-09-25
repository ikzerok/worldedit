//! 展示预设：显式保存地图/网络/范围的展示选择，不生成世界语义。
use super::{Tab, WorldeditApp};
use crate::theme::{self, *};
use std::collections::BTreeMap;
use worldline_core::presentation::{MapDocument, MapGeometry};
use worldline_core::presentation_presets::{
    PresentationPresetDraft, PresetCommand, PresetGeometryRef,
};
use worldline_core::TargetRef;

#[derive(Clone)]
pub(super) struct PresetEditor {
    pub original: Option<String>,
    pub draft: PresentationPresetDraft,
    baseline: String,
    version: u64,
}

fn default_layers(map: &MapDocument) -> BTreeMap<String, bool> {
    map.layers
        .iter()
        .map(|(id, layer)| (id.clone(), layer.visible_default))
        .collect()
}

fn scope_label(catalog: &worldline_core::Catalog, target: &TargetRef) -> String {
    catalog
        .object(target)
        .map(|object| format!("{} · {}:{}", object.display, target.kind, target.id))
        .unwrap_or_else(|| format!("{}:{}", target.kind, target.id))
}
impl WorldeditApp {
    pub(super) fn open_preset_editor(&mut self, id: Option<&str>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let draft = if let Some(id) = id {
            let Some(preset) = snapshot.preset_index.presets.get(id) else {
                self.io_error = Some("展示预设已不存在，请刷新后重试".into());
                return;
            };
            preset.draft.clone()
        } else {
            let mut number = 1;
            while snapshot
                .preset_index
                .presets
                .contains_key(&format!("preset_{number}"))
            {
                number += 1;
            }
            let map_id = self
                .map_selection
                .clone()
                .or_else(|| snapshot.map_index.maps.keys().next().cloned());
            let graph_view_id = self
                .network_loaded_view
                .clone()
                .or_else(|| snapshot.graph_index.views.keys().next().cloned());
            let layer_visibility = map_id
                .as_ref()
                .and_then(|map_id| {
                    if self.map_canvas.map_id() == map_id {
                        Some(
                            self.map_canvas
                                .layer_details()
                                .into_iter()
                                .map(|(id, _, visible, _, _)| (id, visible))
                                .collect(),
                        )
                    } else {
                        snapshot.map_index.maps.get(map_id).map(default_layers)
                    }
                })
                .unwrap_or_default();
            PresentationPresetDraft {
                id: format!("preset_{number}"),
                title: "新的展示预设".into(),
                map_id,
                graph_view_id,
                layer_visibility,
                scope_refs: Vec::new(),
                include_unscoped: false,
                include_period_children: false,
                geometry_refs: Vec::new(),
            }
        };
        self.preset_editor = Some(PresetEditor {
            original: id.map(str::to_owned),
            draft,
            baseline: self.project.content_baseline(),
            version: self.version,
        });
    }

    pub(super) fn apply_presentation_preset(&mut self, id: &str) {
        let Some((draft, timeline)) = self.snapshot.as_ref().and_then(|snapshot| {
            let preset = snapshot.preset_index.presets.get(id)?;
            Some((
                preset.draft.clone(),
                snapshot.result.analysis.timeline.clone(),
            ))
        }) else {
            self.io_error = Some("展示预设已不存在，请刷新后重试".into());
            return;
        };
        let scope_refs = worldline_core::relations::expand_period_scope_refs(
            &timeline,
            &draft.scope_refs,
            draft.include_period_children,
        );
        if let Some(map_id) = &draft.map_id {
            self.map_selection = Some(map_id.clone());
            self.pending_preset_layers = Some((map_id.clone(), draft.layer_visibility.clone()));
            if draft.graph_view_id.is_none() {
                self.tab = Tab::Map;
            }
        }
        if let Some(view_id) = &draft.graph_view_id {
            self.load_graph_view(view_id);
        }
        if self.network_state.focus.is_some() {
            self.network_state.scope_refs = scope_refs;
            self.network_state.include_unscoped = draft.include_unscoped;
        }
        self.message = Some(format!(
            "已应用展示预设“{}”；只改变当前浏览筛选与图层，不执行故事",
            draft.title
        ));
    }

    fn save_preset_editor(&mut self, form: &PresetEditor) -> bool {
        if form.version != self.version || form.baseline != self.project.content_baseline() {
            self.io_error = Some("预设打开后工程已变化。输入已保留，请重新打开后合并。".into());
            return false;
        }
        let command = PresetCommand {
            expected_revision: self.map_revision,
            expected_baseline: form.baseline.clone(),
            original: form.original.clone(),
            draft: form.draft.clone(),
        };
        let before = self.project.clone();
        match worldline_core::presentation_presets::apply(
            &mut self.project,
            &mut self.map_revision,
            command,
        ) {
            Ok(_) => {
                self.remember(before);
                self.refresh_presentation_after_map_command();
                self.io_error = None;
                self.message = Some("展示预设已更新；保存全部可写入作品目录".into());
                true
            }
            Err(error) => {
                self.io_error = Some(error);
                false
            }
        }
    }

    pub(super) fn preset_editor_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.preset_editor.take() else {
            return;
        };
        let maps = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.map_index.maps.clone())
            .unwrap_or_default();
        let graph_views = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .graph_index
                    .views
                    .iter()
                    .map(|(id, view)| (id.clone(), view.draft.title.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let catalog = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.result.analysis.catalog.clone())
            .unwrap_or_default();
        let saved = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .preset_index
                    .presets
                    .values()
                    .map(|preset| (preset.draft.id.clone(), preset.draft.title.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let scope_choices = catalog
            .objects
            .iter()
            .filter(|object| {
                object.target.kind == "period"
                    || object.target.kind == "event"
                    || object.target.kind == "scene"
                    || (object.target.kind == "entity"
                        && catalog
                            .entities
                            .get(&object.target.id)
                            .is_some_and(|entity| entity.entity_type == "version"))
            })
            .map(|object| object.target.clone())
            .collect::<Vec<_>>();
        let mut open = true;
        let mut saved_now = false;
        let mut switch_to = None;
        egui::Window::new(if form.original.is_some() {
            "编辑展示预设"
        } else {
            "新建展示预设"
        })
        .id(egui::Id::new("presentation-preset-editor"))
        .open(&mut open)
        .default_width(680.0)
        .default_height(760.0)
        .vscroll(true)
        .show(ctx, |ui| {
            if !saved.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::muted("已有预设"));
                    for (id, title) in &saved {
                        if ui.small_button(format!("{title} · {id}")).clicked() {
                            switch_to = Some(id.clone());
                        }
                    }
                });
                ui.separator();
            }
            ui.label("标题");
            ui.text_edit_singleline(&mut form.draft.title);
            ui.label(theme::muted("稳定 ID"));
            ui.add_enabled(
                form.original.is_none(),
                egui::TextEdit::singleline(&mut form.draft.id),
            );
            let old_map = form.draft.map_id.clone();
            egui::ComboBox::from_id_salt("preset-map")
                .selected_text(form.draft.map_id.as_deref().unwrap_or("不指定地图"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.draft.map_id, None, "不指定地图");
                    for map in maps.values() {
                        ui.selectable_value(
                            &mut form.draft.map_id,
                            Some(map.id.clone()),
                            format!("{} · {}", map.title, map.id),
                        );
                    }
                });
            if old_map != form.draft.map_id {
                form.draft.layer_visibility = form
                    .draft
                    .map_id
                    .as_ref()
                    .and_then(|id| maps.get(id))
                    .map(default_layers)
                    .unwrap_or_default();
                form.draft.geometry_refs.clear();
            }
            egui::ComboBox::from_id_salt("preset-graph")
                .selected_text(
                    form.draft
                        .graph_view_id
                        .as_deref()
                        .unwrap_or("不指定网络布局"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.draft.graph_view_id, None, "不指定网络布局");
                    for (id, title) in &graph_views {
                        ui.selectable_value(
                            &mut form.draft.graph_view_id,
                            Some(id.clone()),
                            format!("{title} · {id}"),
                        );
                    }
                });
            if let Some(map_id) = &form.draft.map_id {
                if let Some(map) = maps.get(map_id) {
                    ui.separator();
                    ui.label(egui::RichText::new("地图图层").strong());
                    for layer_id in &map.layer_order {
                        if let Some(layer) = map.layers.get(layer_id) {
                            let visible = form
                                .draft
                                .layer_visibility
                                .entry(layer_id.clone())
                                .or_insert(layer.visible_default);
                            ui.checkbox(visible, format!("{} · {}", layer.title, layer.id));
                        }
                    }
                    ui.collapsing("路径说明与分布区", |ui| {
                        for placement in map.placements.values() {
                            let purpose = match placement.geometry {
                                MapGeometry::Polyline { .. } => Some("path"),
                                MapGeometry::Polygon { .. } => Some("distribution"),
                                MapGeometry::Point { .. } => None,
                            };
                            let Some(purpose) = purpose else {
                                continue;
                            };
                            let index = form
                                .draft
                                .geometry_refs
                                .iter()
                                .position(|item| item.placement_id == placement.id);
                            let mut selected = index.is_some();
                            if ui
                                .checkbox(
                                    &mut selected,
                                    format!(
                                        "{} · {}",
                                        if purpose == "path" {
                                            "路径"
                                        } else {
                                            "分布区"
                                        },
                                        placement.id
                                    ),
                                )
                                .changed()
                            {
                                if selected {
                                    form.draft.geometry_refs.push(PresetGeometryRef {
                                        placement_id: placement.id.clone(),
                                        purpose: purpose.into(),
                                        note: placement.annotation.clone(),
                                    });
                                } else {
                                    form.draft
                                        .geometry_refs
                                        .retain(|item| item.placement_id != placement.id);
                                }
                            }
                            if let Some(item) = form
                                .draft
                                .geometry_refs
                                .iter_mut()
                                .find(|item| item.placement_id == placement.id)
                            {
                                ui.add(
                                    egui::TextEdit::singleline(&mut item.note)
                                        .hint_text("展示说明"),
                                );
                            }
                        }
                    });
                }
            }
            ui.separator();
            ui.label(egui::RichText::new("作者范围筛选").strong());
            ui.checkbox(&mut form.draft.include_unscoped, "同时包含未标范围的关系");
            ui.checkbox(
                &mut form.draft.include_period_children,
                "所选时期显式包含其子时期",
            );
            ui.collapsing("选择时期 / 故事 / 版本", |ui| {
                for target in &scope_choices {
                    let mut selected = form.draft.scope_refs.contains(target);
                    if ui
                        .checkbox(&mut selected, scope_label(&catalog, target))
                        .changed()
                    {
                        if selected {
                            form.draft.scope_refs.push(target.clone());
                            form.draft.scope_refs.sort();
                            form.draft.scope_refs.dedup();
                        } else {
                            form.draft.scope_refs.retain(|item| item != target);
                        }
                    }
                }
                if scope_choices.is_empty() {
                    ui.label(theme::muted("当前工程没有可选的时期、故事或版本范围。"));
                }
            });
            ui.label(theme::muted(
                "范围只筛选作者明确标注的记录；不推算中间历史，也不从地图几何推断归属。",
            ));
            let current =
                form.version == self.version && form.baseline == self.project.content_baseline();
            if !current {
                ui.colored_label(GOLD, "工程已变化；预设输入保留，但旧基线不能覆盖当前内容。");
            }
            if ui
                .add_enabled(
                    current
                        && !form.draft.title.trim().is_empty()
                        && (form.draft.map_id.is_some() || form.draft.graph_view_id.is_some()),
                    theme::primary("保存展示预设"),
                )
                .clicked()
            {
                saved_now = self.save_preset_editor(&form);
                if saved_now {
                    form.original = Some(form.draft.id.clone());
                    form.baseline = self.project.content_baseline();
                    form.version = self.version;
                }
            }
            if form.original.is_some() && ui.button("应用到当前浏览").clicked() {
                self.apply_presentation_preset(&form.draft.id);
            }
        });
        if let Some(id) = switch_to {
            self.open_preset_editor(Some(&id));
        } else if open {
            self.preset_editor = Some(form);
        } else if !saved_now {
            self.preset_editor = None;
        }
    }
}
