use super::super::inspector::{field, properties};
use super::{kind_label, WorldeditApp};
use crate::theme::{self, *};
use egui::RichText;
use worldline_core::authoring::WorldDraft;
use worldline_core::catalog::TargetRef;
use worldline_core::catalog_edit::AssetDraft;
impl WorldeditApp {
    pub(in crate::app) fn catalog_tab(&mut self, ctx: &egui::Context) {
        if self.catalog_workbench.open {
            self.catalog_query_tab(ctx);
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let catalog = snapshot.result.analysis.catalog.clone();
        let compact_index = ctx.available_rect().width() < 742.0;
        if !compact_index {
            egui::SidePanel::right("catalog-index")
                .default_width(270.0)
                .width_range(230.0..=(ctx.available_rect().width() - 512.0).min(380.0))
                .frame(theme::panel())
                .show(ctx, |ui| {
                    self.catalog_index_ui(ui, &catalog);
                });
            self.personal.catalog_drawer_open = false;
        }
        if compact_index && self.personal.catalog_drawer_open {
            let mut open = true;
            egui::Window::new("资料索引")
                .id(egui::Id::new("catalog-index-drawer"))
                .open(&mut open)
                .collapsible(false)
                .default_width(300.0)
                .max_height((ctx.screen_rect().height() - 120.0).max(160.0))
                .show(ctx, |ui| {
                    if ui.button("收起索引").clicked() || self.catalog_index_ui(ui, &catalog) {
                        self.personal.catalog_drawer_open = false;
                    }
                });
            if !open {
                self.personal.catalog_drawer_open = false;
            }
        }
        egui::CentralPanel::default().frame(theme::panel().fill(BG())).show(ctx, |ui| {
            theme::page_heading(ui, "资料与状态", &format!("{} 个标签 · {} 个状态 · {} 份素材", catalog.tags.len(), catalog.states.len(), catalog.assets.len()));
            theme::toolbar(ui, |ui| {
                if compact_index && ui.button("资料索引（窄窗）").clicked() {
                    self.personal.catalog_drawer_open = !self.personal.catalog_drawer_open;
                }

                if ui.button("组合查询与待办").clicked() {
                    self.catalog_workbench.open = true;
                }
                if ui.button("全部标签").clicked() && !self.prevent_catalog_switch() {
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
                if ui.add(theme::primary("＋ 新建标签")).clicked() && !self.prevent_catalog_switch() {
                    let mut i = 1; while catalog.tags.contains_key(&format!("tag_{i}")) { i += 1; }
                    self.tag_editor = Some((None, WorldDraft { id: format!("tag_{i}"), display: "新的标签".into(), ..Default::default() }));
                    self.reset_new_draft_baseline("标签");
                    self.catalog_target = None;
                    self.state_editor = None;
                self.anchor_editor = None;
                }
            });
            ui.separator();
            if let Some(target) = &self.catalog_target {
                if target.kind == "tag" && self.tag_editor.is_none() {
                    if let Some(tag) = catalog.tags.get(&target.id).filter(|t| t.declared) {
                        self.tag_editor = Some((Some(tag.id.clone()), WorldDraft { id: tag.id.clone(), display: tag.display.clone(), description: tag.description.clone(), properties: tag.properties.clone().into_iter().collect() }));
                    }
                }
            }
            self.capture_new_draft_baselines();
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
                            ui.label(RichText::new(if asset.available { "文件可用" } else { "文件缺失或格式不可用" }).color(if asset.available { ACCENT() } else { GOLD() }));
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
