//! @补全与普通对象页共享core匹配；只在作者明确选择后插入完整引用。
use super::*;
use crate::app::object_picker::{candidate_row_at_revision, filter, CandidatePage};
use worldline_core::catalog::{CatalogObject, TargetRef};

#[derive(Clone, Default)]
struct MentionState {
    page: CandidatePage,
    visible: Vec<TargetRef>,
    selected: usize,
    changed_frame: Option<u64>,
    popup_focus: Option<egui::Id>,
    popup_rect: Option<egui::Rect>,
}
pub(super) struct MentionPage {
    pub items: Vec<CatalogObject>,
    pub stale: bool,
    pub visible: Vec<TargetRef>,
}
impl WorldeditApp {
    pub(super) fn source_mention_has_input(
        &self,
        ctx: &egui::Context,
        path: &std::path::Path,
    ) -> bool {
        let focused = ctx.memory(|memory| memory.focused());
        focused == Some(egui::Id::new(("source", path)))
            || focused.is_some_and(|focused| {
                ctx.data(|data| data.get_temp::<MentionState>(self.mention_id(path)))
                    .is_some_and(|state| state.popup_focus == Some(focused))
            })
            || ctx
                .data(|data| data.get_temp::<MentionState>(self.mention_id(path)))
                .and_then(|state| state.popup_rect)
                .is_some_and(|rect| {
                    ctx.input(|input| {
                        (input.pointer.any_down() || input.pointer.any_released())
                            && input
                                .pointer
                                .interact_pos()
                                .is_some_and(|pos| rect.contains(pos))
                    })
                })
    }

    pub(in crate::app) fn source_mention_owns_escape(&self, ctx: &egui::Context) -> bool {
        if self.tab != crate::app::Tab::Edit
            || self.ime_composing
            || self
                .command_palette
                .focus_stack
                .iter()
                .any(|(kind, _)| *kind != "problems")
        {
            return false;
        }
        let path = &self.active_file;
        let editor = egui::Id::new(("source", path));
        if !ctx.memory(|memory| memory.has_focus(editor)) {
            return false;
        }
        let Some(range) =
            egui::TextEdit::load_state(ctx, editor).and_then(|state| state.cursor.char_range())
        else {
            return false;
        };
        if range.primary.index != range.secondary.index {
            return false;
        }
        self.project
            .document(path)
            .ok()
            .and_then(|source| active_mention(source, range.primary.index))
            .is_some_and(|(at, query)| {
                !query.is_empty()
                    && self.mention_suppression.as_ref() != Some(&(path.clone(), at, query))
            })
    }

