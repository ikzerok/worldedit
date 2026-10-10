//! 正式台词的瞬态编辑输入；只消费 core 投影与计划，不解释 DSL。
mod fields;
mod focus;
mod form;
mod ime;
#[cfg(test)]
mod interaction_tests;
mod preview;
mod preview_keyboard;
mod render;
mod render_state;
pub(super) mod retained;
#[cfg(test)]
mod tests;
use super::*;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use worldline_core::manuscript::*;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct Key {
    pub path: PathBuf,
    pub target: TargetRef,
}
impl Key {
    fn new(buffer: &WritingBuffer, target: &TargetRef) -> Self {
        Self {
            path: buffer.path().into(),
            target: target.clone(),
        }
    }
}
pub(super) struct Form {
    request: DialogueEditRequest,
    original: DialogueOperation,
    pub(super) plan: Option<DialogueEditPlan>,
    error: Option<String>,
    label: String,
    speakers: Vec<ReviewSpeaker>,
    return_focus: Option<egui::Id>,
    input_ids: std::collections::HashSet<egui::Id>,
    focus: bool,
    discard_confirm: bool,
    rebind: Option<String>,
    migration_confirmed: bool,
    continue_after: bool,
    batch_owner: Option<egui::Id>,
    ime_owner: Option<egui::Id>,
    last_input: Option<egui::Id>,
    drawn_inputs: Vec<egui::Id>,
    refocus: bool,
    navigation: preview_keyboard::Navigation,
    ime: std::collections::HashMap<egui::Id, ime::ReceiverState>,
}
impl Form {
    fn protected(&self) -> bool {
        self.request.operation != self.original
            || !matches!(self.original, DialogueOperation::Update { .. })
            || self.request.enable_language_1_11
    }
}
#[derive(Default)]
pub(super) struct State {
    pub(super) enabled: bool,
    cache_key: String,
    cache: Option<Result<Arc<DialogueProjection>, String>>,
    statement_indices: Arc<BTreeMap<String, usize>>,
    pending: Option<render_state::Pending>,
    page_key: Option<Key>,
    row_offset: usize,
    #[cfg(test)]
    cache_builds: usize,
    #[cfg(test)]
    rendered_rows: usize,
    forms: BTreeMap<Key, Form>,
    insert_requested: bool,
    anchor: Option<String>,
    pub(super) viewport: Option<egui::Rect>,
}
impl State {
    pub(super) fn invalidate(&mut self) {
        self.cache_key.clear();
        self.cache = None;
        for form in self.forms.values_mut() {
            form.navigation = Default::default();
        }
    }
    fn projection(
        &mut self,
        project: &Project,
        buffer: &WritingBuffer,
        target: &TargetRef,
    ) -> Result<Arc<DialogueProjection>, String> {
        // Project 内容应用/历史由宿主 recompile→invalidate_projection 失效；附件观察另绑无 IO 的 core 缓存键。
        // 稳定帧只读 core 缓存的完整 buffer 身份，不能按回滚后可重复的 generation 缓存。
        let key = format!(
            "{}|{}|{}:{}",
            buffer.identity(),
            project.catalog_scope_observation_key(),
            target.kind,
            target.id
        );
        if self.cache_key != key || self.cache.is_none() {
            for form in self.forms.values_mut() {
                form.navigation = Default::default();
            }
            self.cache = Some(
                project
                    .project_dialogue_buffer(buffer, target)
                    .map(Arc::new)
                    .map_err(|error| error.to_string()),
            );
            if let Some(Ok(projection)) = &self.cache {
                self.statement_indices = Arc::new(
                    projection
                        .statements
                        .iter()
                        .enumerate()
                        .map(|(index, statement)| (statement.id.clone(), index))
                        .collect(),
                );
            } else {
                self.statement_indices = Arc::new(BTreeMap::new());
            }
            #[cfg(test)]
            {
                self.cache_builds += 1;
            }
            self.cache_key = key;
        }
        self.cache.as_ref().expect("台词投影已建立").clone()
    }
    fn begin(
        &mut self,
        _ui: &egui::Ui,
        buffer: &WritingBuffer,
        projection: &DialogueProjection,
        operation: DialogueOperation,
        label: String,
    ) {
        self.queue_replacement(buffer, projection, operation, label);
    }
    fn begin_context(
        &mut self,
        ctx: &egui::Context,
        buffer: &WritingBuffer,
        projection: &DialogueProjection,
        operation: DialogueOperation,
        label: String,
    ) {
        let key = Key::new(buffer, &projection.target);
        if self.forms.get(&key).is_some_and(Form::protected) {
            return;
        }
        self.forms.insert(
            key,
            Form {
                request: DialogueEditRequest {
                    schema_version: 1,
                    expected_baseline: projection.baseline.clone(),
                    target: projection.target.clone(),
                    generation: projection.generation,
                    operation: operation.clone(),
                    enable_language_1_11: false,
                },
                original: operation,
                plan: None,
                error: None,
                label,
                speakers: projection.speakers.clone(),
                return_focus: ctx.memory(|memory| memory.focused()),
                input_ids: Default::default(),
                focus: true,
                discard_confirm: false,
                rebind: None,
                migration_confirmed: false,
                continue_after: false,
                batch_owner: None,
                ime_owner: None,
                last_input: None,
                drawn_inputs: Vec::new(),
                refocus: false,
                navigation: Default::default(),
                ime: Default::default(),
            },
        );
        self.enabled = true;
    }
}
impl ViewState {
    pub(in crate::app) fn has_dialogue_input(&self) -> bool {
        self.dialogue.forms.values().any(Form::protected)
    }
    pub(in crate::app) fn begin_dialogue_continuation(
        &mut self,
        ctx: &egui::Context,
        project: &Project,
        buffer: &WritingBuffer,
        plan: &DialogueEditPlan,
    ) -> Result<(), String> {
        let anchor = project
            .dialogue_continuation(buffer, plan)
            .map_err(|error| error.to_string())?;
        let projection = project
            .project_dialogue_buffer(buffer, &plan.request.target)
            .map_err(|error| error.to_string())?;
        self.dialogue.begin_context(
            ctx,
            buffer,
            &projection,
            DialogueOperation::Insert {
                anchor_id: anchor.id,
                draft: DialogueDraft {
                    kind: DialogueKind::Say,
                    speaker: plan.new_speaker.clone(),
                    direction: None,
                    parts: vec![DialoguePart::Literal {
                        text: String::new(),
                    }],
                },
            },
            "下一句 · 沿用已确认角色，可修改".into(),
        );
        Ok(())
    }
    pub(in crate::app) fn dialogue_plan_result(
        &mut self,
        path: &Path,
        target: &TargetRef,
        error: Option<String>,
    ) {
        let key = Key {
            path: path.into(),
            target: target.clone(),
        };
        if let Some(error) = error {
            if let Some(form) = self.dialogue.forms.get_mut(&key) {
                form.error = Some(error);
                form.plan = None;
            }
        } else {
            self.dialogue.forms.remove(&key);
            self.dialogue.invalidate();
        }
    }
    pub(in crate::app) fn dialogue_plan_is_current(
        &self,
        path: &Path,
        plan: &DialogueEditPlan,
    ) -> bool {
        self.dialogue
            .forms
            .get(&Key {
                path: path.into(),
                target: plan.request.target.clone(),
            })
            .is_some_and(|form| {
                form.request == plan.request
                    && form
                        .plan
                        .as_ref()
                        .is_some_and(|current| current.plan_digest == plan.plan_digest)
            })
    }
    pub(in crate::app) fn close_dialogue_on_escape(&mut self, ctx: &egui::Context) -> bool {
        if self.input_blocked(ctx) {
            return false;
        }
        let focus = ctx.memory(|memory| memory.focused());
        let preview_navigation = self.mode == Mode::Prose && self.dialogue.enabled;
        if let Some(form) = self.dialogue.forms.values_mut().find(|form| {
            preview_navigation
                && form
                    .plan
                    .as_ref()
                    .is_some_and(|plan| focus.is_some_and(|id| form.navigation.owns(ctx, plan, id)))
        }) {
            form.plan = None;
            form.migration_confirmed = false;
            form.navigation.withdrawn();
            return true;
        }
        let key = self
            .dialogue
            .forms
            .iter()
            .find(|(_, form)| focus.is_some_and(|id| form.input_ids.contains(&id)))
            .map(|(key, _)| key.clone());
        let Some(key) = key else {
            return false;
        };
        let form = self.dialogue.forms.get_mut(&key).unwrap();
        // The local migration TextEdit keeps Escape for this host. Its remembered
        // focus must still yield to a newly opened popup/modal/visible upper window.
        if form
            .plan
            .as_ref()
            .is_some_and(|plan| plan.migration.is_some())
            && !preview_keyboard::available(ctx)
        {
            return false;
        }
        if form.protected() {
            form.discard_confirm = true;
        } else {
            let focus = form.return_focus;
            self.dialogue.forms.remove(&key);
            if let Some(focus) = focus {
                ctx.memory_mut(|memory| memory.request_focus(focus));
            }
        }
        true
    }
}

