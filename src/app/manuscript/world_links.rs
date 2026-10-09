//! 选词到资料的作者上下文；窗口关闭保留输入，来源/代次不随导航偷偷换目标。
use super::*;
use crate::app::{search, Tab, WorldeditApp};
use worldline_core::authoring::{CharacterDraft, EntityDraft};
use worldline_core::authoring_intents::{IntentTarget, TextSelection};
use worldline_core::catalog::Catalog;
use worldline_core::manuscript::{WritingAuthoringPlan, WritingAuthoringRequest};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Kind {
    #[default]
    Existing,
    Character,
    Entity,
}

pub(super) struct State {
    pub open: bool,
    pub touched: bool,
    pub kind: Kind,
    pub source: TargetRef,
    pub selection: TextSelection,
    pub generation: u64,
    pub baseline: String,
    pub origin: ManuscriptSession,
    pub query: String,
    pub chosen: Option<TargetRef>,
    pub id: String,
    pub display: String,
    pub entity_type: String,
    pub description: String,
    pub destination: PathBuf,
    pub enable_entities: bool,
    pub plan: Option<WritingAuthoringPlan>,
    pub error: Option<String>,
    pub focus_result: bool,
    pub page: crate::app::object_picker::CandidatePage,
    pub catalog: Option<Catalog>,
    pub catalog_basis: String,
    pub catalog_warning: Option<String>,
    pub peek: Option<TargetRef>,
    pub previous_reading: Option<TargetRef>,
}

impl State {
    pub fn request(&self) -> Result<WritingAuthoringRequest, String> {
        let target = match self.kind {
            Kind::Existing => {
                IntentTarget::Existing(self.chosen.clone().ok_or("请明确选择资料对象")?)
            }
            Kind::Character => IntentTarget::CreateCharacter {
                path: self.destination.clone(),
                draft: CharacterDraft {
                    id: self.id.clone(),
                    display: self.display.clone(),
                    ..Default::default()
                },
            },
            Kind::Entity => IntentTarget::CreateEntity {
                path: self.destination.clone(),
                draft: EntityDraft {
                    id: self.id.clone(),
                    entity_type: self.entity_type.clone(),
                    display: self.display.clone(),
                    description: self.description.clone(),
                    ..Default::default()
                },
            },
        };
        Ok(WritingAuthoringRequest {
            expected_baseline: self.baseline.clone(),
            source: self.source.clone(),
            generation: self.generation,
            selection: self.selection.clone(),
            target,
            enable_entities: self.enable_entities,
        })
    }
}

impl WorldeditApp {
    pub(super) fn begin_manuscript_world_links(&mut self, ctx: &egui::Context) {
        if let Some(reason) = self.review_input_blocker(ctx) {
            self.io_error = Some(reason);
            return;
        }
        if let Some(state) = self.manuscript.world_links.as_mut() {
            state.open = true;
            return;
        }
        match self.capture_world_link_selection(ctx) {
            Ok((source, selection, generation, baseline, origin)) => {
                let text = selection.expected_text.clone();
                self.manuscript.world_links = Some(State {
                    open: true,
                    touched: false,
                    kind: Kind::Existing,
                    destination: selection.path.clone(),
                    source,
                    selection,
                    generation,
                    baseline,
                    origin,
                    query: text.clone(),
                    display: text,
                    id: String::new(),
                    entity_type: "place".into(),
                    description: String::new(),
                    chosen: None,
                    enable_entities: false,
                    plan: None,
                    error: None,
                    focus_result: false,
                    page: Default::default(),
                    catalog: None,
                    catalog_basis: String::new(),
                    catalog_warning: None,
                    peek: None,
                    previous_reading: self.reading_target.clone(),
                });
            }
            Err(error) => self.io_error = Some(error),
        }
    }

