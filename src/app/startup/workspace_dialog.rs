//! 应用内路径输入始终可取消、重试；系统目录选择器只是明确点击后的辅助。
use super::{PathBuf, WorldeditApp};
use crate::app::Pending;
use crate::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum WorkspaceOperation {
    New,
    Open,
}

pub(in crate::app) struct WorkspaceDialog {
    pub operation: WorkspaceOperation,
    pub path: String,
    focus: bool,
}

impl WorkspaceDialog {
    fn new(operation: WorkspaceOperation, path: String) -> Self {
        Self {
            operation,
            path,
            focus: true,
        }
    }

    fn accept_native_selection(&mut self, selected: Option<PathBuf>) {
        if let Some(path) = selected {
            self.path = path.to_string_lossy().into_owned();
            self.focus = true;
        }
    }
}

impl WorldeditApp {
    pub(in crate::app) fn begin_workspace_dialog(&mut self, operation: WorkspaceOperation) {
        let path = if operation == WorkspaceOperation::Open && self.saved_location {
            self.project.root.to_string_lossy().into_owned()
        } else {
            String::new()
        };
        self.native_startup.dialog = Some(WorkspaceDialog::new(operation, path));
        self.io_error = None;
    }

    pub(in crate::app) fn workspace_dialog(&mut self, ctx: &egui::Context) {
        let composing = self.workspace_input_composing(ctx);
        let Some(mut dialog) = self.native_startup.dialog.take() else {
            return;
        };
        let creating = dialog.operation == WorkspaceOperation::New;
        let mut confirm = false;
        let mut cancel = !composing
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let width = (ctx.screen_rect().width() - 64.0).clamp(220.0, 600.0);
        egui::Modal::new(egui::Id::new("workspace-startup-dialog")).show(ctx, |ui| {
            ui.set_width(width);
            ui.heading(if creating { "新建作品" } else { "打开工程" });
            egui::ScrollArea::vertical()
                .max_height((ctx.screen_rect().height() - 180.0).max(100.0))
                .show(ui, |ui| {
                    ui.label(if creating {
                        "填写新工作区文件夹的完整路径，也可以选择已有空文件夹。确认创建后才写入空白作品。"
                    } else {
                        "选择已有工程文件夹，或填写入口 .wl 文件的完整路径。打开不会将空文件夹初始化成工程。"
                    });
                    ui.add_space(12.0);
                    ui.label(if creating { "新工作区路径" } else { "工程路径" });
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut dialog.path)
                            .id(egui::Id::new("workspace-startup-path"))
                            .hint_text("输入文件夹的完整路径")
                            .desired_width(f32::INFINITY),
                    );
                    if dialog.focus {
                        response.request_focus();
                        dialog.focus = false;
                    }
                    confirm |= !composing
                        && (response.has_focus() || response.lost_focus())
                        && ctx.input_mut(|input| {
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                        });
                    if ui.button("选择文件夹（可选）").clicked() {
                        let mut picker = rfd::FileDialog::new().set_title(if creating {
                            "选择新作品的空文件夹"
                        } else {
                            "选择已有工程文件夹"
                        });
                        if self.saved_location {
                            picker = picker.set_directory(&self.project.root);
                        }
                        dialog.accept_native_selection(picker.pick_folder());
                    }
                    if let Some(error) = &self.io_error {
                        ui.add_space(8.0);
                        ui.colored_label(theme::ERROR(), error);
                    }
                });
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                confirm |= theme::add_enabled(
                    ui,
                    !composing,
                    theme::primary(if creating { "创建作品" } else { "打开工程" }),
                )
                .clicked();
                cancel |= ui.button("取消").clicked();
            });
        });
        if cancel {
            return;
        }
        if confirm {
            let path = PathBuf::from(dialog.path.trim());
            if dialog.path.trim().is_empty() {
                self.io_error = Some("请填写工作区路径，或选择文件夹".into());
            } else if !path.is_absolute() {
                self.io_error = Some("请填写工作区的绝对完整路径".into());
            } else if creating {
                if self.create_native_project(&path) {
                    return;
                }
            } else {
                self.io_error = None;
                self.request_action(Pending::Open(path), ctx);
                if self.io_error.is_none() {
                    return;
                }
            }
        }
        self.native_startup.dialog = Some(dialog);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_picker_cancel_keeps_the_typed_path_and_can_be_repeated() {
        let mut dialog = WorkspaceDialog::new(WorkspaceOperation::New, "/original".into());
        for _ in 0..3 {
            dialog.accept_native_selection(None);
            assert_eq!(dialog.path, "/original");
        }
        dialog.accept_native_selection(Some(PathBuf::from("/chosen")));
        assert_eq!(dialog.path, "/chosen");
        dialog.accept_native_selection(None);
        assert_eq!(dialog.path, "/chosen");
    }
}
