//! 只保存作者意图和已预览 core 计划；正文生成与事务均由 core 执行。
use super::*;
use worldline_core::manuscript::{
    ManuscriptBookDestination, ManuscriptChapterCreatePlan, ManuscriptChapterCreateRequest,
    ManuscriptChapterDraft, ManuscriptChapterSource, ManuscriptSourceDestination,
};
use worldline_core::project::Project;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceChoice {
    Unselected,
    Existing,
    NewEvent,
}

pub(super) struct CreationState {
    pub book_id: String,
    pub book_title: String,
    pub existing_book: Option<String>,
    pub chapter_id: String,
    pub chapter_title: String,
    pub parent: Option<String>,
    pub after: Option<String>,
    pub source_choice: SourceChoice,
    pub target: Option<TargetRef>,
    pub event_id: String,
    pub source_path: String,
    pub new_file: bool,
    pub storyline: String,
    pub touched: bool,
    pub focus_title: bool,
    pub clear_confirm: bool,
    pub reviewing: bool,
    pub error: Option<String>,
    pub preview: Option<(ManuscriptChapterCreateRequest, ManuscriptChapterCreatePlan)>,
}

impl CreationState {
    pub fn new(project: &Project, local: Option<&LocalBook>) -> Self {
        let books = project.manuscript_indices();
        let book_id = suggest_id("book", |id| books.contains_key(id));
        let content = project.compile_writing_drafts(&[]).ok();
        let event_id = suggest_id("chapter", |id| {
            content.as_ref().is_some_and(|content| {
                content
                    .analysis
                    .catalog
                    .object(&TargetRef::new("event", id))
                    .is_some()
            })
        });
        Self {
            book_id,
            book_title: String::new(),
            existing_book: local.map(|local| local.draft.id.clone()),
            chapter_id: local.map_or_else(
                || "chapter".into(),
                |local| unique_id(&local.draft.entries, "chapter"),
            ),
            chapter_title: String::new(),
            parent: local.and_then(editing::selected_section),
            after: local
                .and_then(|local| local.selected_entry.clone())
                .filter(|id| {
                    local.is_some_and(|local| {
                        local.draft.entries.iter().any(|entry| {
                            &entry.id == id && entry.kind == ManuscriptEntryKind::Chapter
                        })
                    })
                }),
            source_choice: SourceChoice::Unselected,
            target: None,
            event_id,
            source_path: project
                .entry
                .strip_prefix(&project.root)
                .unwrap_or(&project.entry)
                .to_string_lossy()
                .into_owned(),
            new_file: false,
            storyline: "main".into(),
            touched: false,
            focus_title: true,
            clear_confirm: false,
            reviewing: false,
            error: None,
            preview: None,
        }
    }

    pub fn invalidate(&mut self) {
        self.touched = true;
        self.preview = None;
        self.reviewing = false;
        self.error = None;
    }

    pub fn request(
        &self,
        project: &Project,
        revision: Revision,
    ) -> Result<ManuscriptChapterCreateRequest, String> {
        if self.chapter_title.trim().is_empty() {
            return Err("请填写章名，输入仍保留。".into());
        }
        let book = if let Some(id) = &self.existing_book {
            ManuscriptBookDestination::Existing { id: id.clone() }
        } else {
            if self.book_title.trim().is_empty() {
                return Err("请填写书名，输入仍保留。".into());
            }
            ManuscriptBookDestination::New {
                id: self.book_id.trim().into(),
                title: self.book_title.trim().into(),
            }
        };
        let source = match self.source_choice {
            SourceChoice::Unselected => {
                return Err("请明确选择正文来源；不会默认引用目录中的第一个对象。".into())
            }
            SourceChoice::Existing => ManuscriptChapterSource::Existing {
                target: self.target.clone().ok_or("请明确选择已有正文来源。")?,
            },
            SourceChoice::NewEvent => ManuscriptChapterSource::NewEvent {
                id: self.event_id.trim().into(),
                destination: if self.new_file {
                    ManuscriptSourceDestination::NewActiveSource {
                        relative_path: PathBuf::from(self.source_path.trim()),
                    }
                } else {
                    ManuscriptSourceDestination::ExistingActiveSource {
                        relative_path: PathBuf::from(self.source_path.trim()),
                    }
                },
                storyline: self.storyline.trim().into(),
            },
        };
        Ok(ManuscriptChapterCreateRequest {
            schema_version: 1,
            expected_baseline: project.content_baseline(),
            expected_revision: revision,
            book,
            chapter: ManuscriptChapterDraft {
                id: self.chapter_id.trim().into(),
                title: self.chapter_title.trim().into(),
                parent_section_id: self.parent.clone(),
                after_sibling_id: self.after.clone(),
            },
            source,
        })
    }
}