    #[allow(clippy::type_complexity)]
    pub(super) fn capture_world_link_selection(
        &self,
        ctx: &egui::Context,
    ) -> Result<(TargetRef, TextSelection, u64, String, ManuscriptSession), String> {
        let (target, path) = self
            .manuscript
            .active_writing_target()
            .ok_or("请先打开书稿章节")?;
        let buffer = self
            .manuscript
            .writing_buffers
            .get(&path)
            .ok_or("正文草稿未打开")?;
        let selected = search::editor_selection(ctx).ok_or("请先选择需要关联的正文文字")?;
        if self
            .manuscript
            .writing_view
            .cursor_for_buffer(buffer, &target)
            .is_none()
            || selected.target.as_ref() != Some(&target)
            || selected.path != path
            || selected.source != buffer.source()
            || selected.range.is_empty()
        {
            return Err("请在当前章节重新选择正文文字，旧选区不会用于关联".into());
        }
        let expected_text = buffer
            .source()
            .get(selected.range.clone())
            .ok_or("选区不在 UTF-8 字符边界")?
            .to_owned();
        Ok((
            target,
            TextSelection {
                path,
                start: selected.range.start,
                end: selected.range.end,
                expected_text,
            },
            buffer.generation(),
            buffer.baseline().into(),
            self.manuscript_session(),
        ))
    }

    pub(super) fn return_world_link_origin(&mut self, ctx: &egui::Context, state: &State) {
        if self.tab == Tab::Manuscript {
            self.manuscript.writing_view.focus_existing_editor();
        }
        if let Err(error) = self.project.verify_review_navigation() {
            self.io_error = Some(error);
            return;
        }
        match self
            .manuscript
            .validate_session(&self.project, &state.origin)
        {
            Ok(true) => {
                self.tab = Tab::Manuscript;
                self.restore_manuscript_session(state.origin.clone());
                self.manuscript.writing_view.focus_existing_editor();
            }
            Ok(false) => {
                self.io_error = Some("原正文已变化，输入保留；请重新选词后继续关联".into())
            }
            Err(error) => self.io_error = Some(error),
        }
        if self.reading_target.as_ref() == state.peek.as_ref() && state.peek.is_some() {
            self.reading_target = state.previous_reading.clone();
        }
        ctx.request_repaint();
    }

    pub(in crate::app) fn close_manuscript_world_links_on_escape(
        &mut self,
        ctx: &egui::Context,
    ) -> bool {
        if self.tab != Tab::Manuscript
            || !self
                .manuscript
                .world_links
                .as_ref()
                .is_some_and(|state| state.open)
            || egui::Popup::is_any_open(ctx)
        {
            return false;
        }
        if self.world_links_input_blocked(ctx) {
            return false;
        }
        if !ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            return false;
        }
        if let Some(mut state) = self.manuscript.world_links.take() {
            state.open = false;
            self.return_world_link_origin(ctx, &state);
            self.manuscript.world_links = Some(state);
        }
        true
    }

    pub(super) fn world_links_input_blocked(&self, ctx: &egui::Context) -> bool {
        self.ime_composing
            || self.manuscript.navigation.input.blocked()
            || self.manuscript.writing_view.input_blocked(ctx)
    }

    pub(super) fn draw_world_link_return(&mut self, ui: &mut egui::Ui) {
        let Some(state) = self.manuscript.world_links.as_ref() else {
            return;
        };
        if state.open {
            return;
        }
        let peeking = state.peek.is_some();
        ui.horizontal_wrapped(|ui| {
            ui.label(crate::theme::muted(if peeking {
                "资料旁查中"
            } else {
                "关联输入已保留"
            }));
            if ui
                .small_button(if peeking {
                    "返回正文关联"
                } else {
                    "继续关联资料"
                })
                .clicked()
            {
                if let Some(mut state) = self.manuscript.world_links.take() {
                    self.return_world_link_origin(ui.ctx(), &state);
                    state.open = true;
                    self.manuscript.world_links = Some(state);
                }
            }
        });
    }
}

impl WorkbenchState {
    pub(in crate::app) fn world_links_open(&self) -> bool {
        self.world_links.as_ref().is_some_and(|state| state.open)
    }
    pub(in crate::app) fn rebase_unchanged_writing_buffers(
        &mut self,
        project: &worldline_core::project::Project,
    ) {
        for buffer in self.writing_buffers.values_mut() {
            let _ = buffer.rebase_unchanged_source(project);
        }
        self.rebase_unchanged_manuscripts(project);
    }

    pub(in crate::app) fn rebase_unchanged_manuscripts(
        &mut self,
        project: &worldline_core::project::Project,
    ) {
        let baseline = project.content_baseline();
        let indices = project.manuscript_indices();
        for (id, local) in &mut self.books {
            if indices
                .get(id)
                .is_some_and(|index| ManuscriptDraft::from_index(index) == local.original)
            {
                local.baseline = baseline.clone();
            }
        }
        self.invalidate_query_cache();
    }
}
