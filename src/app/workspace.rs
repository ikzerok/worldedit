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

fn char_to_byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

fn active_mention(text: &str, cursor_char: usize) -> Option<(usize, String)> {
    let prefix: String = text.chars().take(cursor_char).collect();
    let (at_byte, query) = prefix.rsplit_once('@')?;
    if query.chars().any(char::is_whitespace) || query.contains('@') {
        return None;
    }
    Some((at_byte.chars().count(), query.to_owned()))
}

fn source_link_at_cursor(
    source: &str,
    path: &Path,
    cursor_char: usize,
    links: &[worldline_core::navigation::TextLinkInfo],
) -> Option<worldline_core::TargetRef> {
    let cursor_byte = char_to_byte(source, cursor_char);
    let lines: Vec<_> = source.split_inclusive('\n').collect();
    for link in links.iter().filter(|link| Path::new(&link.file) == path) {
        let line_index = link.line.saturating_sub(1) as usize;
        let Some(raw_line) = lines.get(line_index) else {
            continue;
        };
        let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let Ok(markup) =
            worldline_core::navigation::link_source(&link.target, &link.label, &link.file)
        else {
            continue;
        };
        let line_start: usize = lines.iter().take(line_index).map(|line| line.len()).sum();
        for (offset, _) in line.match_indices(&markup) {
            let start = line_start + offset;
            let end = start + markup.len();
            if (start..=end).contains(&cursor_byte) {
                return Some(link.target.clone());
            }
        }
    }
    None
}

