use super::*;
use crate::theme;
use worldline_core::manuscript::{reading_projection, ManuscriptIndex, ReadingProjection};

#[derive(Default)]
pub(super) struct PreviewCache {
    key: String,
    current: HashMap<TargetRef, ReadingProjection>,
    last_valid: HashMap<TargetRef, ReadingProjection>,
    error: Option<String>,
}

impl PreviewCache {
    fn refresh(
        &mut self,
        project: &worldline_core::project::Project,
        buffers: &[WritingBuffer],
        targets: &[TargetRef],
    ) {
        let mut drafts: Vec<_> = buffers
            .iter()
            .filter(|buffer| buffer.is_changed())
            .collect();
        drafts.sort_by_key(|buffer| buffer.path());
        let key = format!(
            "{}|{:?}|{:?}",
            project.content_baseline(),
            targets,
            drafts
                .iter()
                .map(|buffer| (
                    buffer.path(),
                    buffer.baseline(),
                    buffer.generation(),
                    buffer.source()
                ))
                .collect::<Vec<_>>()
        );
        if self.key == key {
            return;
        }
        self.key = key;
        self.current.clear();
        self.error = None;
        match project.compile_writing_drafts(buffers) {
            Ok(result) => {
                for target in targets {
                    match reading_projection(&result, target) {
                        Ok(projection) => {
                            self.last_valid.insert(target.clone(), projection.clone());
                            self.current.insert(target.clone(), projection);
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
            }
            Err(error) => self.error = Some(error),
        }
    }
}

pub(super) fn draw_reader_preview(
    app: &mut super::super::WorldeditApp,
    ui: &mut egui::Ui,
    index: &ManuscriptIndex,
    selected: Option<&ManuscriptEntryDraft>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.heading("阅读预览");
        ui.selectable_value(&mut app.manuscript.reader_whole_book, false, "当前章节");
        ui.selectable_value(&mut app.manuscript.reader_whole_book, true, "整书");
    });
    let entries: Vec<(String, String, Option<TargetRef>)> = if app.manuscript.reader_whole_book {
        let mut chapters = Vec::new();
        let mut offset = 0;
        loop {
            let page = index.page(offset, 100);
            chapters.extend(
                page.chapters
                    .into_iter()
                    .map(|entry| (entry.id, entry.title, entry.target_ref)),
            );
            let Some(next) = page.next_offset else {
                break;
            };
            offset = next;
        }
        chapters
    } else {
        selected
            .filter(|entry| entry.kind == ManuscriptEntryKind::Chapter)
            .map(|entry| {
                vec![(
                    entry.id.clone(),
                    entry.title.clone(),
                    entry.target_ref.clone(),
                )]
            })
            .unwrap_or_default()
    };
    let targets: Vec<_> = entries
        .iter()
        .filter_map(|(_, _, target)| target.clone())
        .collect();
    let buffers = app.manuscript.writing_buffers();
    app.manuscript
        .preview_cache
        .refresh(&app.project, &buffers, &targets);
    let error = app.manuscript.preview_cache.error.clone();
    if let Some(error) = error {
        ui.colored_label(theme::ERROR(), format!("预览过期 · {error}"));
        ui.label(theme::muted(
            "以下仅显示同一章节的上次有效预览；全部当前输入仍保留。",
        ));
    } else {
        ui.label(theme::muted(
            if buffers.iter().any(WritingBuffer::is_changed) {
                "当前稿 · 包含未应用输入；只读静态预览"
            } else {
                "当前工程稿 · 只读静态预览"
            },
        ));
    }
    egui::CollapsingHeader::new("预览范围说明").show(ui, |ui| {
        ui.label("分支按源码顺序展示；动态内容保留标记，调用不展开。不会执行、应用或保存。整书按书稿编排顺序读取。");
    });
    let cache = &app.manuscript.preview_cache;
    let lines: Vec<_> = entries
        .into_iter()
        .map(|(id, title, target)| {
            let projection = target
                .as_ref()
                .and_then(|target| {
                    cache
                        .current
                        .get(target)
                        .or_else(|| cache.last_valid.get(target))
                })
                .cloned();
            (id, title, target, projection)
        })
        .collect();
    let mut open_target = None;
    let size = app.personal.settings.body_size;
    let spacing = app.personal.settings.line_spacing;
    egui::ScrollArea::vertical()
        .id_salt((
            "manuscript-preview-scroll",
            &index.id,
            app.manuscript.reader_whole_book,
        ))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (id, title, target, projection) in lines {
                ui.push_id((id, "reader"), |ui| {
                    ui.separator();
                    ui.label(egui::RichText::new(title).strong());
                    let Some(projection) = projection else {
                        ui.colored_label(
                            theme::ERROR(),
                            if target.is_none() {
                                "章节尚未选择正文来源。"
                            } else {
                                "当前章节没有可用预览；请保留草稿并修复来源。"
                            },
                        );
                        return;
                    };
                    for line in &projection.lines {
                        ui.horizontal_wrapped(|ui| {
                            for part in line {
                                let text = egui::RichText::new(&part.text)
                                    .size(size)
                                    .line_height(Some(size * spacing));
                                if let Some(target) = &part.target {
                                    if ui.link(text).clicked() {
                                        open_target = Some(target.clone());
                                    }
                                } else {
                                    ui.label(text);
                                }
                            }
                        });
                    }
                });
            }
            if targets.is_empty() {
                ui.label(theme::muted("请选择带正文来源的章节。"));
            }
        });
    if let Some(target) = open_target {
        app.open_reading(target);
    }
}
