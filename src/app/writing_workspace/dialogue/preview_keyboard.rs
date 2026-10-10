//! 当前表单的局部几何；只有有效语句计划获得阅读与撤回导航资格。
use super::*;
use crate::app::writing_workspace::preview_navigation as shared;
pub(super) use shared::available;
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct Navigation {
    digest: Option<String>,
    ids: HashSet<egui::Id>,
    frame: Option<u64>,
    pub return_to_preview: bool,
}
impl Navigation {
    pub fn owns(&self, ctx: &egui::Context, plan: &DialogueEditPlan, id: egui::Id) -> bool {
        available(ctx)
            && self.digest.as_deref() == Some(plan.plan_digest.as_str())
            && self
                .frame
                .is_some_and(|frame| frame.saturating_add(1) >= ctx.cumulative_frame_nr())
            && self.ids.contains(&id)
    }
    pub fn withdrawn(&mut self) {
        *self = Self {
            return_to_preview: true,
            ..Default::default()
        };
    }
}
#[derive(Clone)]
struct Active {
    identity: egui::Id,
    digest: Option<String>,
    migration: bool,
    ids: HashSet<egui::Id>,
    viewport: egui::Rect,
    layer: egui::LayerId,
    enabled: bool,
    feedback_owner: Option<egui::Id>,
}
fn active_id() -> egui::Id {
    egui::Id::new("dialogue-migration-keyboard-scope")
}
/// Only the current form's draw closure installs this context. Popup layers are excluded.
pub(super) struct Scope {
    ctx: egui::Context,
    key: egui::Id,
    viewport: egui::Rect,
    layer: egui::LayerId,
    enabled: bool,
    previous: Option<Active>,
    feedback_owner: Option<egui::Id>,
}
impl Scope {
    pub fn new(
        ui: &egui::Ui,
        key: &Key,
        viewport: egui::Rect,
        enabled: bool,
        feedback_owner: Option<egui::Id>,
    ) -> Self {
        let previous = ui.ctx().data_mut(|data| {
            let previous = data.get_temp::<Active>(active_id());
            data.remove::<Active>(active_id());
            previous
        });
        Self {
            ctx: ui.ctx().clone(),
            key: egui::Id::new(("migration-navigation", key)),
            viewport,
            layer: ui.layer_id(),
            enabled: enabled
                && ui.is_enabled()
                && viewport.width() > 16.0
                && viewport.height() > 16.0,
            previous,
            feedback_owner,
        }
    }
    pub fn sync(&self, plan: Option<&DialogueEditPlan>) {
        self.ctx.data_mut(|data| {
            let old = data.get_temp::<Active>(active_id());
            let digest = plan.map(|plan| plan.plan_digest.clone());
            let ids = old
                .filter(|active| digest.is_some() && active.digest == digest)
                .map(|active| active.ids)
                .unwrap_or_default();
            // Geometry belongs to this live form, even before a valid plan exists.
            // None still clears every plan-owned navigation credential.
            data.insert_temp(
                active_id(),
                Active {
                    identity: digest
                        .as_ref()
                        .map_or(self.key, |digest| self.key.with(digest)),
                    digest,
                    migration: plan.is_some_and(|plan| plan.migration.is_some()),
                    ids,
                    viewport: self.viewport,
                    layer: self.layer,
                    enabled: self.enabled,
                    feedback_owner: self.feedback_owner,
                },
            );
        });
    }
    pub fn finish(&self, navigation: &mut Navigation) {
        let active = self.ctx.data(|data| data.get_temp::<Active>(active_id()));
        let return_to_preview = navigation.return_to_preview;
        *navigation = active
            .filter(|active| active.digest.is_some())
            .map(|active| Navigation {
                digest: active.digest,
                ids: active.ids,
                frame: Some(self.ctx.cumulative_frame_nr()),
                return_to_preview,
            })
            .unwrap_or_else(|| Navigation {
                return_to_preview,
                ..Default::default()
            });
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        self.ctx.data_mut(|data| {
            data.remove::<Active>(active_id());
            if let Some(previous) = self.previous.take() {
                data.insert_temp(active_id(), previous);
            }
        });
    }
}
pub(super) fn control(ui: &egui::Ui, response: egui::Response) -> egui::Response {
    let active = ui.ctx().data(|data| data.get_temp::<Active>(active_id()));
    if let Some(active) =
        active.filter(|active| active.enabled && active.layer == response.layer_id)
    {
        if active.digest.is_some() {
            ui.ctx().data_mut(|data| {
                let mut active = active.clone();
                active.ids.insert(response.id);
                data.insert_temp(active_id(), active);
            });
        }
        reveal(ui, &response, active.feedback_owner != Some(response.id));
        if response.has_focus() && available(ui.ctx()) {
            ui.ctx().memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        escape: active.digest.is_some(),
                        ..Default::default()
                    },
                );
            });
        }
    }
    response
}
/// Reveal the original TextEdit without admitting it to preview/Escape navigation.
pub(super) fn text_control(ui: &egui::Ui, response: &egui::Response) {
    let active = ui.ctx().data(|data| data.get_temp::<Active>(active_id()));
    if let Some(active) =
        active.filter(|active| active.enabled && active.layer == response.layer_id)
    {
        reveal(ui, response, active.feedback_owner != Some(response.id));
        if active.digest.is_some() && response.has_focus() && available(ui.ctx()) {
            // TextEdit already installed its normal arrows/Tab policy. Retain
            // Escape only for the existing valid-plan host cancellation path.
            ui.ctx().memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                        ..Default::default()
                    },
                );
            });
        }
    }
}
fn reveal(ui: &egui::Ui, response: &egui::Response, keep_visible: bool) {
    if keep_visible {
        shared::reveal(ui, response, true, true);
    } else if response.enabled() && response.has_focus() && available(ui.ctx()) {
        shared::outline(ui, response.rect.expand(2.0));
    }
}

