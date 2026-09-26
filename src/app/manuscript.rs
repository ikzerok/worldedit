//! 注册书稿工作台：结构和统计来自 core，正文始终读取原有源码。
use super::WorldeditApp;
use crate::theme;
use egui::RichText;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use worldline_core::ast::{Stmt, TextPart};
use worldline_core::catalog::{CatalogObject, TargetRef};
use worldline_core::manuscript::{
    ManuscriptCommand, ManuscriptDraft, ManuscriptEntryDraft, ManuscriptEntryKind, ManuscriptIndex,
    ManuscriptReferenceStatus,
};
use worldline_core::presentation_commands::Revision;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Layout {
    #[default]
    Tree,
    List,
    Cards,
}

struct LocalBook {
    draft: ManuscriptDraft,
    baseline: String,
    revision: Revision,
    selected_entry: Option<String>,
    changed: bool,
}

struct BodyDraft {
    path: PathBuf,
    original: String,
    text: String,
    baseline: String,
    open: bool,
}

struct ReaderPart {
    text: String,
    target: Option<TargetRef>,
}

pub(super) struct WorkbenchState {
    selected_book: Option<String>,
    creating_new: bool,
    books: BTreeMap<String, LocalBook>,
    body_drafts: HashMap<(String, String), BodyDraft>,
    layout: Layout,
    reader_open: bool,
    new_id: String,
    new_title: String,
    create_touched: bool,
}

impl Default for WorkbenchState {
    fn default() -> Self {
        Self {
            selected_book: None,
            creating_new: false,
            books: BTreeMap::new(),
            body_drafts: HashMap::new(),
            layout: Layout::Tree,
            reader_open: true,
            new_id: String::new(),
            new_title: String::new(),
            create_touched: false,
        }
    }
}

impl WorkbenchState {
    pub(super) fn has_unsubmitted_work(&self) -> bool {
        self.create_touched
            || self.books.values().any(|book| book.changed)
            || self
                .body_drafts
                .values()
                .any(|draft| draft.text != draft.original)
    }

    pub(super) fn rebase_clean(&mut self, project: &worldline_core::project::Project) {
        let indices = project.manuscript_indices();
        self.books
            .retain(|id, local| indices.contains_key(id) || local.changed);
        for (id, local) in &mut self.books {
            if local.changed {
                continue;
            }
            let Some(index) = indices.get(id) else {
                continue;
            };
            local.draft = ManuscriptDraft::from_index(index);
            local.baseline = project.content_baseline();
            local.selected_entry = local
                .draft
                .entries
                .iter()
                .find(|entry| entry.kind == ManuscriptEntryKind::Chapter)
                .or_else(|| local.draft.entries.first())
                .map(|entry| entry.id.clone());
        }
    }
}

fn copy_draft(draft: &ManuscriptDraft) -> ManuscriptDraft {
    ManuscriptDraft {
        id: draft.id.clone(),
        title: draft.title.clone(),
        entries: draft
            .entries
            .iter()
            .map(|entry| ManuscriptEntryDraft {
                id: entry.id.clone(),
                kind: entry.kind,
                parent_id: entry.parent_id.clone(),
                title: entry.title.clone(),
                summary: entry.summary.clone(),
                pov: entry.pov.clone(),
                status: entry.status.clone(),
                goal: entry.goal.clone(),
                target_ref: entry.target_ref.clone(),
            })
            .collect(),
    }
}

fn unique_id(entries: &[ManuscriptEntryDraft], base: &str) -> String {
    let used = |candidate: &str| entries.iter().any(|entry| entry.id == candidate);
    if !used(base) {
        return base.into();
    }
    (2..)
        .map(|number| format!("{base}_{number}"))
        .find(|candidate| !used(candidate))
        .unwrap_or_else(|| base.into())
}

