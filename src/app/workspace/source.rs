mod editor;
mod gutter;
mod readonly;
mod text;

use super::super::{Tab, WorldeditApp};
use crate::theme::{self, *};
use egui::RichText;
use std::path::Path;
use worldline_core::Severity;

impl WorldeditApp {
    pub(in crate::app) fn source_tab(&mut self, ctx: &egui::Context) {
        if !self.active_file.is_absolute() {
            self.active_file = super::workspace_source_path(&self.project, &self.active_file);
        }
        if self.personal.settings.diagnostics && !self.personal.settings.focus {
            egui::SidePanel::right("diagnostics")
                .default_width(300.0)
                .width_range(240.0..=420.0)
                .frame(theme::panel())
                .show(ctx, |ui| {
                    ui.label(RichText::new("工程诊断").strong().size(17.0));
                    ui.label(theme::muted("检查工作区全部源码，点击定位"));
                    ui.separator();
                    let diagnostics = self.diagnostics().to_vec();
                    if diagnostics.is_empty() {
                        ui.colored_label(ACCENT(), "✓ 所有文件校验通过");
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("diagnostics-scroll")
                        .show(ui, |ui| {
                            for (i, d) in diagnostics.iter().enumerate() {
                                ui.push_id(i, |ui| {
                                    let color = match d.severity {
                                        Severity::Error => ERROR(),
                                        Severity::Warning => GOLD(),
                                        Severity::Hint => BLUE(),
                                    };
                                    theme::card().show(ui, |ui| {
                                        if ui
                                            .add(egui::Button::new(
                                                RichText::new(format!(
                                                    "{}  {}:{}",
                                                    d.code,
                                                    Path::new(&d.file)
                                                        .file_name()
                                                        .unwrap_or_default()
                                                        .to_string_lossy(),
                                                    d.span.line
                                                ))
                                                .size(12.0)
                                                .color(color),
                                            ))
                                            .clicked()
                                        {
                                            self.jump_to_file(&d.file, d.span.line, d.span.column);
                                        }
                                        ui.label(RichText::new(&d.message).size(12.0));
                                        if let Some(note) = &d.note {
                                            ui.label(theme::muted(note));
                                        }
                                    });
                                });
                            }
                        });
                });
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
