//! Wiki 的列表、词条表单与共用文字链接；匹配和出现位置由 core 提供。
use super::{Tab, WorldeditApp};
use crate::theme::{self, ACCENT, BG};
use egui::RichText;
use worldline_core::authoring::WorldDraft;
use worldline_core::catalog::{Catalog, TargetRef};
use worldline_core::navigation::RenderedLink;
use worldline_core::wiki::KeywordIndex;

pub(super) struct WikiEditor {
    original: Option<String>,
    draft: WorldDraft,
    aliases: String,
    version: u64,
}

pub(super) fn keyword_inline(
    ui: &mut egui::Ui,
    text: &str,
    index: &KeywordIndex,
    catalog: &Catalog,
    links: &[RenderedLink],
    size: f32,
) -> Option<TargetRef> {
    let mut selected = None;
    let mut end = 0;
    for found in index.find_with_links(text, links) {
        if found.start > end {
            ui.add(egui::Label::new(RichText::new(&text[end..found.start]).size(size)).wrap());
        }
        let label = &text[found.start..found.end];
        ui.push_id(found.start, |ui| {
            if found.targets.len() == 1 {
                if ui
                    .link(RichText::new(label).size(size).color(ACCENT).underline())
                    .on_hover_text("查看 Wiki 释义与出现位置")
                    .clicked()
                {
                    selected = found.targets.first().cloned();
                }
            } else {
                ui.spacing_mut().button_padding = egui::Vec2::ZERO;
                ui.menu_button(
                    RichText::new(label).size(size).color(ACCENT).underline(),
                    |ui| {
                        ui.label(theme::muted("同名词条，请选择要查看的释义"));
                        for target in &found.targets {
                            if let Some(object) = catalog.object(target) {
                                if ui
                                    .button(format!(
                                        "{} · {} · {}",
                                        object.display,
                                        super::catalog::kind_label(&target.kind),
                                        target.id
                                    ))
                                    .clicked()
                                {
                                    selected = Some(target.clone());
                                    ui.close();
                                }
                            }
                        }
                    },
                );
            }
        });
        end = found.end;
    }
    if end < text.len() {
        ui.add(egui::Label::new(RichText::new(&text[end..]).size(size)).wrap());
    }
    selected
}

pub(super) fn keyword_text(
    ui: &mut egui::Ui,
    text: &str,
    index: &KeywordIndex,
    catalog: &Catalog,
    links: &[RenderedLink],
    size: f32,
) -> Option<TargetRef> {
    let mut selected = None;
    let mut offset = 0;
    for (line, text) in text.split('\n').enumerate() {
        let line_links: Vec<_> = links
            .iter()
            .filter(|link| link.start >= offset && link.end <= offset + text.len())
            .map(|link| RenderedLink {
                target: link.target.clone(),
                start: link.start - offset,
                end: link.end - offset,
            })
            .collect();
        ui.push_id(line, |ui| {
            if text.is_empty() {
                ui.add_space(size);
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if let Some(target) =
                        keyword_inline(ui, text, index, catalog, &line_links, size)
                    {
                        selected = Some(target);
                    }
                });
            }
        });
        offset += text.len() + 1;
    }
    selected
}

impl WorldeditApp {
    pub(super) fn wiki_inline(&mut self, ui: &mut egui::Ui, text: &str, size: f32) {
        if let Some(snapshot) = &self.snapshot {
            if let Some(target) = keyword_inline(
                ui,
                text,
                &snapshot.wiki,
                &snapshot.result.analysis.catalog,
                &[],
                size,
            ) {
                self.open_reading(target);
            }
        }
    }

    pub(super) fn wiki_text(&mut self, ui: &mut egui::Ui, text: &str) {
        if let Some(snapshot) = &self.snapshot {
            if let Some(target) = keyword_text(
                ui,
                text,
                &snapshot.wiki,
                &snapshot.result.analysis.catalog,
                &[],
                16.0,
            ) {
                self.open_reading(target);
            }
        }
    }

