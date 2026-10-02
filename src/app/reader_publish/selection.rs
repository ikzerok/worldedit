use super::super::WorldeditApp;
use super::*;
use worldline_core::manuscript::ManuscriptEntryKind;
use worldline_core::project::Project;

impl ReaderPublishState {
    pub(super) fn new() -> Self {
        Self {
            site_title: "离线阅读包".into(),
            #[cfg(not(target_arch = "wasm32"))]
            destination: "reader-site.zip".into(),
            ..Self::default()
        }
    }

    pub(super) fn invalidate_review(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.reviewed = None;
        self.step = PublishStep::Select;
        self.confirmed = false;
        self.status = None;
        #[cfg(target_arch = "wasm32")]
        {
            self.web_job = None;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.job = None;
            self.browser_preview_job = None;
            if self.delivery_job.as_ref().is_some_and(|job| job.cancel()) {
                self.delivery_job = None;
            }
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
                                | "fragment"
                                | "rule"
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
                        fields: snapshot.result.reader_field_candidates(&object.target),
                        target: object.target.clone(),
                        display: object.display.clone(),
                        aliases: snapshot.result.analysis.catalog.aliases_for(&object.target),
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

        // 已保存授权不能随候选变化缩减；失效项留给显式移除和core拒绝。
        self.refresh_profiles(project);
        self.object_choices = object_choices;
        self.manuscript_choices = manuscript_choices;
        self.attachment_choices = attachment_choices;
        self.invalidate_review();
    }
}

impl WorldeditApp {
    pub(in crate::app) fn open_reader_publish(&mut self) {
        // 重复打开只把现有向导带回前台，不能丢选择或断开正在提交的结果通道。
        if self.reader_publish.open || self.reader_publish.busy() {
            self.reader_publish.open = true;
            return;
        }
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