fn suggest_id(base: &str, used: impl Fn(&str) -> bool) -> String {
    std::iter::once(base.to_owned())
        .chain((2..).map(|n| format!("{base}_{n}")))
        .find(|id| !used(id))
        .unwrap_or_else(|| base.into())
}

impl super::super::WorldeditApp {
    pub(super) fn begin_chapter_creation(&mut self, existing: Option<&LocalBook>) {
        if self
            .manuscript
            .creation
            .as_ref()
            .is_none_or(|form| !form.touched)
        {
            self.manuscript.creation = Some(CreationState::new(&self.project, existing));
        }
        if self
            .manuscript
            .creation
            .as_ref()
            .is_some_and(|form| form.touched)
        {
            self.message = Some("已恢复未完成的章节创建；原输入和已选来源保留。".into());
        }
        self.manuscript.creation_dismissed = false;
        self.manuscript.creating_new = true;
    }

    pub(super) fn chapter_creation_blocker(&self, ctx: &egui::Context) -> Option<String> {
        self.review_input_blocker(ctx)
            .or_else(|| {
                (self.manuscript.writing_view.has_retained_input()
                    || self.manuscript.books.values().any(|book| book.changed)
                    || self
                        .manuscript
                        .writing_buffers
                        .values()
                        .any(WritingBuffer::is_changed))
                .then(|| {
                    "仍有未应用的书稿编排或正文；请返回原稿处理后继续创建，创建输入会保留。".into()
                })
            })
            .or_else(|| {
                self.dirty_draft_names()
                    .into_iter()
                    .find(|name| *name != "书稿 / 正文草稿")
                    .map(|name| {
                        format!("仍有未提交的{name}输入；请先处理该草稿，当前创建输入会保留。")
                    })
            })
    }

    pub(super) fn preview_chapter_creation(&mut self, form: &mut CreationState) {
        let revision = form
            .existing_book
            .as_ref()
            .and_then(|id| self.manuscript.books.get(id))
            .map_or(Revision::default(), |local| local.revision);
        let result = form.request(&self.project, revision).and_then(|request| {
            self.project
                .preview_manuscript_chapter_create(revision, &request)
                .map(|plan| (request, plan))
                .map_err(|error| error.to_string())
        });
        match result {
            Ok(preview) => {
                form.preview = Some(preview);
                form.reviewing = true;
                form.error = None;
            }
            Err(error) => {
                form.preview = None;
                form.error = Some(error);
            }
        }
    }

    pub(super) fn apply_chapter_creation(&mut self, form: &mut CreationState) -> bool {
        let Some((request, plan)) = &form.preview else {
            return false;
        };
        let mut revision = form
            .existing_book
            .as_ref()
            .and_then(|id| self.manuscript.books.get(id))
            .map_or(Revision::default(), |local| local.revision);
        let before = self.project.clone();
        match self.project.apply_manuscript_chapter_create(
            &mut revision,
            request,
            &plan.plan_digest,
        ) {
            Ok(result) => {
                self.remember(before);
                self.manuscript.rebase_clean(&self.project);
                if let Ok(index) = self.project.manuscript_index(&result.book_id) {
                    let draft = ManuscriptDraft::from_index(&index);
                    self.manuscript.books.insert(
                        result.book_id.clone(),
                        LocalBook {
                            original: draft.clone(),
                            draft,
                            baseline: self.project.content_baseline(),
                            revision,
                            selected_entry: Some(result.chapter_id.clone()),
                            changed: false,
                            collapsed: HashSet::new(),
                        },
                    );
                }
                self.manuscript.selected_book = Some(result.book_id.clone());
                if let Ok(buffer) = self.project.open_writing_buffer(&result.target) {
                    let path = buffer.path().to_owned();
                    self.manuscript.chapter_sources.insert(
                        (result.book_id, result.chapter_id),
                        (result.target, path.clone()),
                    );
                    self.manuscript.writing_buffers.insert(path, buffer);
                }
                self.manuscript.creating_new = false;
                self.manuscript.creation_dismissed = false;
                self.manuscript.narrow_preview = false;
                self.manuscript.review_focus = false;
                self.manuscript.pending_scroll = Some(0.0);
                self.manuscript
                    .writing_view
                    .restore_mode(super::super::writing_workspace::Mode::Prose);
                self.recompile();
                self.message =
                    Some("章节已创建，可直接写作；这是一次可撤销修改，保存后才写入磁盘。".into());
                self.io_error = None;
                true
            }
            Err(error) => {
                form.error = Some(format!("未创建，输入已保留：{error}"));
                false
            }
        }
    }
}
