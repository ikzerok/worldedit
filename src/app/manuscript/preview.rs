use super::super::WorldeditApp;
use super::*;
use crate::theme;
use std::path::Path;
use worldline_core::ast::{Stmt, TextPart};
use worldline_core::manuscript::ManuscriptIndex;
pub(super) fn draw_reader_preview(
    app: &mut WorldeditApp,
    ui: &mut egui::Ui,
    index: &ManuscriptIndex,
    repair_chapter: &mut Option<String>,
) {
    let mut offset = 0;
    let section_names: HashMap<_, _> = index
        .entries
        .iter()
        .filter(|entry| entry.kind == ManuscriptEntryKind::Section)
        .map(|entry| (entry.id.as_str(), entry.title.as_str()))
        .collect();
    loop {
        let page = index.page(offset, 100);
        for chapter in page.chapters {
            ui.push_id((&chapter.id, "reader"), |ui| {
                ui.separator();
                let section = if chapter.section_path.is_empty() {
                    String::new()
                } else {
                    format!(
                        "{} / ",
                        chapter
                            .section_path
                            .iter()
                            .map(|id| {
                                section_names
                                    .get(id.as_str())
                                    .copied()
                                    .unwrap_or(id.as_str())
                            })
                            .collect::<Vec<_>>()
                            .join(" / ")
                    )
                };
                ui.heading(format!("{}{}", section, chapter.title));
                if let Some(summary) = &chapter.summary {
                    ui.label(theme::muted(summary));
                }
                if let Some(target) = &chapter.perspective {
                    let display = app
                        .snapshot
                        .as_ref()
                        .and_then(|snapshot| snapshot.result.analysis.catalog.object(target))
                        .map(|object| object.display.as_str())
                        .unwrap_or(&target.id);
                    ui.label(theme::muted(format!(
                        "视角：{} · {}:{}",
                        display, target.kind, target.id
                    )));
                }
                if let Some(status) = &chapter.status {
                    ui.label(theme::muted(format!("状态：{status}")));
                }
                if let Some(goal) = &chapter.goal {
                    ui.label(theme::muted(format!("目标：{goal}")));
                }
                let Some(target) = &chapter.target_ref else {
                    ui.colored_label(theme::ERROR, "章节尚未选择正文来源。");
                    return;
                };
                let Some(source) = &chapter.source else {
                    ui.colored_label(
                        theme::ERROR,
                        format!("来源不可用：{}:{}", target.kind, target.id),
                    );
                    return;
                };
                if source.status != ManuscriptReferenceStatus::Resolved {
                    ui.colored_label(theme::ERROR, source_status_text(source.status));
                }
                if let Some(location) = &source.location {
                    ui.label(theme::muted(format!(
                        "{}:{} · {}:{}",
                        target.kind, target.id, location.file, location.line
                    )));
                }
                match target.kind.as_str() {
                    "event" => {
                        let event_parts = app.snapshot.as_ref().and_then(|snapshot| {
                            snapshot
                                .result
                                .program
                                .events
                                .iter()
                                .zip(&snapshot.result.program.event_files)
                                .find(|(event, file)| {
                                    event.name == target.id
                                        && source.location.as_ref().is_some_and(|location| {
                                            Path::new(file.as_str()) == Path::new(&location.file)
                                        })
                                })
                                .map(|(event, _)| reader_parts(&event.body))
                        });
                        if let Some(parts) = event_parts {
                            render_static_body(ui, app, &parts);
                        } else {
                            ui.colored_label(theme::ERROR, "事件来源无法在当前 core 快照中定位。");
                        }
                    }
                    "scene" => {
                        let scene_parts = source.location.as_ref().and_then(|location| {
                            let snapshot = app.snapshot.as_ref()?;
                            snapshot
                                .result
                                .program
                                .events
                                .iter()
                                .zip(&snapshot.result.program.event_files)
                                .filter(|(_, file)| {
                                    Path::new(file.as_str()) == Path::new(&location.file)
                                })
                                .find_map(|(event, _)| find_scene_body(&event.body, location.line))
                                .map(reader_parts)
                        });
                        if let Some(parts) = scene_parts {
                            render_static_body(ui, app, &parts);
                        } else {
                            ui.colored_label(theme::ERROR, "场景来源无法在当前 core 快照中定位。");
                        }
                    }
                    "entity" => {
                        let description = app
                            .snapshot
                            .as_ref()
                            .and_then(|snapshot| {
                                snapshot.result.analysis.catalog.entities.get(&target.id)
                            })
                            .map(|entity| entity.description.clone());
                        match description {
                            Some(description) if !description.trim().is_empty() => {
                                ui.label(description);
                            }
                            Some(_) => {
                                ui.label(theme::muted("该实体尚无 description 正文。"));
                            }
                            None => {
                                ui.colored_label(theme::ERROR, "实体正文来源不可用。");
                            }
                        }
                    }
                    _ => {
                        ui.colored_label(
                            theme::ERROR,
                            format!("不支持的章节来源：{}", target.kind),
                        );
                    }
                }
                if source.location.is_none() && ui.button("选择来源修复章节").clicked() {
                    *repair_chapter = Some(chapter.id.clone());
                }
            });
        }
        let Some(next) = page.next_offset else {
            break;
        };
        offset = next;
    }
    if index.page(0, 1).total == 0 {
        ui.label(theme::muted("书稿还没有章节。"));
    }
}

