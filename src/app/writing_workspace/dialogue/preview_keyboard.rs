//! 当前迁移计划的局部导航；不登记为文字/IME 接收者，不接管其他编辑模式。
use super::*;
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
        plan.migration.is_some()
            && available(ctx)
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
    digest: String,
    ids: HashSet<egui::Id>,
    viewport: egui::Rect,
    layer: egui::LayerId,
    enabled: bool,
}
fn active_id() -> egui::Id {
    egui::Id::new("dialogue-migration-keyboard-scope")
}
pub(super) fn available(ctx: &egui::Context) -> bool {
    ctx.input(|input| {
        input.focused && !input.events.iter().any(|event| {
            matches!(event, egui::Event::Ime(egui::ImeEvent::Commit(_)))
                || matches!(event, egui::Event::Ime(egui::ImeEvent::Preedit(text)) if !text.is_empty())
        })
    }) && !egui::Popup::is_any_open(ctx)
        && ctx.memory(|memory| {
            memory.top_modal_layer().is_none()
                && !memory.areas().visible_layer_ids().iter()
                    .any(|layer| layer.order == egui::Order::Middle)
        })
}
/// Only the current form's draw closure installs this context. Popup layers are excluded.
pub(super) struct Scope {
    ctx: egui::Context,
    key: egui::Id,
    viewport: egui::Rect,
    layer: egui::LayerId,
    enabled: bool,
    previous: Option<Active>,
}
impl Scope {
    pub fn new(ui: &egui::Ui, key: &Key, viewport: egui::Rect, enabled: bool) -> Self {
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
        }
    }
    pub fn sync(&self, plan: Option<&DialogueEditPlan>) {
        let plan = plan.filter(|plan| plan.migration.is_some());
        self.ctx.data_mut(|data| {
            let old = data.get_temp::<Active>(active_id());
            data.remove::<Active>(active_id());
            if let Some(plan) = plan {
                let ids = old
                    .filter(|active| active.digest == plan.plan_digest)
                    .map(|active| active.ids)
                    .unwrap_or_default();
                data.insert_temp(
                    active_id(),
                    Active {
                        identity: self.key.with(&plan.plan_digest),
                        digest: plan.plan_digest.clone(),
                        ids,
                        viewport: self.viewport,
                        layer: self.layer,
                        enabled: self.enabled,
                    },
                );
            }
        });
    }
    pub fn finish(&self, navigation: &mut Navigation) {
        let active = self.ctx.data(|data| data.get_temp::<Active>(active_id()));
        let return_to_preview = navigation.return_to_preview;
        *navigation = active
            .map(|active| Navigation {
                digest: Some(active.digest),
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
        ui.ctx().data_mut(|data| {
            let mut active = active.clone();
            active.ids.insert(response.id);
            data.insert_temp(active_id(), active);
        });
        reveal(ui, &response, true);
        if response.has_focus() && available(ui.ctx()) {
            ui.ctx().memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        escape: true,
                        ..Default::default()
                    },
                )
            });
        }
    }
    response
}
/// Reveal the original TextEdit without admitting it to preview/Escape navigation.
pub(super) fn text_control(ui: &egui::Ui, response: &egui::Response) {
    if ui
        .ctx()
        .data(|data| data.get_temp::<Active>(active_id()))
        .is_some_and(|active| active.enabled && active.layer == response.layer_id)
    {
        reveal(ui, response, true);
        if response.has_focus() && available(ui.ctx()) {
            // These formal fields use TextEdit's default arrows and Tab behavior.
            // Only Escape is retained so the existing host can protect this same F.
            ui.ctx().memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                        ..Default::default()
                    },
                )
            });
        }
    }
}
fn reveal(ui: &egui::Ui, response: &egui::Response, keep_visible: bool) {
    if response.enabled() && response.has_focus() && available(ui.ctx()) {
        // Focus can arrive after key-down or layout can move its owner. Keep normal
        // controls visible; only the reading button is allowed to scroll offscreen.
        if (keep_visible || response.gained_focus())
            && !ui.clip_rect().contains_rect(response.rect.expand(2.0))
        {
            response.scroll_to_me(Some(egui::Align::Center));
        }
        outline(ui, response.rect.expand(2.0));
    }
}

fn outline(ui: &egui::Ui, rect: egui::Rect) {
    let theme = theme::resolved(ui.ctx());
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(theme.focus_width, theme.colors.focus),
        egui::StrokeKind::Inside,
    );
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
    let Some(active) = ui.ctx().data(|data| data.get_temp::<Active>(active_id())) else {
        return;
    };
    let response = ui
        .push_id(active.identity, |ui| {
            let response = ui.add_enabled(
                active.enabled,
                egui::Button::new("迁移预览阅读区 · ↑↓滚动").wrap(),
            );
            // Register this stop without forcing it back onscreen after every Down.
            if active.enabled {
                ui.ctx().data_mut(|data| {
                    let mut active = active.clone();
                    active.ids.insert(response.id);
                    data.insert_temp(active_id(), active);
                });
                reveal(ui, &response, false);
            }
            response
        })
        .inner;
    if response.has_focus() && active.enabled && available(ui.ctx()) {
        ui.ctx().memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    vertical_arrows: true,
                    escape: true,
                    ..Default::default()
                },
            )
        });
        if ui.input(|input| input.modifiers == egui::Modifiers::NONE) {
            let up =
                ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
            let down = ui
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
            if up || down {
                ui.scroll_with_delta(egui::vec2(
                    0.0,
                    (i32::from(up) - i32::from(down)) as f32 * line_height.max(16.0),
                ));
            }
        }
        let theme = theme::resolved(ui.ctx());
        ui.painter().with_clip_rect(active.viewport).rect_stroke(
            active.viewport.shrink(theme.focus_width * 0.5),
            2.0,
            egui::Stroke::new(theme.focus_width, theme.colors.focus),
            egui::StrokeKind::Inside,
        );
    }
}
