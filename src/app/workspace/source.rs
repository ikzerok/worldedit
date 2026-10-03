mod editor;
mod gutter;
mod readonly;
mod text;

use super::super::{Tab, WorldeditApp};
use crate::theme::{self, *};

impl WorldeditApp {
    pub(in crate::app) fn source_tab(&mut self, ctx: &egui::Context) {
        if !self.active_file.is_absolute() {
            self.active_file = super::workspace_source_path(&self.project, &self.active_file);
        }
        egui::CentralPanel::default()
            .frame(theme::panel().fill(BG()))
            .show(ctx, |ui| {
                let path = self.active_file.clone();
                let relative = path
                    .strip_prefix(&self.project.root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                let pending_path = self
                    .ime_source_draft
                    .as_ref()
                    .map(|(pending_path, _, _)| pending_path.clone())
                    .or_else(|| {
                        if self.ime_composing {
                            self.ime_source_baseline
                                .as_ref()
                                .map(|(pending_path, _)| pending_path.clone())
                        } else {
                            None
                        }
                    });
                if let Some(pending_path) = pending_path {
                    if pending_path != path {
                        self.page_heading(ui, &relative, "输入法草稿仍待处理");
                        ui.colored_label(
                            theme::GOLD(),
                            "另一份源码仍有未提交的输入法草稿。请返回该文件并处理草稿后再继续。",
                        );
                        if ui.button("返回未提交源码").clicked() {
                            self.active_file = pending_path;
                            self.tab = Tab::Edit;
                        }
                        return;
                    }
                }
                let Ok(text) = self.project.document(&path).map(str::to_string) else {
                    self.readonly_source_document(ctx, ui, &path, &relative);
                    return;
                };
                self.source_text_editor(ctx, ui, path, relative, text);
            });
    }
}