fn find_scene_body(stmts: &[Stmt], line: u32) -> Option<&[Stmt]> {
    for stmt in stmts {
        match stmt {
            Stmt::Scene(scene) if scene.loc.line == line => return Some(&scene.body),
            Stmt::Scene(scene) => {
                if let Some(body) = find_scene_body(&scene.body, line) {
                    return Some(body);
                }
            }
            Stmt::Choice(choice) => {
                if let Some(body) = find_scene_body(&choice.body, line) {
                    return Some(body);
                }
            }
            Stmt::If(branches) => {
                for (_, branch) in &branches.branches {
                    if let Some(body) = find_scene_body(branch, line) {
                        return Some(body);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn reader_parts(stmts: &[Stmt]) -> Vec<Vec<ReaderPart>> {
    let mut lines = Vec::new();
    collect_reader_parts(stmts, &mut lines);
    lines
}

fn collect_reader_parts(stmts: &[Stmt], lines: &mut Vec<Vec<ReaderPart>>) {
    for stmt in stmts {
        match stmt {
            Stmt::Text(text) => lines.push(reader_parts_from_text(&text.parts, None)),
            Stmt::Choice(choice) => {
                lines.push(reader_parts_from_text(&choice.label, Some("选项：")));
                collect_reader_parts(&choice.body, lines);
            }
            Stmt::If(branches) => {
                for (_, branch) in &branches.branches {
                    collect_reader_parts(branch, lines);
                }
            }
            Stmt::Scene(scene) => collect_reader_parts(&scene.body, lines),
            Stmt::Divert(_)
            | Stmt::Let(_)
            | Stmt::Set(_)
            | Stmt::Change(_)
            | Stmt::Anchor(_)
            | Stmt::Effect(_) => {}
        }
    }
}

fn reader_parts_from_text(parts: &[TextPart], prefix: Option<&str>) -> Vec<ReaderPart> {
    let mut result = Vec::new();
    if let Some(prefix) = prefix {
        result.push(ReaderPart {
            text: prefix.into(),
            target: None,
        });
    }
    for part in parts {
        match part {
            TextPart::Str(text) => result.push(ReaderPart {
                text: text.clone(),
                target: None,
            }),
            TextPart::Link(link) => result.push(ReaderPart {
                text: link.label.clone(),
                target: Some(link.target.clone()),
            }),
            TextPart::Expr(_) => result.push(ReaderPart {
                text: "〔动态内容〕".into(),
                target: None,
            }),
        }
    }
    result
}

fn render_static_body(ui: &mut egui::Ui, app: &mut WorldeditApp, lines: &[Vec<ReaderPart>]) {
    for line in lines {
        ui.horizontal_wrapped(|ui| {
            for part in line {
                if let Some(target) = &part.target {
                    if ui.link(&part.text).clicked() {
                        app.open_reading(target.clone());
                    }
                } else {
                    ui.label(&part.text);
                }
            }
        });
    }
}
