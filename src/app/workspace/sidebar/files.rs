//! 文件索引保留全部工作区文件、完整来源和安全移动入口。
use crate::app::{Tab, WorldeditApp};
use crate::theme;
use egui::RichText;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

impl WorldeditApp {
    pub(super) fn sidebar_files(&mut self, ui: &mut egui::Ui) {
        #[cfg(target_arch = "wasm32")]
        let ctx = ui.ctx().clone();
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
                crate::web::select_files(&ctx, false, ".wl", crate::web::FileAction::Include);
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
        let other_files: Vec<PathBuf> = self.disk_stamp.iter().map(|(p, _, _)| p.clone()).collect();
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
                            let label =
                                format!("{prefix}  {name}{}", if *dirty { "  ●" } else { "" });
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
    }
}