fn entry_depth(entries: &[ManuscriptEntryDraft], entry: &ManuscriptEntryDraft) -> usize {
    let mut depth = 0;
    let mut parent = entry.parent_id.as_deref();
    while let Some(id) = parent {
        depth += 1;
        parent = entries
            .iter()
            .find(|candidate| candidate.id == id)
            .and_then(|candidate| candidate.parent_id.as_deref());
        if depth > entries.len() {
            break;
        }
    }
    depth
}

fn move_entry(entries: &mut [ManuscriptEntryDraft], id: &str, delta: isize) -> bool {
    let Some(index) = entries.iter().position(|entry| entry.id == id) else {
        return false;
    };
    let parent = entries[index].parent_id.clone();
    let siblings: Vec<_> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| (entry.parent_id == parent).then_some(index))
        .collect();
    let Some(position) = siblings.iter().position(|sibling| *sibling == index) else {
        return false;
    };
    let target = position as isize + delta;
    if !(0..siblings.len() as isize).contains(&target) {
        return false;
    }
    entries.swap(index, siblings[target as usize]);
    true
}

fn target_label(object: &CatalogObject) -> String {
    format!(
        "{} · {}:{}",
        object.display, object.target.kind, object.target.id
    )
}

fn source_status_text(status: ManuscriptReferenceStatus) -> &'static str {
    match status {
        ManuscriptReferenceStatus::Resolved => "来源已解析",
        ManuscriptReferenceStatus::Missing => "来源缺失，可在此修复引用",
        ManuscriptReferenceStatus::Unresolved => "编译错误或语言版本限制使来源暂不可确认",
        ManuscriptReferenceStatus::Invalid => "引用类型无效",
    }
}

impl WorldeditApp {
    pub(super) fn manuscript_tab(&mut self, ctx: &egui::Context) {
        if self.manuscript.creating_new {
            self.manuscript_creation_tab(ctx);
            return;
        }
        let indices = self.project.manuscript_indices();
        if self.manuscript.selected_book.is_none() {
            self.manuscript.selected_book = indices.keys().next().cloned();
        }
        if let Some(id) = self.manuscript.selected_book.clone() {
            if !indices.contains_key(&id) {
                self.manuscript.selected_book = indices.keys().next().cloned();
            }
        }
        let Some(book_id) = self.manuscript.selected_book.clone() else {
            self.manuscript_creation_tab(ctx);
            return;
        };
        let Some(index) = indices.get(&book_id).cloned() else {
            self.manuscript_creation_tab(ctx);
            return;
        };
        if !self.manuscript.books.contains_key(&book_id) {
            let draft = ManuscriptDraft::from_index(&index);
            let selected_entry = draft
                .entries
                .iter()
                .find(|entry| entry.kind == ManuscriptEntryKind::Chapter)
                .or_else(|| draft.entries.first())
                .map(|entry| entry.id.clone());
            self.manuscript.books.insert(
                book_id.clone(),
                LocalBook {
                    draft,
                    baseline: self.project.content_baseline(),
                    revision: Revision::default(),
                    selected_entry,
                    changed: false,
                },
            );
        }
        let read_only = index.read_only;
        let current_baseline = self.project.content_baseline();
        let (revision, expected_baseline, draft_snapshot) = {
            let local = self.manuscript.books.get(&book_id).unwrap();
            (
                local.revision,
                local.baseline.clone(),
                copy_draft(&local.draft),
            )
        };
        let command = ManuscriptCommand {
            expected_revision: revision,
            expected_baseline: expected_baseline.clone(),
            original: Some(book_id.clone()),
            draft: draft_snapshot,
        };
        let (preview_index, preview_error) = if read_only {
            (index.clone(), None)
        } else {
            match self.project.preview_manuscript(revision, &command) {
                Ok(preview) => (preview, None),
                Err(error) => (index.clone(), Some(error)),
            }
        };
        let objects = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.result.analysis.catalog.objects.clone())
            .unwrap_or_default();
        let mut local = self.manuscript.books.remove(&book_id).unwrap();