    pub(super) fn wiki_occurrences(&mut self, ui: &mut egui::Ui, target: &TargetRef) {
        let hits = self
            .snapshot
            .as_ref()
            .map(|s| s.wiki.occurrences(target).to_vec())
            .unwrap_or_default();
        egui::CollapsingHeader::new(format!("关键词出现位置 · {} 处", hits.len()))
            .id_salt("wiki-occurrences")
            .default_open(true)
            .show(ui, |ui| {
                ui.label(theme::muted(
                    "包含当前源码中的名称、别名与显式链接，点击跳到原文。",
                ));
                egui::ScrollArea::vertical()
                    .id_salt("wiki-hits")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for (i, hit) in hits.iter().enumerate() {
                            ui.push_id(i, |ui| {
                                let file = hit
                                    .file
                                    .strip_prefix(&self.project.root)
                                    .unwrap_or(&hit.file);
                                if ui
                                    .link(format!("{}:{}:{}", file.display(), hit.line, hit.column))
                                    .clicked()
                                {
                                    self.jump_to_file(
                                        &hit.file.to_string_lossy(),
                                        hit.line,
                                        hit.column,
                                    );
                                    self.close_transient_reading();
                                }
                                ui.push_id("preview", |ui| {
                                    self.linked_source(
                                        ui,
                                        &hit.preview,
                                        &hit.file.to_string_lossy(),
                                    )
                                });
                            });
                        }
                        if hits.is_empty() {
                            ui.label(theme::muted("当前源码尚未出现此关键词。"));
                        }
                    });
            });
    }

    pub(super) fn edit_wiki_entry(&mut self, id: Option<&str>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let catalog = &snapshot.result.analysis.catalog;
        let draft = if let Some(id) = id {
            let Some(tag) = catalog.tags.get(id).filter(|tag| tag.declared) else {
                return;
            };
            WorldDraft {
                id: tag.id.clone(),
                display: tag.display.clone(),
                description: tag.description.clone(),
                properties: tag.properties.clone().into_iter().collect(),
            }
        } else {
            let mut number = 1;
            while catalog.tags.contains_key(&format!("wiki_{number}")) {
                number += 1;
            }
            WorldDraft {
                id: format!("wiki_{number}"),
                ..Default::default()
            }
        };
        let aliases = catalog
            .aliases_for(&TargetRef::new("tag", &draft.id))
            .join("\n");
        self.wiki_editor = Some(WikiEditor {
            original: id.map(str::to_owned),
            draft,
            aliases,
            version: self.version,
        });
    }

    pub(super) fn wiki_tab(&mut self, ctx: &egui::Context) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let catalog = snapshot.result.analysis.catalog.clone();
        egui::SidePanel::right("wiki-index")
            .default_width(290.0)
            .width_range(230.0..=400.0)
            .frame(theme::panel())
            .show(ctx, |ui| {
                ui.heading("关键词索引");
                ui.add(
                    egui::TextEdit::singleline(&mut self.wiki_query)
                        .hint_text("搜索关键词、别名或 ID")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);
                let objects = catalog.search_objects(&self.wiki_query);
                ui.label(theme::muted(format!("{} 个词条", objects.len())));
                egui::ScrollArea::vertical()
                    .id_salt("wiki-entries")
                    .show(ui, |ui| {
                        for object in objects {
                            ui.push_id((&object.target.kind, &object.target.id), |ui| {
                                if ui
                                    .add_sized(
                                        [ui.available_width(), 38.0],
                                        egui::Button::selectable(
                                            self.wiki_target.as_ref() == Some(&object.target),
                                            &object.display,
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.wiki_target = Some(object.target.clone());
                                    self.alias_input.clear();
                                }
                                ui.label(theme::muted(format!(
                                    "{} · {}",
                                    super::catalog::kind_label(&object.target.kind),
                                    object.target.id
                                )));
                            });
                        }
                        if catalog.search_objects(&self.wiki_query).is_empty() {
                            ui.label("没有匹配的词条，可用左侧按钮创建。");
                        }
                    });
            });
        egui::CentralPanel::default().frame(theme::panel().fill(BG)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Wiki");
                if ui.add(theme::primary("＋ 新建词条")).clicked() { self.edit_wiki_entry(None); }
            });
            ui.label(theme::muted("为关键词写下释义。在正文、资料和试玩中点击关键词，即可查看注释与出现位置。"));
            ui.separator();
            egui::ScrollArea::vertical().id_salt(("wiki-detail", &self.wiki_target)).show(ui, |ui| {
                if let Some(target) = self.wiki_target.clone().filter(|t| catalog.object(t).is_some()) {
                    self.reading_content(ui, target);
                } else {
                    ui.add_space(30.0);
                    ui.heading("让每个名字都有来处");
                    ui.label("从右侧索引选择人物、地点或设定，也可以新建词条，填写关键词、释义和别名。");
                    ui.label(theme::muted("现有资料会自动进入索引；同名词条分别保留。释义中出现的其他关键词也可以继续点击。"));
                }
            });
        });
    }

    pub(super) fn wiki_editor_window(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.wiki_editor.take() else {
            return;
        };
        let mut open = true;
        let mut applied = false;
        egui::Window::new(if editor.original.is_some() {
            "编辑 Wiki 词条"
        } else {
            "新建 Wiki 词条"
        })
        .id(egui::Id::new("wiki-editor"))
        .open(&mut open)
        .default_width(560.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.label("关键词");
            ui.add(
                egui::TextEdit::singleline(&mut editor.draft.display)
                    .hint_text("例如：雾港、潮汐纪元")
                    .desired_width(f32::INFINITY),
            );
            ui.label("释义 / 注释");
            ui.add(
                egui::TextEdit::multiline(&mut editor.draft.description)
                    .hint_text("写下定义、背景与需要读者知道的内容…")
                    .desired_rows(8)
                    .desired_width(f32::INFINITY),
            );
            ui.label("别名 · 每行一个");
            ui.add(
                egui::TextEdit::multiline(&mut editor.aliases)
                    .hint_text("简称、旧称、其他写法")
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            let stale = editor.version != self.version;
            if stale {
                ui.colored_label(
                    theme::GOLD,
                    "词条打开后工程已更新。请复制需要保留的输入，关闭并重新打开词条后合并。",
                );
            }
            if ui
                .add_enabled(
                    !stale && !editor.draft.display.trim().is_empty(),
                    theme::primary("应用词条"),
                )
                .clicked()
            {
                editor.draft.display = editor.draft.display.trim().into();
                let aliases: Vec<_> = editor
                    .aliases
                    .lines()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect();
                applied = self.commit(
                    "Wiki 词条已更新；保存全部可写入作品目录",
                    |p| p.write_wiki_entry(editor.original.as_deref(), &editor.draft, &aliases),
                );
                if applied {
                    self.wiki_target = Some(TargetRef::new("tag", &editor.draft.id));
                    self.tab = Tab::Wiki;
                    self.close_transient_reading();
                    self.tag_editor = None;
                }
            }
        });
        if open && !applied {
            self.wiki_editor = Some(editor);
        }
    }
}
