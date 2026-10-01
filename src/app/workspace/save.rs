#[cfg(not(target_arch = "wasm32"))]
use super::super::{DirectoryDialog, DirectoryOperation, Pending};
use super::super::{Tab, WorldeditApp};
use crate::theme;
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

impl WorldeditApp {
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn open_dialog(&mut self, ctx: &egui::Context, _folder: bool) {
        let dialog = rfd::FileDialog::new().set_directory(&self.project.root);
        let path = dialog.pick_folder();
        if let Some(path) = path {
            self.request_action(Pending::Open(path), ctx);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn save(&mut self) -> bool {
        if self.ime_composing || self.ime_source_draft.is_some() {
            self.io_error = Some("正文仍有未提交的输入法草稿，请先完成输入或恢复外部版本。".into());
            return false;
        }
        if !self.saved_location {
            self.directory_dialog(false);
            return false;
        }
        match self.project.save() {
            Ok(()) => {
                self.recompile();
                self.message = Some("全部文件已保存".into());
                self.io_error = None;
                true
            }
            Err(e) => {
                self.io_error = Some(e);
                false
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn directory_dialog(&mut self, export: bool) {
        if !export && (self.ime_composing || self.ime_source_draft.is_some()) {
            self.io_error = Some("正文仍有未提交的输入法草稿，请先完成输入或恢复外部版本。".into());
            return;
        }
        let id = self
            .snapshot
            .as_ref()
            .and_then(|s| s.result.analysis.world.as_ref())
            .map(|w| w.id.as_str())
            .unwrap_or("world-project");
        let parent = if self.saved_location {
            self.project.root.parent().unwrap_or(Path::new("."))
        } else {
            Path::new(".")
        };
        self.directory = Some(DirectoryDialog {
            operation: if export {
                DirectoryOperation::ExportDirectory
            } else {
                DirectoryOperation::SaveAs
            },
            path: parent
                .join(format!("{id}{}", if export { "-export" } else { "" }))
                .to_string_lossy()
                .into_owned(),
        });
    }
    pub(in crate::app) fn dialogs(&mut self, ctx: &egui::Context) {
        self.capture_new_draft_baselines();
        if let Some((mut id, mut display, mut parent)) = self.new_period.take() {
            let mut save = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("period-dialog")).show(ctx, |ui| {
                ui.set_width(390.0);
                ui.heading("时段档案");
                let existing = self.snapshot.as_ref().is_some_and(|s| {
                    s.result
                        .analysis
                        .timeline
                        .periods
                        .iter()
                        .any(|p| p.id == id)
                });
                ui.add_enabled_ui(!existing, |ui| {
                    super::super::inspector::field(ui, "时段 ID", &mut id)
                });
                super::super::inspector::field(ui, "时段名称 / 起止时间", &mut display);
                ui.label(theme::muted("上级时段（大时段包含此时段）"));
                egui::ComboBox::from_id_salt("parent-period")
                    .selected_text(parent.as_deref().unwrap_or("无，作为顶层时段"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut parent, None, "无，作为顶层时段");
                        if let Some(snapshot) = &self.snapshot {
                            for period in snapshot
                                .result
                                .analysis
                                .timeline
                                .periods
                                .iter()
                                .filter(|p| p.id != id)
                            {
                                ui.selectable_value(
                                    &mut parent,
                                    Some(period.id.clone()),
                                    format!("{} · {}", period.display, period.id),
                                );
                            }
                        }
                    });
                ui.label(theme::muted(
                    "同一时段的事件默认无序;可为其中一部分明确指定先后。",
                ));
                ui.horizontal(|ui| {
                    save = ui.add(theme::primary("保存时段")).clicked();
                    cancel = ui.button("取消").clicked();
                });
            });
            if save
                && self.commit("时段已保存", |p| {
                    p.write_period_with_parent(&id, &display, parent.as_deref())
                })
            {
                cancel = true;
            }
            if !cancel {
                self.new_period = Some((id, display, parent));
            }
        }
        if self.pending.is_some() && self.directory.is_none() {
            let mut save = false;
            let mut discard = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("unsaved")).show(ctx, |ui| {
                ui.heading("保存当前工程？");
                ui.label("部分文件有未保存修改。保存后再继续,或放弃本次修改。");
                ui.horizontal(|ui| {
                    save = ui.add(theme::primary("保存并继续")).clicked();
                    discard = ui.button("放弃修改").clicked();
                    cancel = ui.button("取消").clicked();
                });
            });
            if cancel {
                self.pending = None;
            }
            if discard || (save && self.save()) {
                if let Some(action) = self.pending.take() {
                    self.perform_action(action, ctx);
                }
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(mut dialog) = self.directory.take() {
            let mut confirm = false;
            let mut cancel = false;
            let export = dialog.operation != DirectoryOperation::SaveAs;
            let zip = dialog.operation == DirectoryOperation::ExportZip;
            egui::Modal::new(egui::Id::new("directory")).show(ctx, |ui| {
                ui.set_width(540.0);
                ui.heading(match dialog.operation {
                    DirectoryOperation::ExportZip => "导出 ZIP 工程包 · 选择目标",
                    DirectoryOperation::ExportDirectory => "导出完整世界工程",
                    DirectoryOperation::SaveAs => "保存到新工程文件夹",
                });
                ui.label(theme::muted(if export {
                    "导出已应用工程的全部文件与子目录，包括未引用资料；未应用输入会另行确认。"
                } else {
                    "将所有文件与未完成的修改保存在同一个文件夹。"
                }));
                ui.add_space(12.0);
                ui.label(if zip {
                    "新 ZIP 文件的绝对完整路径"
                } else {
                    "新文件夹的完整路径"
                });
                if zip {
                    ui.label(theme::muted(
                        "预填路径不会自动写入；可直接改路径，系统选择器仅为可选辅助。",
                    ));
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.path)
                            .id(egui::Id::new("export-target-path"))
                            .desired_width(360.0),
                    );
                    if ui
                        .button(if zip {
                            "系统选择器（可选）"
                        } else {
                            "选择位置"
                        })
                        .clicked()
                    {
                        let selected = if zip {
                            let path = Path::new(&dialog.path);
                            rfd::FileDialog::new()
                                .set_directory(path.parent().unwrap_or(Path::new(".")))
                                .set_file_name(
                                    path.file_name().unwrap_or_default().to_string_lossy(),
                                )
                                .add_filter("ZIP 工程包", &["zip"])
                                .save_file()
                        } else {
                            let name = Path::new(&dialog.path)
                                .file_name()
                                .unwrap_or_default()
                                .to_owned();
                            rfd::FileDialog::new()
                                .pick_folder()
                                .map(|parent| parent.join(name))
                        };
                        dialog.accept_native_selection(selected);
                    }
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    confirm = ui
                        .add(theme::primary(if export {
                            "校验并导出"
                        } else {
                            "保存工程"
                        }))
                        .clicked();
                    cancel = ui.button("取消").clicked();
                });
                if let Some(error) = &self.io_error {
                    ui.colored_label(theme::ERROR(), error);
                }
            });
            if confirm && dialog.path.trim().is_empty() {
                self.io_error = Some("请填写目标完整路径".into());
            } else if confirm && zip && !Path::new(dialog.path.trim()).is_absolute() {
                self.io_error = Some("ZIP 目标必须是绝对完整路径，请选择工作区外的新文件。".into());
            } else if confirm && export {
                let target = PathBuf::from(dialog.path.trim());
                let destination = if zip {
                    super::super::export_scope::ExportDestination::Zip(target)
                } else {
                    super::super::export_scope::ExportDestination::Directory(target)
                };
                cancel = self.request_strict_export(destination);
            } else if confirm {
                let destination = PathBuf::from(dialog.path.trim());
                let active = self
                    .active_file
                    .strip_prefix(&self.project.root)
                    .unwrap_or(Path::new("world.wl"))
                    .to_path_buf();
                let result = if dialog.path.trim().is_empty() {
                    Err("请填写文件夹路径".into())
                } else {
                    self.project.save_as(&destination)
                };
                match result {
                    Ok(()) => {
                        self.io_error = None;
                        self.message = Some(format!("已保存:{}", destination.display()));
                        self.active_file = self.project.root.join(active);
                        self.saved_location = true;
                        self.reset_views();
                        self.recompile();
                        if let Some(action) = self.pending.take() {
                            self.perform_action(action, ctx);
                        }
                        cancel = true;
                    }
                    Err(e) => {
                        self.io_error = Some(e);
                    }
                }
            }
            if !cancel {
                self.directory = Some(dialog);
            }
        }
        if let Some(mut name) = self.new_file.take() {
            let mut confirm = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("new-file")).show(ctx, |ui| {
                ui.heading("新建源文件");
                ui.label(theme::muted("相对工程的路径,创建后自动加入总入口"));
                ui.add(egui::TextEdit::singleline(&mut name).desired_width(370.0));
                ui.horizontal(|ui| {
                    confirm = ui.add(theme::primary("创建文件")).clicked();
                    cancel = ui.button("取消").clicked();
                });
            });
            if confirm {
                let before = self.project.clone();
                match self.project.add_file(Path::new(name.trim())) {
                    Ok(path) => {
                        self.remember(before);
                        self.active_file = path;
                        self.recompile();
                        self.tab = Tab::Edit;
                        cancel = true;
                        self.io_error = None;
                    }
                    Err(e) => self.io_error = Some(e),
                }
            }
            if !cancel {
                self.new_file = Some(name);
            }
        }
    }
}