        let mut apply_book = false;
        let mut load_body_for = None;
        let mut apply_body_for = None;
        let mut create_chapter = false;
        let mut create_section = false;
        let mut repair_chapter = None;
        egui::CentralPanel::default()
            .frame(theme::panel())
            .show(ctx, |ui| {
                ui.heading("书稿工作台");
                ui.label(theme::muted(
                    "书稿只决定阅读顺序，独立于世界时间与事件控制流；不改变事件出口或运行指纹。",
                ));
                ui.horizontal_wrapped(|ui| {
                    if ui.button("新建书稿").clicked() {
                        self.manuscript.creating_new = true;
                    }
                    if indices.len() > 1 {
                        egui::ComboBox::from_id_salt("manuscript-book-picker")
                            .selected_text(index.title.as_deref().unwrap_or(&book_id))
                            .show_ui(ui, |ui| {
                                for (id, candidate) in &indices {
                                    ui.selectable_value(
                                        &mut self.manuscript.selected_book,
                                        Some(id.clone()),
                                        format!(
                                            "{} · {}",
                                            candidate.title.as_deref().unwrap_or(id),
                                            id
                                        ),
                                    );
                                }
                            });
                    } else {
                        ui.label(
                            RichText::new(index.title.as_deref().unwrap_or(&book_id)).strong(),
                        );
                    }
                    ui.selectable_value(&mut self.manuscript.layout, Layout::Tree, "章节树");
                    ui.selectable_value(&mut self.manuscript.layout, Layout::List, "列表");
                    ui.selectable_value(&mut self.manuscript.layout, Layout::Cards, "卡片");
                    ui.checkbox(&mut self.manuscript.reader_open, "阅读预览");
                    if read_only {
                        ui.label(theme::muted("只读书稿"));
                    }
                });
                if !index.diagnostics.is_empty() {
                    for diagnostic in &index.diagnostics {
                        ui.colored_label(
                            theme::ERROR,
                            format!("{}：{}", diagnostic.code, diagnostic.message),
                        );
                    }
                }
                if let Some(error) = &preview_error {
                    ui.colored_label(
                        theme::ERROR,
                        format!("书稿基线不可用：{error}。草稿已保留。"),
                    );
                } else if expected_baseline != current_baseline {
                    ui.colored_label(
                        theme::ERROR,
                        "工程内容已变化；保留当前书稿草稿，应用前需解决过期基线。",
                    );
                }
                if local.changed {
                    ui.label(theme::muted("有未应用的书稿草稿；切换视图会保留输入。"));
                }
                ui.horizontal(|ui| {
                    ui.label("书名");
                    if ui.text_edit_singleline(&mut local.draft.title).changed() {
                        local.changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!read_only, egui::Button::new("插入分节"))
                        .clicked()
                    {
                        create_section = true;
                    }
                    if ui
                        .add_enabled(!read_only, egui::Button::new("插入章节"))
                        .clicked()
                    {
                        create_chapter = true;
                    }
                    if ui
                        .add_enabled(
                            !read_only && local.changed && preview_error.is_none(),
                            egui::Button::new("应用书稿"),
                        )
                        .clicked()
                    {
                        apply_book = true;
                    }
                });

                if create_section {
                    let parent_id = selected_section(&local);
                    let id = unique_id(&local.draft.entries, "section");
                    local.draft.entries.push(ManuscriptEntryDraft {
                        id: id.clone(),
                        kind: ManuscriptEntryKind::Section,
                        parent_id,
                        title: "新分节".into(),
                        summary: None,
                        pov: None,
                        status: None,
                        goal: None,
                        target_ref: None,
                    });
                    local.selected_entry = Some(id);
                    local.changed = true;
                }
                if create_chapter {
                    let parent_id = selected_section(&local);
                    let id = unique_id(&local.draft.entries, "chapter");
                    let target_ref = objects
                        .iter()
                        .find(|object| {
                            matches!(object.target.kind.as_str(), "event" | "scene" | "entity")
                        })
                        .map(|object| object.target.clone());
                    local.draft.entries.push(ManuscriptEntryDraft {
                        id: id.clone(),
                        kind: ManuscriptEntryKind::Chapter,
                        parent_id,
                        title: "新章节".into(),
                        summary: None,
                        pov: None,
                        status: Some("draft".into()),
                        goal: None,
                        target_ref,
                    });
                    local.selected_entry = Some(id);
                    local.changed = true;
                }

                ui.separator();
                ui.columns(2, |columns| {
                    columns[0].heading("章节");
                    columns[1].heading("编排与来源");
                    let entries: Vec<_> = local
                        .draft
                        .entries
                        .iter()
                        .map(|entry| {
                            (
                                entry.id.clone(),
                                entry.kind,
                                entry.title.clone(),
                                entry.parent_id.as_ref().map(|parent| {
                                    local
                                        .draft
                                        .entries
                                        .iter()
                                        .find(|candidate| &candidate.id == parent)
                                        .map(|candidate| candidate.title.clone())
                                        .unwrap_or_else(|| parent.clone())
                                }),
                                entry_depth(&local.draft.entries, entry),
                            )
                        })
                        .collect();
                    egui::ScrollArea::vertical()
                        .id_salt("manuscript-entry-list")
                        .max_height(480.0)
                        .show(&mut columns[0], |ui| match self.manuscript.layout {
                            Layout::Tree => {
                                for (id, kind, title, parent, depth) in &entries {
                                    let prefix = if *kind == ManuscriptEntryKind::Section {
                                        "▾ "
                                    } else {
                                        ""
                                    };
                                    let relation = parent
                                        .as_deref()
                                        .map(|p| format!(" · {p}"))
                                        .unwrap_or_default();
                                    let label = format!(
                                        "{}{}{}{}",
                                        "  ".repeat(*depth),
                                        prefix,
                                        title,
                                        relation
                                    );
                                    if ui
                                        .selectable_label(
                                            local.selected_entry.as_deref() == Some(id),
                                            label,
                                        )
                                        .clicked()
                                    {
                                        local.selected_entry = Some(id.clone());
                                    }
                                }
                            }
                            Layout::List => {
                                for (id, kind, title, parent, _) in &entries {
                                    if *kind != ManuscriptEntryKind::Chapter {
                                        continue;
                                    }
                                    let section = parent
                                        .as_deref()
                                        .map(|p| format!("{p} / "))
                                        .unwrap_or_default();
                                    if ui
                                        .selectable_label(
                                            local.selected_entry.as_deref() == Some(id),
                                            format!("{section}{title}"),
                                        )
                                        .clicked()
                                    {
                                        local.selected_entry = Some(id.clone());
                                    }
                                }
                            }
                            Layout::Cards => {
                                for (id, kind, title, parent, _) in &entries {
                                    if *kind != ManuscriptEntryKind::Chapter {
                                        continue;
                                    }
                                    let section = parent
                                        .as_deref()
                                        .map(|p| format!("分节：{p}"))
                                        .unwrap_or_else(|| "根章节".into());
                                    theme::card().show(ui, |ui| {
                                        ui.label(RichText::new(title).strong());
                                        ui.label(theme::muted(section));
                                        if ui
                                            .selectable_label(
                                                local.selected_entry.as_deref() == Some(id),
                                                "查看章节",
                                            )
                                            .clicked()
                                        {
                                            local.selected_entry = Some(id.clone());
                                        }
                                    });
                                }
                            }
                        });

                    let selected_id = local.selected_entry.clone();
                    let selected_index = selected_id.as_ref().and_then(|id| {
                        local.draft.entries.iter().position(|entry| &entry.id == id)
                    });
                    if let Some(position) = selected_index {
                        let selected_entry_id = local.draft.entries[position].id.clone();
                        let selected_kind = local.draft.entries[position].kind;
                        columns[1].label(theme::muted(format!(
                            "稳定 ID：{} · {:?}",
                            selected_entry_id, selected_kind
                        )));
                        {
                            let entry = &mut local.draft.entries[position];
                            if columns[1].text_edit_singleline(&mut entry.title).changed() {
                                local.changed = true;
                            }
                            columns[1].label("摘要");
                            let mut summary = entry.summary.clone().unwrap_or_default();
                            if columns[1]
                                .add(egui::TextEdit::multiline(&mut summary).desired_rows(3))
                                .changed()
                            {
                                entry.summary = (!summary.is_empty()).then_some(summary);
                                local.changed = true;
                            }
                        }
                        columns[1].horizontal(|ui| {
                            if ui
                                .add_enabled(!read_only, egui::Button::new("上移"))
                                .clicked()
                                && move_entry(&mut local.draft.entries, &selected_entry_id, -1)
                            {
                                local.changed = true;
                            }
                            if ui
                                .add_enabled(!read_only, egui::Button::new("下移"))
                                .clicked()
                                && move_entry(&mut local.draft.entries, &selected_entry_id, 1)
                            {
                                local.changed = true;
                            }
                        });
                        {
                            let entry = &mut local.draft.entries[position];
                            draw_status_goal(&mut columns[1], entry, &mut local.changed);
                        }
                        if selected_kind == ManuscriptEntryKind::Chapter {
                            let entry = &mut local.draft.entries[position];
                            draw_metadata_target(
                                &mut columns[1],
                                entry,
                                &objects,
                                &mut local.changed,
                            );
                            if let Some(index_entry) = preview_index
                                .entries
                                .iter()
                                .find(|candidate| candidate.id == entry.id)
                            {
                                if let Some(source) = &index_entry.source {
                                    columns[1].label(source_status_text(source.status));
                                    if let Some(location) = &source.location {
                                        columns[1].label(theme::muted(format!(
                                            "来源：{}:{}",
                                            location.file, location.line
                                        )));
                                    }
                                    if let Some(stats) = source.stats {
                                        columns[1].label(format!(
                                            "汉字 {} · 词数 {}",
                                            stats.han_characters, stats.words
                                        ));
                                        if let Some(goal) = entry
                                            .goal
                                            .as_deref()
                                            .and_then(|goal| goal.trim().parse::<u64>().ok())
                                        {
                                            let ratio = if goal == 0 {
                                                1.0
                                            } else {
                                                (stats.words as f32 / goal as f32).clamp(0.0, 1.0)
                                            };
                                            columns[1].add(egui::ProgressBar::new(ratio).text(
                                                format!("{} / {} 词目标", stats.words, goal),
                                            ));
                                        } else if entry
                                            .goal
                                            .as_deref()
                                            .is_some_and(|goal| !goal.trim().is_empty())
                                        {
                                            columns[1].label(theme::muted(
                                                "目标说明不是数字；输入数字可显示词数进度。",
                                            ));
                                        }
                                    }
                                }
                            }
                            let body_key = (book_id.clone(), selected_entry_id.clone());
                            if let Some(body) = self.manuscript.body_drafts.get_mut(&body_key) {
                                ui_body_draft(
                                    &mut columns[1],
                                    body,
                                    &mut apply_body_for,
                                    &book_id,
                                    &selected_entry_id,
                                );
                            } else {
                                let has_source = preview_index
                                    .entries
                                    .iter()
                                    .find(|candidate| candidate.id == selected_entry_id)
                                    .and_then(|candidate| candidate.source.as_ref())
                                    .and_then(|source| source.location.as_ref())
                                    .is_some();
                                if columns[1]
                                    .add_enabled(
                                        !read_only && has_source,
                                        egui::Button::new("编辑来源文件"),
                                    )
                                    .clicked()
                                {
                                    load_body_for =
                                        Some((book_id.clone(), selected_entry_id.clone()));
                                }
                                if !has_source {
                                    columns[1].label(theme::muted(
                                        "来源缺失时先选择一个可确认的事件、场景或实体。",
                                    ));
                                }
                            }
                        }
                    } else {
                        columns[1].label("选择章节或分节以编辑编排。");
                    }
                });

                if self.manuscript.reader_open {
                    ui.separator();
                    ui.heading("阅读预览");
                ui.label(theme::muted(
                    "按书稿章节顺序展示 core 编译的静态文本；分支按源码顺序列出，不模拟世界时间或事件控制流。",
                ));
                    egui::ScrollArea::vertical()
                        .id_salt("manuscript-reader-preview")
                        .max_height(420.0)
                        .show(ui, |ui| {
                            draw_reader_preview(self, ui, &preview_index, &mut repair_chapter);
                        });
                }
            });