fn source_selection(
    source: &str,
    path: &Path,
    range: egui::text::CCursorRange,
) -> Option<worldline_core::authoring_intents::TextSelection> {
    let start = range.primary.index.min(range.secondary.index);
    let end = range.primary.index.max(range.secondary.index);
    if start == end {
        return None;
    }
    let byte_start = char_to_byte(source, start);
    let byte_end = char_to_byte(source, end);
    let selected = source.get(byte_start..byte_end)?;
    if selected.trim().is_empty() || selected.contains(['\n', '\r']) {
        return None;
    }
    Some(worldline_core::authoring_intents::TextSelection {
        path: path.to_path_buf(),
        start: byte_start,
        end: byte_end,
        expected_text: selected.to_owned(),
    })
}

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
                    Tab::Network,
                    Tab::Review,
                    Tab::Map,
                    Tab::Characters,
                    Tab::World,
                    Tab::Catalog,
                    Tab::Wiki,
                    Tab::Overview,
                    Tab::Manuscript,
                    Tab::Templates,
                    Tab::CheckpointHistory,
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
        if !self.active_file.is_absolute() {
            self.active_file =
                super::workspace_source_path(&self.project, &self.active_file);
        }
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
                            theme::GOLD,
                            "另一份源码仍有未提交的输入法草稿。请返回该文件并处理草稿后再继续。",
                        );
                        if ui.button("返回未提交源码").clicked() {
                            self.active_file = pending_path;
                            self.tab = Tab::Edit;
                        }
                        return;
                    }
                }
                let Ok(mut text) = self.project.document(&path).map(str::to_string) else {
                    let Ok(document) = self.project.authoring_document(&path) else {
                        return;
                    };
                    let raw = document.bytes().to_vec();
                    let read_only = document.is_read_only();
                    let mut text = match String::from_utf8(raw.clone()) {
                        Ok(text) => text,
                        Err(_) => {
                            self.page_heading(
                                ui,
                                &relative,
                                "展示文档原始字节（只读预览）",
                            );
                            ui.label(theme::muted(
                                "原始文档不是有效 UTF-8，本视图只读，原字节保持不变；请通过外部编辑器或另存入口修复。",
                            ));
                            let mut preview = String::from_utf8_lossy(&raw).into_owned();
                            ui.add(
                                egui::TextEdit::multiline(&mut preview)
                                    .code_editor()
                                    .font(egui::FontId::monospace(14.0))
                                    .desired_width(ui.available_width().max(500.0))
                                    .desired_rows(36)
                                    .interactive(false),
                            );
                            return;
                        }
                    };
                    let id = egui::Id::new(("authoring-source", &path));
                    let target = self.jump.take();
                    let mut changed = false;
                    self.page_heading(
                        ui,
                        &relative,
                        "展示文档原文 · 可修复损坏 JSON；Ctrl+S 保存全部文件",
                    );
                    ui.label(theme::muted(
                        "地图和其他展示文档由 core 保留原始字节；结构化命令仍从地图画布提交。",
                    ));
                    ui.add_enabled_ui(!read_only, |ui| {
                        let mut output = egui::TextEdit::multiline(&mut text)
                            .id(id)
                            .code_editor()
                            .font(egui::FontId::monospace(14.0))
                            .desired_width(ui.available_width().max(500.0))
                            .desired_rows(36)
                            .show(ui);
                        changed = output.response.changed();
                        if let Some((line, column)) = target {
                            let offset = text
                                .split_inclusive('\n')
                                .take(line.saturating_sub(1) as usize)
                                .map(|line| line.chars().count())
                                .sum::<usize>()
                                + column.saturating_sub(1) as usize;
                            let cursor = egui::text::CCursor::new(offset.min(text.chars().count()));
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
                    if read_only {
                        ui.label(theme::muted("此展示文档格式或能力未知，只读查看。"));
                    }
                    if changed {
                        let before = self.project.clone();
                        match self
                            .project
                            .set_authoring_document(&path, text.into_bytes())
                        {
                            Ok(()) => {
                                self.remember(before);
                                self.recompile();
                            }
                            Err(error) => self.io_error = Some(error),
                        }
                    }
                    return;
                };
                if let Some((ime_path, ime_text, _)) = &self.ime_source_draft {
                    if ime_path == &path {
                        text = ime_text.clone();
                    }
                }
                self.page_heading(
                    ui,
                    &relative,
                    "当前缓冲区与整个工程一起编译 · Ctrl+Enter 打开源码引用或按选中文本建档 · Ctrl+S 保存全部文件",
                );
                let stale_ime_draft = self
                    .ime_source_draft
                    .as_ref()
                    .filter(|(draft_path, _, _)| draft_path == &path)
                    .is_some_and(|(_, _, baseline)| {
                        self.project.document(&path).ok() != Some(baseline.as_str())
                    });
                if stale_ime_draft {
                    ui.colored_label(
                        theme::GOLD,
                        "源码在输入法组合期间被外部修改。草稿已保留，尚未写入工程。",
                    );
                    if ui.button("放弃本地草稿并恢复外部版本").clicked() {
                        self.ime_source_draft = None;
                        self.ime_source_baseline = None;
                        self.ime_composing = false;
                        return;
                    }
                }
                let id = egui::Id::new(("source", &path));
                let target = self.jump.take();
                let mut changed = false;
                let ime_events = ctx.input(|input| input.events.clone());
                for event in &ime_events {
                    match event {
                        egui::Event::Ime(egui::ImeEvent::Enabled | egui::ImeEvent::Preedit(_)) => {
                            self.ime_composing = true;
                            if self
                                .ime_source_baseline
                                .as_ref()
                                .is_none_or(|(baseline_path, _)| baseline_path != &path)
                            {
                                self.ime_source_baseline = self
                                    .project
                                    .document(&path)
                                    .ok()
                                    .map(|baseline| (path.clone(), baseline.to_owned()));
                            }
                        }
                        egui::Event::Ime(egui::ImeEvent::Commit(_) | egui::ImeEvent::Disabled) => {
                            self.ime_composing = false;
                        }
                        _ => {}
                    }
                }
                let mut mention_action = None;
                let mut mention_popup = None;
                let mut mention_anchor = None;
                let mut create_from_selection = None;
                let mut selected_source_text = None;
                let mut selection_anchor = None;
                let mut source_link_action = None;
                let language_version = self.project.language_version_kind();
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
                                        f.layout_job(highlight::layout_job(
                                            buffer.as_str(),
                                            14.0,
                                            language_version,
                                        ))
                                    })
                                };
                            let editor_state =
                                egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                            let editor_focused = ctx.memory(|memory| memory.has_focus(id));
                            if editor_focused && !self.ime_composing {
                                if let Some(range) = editor_state.cursor.char_range() {
                                    let cursor_char = range.primary.index;
                                    let mention = (range.primary.index == range.secondary.index)
                                        .then(|| active_mention(&text, cursor_char))
                                        .flatten()
                                        .filter(|(_, query)| !query.is_empty());
                                    if let Some((at_char, query)) = mention {
                                        let candidates = self
                                            .snapshot
                                            .as_ref()
                                            .map(|snapshot| {
                                                snapshot
                                                    .result
                                                    .analysis
                                                    .catalog
                                                    .search_objects(&query)
                                            })
                                            .unwrap_or_default();
                                        if !candidates.is_empty() {
                                            let matches_current = self
                                                .mention_selection
                                                .as_ref()
                                                .is_some_and(|(selected_path, selected_at, selected_query, _)| {
                                                    selected_path == &path
                                                        && *selected_at == at_char
                                                        && selected_query == &query
                                                });
                                            let mut selected_index = self
                                                .mention_selection
                                                .as_ref()
                                                .filter(|_| matches_current)
                                                .map(|(_, _, _, index)| *index)
                                                .unwrap_or(0)
                                                .min(candidates.len() - 1);
                                            let moved_down = ctx.input_mut(|input| {
                                                input.consume_key(
                                                    egui::Modifiers::NONE,
                                                    egui::Key::ArrowDown,
                                                )
                                            });
                                            let moved_up = ctx.input_mut(|input| {
                                                input.consume_key(
                                                    egui::Modifiers::NONE,
                                                    egui::Key::ArrowUp,
                                                )
                                            });
                                            if moved_down {
                                                selected_index =
                                                    (selected_index + 1) % candidates.len();
                                            } else if moved_up {
                                                selected_index = if selected_index == 0 {
                                                    candidates.len() - 1
                                                } else {
                                                    selected_index - 1
                                                };
                                            }
                                            if moved_down || moved_up {
                                                self.mention_selection = Some((
                                                    path.clone(),
                                                    at_char,
                                                    query.clone(),
                                                    selected_index,
                                                ));
                                            }
                                            if ctx.input_mut(|input| {
                                                input.consume_key(
                                                    egui::Modifiers::NONE,
                                                    egui::Key::Escape,
                                                )
                                            }) {
                                                self.mention_suppression = Some((
                                                    path.clone(),
                                                    at_char,
                                                    query.clone(),
                                                ));
                                            } else if ctx.input_mut(|input| {
                                                input.consume_key(
                                                    egui::Modifiers::NONE,
                                                    egui::Key::Enter,
                                                )
                                            }) {
                                                mention_action = Some((
                                                    at_char,
                                                    query.clone(),
                                                    candidates[selected_index].target.clone(),
                                                ));
                                            }
                                        }
                                    }

                                    if let Some(selection) = source_selection(&text, &path, range) {
                                        if ctx.input_mut(|input| {
                                            input.consume_key(
                                                egui::Modifiers::COMMAND,
                                                egui::Key::Enter,
                                            )
                                        }) {
                                            create_from_selection = Some(selection);
                                        }
                                    } else if range.primary.index == range.secondary.index {
                                        let target = self.snapshot.as_ref().and_then(|snapshot| {
                                            source_link_at_cursor(
                                                &text,
                                                &path,
                                                cursor_char,
                                                &snapshot.result.analysis.catalog.text_links,
                                            )
                                        });
                                        if let Some(target) = target {
                                            if ctx.input_mut(|input| {
                                                input.consume_key(
                                                    egui::Modifiers::COMMAND,
                                                    egui::Key::Enter,
                                                )
                                            }) {
                                                source_link_action = Some((target, cursor_char));
                                            }
                                        }
                                    }
                                }
                            }
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
                            if output.response.clicked() {
                                if let (Some(pointer), Some(snapshot)) = (
                                    output.response.interact_pointer_pos(),
                                    self.snapshot.as_ref(),
                                ) {
                                    let cursor = output
                                        .galley
                                        .cursor_from_pos(pointer - output.galley_pos)
                                        .index;
                                    if let Some(target) = source_link_at_cursor(
                                        &text,
                                        &path,
                                        cursor,
                                        &snapshot.result.analysis.catalog.text_links,
                                    ) {
                                        source_link_action = Some((target, cursor));
                                    }
                                }
                            }
                            let active = if self.ime_composing {
                                None
                            } else {
                                output
                                    .state
                                    .cursor
                                    .char_range()
                                    .filter(|range| range.primary.index == range.secondary.index)
                                    .and_then(|range| {
                                        active_mention(&text, range.primary.index)
                                    })
                                    .filter(|(_, query)| !query.is_empty())
                            };
                            let active_key = active
                                .as_ref()
                                .map(|(at_char, query)| (path.clone(), *at_char, query.clone()));
                            if self
                                .mention_suppression
                                .as_ref()
                                .is_some_and(|suppressed| Some(suppressed) != active_key.as_ref())
                            {
                                self.mention_suppression = None;
                            }
                            if self.mention_selection.as_ref().is_some_and(
                                |(selected_path, selected_at, selected_query, _)| {
                                    !active_key.as_ref().is_some_and(
                                        |(active_path, active_at, active_query)| {
                                            active_path == selected_path
                                                && active_at == selected_at
                                                && active_query == selected_query
                                        },
                                    )
                                },
                            ) {
                                self.mention_selection = None;
                            }
                            if let Some((at_char, query)) = active {
                                let key = (path.clone(), at_char, query.clone());
                                if self.mention_suppression.as_ref() != Some(&key) {
                                    if ctx.input_mut(|input| {
                                        input.consume_key(
                                            egui::Modifiers::NONE,
                                            egui::Key::Escape,
                                        )
                                    }) {
                                        self.mention_suppression = Some(key);
                                    } else {
                                        let candidates = self
                                            .snapshot
                                            .as_ref()
                                            .map(|snapshot| {
                                                snapshot
                                                    .result
                                                    .analysis
                                                    .catalog
                                                    .search_objects(&query)
                                            })
                                            .unwrap_or_default();
                                        let index = self
                                            .mention_selection
                                            .as_ref()
                                            .filter(|(
                                                selected_path,
                                                selected_at,
                                                selected_query,
                                                _,
                                            )| {
                                                selected_path == &path
                                                    && *selected_at == at_char
                                                    && selected_query == &query
                                            })
                                            .map(|(_, _, _, index)| *index)
                                            .unwrap_or(0)
                                            .min(candidates.len().saturating_sub(1));
                                        let cursor = egui::text::CCursor::new(
                                            output
                                                .state
                                                .cursor
                                                .char_range()
                                                .map(|range| range.primary.index)
                                                .unwrap_or_default(),
                                        );
                                        mention_anchor = Some(
                                            output
                                                .galley
                                                .pos_from_cursor(cursor)
                                                .translate(output.galley_pos.to_vec2())
                                                .left_bottom(),
                                        );
                                        if !candidates.is_empty() {
                                            mention_popup =
                                                Some((at_char, query, candidates, index));
                                        }
                                    }
                                }
                            }
                            if let Some(range) = output.state.cursor.char_range() {
                                if let Some(selection) = source_selection(&text, &path, range) {
                                    let start = range.primary.index.min(range.secondary.index);
                                    let cursor = egui::text::CCursor::new(start);
                                    selection_anchor = Some(
                                        output
                                            .galley
                                            .pos_from_cursor(cursor)
                                            .translate(output.galley_pos.to_vec2())
                                            .left_bottom(),
                                    );
                                    selected_source_text = Some(selection);
                                }
                            }
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
                if let (Some((at_char, query, candidates, selected_index)), Some(anchor)) =
                    (mention_popup.take(), mention_anchor)
                {
                    let screen = ctx.screen_rect();
                    let pos = egui::pos2(
                        anchor.x.clamp(screen.left() + 8.0, (screen.right() - 440.0).max(screen.left() + 8.0)),
                        anchor.y.clamp(screen.top() + 8.0, (screen.bottom() - 120.0).max(screen.top() + 8.0)),
                    );
                    egui::Area::new(egui::Id::new(("source-mention-popup", &path)))
                        .order(egui::Order::Foreground)
                        .fixed_pos(pos)
                        .show(ctx, |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                ui.label(format!("引用 @{query}："));
                                ui.horizontal_wrapped(|ui| {
                                    for (index, candidate) in candidates.into_iter().enumerate() {
                                        let label = format!(
                                            "{} · {} · {}:{}",
                                            super::catalog::kind_label(&candidate.target.kind),
                                            candidate.display,
                                            candidate.target.kind,
                                            candidate.target.id
                                        );
                                        if ui.selectable_label(index == selected_index, &label).clicked() {
                                            mention_action = Some((
                                                at_char,
                                                query.clone(),
                                                candidate.target,
                                            ));
                                        }
                                    }
                                });
                                ui.label(theme::muted("↑ / ↓ 选择 · Enter 插入 · Esc 收起"));
                            });
                        });
                }
                if let (Some(selection), Some(anchor)) =
                    (selected_source_text.take(), selection_anchor)
                {
                    let screen = ctx.screen_rect();
                    let pos = egui::pos2(
                        anchor.x.clamp(screen.left() + 8.0, (screen.right() - 220.0).max(screen.left() + 8.0)),
                        anchor.y.clamp(screen.top() + 8.0, (screen.bottom() - 44.0).max(screen.top() + 8.0)),
                    );
                    egui::Area::new(egui::Id::new(("source-selection-action", &path)))
                        .order(egui::Order::Foreground)
                        .fixed_pos(pos)
                        .show(ctx, |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                ui.label(theme::muted("Ctrl+Enter 也可从选中文本建档"));
                                if ui.button("从选中文本建档").clicked() {
                                    create_from_selection = Some(selection);
                                }
                            });
                        });
                }
                if self.ime_composing {
                    if changed {
                        if let Some((baseline_path, baseline)) = &self.ime_source_baseline {
                            if baseline_path == &path {
                                self.ime_source_draft =
                                    Some((path.clone(), text, baseline.clone()));
                            }
                        }
                    }
                } else if changed {
                    let baseline = self
                        .ime_source_draft
                        .as_ref()
                        .filter(|(draft_path, _, _)| draft_path == &path)
                        .map(|(_, _, baseline)| baseline.clone())
                        .or_else(|| {
                            self.ime_source_baseline
                                .as_ref()
                                .filter(|(baseline_path, _)| baseline_path == &path)
                                .map(|(_, baseline)| baseline.clone())
                        });
                    let current = self.project.document(&path).ok().map(str::to_owned);
                    if baseline.as_ref().is_some_and(|baseline| {
                        current.as_deref() != Some(baseline.as_str())
                    }) {
                        let baseline = baseline.unwrap();
                        self.ime_source_draft = Some((path.clone(), text, baseline));
                        self.io_error = Some(
                            "源码在输入法组合期间被外部修改。输入已保留；可复制草稿，或放弃草稿并恢复外部版本。".into(),
                        );
                    } else {
                        let before = self.project.clone();
                        if self.project.set_text(&path, text).is_ok() {
                            self.remember(before);
                            self.recompile();
                            self.ime_source_draft = None;
                            self.ime_source_baseline = None;
                        }
                    }
                } else if !self.ime_composing {
                    self.ime_source_baseline = None;
                }
                if let Some((at_char, query, target)) = mention_action {
                    self.apply_source_mention(ctx, &path, at_char, &query, target);
                }
                if let Some(selection) = create_from_selection {
                    self.edit_entity(None);
                    if let Some(form) = self.entity_editor.as_mut() {
                        form.source_selection = Some(selection.clone());
                        form.draft.display = selection.expected_text.clone();
                        form._focus_name_on_open = true;
                    }
                }
                if let Some((target, cursor)) = source_link_action {
                    self.reading_return = Some((path.clone(), cursor));
                    self.open_reading(target);
                }
            });
    }

    fn apply_source_mention(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        at_char: usize,
        query: &str,
        target: worldline_core::TargetRef,
    ) {
        use worldline_core::authoring_intents::{AuthoringIntent, IntentTarget, TextSelection};

        let Some(source) = self.project.document(path).ok().map(str::to_owned) else {
            self.io_error = Some("活动源码已关闭，未插入引用".into());
            return;
        };
        let trigger = char_to_byte(&source, at_char);
        let Some(after_trigger) = trigger.checked_add(1) else {
            self.io_error = Some("@ 引用位置已失效".into());
            return;
        };
        if source.get(trigger..after_trigger) != Some("@") {
            self.io_error = Some("@ 引用位置已变化，请重新输入".into());
            return;
        }
        let mut candidate = self.project.clone();
        let mut staged_source = source;
        staged_source.replace_range(trigger..after_trigger, "");
        if let Err(error) = candidate.set_text(path, staged_source.clone()) {
            self.io_error = Some(error);
            return;
        }
        let selection_start = char_to_byte(&staged_source, at_char);
        let selection_end = selection_start + query.len();
        let intent = AuthoringIntent {
            expected_baseline: candidate.content_baseline(),
            target: IntentTarget::Existing(target.clone()),
            selection: Some(TextSelection {
                path: path.to_path_buf(),
                start: selection_start,
                end: selection_end,
                expected_text: query.to_owned(),
            }),
            placement: None,
        };
        let before = self.project.clone();
        match candidate.apply_authoring_intent(&intent) {
            Ok(_) => {
                self.project = candidate;
                let applied = self.finish_content_command(
                    before,
                    Ok(()),
                    "正文引用已插入；保存全部可写入作品目录",
                );
                if applied {
                    let link = worldline_core::navigation::link_source(
                        &target,
                        query,
                        &path.to_string_lossy(),
                    );
                    if let Ok(link) = link {
                        let cursor = at_char + link.chars().count();
                        let id = egui::Id::new(("source", path));
                        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(
                                egui::text::CCursor::new(cursor),
                            )));
                        egui::TextEdit::store_state(ctx, id, state);
                        ctx.memory_mut(|memory| memory.request_focus(id));
                    }
                }
            }
            Err(error) => self.io_error = Some(error),
        }
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
