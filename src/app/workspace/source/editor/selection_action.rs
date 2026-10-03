//! 被动建档建议不等于程序来源定位；显式作者动作继续由编辑器处理。
use crate::{app::WorldeditApp, theme};
use std::path::Path;
use worldline_core::authoring_intents::TextSelection;

impl WorldeditApp {
    pub(super) fn source_selection_suggestion(
        &self,
        ctx: &egui::Context,
        id: egui::Id,
        path: &Path,
        source_focused: bool,
        mut selection: Option<TextSelection>,
        anchor: Option<egui::Pos2>,
    ) -> Option<TextSelection> {
        let mut chosen = None;
        let selection_area_id = egui::Id::new(("source-selection-action", &path));
        // 点建议自身时TextEdit会因外部按下而暂失焦；保留这次明确的建档点击。
        let pressed_on_suggestion = ctx
            .input(|input| {
                input
                    .pointer
                    .primary_pressed()
                    .then(|| input.pointer.interact_pos())
                    .flatten()
            })
            .is_some_and(|position| {
                ctx.layer_id_at(position)
                    == Some(egui::LayerId::new(
                        egui::Order::Foreground,
                        selection_area_id,
                    ))
            });
        if source_focused && pressed_on_suggestion {
            ctx.memory_mut(|memory| memory.request_focus(id));
        }
        let range = egui::TextEdit::load_state(ctx, id)
            .and_then(|state| state.cursor.char_range());
        // 诊断抑制仅跟随本次程序选区，人工选择或显式Ctrl+Enter照常工作。
        if crate::app::search::selection_is_diagnostic(ctx, id, range) {
            selection = None;
        }
        // 选区建议属于源码编辑焦点，不能越过命令/搜索或受保护上层。
        if !ctx.memory(|memory| memory.has_focus(id))
            || self.ime_composing
            || self.command_palette.ime
            || self.command_palette.ime_frame
            || self.command_palette.open
            || self.search_open
            || self.personal.preferences_open
            || self
                .command_palette
                .focus_stack
                .iter()
                .any(|(kind, _)| *kind != "problems")
        {
            selection = None;
        }
        if let (Some(selection), Some(anchor)) = (selection, anchor) {
            let screen = ctx.screen_rect();
            let pos = egui::pos2(
                anchor.x.clamp(
                    screen.left() + 8.0,
                    (screen.right() - 220.0).max(screen.left() + 8.0),
                ),
                anchor.y.clamp(
                    screen.top() + 8.0,
                    (screen.bottom() - 44.0).max(screen.top() + 8.0),
                ),
            );
            egui::Area::new(selection_area_id)
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(theme::muted("Ctrl+Enter 也可从选中文本建档"));
                        if ui.button("从选中文本建档").clicked() {
                            chosen = Some(selection);
                        }
                    });
                });
        }
        chosen
    }
}