        if let Some(chapter) = repair_chapter {
            local.selected_entry = Some(chapter);
        }
        self.manuscript.books.insert(book_id.clone(), local);

        if let Some((book, chapter)) = load_body_for {
            self.load_manuscript_body(&book, &chapter, &preview_index);
        }
        if let Some((book, chapter)) = apply_body_for {
            self.apply_manuscript_body(&book, &chapter);
        }
        if apply_book {
            self.apply_manuscript_book(&book_id);
        }
    }

    fn manuscript_creation_tab(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(theme::panel())
            .show(ctx, |ui| {
                ui.heading("新建书稿");
                ui.label("只有清单注册的书稿会显示在这里；创建时由 core 同步更新注册和书稿文档。");
                if self.manuscript.selected_book.is_some() && ui.button("返回当前书稿").clicked()
                {
                    self.manuscript.creating_new = false;
                }
                ui.horizontal(|ui| {
                    ui.label("稳定 ID");
                    self.manuscript.create_touched |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.manuscript.new_id)
                                .hint_text("例如 novel"),
                        )
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label("书名");
                    self.manuscript.create_touched |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.manuscript.new_title)
                                .hint_text("书稿名称"),
                        )
                        .changed();
                });
                if ui
                    .add_enabled(
                        !self.manuscript.new_id.trim().is_empty()
                            && !self.manuscript.new_title.trim().is_empty(),
                        egui::Button::new("创建并打开书稿"),
                    )
                    .clicked()
                {
                    self.create_manuscript();
                }
            });
    }

    fn create_manuscript(&mut self) {
        let draft = ManuscriptDraft {
            id: self.manuscript.new_id.trim().to_owned(),
            title: self.manuscript.new_title.trim().to_owned(),
            entries: Vec::new(),
        };
        let mut revision = Revision::default();
        let command = ManuscriptCommand {
            expected_revision: revision,
            expected_baseline: self.project.content_baseline(),
            original: None,
            draft,
        };
        if let Err(error) = self.project.preview_manuscript(revision, &command) {
            self.io_error = Some(error);
            return;
        }
        let before = self.project.clone();
        match self.project.apply_manuscript(&mut revision, command) {
            Ok(_) => {
                self.remember(before);
                let id = self.manuscript.new_id.trim().to_owned();
                if let Ok(index) = self.project.manuscript_index(&id) {
                    self.manuscript.books.insert(
                        id.clone(),
                        LocalBook {
                            draft: ManuscriptDraft::from_index(&index),
                            baseline: self.project.content_baseline(),
                            revision,
                            selected_entry: None,
                            changed: false,
                        },
                    );
                    self.manuscript.selected_book = Some(id);
                }
                self.manuscript.create_touched = false;
                self.manuscript.creating_new = false;
                self.recompile();
                self.message = Some("书稿已创建；书稿与注册由 core 一次写入".into());
                self.io_error = None;
            }
            Err(error) => self.io_error = Some(error),
        }
    }

    fn apply_manuscript_book(&mut self, id: &str) {
        let Some(local) = self.manuscript.books.get(id) else {
            return;
        };
        let mut revision = local.revision;
        let previous_baseline = local.baseline.clone();
        let command = ManuscriptCommand {
            expected_revision: revision,
            expected_baseline: previous_baseline.clone(),
            original: Some(id.into()),
            draft: copy_draft(&local.draft),
        };
        if let Err(error) = self.project.preview_manuscript(revision, &command) {
            self.io_error = Some(format!("书稿未应用，输入已保留：{error}"));
            return;
        }
        let before = self.project.clone();
        match self.project.apply_manuscript(&mut revision, command) {
            Ok(_) => {
                let new_baseline = self.project.content_baseline();
                self.remember(before);
                if let Some(local) = self.manuscript.books.get_mut(id) {
                    if let Ok(index) = self.project.manuscript_index(id) {
                        local.draft = ManuscriptDraft::from_index(&index);
                    }
                    local.baseline = new_baseline.clone();
                    local.revision = revision;
                    local.changed = false;
                }
                for ((book, _), body) in &mut self.manuscript.body_drafts {
                    if book == id && body.baseline == previous_baseline {
                        body.baseline = new_baseline.clone();
                    }
                }
                self.recompile();
                self.message = Some("书稿编排已应用；运行内容指纹不变".into());
                self.io_error = None;
            }
            Err(error) => self.io_error = Some(format!("书稿未应用，输入已保留：{error}")),
        }
    }

    fn load_manuscript_body(&mut self, book: &str, chapter: &str, index: &ManuscriptIndex) {
        let Some(location) = index
            .entries
            .iter()
            .find(|entry| entry.id == chapter)
            .and_then(|entry| entry.source.as_ref())
            .and_then(|source| source.location.as_ref())
        else {
            self.io_error = Some("章节来源尚未解析，不能打开正文草稿".into());
            return;
        };
        let path = super::workspace_source_path(&self.project, Path::new(&location.file));
        match self.project.document(&path) {
            Ok(source) => {
                self.manuscript.body_drafts.insert(
                    (book.into(), chapter.into()),
                    BodyDraft {
                        path,
                        original: source.to_owned(),
                        text: source.to_owned(),
                        baseline: self.project.content_baseline(),
                        open: true,
                    },
                );
                self.io_error = None;
            }
            Err(error) => self.io_error = Some(error),
        }
    }

    fn apply_manuscript_body(&mut self, book: &str, chapter: &str) {
        let key = (book.to_owned(), chapter.to_owned());
        let Some(body) = self.manuscript.body_drafts.get(&key) else {
            return;
        };
        if body.baseline != self.project.content_baseline() {
            self.io_error =
                Some("正文草稿基线已过期；输入已保留，请检查外部修改后重新打开来源。".into());
            return;
        }
        if body.text == body.original {
            self.io_error = Some("正文没有未提交的修改".into());
            return;
        }
        let path = body.path.clone();
        let text = body.text.clone();
        let previous_baseline = body.baseline.clone();
        let before = self.project.clone();
        match self.project.edit(|project| project.set_text(&path, text)) {
            Ok(()) => {
                let new_baseline = self.project.content_baseline();
                self.remember(before);
                self.manuscript.body_drafts.remove(&key);
                if let Some(local) = self.manuscript.books.get_mut(book) {
                    if local.baseline == previous_baseline {
                        local.baseline = new_baseline;
                    }
                }
                self.recompile();
                self.message = Some("正文来源已应用；此操作可撤销".into());
                self.io_error = None;
            }
            Err(error) => self.io_error = Some(error),
        }
    }
}