pub(super) fn toolbar_menu(
    ui: &mut egui::Ui,
    buffer: &WritingBuffer,
    target: &TargetRef,
    view: &mut ViewState,
    action: &mut Action,
    input_busy: bool,
) {
    let contents = |ui: &mut egui::Ui| {
        toolbar(ui, buffer, target, view, action, input_busy);
        if view.has_mode_request(ui.ctx(), buffer, target) || action.production {
            ui.close();
        }
    };
    let response = if egui::containers::menu::is_in_menu(ui) {
        // 嵌套菜单保持 egui SubMenuButton 的父子关闭协议。
        ui.menu_button("对白工具", contents).response
    } else {
        let key = egui::Id::new((
            "dialogue-tools-root-menu",
            buffer.path(),
            &target.kind,
            &target.id,
        ));
        let session = super::toolbar_menu::RootSession::begin(ui.ctx(), key);
        let menu = egui::containers::menu::MenuButton::new("对白工具")
            .config(
                egui::containers::menu::MenuConfig::default()
                    .close_behavior(session.policy(egui::PopupCloseBehavior::CloseOnClick)),
            )
            .ui(ui, contents);
        session.finish(ui.ctx(), &menu.0);
        menu.0
    };
    view.protect_toolbar_input(ui, &response, buffer, target);
    response.on_hover_text("逐句对白、插入正式台词与角色台本；与正文共用同一份草稿");
}

