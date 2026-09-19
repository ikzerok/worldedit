//! 工程导航、源文件与诊断联动、保存和导出对话框。
use super::{Tab, WorldeditApp};
use crate::{
    highlight,
    theme::{self, *},
};
use egui::RichText;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use worldline_core::Severity;

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
        Tab::Graph => {
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
    pub(super) fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("project")
            .resizable(true)
            .default_width(228.0)
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
                ui.add_space(22.0);
                for tab in [
                    Tab::Timeline,
                    Tab::Graph,
                    Tab::Map,
                    Tab::Characters,
                    Tab::World,
                    Tab::Catalog,
                    Tab::Wiki,
                    Tab::Overview,
                    Tab::Edit,
                ] {
                    let selected = self.tab == tab;
                    let response = ui.add_sized(
                        [ui.available_width(), 38.0],
                        egui::Button::selectable(
                            selected,
                            RichText::new(tab.title()).color(if selected { ACCENT } else { TEXT }),
                        ),
                    );
                    nav_icon(
                        ui.painter(),
                        response.rect.left_center() + egui::vec2(20.0, 0.0),
                        tab,
                        if selected { ACCENT } else { MUTED },
                    );
                    if response.clicked() {
                        self.tab = tab;
                    }
                }
                ui.add_space(20.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(theme::muted("工程文件"));
                    if ui
                        .small_button("＋")
                        .on_hover_text("新建文件并加入总入口")
                        .clicked()
                    {
                        self.new_file = Some("events/chapter.wl".into());
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
                egui::ScrollArea::vertical()
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
                                    if ui
                                        .add(egui::Button::selectable(
                                            self.active_file == *path,
                                            RichText::new(label).size(12.0),
                                        ))
                                        .on_hover_text(path.display().to_string())
                                        .clicked()
                                    {
                                        if self.project.documents.contains_key(path) {
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
                            .color(ACCENT),
                    );
                    ui.label(theme::muted(
                        "工作区及子目录递归索引。其他资料随完整目录导出。",
                    ));
                });
            });
    }

    pub(super) fn source_tab(&mut self, ctx: &egui::Context) {
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
                    ui.colored_label(ACCENT, "✓ 所有文件校验通过");
                }
                egui::ScrollArea::vertical()
                    .id_salt("diagnostics-scroll")
                    .show(ui, |ui| {
                        for (i, d) in diagnostics.iter().enumerate() {
                            ui.push_id(i, |ui| {
                                let color = match d.severity {
                                    Severity::Error => ERROR,
                                    Severity::Warning => GOLD,
                                    Severity::Hint => BLUE,
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
        egui::CentralPanel::default()
            .frame(theme::panel().fill(BG))
            .show(ctx, |ui| {
                let path = self.active_file.clone();
                let relative = path
                    .strip_prefix(&self.project.root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                self.page_heading(
                    ui,
                    &relative,
                    "当前缓冲区与整个工程一起编译 · Ctrl+S 保存全部文件",
                );
                let Ok(mut text) = self.project.document(&path).map(str::to_string) else {
                    return;
                };
                let id = egui::Id::new(("source", &path));
                let target = self.jump.take();
                let mut changed = false;
                egui::ScrollArea::both()
                    .id_salt(("source-scroll", &path))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            let count = text.lines().count().max(1);
                            let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
                            ui.add(egui::Label::new(
                                RichText::new(
                                    (1..=count)
                                        .map(|i| i.to_string())
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                )
                                .monospace()
                                .size(14.0)
                                .line_height(Some(row_height))
                                .color(MUTED),
                            ));
                            ui.separator();
                            let mut layouter =
                                |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, _: f32| {
                                    ui.fonts(|f| {
                                        f.layout_job(highlight::layout_job(buffer.as_str(), 14.0))
                                    })
                                };
                            let mut output = egui::TextEdit::multiline(&mut text)
                                .id(id)
                                .code_editor()
                                .font(egui::FontId::monospace(14.0))
                                .desired_width(ui.available_width().max(500.0))
                                .desired_rows(36)
                                .frame(false)
                                .layouter(&mut layouter)
                                .show(ui);
                            changed = output.response.changed();
                            if let Some((line, column)) = target {
                                let offset = text
                                    .split_inclusive('\n')
                                    .take(line.saturating_sub(1) as usize)
                                    .map(|l| l.chars().count())
                                    .sum::<usize>()
                                    + column.saturating_sub(1) as usize;
                                let cursor =
                                    egui::text::CCursor::new(offset.min(text.chars().count()));
                                output
                                    .state
                                    .cursor
                                    .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
                                output.state.store(ctx, id);
                                output.response.request_focus();
                                let rect = output
                                    .galley
                                    .pos_from_cursor(cursor)
                                    .translate(output.galley_pos.to_vec2());
                                ui.scroll_to_rect(rect, Some(egui::Align::Center));
                            }
                        });
                    });
                if changed {
                    let before = self.project.clone();
                    if self.project.set_text(&path, text).is_ok() {
                        self.remember(before);
                        self.recompile();
                    }
                }
            });
    }

    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
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
                    super::inspector::field(ui, "时段 ID", &mut id)
                });
                super::inspector::field(ui, "时段名称 / 起止时间", &mut display);
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
            egui::Modal::new(egui::Id::new("directory")).show(ctx, |ui| {
                ui.set_width(540.0);
                ui.heading(if dialog.export {
                    "导出完整世界工程"
                } else {
                    "保存到新工程文件夹"
                });
                ui.label(theme::muted(if dialog.export {
                    "保留工作区全部文件与子目录，包括未引用资料。导出前校验整个世界。"
                } else {
                    "将所有文件与未完成的修改保存在同一个文件夹。"
                }));
                ui.add_space(12.0);
                ui.label("新文件夹的完整路径");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut dialog.path).desired_width(420.0));
                    if ui.button("选择位置").clicked() {
                        if let Some(parent) = rfd::FileDialog::new().pick_folder() {
                            let name = Path::new(&dialog.path)
                                .file_name()
                                .unwrap_or_default()
                                .to_owned();
                            dialog.path = parent.join(name).display().to_string();
                        }
                    }
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    confirm = ui
                        .add(theme::primary(if dialog.export {
                            "校验并导出"
                        } else {
                            "保存工程"
                        }))
                        .clicked();
                    cancel = ui.button("取消").clicked();
                });
            });
            if confirm {
                let destination = PathBuf::from(dialog.path.trim());
                let active = self
                    .active_file
                    .strip_prefix(&self.project.root)
                    .unwrap_or(Path::new("world.wl"))
                    .to_path_buf();
                let result = if dialog.path.trim().is_empty() {
                    Err("请填写文件夹路径".into())
                } else if dialog.export {
                    self.project.export(&destination)
                } else {
                    self.project.save_as(&destination)
                };
                match result {
                    Ok(()) => {
                        self.io_error = None;
                        self.message = Some(format!(
                            "{}:{}",
                            if dialog.export {
                                "已导出"
                            } else {
                                "已保存"
                            },
                            destination.display()
                        ));
                        if !dialog.export {
                            self.active_file = self.project.root.join(active);
                            self.saved_location = true;
                            self.reset_views();
                            self.recompile();
                            if let Some(action) = self.pending.take() {
                                self.perform_action(action, ctx);
                            }
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