fn selected_section(local: &LocalBook) -> Option<String> {
    local.selected_entry.as_deref().and_then(|id| {
        local
            .draft
            .entries
            .iter()
            .find(|entry| entry.id == id && entry.kind == ManuscriptEntryKind::Section)
            .map(|entry| entry.id.clone())
    })
}

fn draw_metadata_target(
    ui: &mut egui::Ui,
    entry: &mut ManuscriptEntryDraft,
    objects: &[CatalogObject],
    changed: &mut bool,
) {
    ui.separator();
    ui.label(RichText::new("正文引用").strong());
    let valid_targets: Vec<_> = objects
        .iter()
        .filter(|object| matches!(object.target.kind.as_str(), "event" | "scene" | "entity"))
        .collect();
    let selected = entry
        .target_ref
        .as_ref()
        .map(|target| format!("{}:{}", target.kind, target.id))
        .unwrap_or_else(|| "未选择来源".into());
    egui::ComboBox::from_id_salt(("manuscript-target", &entry.id))
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for object in &valid_targets {
                if ui
                    .selectable_value(
                        &mut entry.target_ref,
                        Some(object.target.clone()),
                        target_label(object),
                    )
                    .changed()
                {
                    *changed = true;
                }
            }
        });
    if let Some(target) = &entry.target_ref {
        if !valid_targets.iter().any(|object| object.target == *target) {
            ui.colored_label(
                theme::ERROR,
                format!(
                    "引用 {}/{} 已失效；从候选列表选择来源可修复。",
                    target.kind, target.id
                ),
            );
        }
    }

    ui.label("视角人物");
    let selected_pov = entry
        .pov
        .as_ref()
        .map(|target| format!("{}:{}", target.kind, target.id))
        .unwrap_or_else(|| "不指定".into());
    egui::ComboBox::from_id_salt(("manuscript-pov", &entry.id))
        .selected_text(selected_pov)
        .show_ui(ui, |ui| {
            if ui
                .selectable_value(&mut entry.pov, None, "不指定")
                .changed()
            {
                *changed = true;
            }
            for object in objects
                .iter()
                .filter(|object| object.target.kind == "character")
            {
                if ui
                    .selectable_value(
                        &mut entry.pov,
                        Some(object.target.clone()),
                        target_label(object),
                    )
                    .changed()
                {
                    *changed = true;
                }
            }
        });
}

