//! 四类资料共用的 schema 作者工作表；约束、候选和事务全部来自 core。
mod fields;
mod integration;
#[cfg(test)]
mod tests;
mod window;
use std::collections::BTreeMap;
use worldline_core::ast::PropertyValue;
use worldline_core::schemas::{
    SchemaAuthoringPreview, SchemaAuthoringProjection, SchemaAuthoringRequest,
    SchemaBindingMutation, SchemaFieldMutation,
};
use worldline_core::TargetRef;
pub(super) const DRAFT_KIND: &str = "约束资料填写";
const MAX_SESSIONS: usize = 32;
#[derive(Default)]
pub(in crate::app) struct SchemaCoverage {
    pub keys: Vec<String>,
    pub projection: Option<std::sync::Arc<SchemaAuthoringProjection>>,
}
#[derive(Default)]
pub(super) struct SchemaAuthoringState {
    pub open: bool,
    pub return_focus: Option<egui::Id>,
    focus_pending: bool,
    restore_focus: Option<egui::Id>,
    last_focus: Option<egui::Id>,
    // 只记录本工作表真正的预编辑归属，不把平台 Enabled 当作正在组合。
    ime_owner: Option<(TargetRef, egui::Id)>,
    last_viewport: Option<egui::Rect>,
    #[cfg(test)]
    body_scroll: Option<(egui::Id, egui::Rect, egui::Vec2)>,
    active: Option<TargetRef>,
    sessions: BTreeMap<TargetRef, Session>,
    pub source_return: bool,
    error: Option<String>,
}
impl SchemaAuthoringState {
    fn ime_guard(&mut self, ctx: &egui::Context, top: bool) -> (bool, Option<egui::Id>, bool) {
        if !self.open || !top || self.focus_pending {
            self.ime_owner = None;
            return (false, None, false);
        }
        let focused = ctx.memory(|memory| memory.focused());
        let owns_focus = |id| {
            focused == Some(id)
                || focused.is_none()
                || ctx.memory(|memory| memory.had_focus_last_frame(id))
        };
        if self
            .ime_owner
            .as_ref()
            .is_some_and(|(target, id)| self.active.as_ref() != Some(target) || !owns_focus(*id))
        {
            self.ime_owner = None;
        }
        let text_focus = self
            .last_focus
            .filter(|id| owns_focus(*id) && egui::TextEdit::load_state(ctx, *id).is_some());
        let mut guarded = self.ime_owner.is_some();
        let mut pending_preedit = false;
        let mut owner = self.ime_owner.as_ref().map(|(_, id)| *id).or(text_focus);
        ctx.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::Ime(egui::ImeEvent::Preedit(_)) => {
                        // 同批 Tab 可在随后 render 中才首次将焦点交给文本框。
                        guarded = true;
                        pending_preedit = true;
                        // 候选 Escape 可先令 egui 焦点为 None，保留已知文本 owner。
                        if let (Some(target), Some(id)) = (&self.active, owner) {
                            self.ime_owner = Some((target.clone(), id));
                        }
                    }
                    egui::Event::Ime(egui::ImeEvent::Commit(_)) => {
                        // 直接 Commit 也不能同帧触发 Enter / Apply / 来源 / 关闭。
                        guarded = true;
                        pending_preedit = false;
                        self.ime_owner = None;
                    }
                    egui::Event::Ime(egui::ImeEvent::Disabled) => {
                        // 已有组合的结束帧仍保护；普通失焦清理不能禁掉新按钮。
                        guarded |= self.ime_owner.is_some();
                        pending_preedit = false;
                        self.ime_owner = None;
                    }
                    _ => {}
                }
            }
        });
        if !guarded {
            owner = None;
        }
        (guarded, owner, pending_preedit)
    }
    fn reserve_target(&mut self, target: &TargetRef) -> Result<(), String> {
        if !self.sessions.contains_key(target) && self.sessions.len() >= MAX_SESSIONS {
            let clean = self
                .sessions
                .iter()
                .find(|(_, session)| !session.dirty())
                .map(|(target, _)| target.clone());
            if let Some(clean) = clean {
                self.sessions.remove(&clean);
            } else {
                return Err(
                    "已保留32份未应用资料输入；请先处理一份再打开新对象。现有输入全部保留。".into(),
                );
            }
        }
        Ok(())
    }
    pub(super) fn has_unsubmitted_work(&self) -> bool {
        self.sessions.values().any(Session::dirty)
    }
    pub(super) fn draft_sources(&self) -> Vec<String> {
        self.sessions
            .values()
            .filter(|session| session.dirty())
            .map(|session| {
                format!(
                    "{}:{} · {}",
                    session.base.target.kind, session.base.target.id, session.base.object.file
                )
            })
            .collect()
    }
    pub(super) fn signature(&self) -> String {
        format!(
            "{:?}",
            self.sessions
                .values()
                .map(|session| (&session.selected_schema, session.request()))
                .collect::<Vec<_>>()
        )
    }
    pub(super) fn close(&mut self, ctx: &egui::Context) {
        self.ime_owner = None;
        self.open = false;
        if let Some(focus) = self.return_focus.take() {
            self.restore_focus = Some(focus);
            ctx.memory_mut(|memory| memory.request_focus(focus));
        }
    }
    pub(super) fn resume(&mut self) {
        if self.active.is_none() {
            self.active = self.sessions.keys().next().cloned();
        }
        self.open = true;
        self.focus_pending = true;
        self.source_return = false;
    }
}
struct Session {
    base: SchemaAuthoringProjection,
    projection: SchemaAuthoringProjection,
    selected_schema: Option<String>,
    binding: SchemaBindingMutation,
    values: BTreeMap<String, Option<PropertyValue>>,
    preview: Option<SchemaAuthoringPreview>,
    error: Option<String>,
    allow_errors: bool,
    allow_runtime_change: bool,
    discard_confirm: bool,
    next_problem: usize,
    scroll_field: Option<String>,
    diagnostics_stale: bool,
}
impl Session {
    fn new(projection: SchemaAuthoringProjection) -> Self {
        Self {
            selected_schema: projection
                .binding
                .as_ref()
                .map(|binding| binding.schema_id.clone()),
            values: projection
                .fields
                .iter()
                .map(|field| (field.id.clone(), field.value.clone()))
                .collect(),
            base: projection.clone(),
            projection,
            binding: SchemaBindingMutation::Keep,
            preview: None,
            error: None,
            allow_errors: false,
            allow_runtime_change: false,
            discard_confirm: false,
            next_problem: 0,
            scroll_field: None,
            diagnostics_stale: false,
        }
    }
    fn field_changes(&self) -> Vec<SchemaFieldMutation> {
        self.projection
            .fields
            .iter()
            .filter_map(|field| {
                let value = self.values.get(&field.id).cloned().flatten();
                if value == field.value {
                    return None;
                }
                Some(match value {
                    Some(value) => SchemaFieldMutation::Set {
                        field_id: field.id.clone(),
                        key: field.key.clone(),
                        value,
                    },
                    None => SchemaFieldMutation::Remove {
                        field_id: field.id.clone(),
                        key: field.key.clone(),
                    },
                })
            })
            .collect()
    }
    fn request(&self) -> SchemaAuthoringRequest {
        SchemaAuthoringRequest {
            schema_version: 1,
            expected_baseline: self.base.baseline.clone(),
            target: self.base.target.clone(),
            binding: self.binding.clone(),
            fields: self.field_changes(),
        }
    }
    fn dirty(&self) -> bool {
        !matches!(self.binding, SchemaBindingMutation::Keep)
            || !self.field_changes().is_empty()
            || self.selected_schema
                != self
                    .base
                    .binding
                    .as_ref()
                    .map(|binding| binding.schema_id.clone())
    }
    fn invalidate_preview(&mut self) {
        self.preview = None;
        self.diagnostics_stale = true;
        self.allow_errors = false;
        self.allow_runtime_change = false;
        self.error = None;
    }
    fn binding_request(&self) -> SchemaBindingMutation {
        match (&self.base.binding, &self.selected_schema) {
            (None, None) => SchemaBindingMutation::Keep,
            (None, Some(schema_id)) => SchemaBindingMutation::Bind {
                schema_id: schema_id.clone(),
            },
            (Some(_), None) => SchemaBindingMutation::Unbind,
            (Some(binding), Some(schema_id)) if binding.schema_id == *schema_id => {
                SchemaBindingMutation::Keep
            }
            (Some(_), Some(schema_id)) => SchemaBindingMutation::Change {
                schema_id: schema_id.clone(),
            },
        }
    }
}