pub(super) fn add(ui: &mut egui::Ui, enabled: bool, widget: impl egui::Widget) -> egui::Response {
    let response = theme::add_enabled(ui, enabled, widget);
    control(ui, response)
}
pub(super) fn button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    add(ui, true, egui::Button::new(label))
}
pub(super) fn checkbox(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    add(ui, true, egui::Checkbox::new(value, label))
}
pub(super) fn collapsing<R>(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    body: impl FnOnce(&mut egui::Ui) -> R,
) {
    let response = ui.collapsing(label, body);
    control(ui, response.header_response);
}
pub(super) fn reading(ui: &mut egui::Ui, line_height: f32) {
    let Some(active) = ui
        .ctx()
        .data(|data| data.get_temp::<Active>(active_id()))
        .filter(|active| active.digest.is_some())
    else {
        return;
    };
    let response = shared::reading(
        ui,
        shared::Reader {
            identity: active.identity,
            label: if active.migration {
                "迁移预览阅读区 · ↑↓滚动"
            } else {
                "语句预览阅读区 · ↑↓滚动"
            },
            viewport: active.viewport,
            enabled: active.enabled,
            horizontal: false,
            vertical: true,
            step: egui::vec2(0.0, line_height.max(16.0)),
            escape: true,
        },
    );
    if active.enabled {
        ui.ctx().data_mut(|data| {
            let mut active = active.clone();
            active.ids.insert(response.id);
            data.insert_temp(active_id(), active);
        });
    }
}

/// Failed-preview positioning is local to this exact Form instance and receiver.
/// It does not own keys, a plan digest or a second input buffer.
#[derive(Default)]
pub(super) struct Feedback {
    owner: Option<egui::Id>,
}
impl Feedback {
    pub fn requested(&mut self, ctx: &egui::Context) {
        self.owner = ctx.memory(|memory| memory.focused());
    }
    pub fn owner(&mut self, ctx: &egui::Context, has_error: bool) -> Option<egui::Id> {
        let editing = ctx.input(|input| {
            input.events.iter().any(|event| match event {
                egui::Event::Text(_) | egui::Event::Paste(_) | egui::Event::Cut => true,
                egui::Event::Ime(egui::ImeEvent::Commit(_)) => true,
                egui::Event::Ime(egui::ImeEvent::Preedit(text)) => !text.is_empty(),
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    matches!(
                        key,
                        egui::Key::ArrowUp
                            | egui::Key::ArrowDown
                            | egui::Key::ArrowLeft
                            | egui::Key::ArrowRight
                            | egui::Key::Home
                            | egui::Key::End
                            | egui::Key::PageUp
                            | egui::Key::PageDown
                            | egui::Key::Backspace
                            | egui::Key::Delete
                            | egui::Key::Enter
                            | egui::Key::Space
                            | egui::Key::Tab
                            | egui::Key::Escape
                    ) || (modifiers.command
                        && matches!(
                            key,
                            egui::Key::A
                                | egui::Key::Z
                                | egui::Key::Y
                                | egui::Key::X
                                | egui::Key::V
                        ))
                }
                egui::Event::PointerButton { pressed: true, .. } => true,
                _ => false,
            })
        });
        if !has_error || editing || ctx.memory(|memory| memory.focused()) != self.owner {
            self.owner = None;
        }
        self.owner
    }
}