fn draw_status_goal(ui: &mut egui::Ui, entry: &mut ManuscriptEntryDraft, changed: &mut bool) {
    ui.horizontal(|ui| {
        ui.label("状态");
        let mut status = entry.status.clone().unwrap_or_default();
        if ui
            .add(egui::TextEdit::singleline(&mut status).hint_text("draft / revised / final"))
            .changed()
        {
            entry.status = (!status.is_empty()).then_some(status);
            *changed = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label("字数目标");
        let mut goal = entry.goal.clone().unwrap_or_default();
        if ui
            .add(egui::TextEdit::singleline(&mut goal).hint_text("输入数字显示进度，也可写说明"))
            .changed()
        {
            entry.goal = (!goal.is_empty()).then_some(goal);
            *changed = true;
        }
    });
}

fn ui_body_draft(
    ui: &mut egui::Ui,
    body: &mut BodyDraft,
    apply_body_for: &mut Option<(String, String)>,
    book: &str,
    chapter: &str,
) {
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new("来源文件草稿").strong());
        if ui
            .button(if body.open {
                "收起编辑器"
            } else {
                "继续编辑正文草稿"
            })
            .clicked()
        {
            body.open = !body.open;
        }
    });
    ui.label(theme::muted(body.path.display().to_string()));
    if body.open {
        ui.add(
            egui::TextEdit::multiline(&mut body.text)
                .id_salt(("manuscript-source", book, chapter))
                .desired_rows(15)
                .code_editor(),
        );
        ui.label(theme::muted(
            "这是引用的源码文件缓冲；未应用的文字会随章节切换保留。",
        ));
        if ui
            .add_enabled(
                body.text != body.original,
                egui::Button::new("应用正文草稿"),
            )
            .clicked()
        {
            *apply_body_for = Some((book.into(), chapter.into()));
        }
    }
}

fn draw_reader_preview(
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
