//! Focus ownership belongs only to the open transient object-reading host.
use crate::app::{writing_workspace::preview_navigation as shared, Tab, WorldeditApp};
use std::path::PathBuf;

fn key() -> egui::Id {
    egui::Id::new("object-reading-keyboard-session")
}
pub(super) fn layer() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Foreground, egui::Id::new("object-reading"))
}
#[derive(Clone)]
pub(super) struct Session {
    root: PathBuf,
    tab: Tab,
    opener: Option<egui::Id>,
    source_return: bool,
    cycle: shared::FocusCycle,
    target: worldline_core::catalog::TargetRef,
}
impl Session {
    pub(super) fn begin(app: &WorldeditApp, ctx: &egui::Context) -> Self {
        let target = app.reading_target.clone().expect("open transient reading");
        let mut session = ctx
            .data(|data| data.get_temp::<Self>(key()))
            .filter(|session| session.root == app.project.root)
            .unwrap_or_else(|| Self {
                root: app.project.root.clone(),
                tab: app.tab,
                opener: ctx
                    .memory(|memory| memory.focused())
                    .or(app.command_palette.frame_focus),
                source_return: app.reading_return.is_some(),
                cycle: shared::FocusCycle::default(),
                target: target.clone(),
            });
        if session.target != target {
            session.target = target;
            session.cycle = shared::FocusCycle::default();
        }
        session
    }
    pub(super) fn advance(&self, ctx: &egui::Context, active: bool) {
        self.cycle.advance(ctx, active);
    }
    pub(super) fn initial(&mut self, response: &egui::Response, active: bool) {
        self.cycle.initial(response, active);
    }
    pub(super) fn finish(mut self, ctx: &egui::Context, active: bool, reader: Option<egui::Id>) {
        self.cycle.finish(ctx, layer(), active, reader);
        ctx.data_mut(|data| data.insert_temp(key(), self));
    }
}
/// Called after the background author UI has registered its current widgets.
/// Successful navigation to another page is never undone by a stale opener.
pub(super) fn finish_closed(app: &WorldeditApp, ctx: &egui::Context) {
    shared::clear_transient_host(ctx, layer());
    let session = ctx.data_mut(|data| {
        let session = data.get_temp::<Session>(key());
        data.remove::<Session>(key());
        session
    });
    let Some(session) = session else {
        return;
    };
    if session.source_return || session.root != app.project.root || session.tab != app.tab {
        return;
    }
    if let Some(id) = session.opener {
        let registered = ctx.viewport(|v| v.this_pass.widgets.get(id).is_some_and(|w| w.enabled));
        if registered {
            ctx.memory_mut(|m| m.request_focus(id));
            ctx.request_repaint();
        }
    }
}
pub(super) fn reveal_focused(ui: &egui::Ui, reader: Option<egui::Id>, outside: &[egui::Id]) {
    let response = ui
        .ctx()
        .memory(|memory| memory.focused())
        .filter(|id| {
            ui.ctx().viewport(|v| {
                v.this_pass
                    .widgets
                    .get(*id)
                    .is_some_and(|w| w.layer_id == ui.layer_id())
            })
        })
        .and_then(|id| ui.ctx().read_response(id));
    if let Some(response) = response {
        if reader != Some(response.id) && !outside.contains(&response.id) {
            shared::reveal(
                ui,
                &response,
                true,
                egui::TextEdit::load_state(ui.ctx(), response.id).is_none(),
            );
        }
    }
}