pub(super) fn toolbar(
    ui: &mut egui::Ui,
    buffer: &WritingBuffer,
    target: &TargetRef,
    view: &mut ViewState,
    action: &mut Action,
    input_busy: bool,
) {
    theme::add_enabled_ui(ui, !input_busy, |ui| {
        let insert = ui.button("插入正式台词");
        view.protect_toolbar_input(ui, &insert, buffer, target);
        if insert.clicked() {
            view.request_dialogue(ui.ctx(), buffer, target, true, true);
        }
        let mut enabled = view.dialogue.enabled;
        let toggle = ui
            .checkbox(&mut enabled, "逐句对白")
            .on_hover_text("明确进入同稿 Text/Say 编辑；取消勾选返回普通正文，未纳入输入仍保留");
        view.protect_toolbar_input(ui, &toggle, buffer, target);
        if toggle.changed() {
            view.request_dialogue(ui.ctx(), buffer, target, enabled, false);
        }
        let production = ui.button("角色台本");
        view.protect_toolbar_input(ui, &production, buffer, target);
        if production.clicked() {
            action.production = true;
        }
    });
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    project: &Project,
    buffer: &WritingBuffer,
    target: &TargetRef,
    view: &mut ViewState,
    typography: Typography,
    action: &mut Action,
) -> bool {
    if !matches!(target.kind.as_str(), "event" | "scene" | "fragment") {
        return false;
    }
    // 子模式由作者明确选择；含 Say 的混合章也保留默认正文直接输入与导航合同。
    if !view.dialogue.enabled {
        return false;
    }
    let projection = match view.dialogue.projection(project, buffer, target) {
        Ok(projection) => projection,
        Err(error) => {
            if view.dialogue.enabled || view.dialogue.forms.contains_key(&Key::new(buffer, target))
            {
                ui.colored_label(theme::WARNING(), format!("对白投影暂不可用：{error}"));
                let key = Key::new(buffer, target);
                let shown = retained::unavailable(ui, view, &key, typography);
                retained::notices_except(ui, view, shown.then_some(&key));
            }
            return false;
        }
    };
    render::draw(ui, project, buffer, view, typography, action, &projection);
    true
}
