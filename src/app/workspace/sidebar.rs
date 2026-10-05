use super::super::{Tab, WorldeditApp};
use crate::theme::{self, *};
use egui::RichText;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn nav_icon(painter: &egui::Painter, center: egui::Pos2, tab: Tab, color: egui::Color32) {
    use egui::{vec2, Stroke};
    let stroke = Stroke::new(1.3_f32, color);
    let line = |a: [f32; 2], b: [f32; 2]| {
        painter.line_segment(
            [center + vec2(a[0], a[1]), center + vec2(b[0], b[1])],
            stroke,
        );
    };
    match tab {
        Tab::Timeline => {
            line([-8.0, -5.0], [3.0, -5.0]);
            line([-3.0, 0.0], [8.0, 0.0]);
            line([-8.0, 5.0], [3.0, 5.0]);
        }
        Tab::Graph | Tab::Network => {
            line([-5.0, -5.0], [5.0, 4.0]);
            line([-5.0, 5.0], [5.0, 4.0]);
            for p in [vec2(-5.0, -5.0), vec2(-5.0, 5.0), vec2(5.0, 4.0)] {
                painter.circle_filled(center + p, 2.5, color);
            }
        }
        Tab::Map => {
            painter.rect_stroke(
                egui::Rect::from_center_size(center, egui::vec2(8.0, 8.0)),
                1.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [
                    center + egui::vec2(-8.0, 0.0),
                    center + egui::vec2(8.0, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    center + egui::vec2(0.0, -8.0),
                    center + egui::vec2(0.0, 8.0),
                ],
                stroke,
            );
        }
        Tab::Characters => {
            painter.circle_stroke(center + vec2(0.0, -4.0), 3.0, stroke);
            painter.add(egui::Shape::line(
                vec![
                    center + vec2(-6.0, 7.0),
                    center + vec2(-5.0, 2.0),
                    center + vec2(0.0, 0.0),
                    center + vec2(5.0, 2.0),
                    center + vec2(6.0, 7.0),
                ],
                stroke,
            ));
        }
        Tab::World => {
            painter.circle_stroke(center, 7.0, stroke);
            line([-7.0, 0.0], [7.0, 0.0]);
            line([0.0, -7.0], [0.0, 7.0]);
        }
        Tab::Catalog => {
            painter.add(egui::Shape::closed_line(
                vec![
                    center + vec2(-7.0, -6.0),
                    center + vec2(1.0, -6.0),
                    center + vec2(8.0, 1.0),
                    center + vec2(1.0, 8.0),
                    center + vec2(-7.0, 0.0),
                ],
                stroke,
            ));
            painter.circle_filled(center + vec2(-3.0, -2.0), 1.4, color);
        }
        _ => {
            for y in [-5.0, 0.0, 5.0] {
                line([-6.0, y], [6.0, y]);
            }
        }
    }
}
impl WorldeditApp {
    pub(in crate::app) fn sidebar(&mut self, ctx: &egui::Context) {
        let panel = egui::SidePanel::left("project")
            .resizable(true)
            .default_width(self.personal.settings.navigation_width)
            .width_range(190.0..=320.0)
            .frame(theme::panel())
            .show(ctx, |ui| {
                let title = self
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.result.analysis.world.as_ref())
                    .map(|w| w.display.as_str())
                    .unwrap_or("Worldline 工程");
                ui.label(RichText::new(title).strong().size(18.0));
                ui.label(theme::muted(format!(
                    "{} 个源码文件 · 一个世界",
                    self.project.documents.len()
                )));
                ui.add_space(12.0);
                egui::ScrollArea::vertical()
                    .id_salt("sidebar-tabs")
                    .max_height((ui.available_height() - 140.0).max(180.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 3.0;
                        for (group, tabs, open) in [
                            (
                                "写作与阅读",
                                &[Tab::Manuscript, Tab::Overview, Tab::Edit][..],
                                true,
                            ),
                            (
                                "世界资料",
                                &[
                                    Tab::World,
                                    Tab::Characters,
                                    Tab::Catalog,
                                    Tab::CatalogImport,
                                    Tab::Wiki,
                                    Tab::Map,
                                    Tab::Network,
                                ][..],
                                true,
                            ),
                            (
                                "结构与审阅",
                                &[Tab::Timeline, Tab::Graph, Tab::Review][..],
                                false,
                            ),
                            (
                                "工程工具",
                                &[Tab::Templates, Tab::Localization, Tab::CheckpointHistory][..],
                                false,
                            ),
                        ] {
                            egui::CollapsingHeader::new(group)
                                .default_open(open)
                                .show(ui, |ui| {
                                    for &tab in tabs {
                                        let selected = self.tab == tab;
                                        let response = ui.add_sized(
                                            [ui.available_width(), 32.0],
                                            egui::Button::selectable(
                                                selected,
                                                RichText::new(tab.title()).color(if selected {
                                                    ACCENT()
                                                } else {
                                                    TEXT()
                                                }),
                                            ),
                                        );
                                        nav_icon(
                                            ui.painter(),
                                            response.rect.left_center() + egui::vec2(14.0, 0.0),
                                            tab,
                                            if selected { ACCENT() } else { MUTED() },
                                        );
                                        if response.clicked() {
                                            self.switch_tab(tab);
                                        }
                                    }
                                });
                        }
                    });
                ui.add_space(10.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(theme::muted("工程文件"));
                    if ui
                        .small_button("＋")
                        .on_hover_text("新建文件并加入总入口")
                        .clicked()
                    {
                        self.new_file = Some("events/chapter.wl".into());
                        self.reset_new_draft_baseline("文件名称");
                    }
                    if ui
                        .small_button("引用")
                        .on_hover_text("引用已合并到工程目录中的文件")
                        .clicked()
                    {
                        #[cfg(target_arch = "wasm32")]
                        crate::web::select_files(
                            ctx,
                            false,
                            ".wl",
                            crate::web::FileAction::Include,
                        );
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some(path) = rfd::FileDialog::new()
                            .set_directory(&self.project.root)
                            .add_filter("Worldline", &["wl"])
                            .pick_file()
                        {
                            let before = self.project.clone();
                            match self.project.include_file(&path) {
                                Ok(()) => {
                                    self.remember(before);
                                    self.recompile();
                                    self.message = Some("已引用文件,请检查全局诊断".into());
                                }
                                Err(e) => self.io_error = Some(e),
                            }
                        }
                    }
                });
                let mut groups: BTreeMap<String, Vec<(PathBuf, String, bool)>> = BTreeMap::new();
                for (path, doc) in &self.project.documents {
                    let relative = path.strip_prefix(&self.project.root).unwrap_or(path);
                    let parent = relative
                        .parent()
                        .unwrap_or(Path::new(""))
                        .to_string_lossy()
                        .replace('\\', "/");
                    groups.entry(parent).or_default().push((
                        path.clone(),
                        relative
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        doc.is_dirty(),
                    ));
                }
                #[cfg(not(target_arch = "wasm32"))]
                let other_files: Vec<PathBuf> =
                    self.disk_stamp.iter().map(|(p, _, _)| p.clone()).collect();
                #[cfg(target_arch = "wasm32")]
                let other_files: Vec<PathBuf> = crate::web::imported()
                    .keys()
                    .map(|p| self.project.root.join(p))
                    .collect();
                for path in other_files {
                    if self.project.documents.contains_key(&path) {
                        continue;
                    }
                    let Ok(relative) = path.strip_prefix(&self.project.root) else {
                        continue;
                    };
                    let parent = relative
                        .parent()
                        .unwrap_or(Path::new(""))
                        .to_string_lossy()
                        .replace('\\', "/");
                    let name = relative
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    groups.entry(parent).or_default().push((path, name, false));
                }
                // 文件路径可能很长；保留全文并水平滚动，不能撑大侧栏布局。
                egui::ScrollArea::both()
                    .auto_shrink([false, true])
                    .id_salt("files")
                    .show(ui, |ui| {
                        for (folder, entries) in groups {
                            let mut draw = |ui: &mut egui::Ui| {
                                for (path, name, dirty) in &entries {
                                    let prefix = if *path == self.project.entry {
                                        "主"
                                    } else {
                                        "·"
                                    };
                                    let label = format!(
                                        "{prefix}  {name}{}",
                                        if *dirty { "  ●" } else { "" }
                                    );
                                    let response = ui
                                        .add(egui::Button::selectable(
                                            self.active_file == *path,
                                            RichText::new(label).size(12.0),
                                        ))
                                        .on_hover_text(path.display().to_string());
                                    response.context_menu(|ui| {
                                        if *path != self.project.entry
                                            && self.project.documents.contains_key(path)
                                            && ui.button("安全移动 / 重命名路径…").clicked()
                                        {
                                            self.begin_source_move(path.clone());
                                            ui.close();
                                        }
                                        if ui.button("复制完整来源").clicked() {
                                            ui.ctx().copy_text(path.display().to_string());
                                            ui.close();
                                        }
                                    });
                                    if response.clicked() {
                                        if self.project.documents.contains_key(path) {
                                            self.remember_author_position();
                                            self.entity_source_navigation = None;
                                            self.active_file = path.clone();
                                            self.tab = Tab::Edit;
                                        } else if let Err(error) =
                                            crate::media::open_reference(&self.project.root, path)
                                        {
                                            self.io_error = Some(error);
                                        }
                                    }
                                }
                            };
                            if folder.is_empty() {
                                draw(ui);
                            } else {
                                egui::CollapsingHeader::new(folder)
                                    .default_open(true)
                                    .show(ui, draw);
                            }
                        }
                    });
                ui.add_space(20.0);
                theme::card().show(ui, |ui| {
                    ui.label(
                        RichText::new("共享 ID · 分文件创作")
                            .size(12.0)
                            .color(ACCENT()),
                    );
                    ui.label(theme::muted(
                        "工作区及子目录递归索引。其他资料随完整目录导出。",
                    ));
                });
            });
        self.personal.settings.navigation_width = panel.response.rect.width();
    }
}
