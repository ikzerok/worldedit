use super::super::WorldeditApp;
use super::*;
use worldline_core::manuscript::ManuscriptEntryKind;
use worldline_core::project::Project;
use worldline_core::reader_export::ReaderManuscriptSelection;

impl ReaderPublishState {
    fn new() -> Self {
        Self {
            site_title: "离线阅读包".into(),
            #[cfg(not(target_arch = "wasm32"))]
            destination: "reader-site.zip".into(),
            ..Self::default()
        }
    }

    pub(super) fn invalidate_review(&mut self) {
        self.reviewed = None;
        self.confirmed = false;
        self.status = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.job = None;
        }
    }

    pub(super) fn selection(&self) -> ReaderExportSelection {
        ReaderExportSelection {
            schema_version: worldline_core::reader_export::READER_EXPORT_SCHEMA_VERSION,
            site_title: self.site_title.clone(),
            objects: self.objects.iter().cloned().collect(),
            manuscripts: self
                .chapters
                .iter()
                .filter(|(_, chapters)| !chapters.is_empty())
                .map(|(id, chapters)| ReaderManuscriptSelection {
                    id: id.clone(),
                    chapters: chapters.iter().cloned().collect(),
                })
                .collect(),
            attachments: self.attachments.iter().cloned().collect(),
            maps: self.maps.values().cloned().collect(),
        }
    }

    pub(super) fn has_selection(&self) -> bool {
        !self.maps.is_empty()
            || !self.objects.is_empty()
            || self.chapters.values().any(|chapters| !chapters.is_empty())
            || !self.attachments.is_empty()
    }

    pub(in crate::app) fn refresh_choices(
        &mut self,
        project: &Project,
        snapshot: Option<&super::super::Snapshot>,
    ) {
        self.refresh_map_choices(project);
        let object_choices = snapshot
            .map(|snapshot| {
                snapshot
                    .result
                    .analysis
                    .catalog
                    .objects
                    .iter()
                    .filter(|object| {
                        matches!(
                            object.target.kind.as_str(),
                            "event"
                                | "scene"
                                | "character"
                                | "entity"
                                | "world"
                                | "storyline"
                                | "period"
                                | "anchor"
                                | "state"
                                | "tag"
                                | "relation"
                                | "variable"
                        )
                    })
                    .map(|object| ObjectChoice {
                        target: object.target.clone(),
                        display: object.display.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let attachment_choices = snapshot
            .map(|snapshot| {
                snapshot
                    .result
                    .analysis
                    .catalog
                    .assets
                    .values()
                    .map(|asset| AttachmentChoice {
                        id: asset.id.clone(),
                        display: asset.display.clone(),
                        available: asset.available,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let manuscript_choices = project
            .manuscript_indices()
            .into_iter()
            .map(|(id, index)| {
                let chapters = index
                    .entries
                    .iter()
                    .filter(|entry| entry.kind == ManuscriptEntryKind::Chapter)
                    .map(|entry| (entry.id.clone(), entry.title.clone()))
                    .collect();
                ManuscriptChoice {
                    id: id.clone(),
                    title: index.title.unwrap_or(id),
                    chapters,
                    unavailable: (index.read_only || !index.diagnostics.is_empty()).then(|| {
                        if index.read_only {
                            "只读书稿，不能公开".into()
                        } else {
                            format!("书稿存在 {} 项结构诊断，不能公开", index.diagnostics.len())
                        }
                    }),
                }
            })
            .collect::<Vec<_>>();

        let allowed_objects: BTreeSet<_> = object_choices
            .iter()
            .map(|choice| choice.target.clone())
            .collect();
        let allowed_attachments: BTreeSet<_> = attachment_choices
            .iter()
            .map(|choice| choice.id.clone())
            .collect();
        self.objects
            .retain(|target| allowed_objects.contains(target));
        self.attachments
            .retain(|id| allowed_attachments.contains(id));
        self.chapters.retain(|book_id, chapters| {
            let Some(book) = manuscript_choices.iter().find(|book| book.id == *book_id) else {
                return false;
            };
            let allowed_chapters: BTreeSet<_> =
                book.chapters.iter().map(|(id, _)| id.as_str()).collect();
            chapters.retain(|id| allowed_chapters.contains(id.as_str()));
            !chapters.is_empty()
        });
        self.object_choices = object_choices;
        self.manuscript_choices = manuscript_choices;
        self.attachment_choices = attachment_choices;
        self.invalidate_review();
    }
}

impl WorldeditApp {
    pub(in crate::app) fn open_reader_publish(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        let state = {
            let mut state = ReaderPublishState::new();
            let stem = self
                .project
                .root
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "world-project".into());
            let parent = self.project.root.parent().unwrap_or(&self.project.root);
            state.destination = parent
                .join(format!("{stem}-reader-site.zip"))
                .to_string_lossy()
                .into_owned();
            state
        };
        #[cfg(target_arch = "wasm32")]
        let state = ReaderPublishState::new();
        self.reader_publish = state;
        self.reader_publish
            .refresh_choices(&self.project, self.snapshot.as_ref());
        self.reader_publish.open = true;
    }
}