    fn mention_id(&self, path: &std::path::Path) -> egui::Id {
        egui::Id::new(("source-mention-page", &self.project.root, path))
    }
    pub(super) fn mention_page(
        &mut self,
        ctx: &egui::Context,
        path: &std::path::Path,
        at: usize,
        query: &str,
        keys: bool,
    ) -> MentionPage {
        let id = self.mention_id(path);
        let mut state = ctx
            .data_mut(|data| data.get_temp::<MentionState>(id))
            .unwrap_or_default();
        let revision = format!("{:?}:{}:{at}", self.project.root, self.version);
        let mut stale = false;
        if let Some(snapshot) = &self.snapshot {
            let catalog = &snapshot.result.analysis.catalog;
            let previous_serial = state.page.serial;
            stale = state
                .page
                .refresh(catalog, query, &filter(&[], None), &revision);
            if previous_serial != state.page.serial {
                self.mention_selection = None;
                state.visible.clear();
            }
            if stale {
                state.changed_frame = Some(ctx.cumulative_frame_nr());
                self.mention_selection = None;
                state.visible.clear();
                if keys {
                    ctx.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
                    });
                }
            }
            if keys
                && !self.ime_composing
                && !self.command_palette.ime
                && !self.command_palette.ime_frame
            {
                let (next, previous) = ctx.input_mut(|input| {
                    (
                        input.consume_key(egui::Modifiers::NONE, egui::Key::PageDown),
                        input.consume_key(egui::Modifiers::NONE, egui::Key::PageUp),
                    )
                });
                if (next && state.page.turn(false)) || (previous && state.page.turn(true)) {
                    state
                        .page
                        .refresh(catalog, query, &filter(&[], None), &revision);
                    self.mention_selection = None;
                    state.visible.clear();
                }
            }
        } else {
            state.page.result = None;
            state.visible.clear();
        }
        let items = state
            .page
            .result
            .as_ref()
            .and_then(|page| page.as_ref().ok())
            .map(|page| page.items.clone())
            .unwrap_or_default();
        let visible = state.visible.clone();
        ctx.data_mut(|data| data.insert_temp(id, state));
        MentionPage {
            items,
            stale,
            visible,
        }
    }

    pub(super) fn mention_popup(
        &mut self,
        ctx: &egui::Context,
        path: &std::path::Path,
        anchor: egui::Pos2,
        at: usize,
        query: &str,
        selected: usize,
    ) -> Option<TargetRef> {
        let screen = ctx.screen_rect();
        let width = 440.0_f32.min(screen.width() - 24.0).max(160.0);
        let pos = egui::pos2(
            anchor.x.clamp(
                screen.left() + 8.0,
                (screen.right() - width - 16.0).max(screen.left() + 8.0),
            ),
            anchor.y.clamp(
                screen.top() + 8.0,
                (screen.bottom() - 350.0).max(screen.top() + 8.0),
            ),
        );
        let id = self.mention_id(path);
        let mut state = ctx
            .data_mut(|data| data.get_temp::<MentionState>(id))
            .unwrap_or_default();
        let mut action = None;
        let ime_busy = self.ime_composing
            || self.command_palette.ime
            || self.command_palette.ime_frame
            || ctx.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Ime(_)))
            });
        let popup = egui::Area::new(egui::Id::new(("source-mention-popup", path)))
            .order(egui::Order::Foreground)
            .enabled(!ime_busy)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                theme::popup().show(ui, |ui| {
                    ui.set_width(width);
                    ui.label(format!("引用 @{query} · 已应用目录"));
                    if self
                        .snapshot
                        .as_ref()
                        .is_some_and(|snapshot| snapshot.result.has_errors())
                    {
                        ui.colored_label(theme::GOLD(), "源码含错误，仅列已解析对象");
                    }
                    let turned = state.page.controls(ui);
                    if turned {
                        ctx.memory_mut(|memory| {
                            memory.request_focus(egui::Id::new(("source", path)))
                        });
                        self.mention_selection = None;
                        if let Some(snapshot) = &self.snapshot {
                            state.page.refresh(
                                &snapshot.result.analysis.catalog,
                                query,
                                &filter(&[], None),
                                &format!("{:?}:{}:{at}", self.project.root, self.version),
                            );
                        }
                    }
                    let selected = if turned { 0 } else { selected };
                    state.visible.clear();
                    if let Some(Ok(page)) = &state.page.result {
                        if page.total == 0 {
                            ui.label("没有匹配对象");
                        }
                        egui::ScrollArea::vertical()
                            .id_salt("source-mention-rows")
                            .max_height(220.0)
                            .show(ui, |ui| {
                                for (index, object) in page.items.iter().enumerate() {
                                    let row = candidate_row_at_revision(
                                        ui,
                                        object,
                                        Some(&self.project.root),
                                        index == selected,
                                        state.page.serial,
                                    );
                                    if ui.is_visible()
                                        && ui.is_enabled()
                                        && ui.clip_rect().contains_rect(row.rect)
                                    {
                                        state.visible.push(object.target.clone());
                                    }
                                    if index == selected && state.selected != selected {
                                        row.scroll_to_me(None);
                                    }
                                    if row.clicked()
                                        && state.changed_frame != Some(ctx.cumulative_frame_nr())
                                    {
                                        action = Some(object.target.clone());
                                    }
                                }
                            });
                    }
                    state.selected = selected;
                    ui.small("↑↓选择 · 翻页键换页 · Enter插入可见项 · Esc收起");
                });
            });
        state.popup_rect = Some(popup.response.rect);
        state.popup_focus = ctx.memory(|memory| memory.focused());
        ctx.data_mut(|data| data.insert_temp(id, state));
        action
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
