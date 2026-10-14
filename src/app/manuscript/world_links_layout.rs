//! 关联窗的真实屏幕预算与局部预览导航，不改变作者草稿。
use crate::app::writing_workspace::preview_navigation as shared;

pub(super) fn layer() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Middle, egui::Id::new("manuscript-world-links"))
}
pub(super) struct WindowLayout {
    pub compact: bool,
    pub frame: egui::Frame,
    pub max_inner: egui::Vec2,
}

impl WindowLayout {
    pub fn new(ctx: &egui::Context) -> Self {
        let screen = ctx.screen_rect();
        let style = ctx.style();
        let compact = screen.width() < 600.0 || screen.height() < 460.0;
        let mut frame = egui::Frame::window(&style);
        if compact {
            frame.inner_margin = egui::Margin::same(8);
        }
        let title_font = egui::TextStyle::Heading.resolve(&style);
        let title_height = ctx
            .fonts(|fonts| fonts.row_height(&title_font))
            .max(style.spacing.interact_size.y)
            + frame.inner_margin.sum().y;
        // Window 的 Resize max_height 不含标题和 frame；必须先扣掉完整外框。
        let outer = frame.total_margin().sum() + egui::vec2(0.0, title_height + frame.stroke.width);
        let max_inner = (screen.size() - outer - egui::Vec2::splat(16.0)).max(egui::Vec2::ZERO);
        Self {
            compact,
            frame,
            max_inner,
        }
    }
}

pub(super) fn scroll_height(ui: &egui::Ui) -> f32 {
    ui.available_height()
        .min(ui.clip_rect().bottom() - ui.cursor().top())
        .min(ui.ctx().screen_rect().bottom() - ui.cursor().top() - 8.0)
        .max(0.0)
}

#[derive(Clone, Copy)]
pub(super) struct FocusReveal {
    enabled: bool,
    preview: bool,
}

impl FocusReveal {
    pub fn for_frame(ctx: &egui::Context, blocked: bool, preview: bool) -> Self {
        let id = egui::Id::new("world-link-keyboard-focus-frame");
        let frame = ctx.cumulative_frame_nr();
        let (tab, newer_intent) = ctx.input(|input| {
            let tab = input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::Tab,
                        pressed: true,
                        ..
                    }
                )
            });
            let newer = input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::PointerButton { .. }
                        | egui::Event::MouseWheel { .. }
                        | egui::Event::Touch { .. }
                        | egui::Event::Ime(_)
                        | egui::Event::Text(_)
                        | egui::Event::Paste(_)
                        | egui::Event::WindowFocused(false)
                ) || matches!(event, egui::Event::Key { key, pressed: true, .. }
                    if *key != egui::Key::Tab)
            });
            (tab, newer)
        });
        if blocked || newer_intent {
            ctx.data_mut(|data| data.remove::<u64>(id));
        } else if tab {
            ctx.data_mut(|data| data.insert_temp(id, frame));
        }
        let enabled = ctx
            .data(|data| data.get_temp::<u64>(id))
            .is_some_and(|last| last <= frame && frame - last <= 1);
        Self {
            enabled,
            preview: preview && !blocked,
        }
    }

    pub fn reveal(self, ui: &egui::Ui, response: &egui::Response) {
        if self.preview {
            shared::reveal(
                ui,
                response,
                true,
                egui::TextEdit::load_state(ui.ctx(), response.id).is_none(),
            );
            return;
        }
        if self.enabled && response.has_focus() && !ui.clip_rect().contains_rect(response.rect) {
            // Shift+Tab 的焦点可能到下一帧才交付；不依赖当帧 gained_focus。
            ui.scroll_to_rect_animation(response.rect, None, egui::style::ScrollAnimation::none());
        }
    }
}

impl FocusReveal {
    pub fn add(
        self,
        ui: &mut egui::Ui,
        enabled: bool,
        widget: impl egui::Widget,
    ) -> egui::Response {
        let response = crate::theme::add_enabled(ui, enabled, widget);
        self.reveal(ui, &response);
        response
    }
    pub fn reveal_focused(self, ui: &egui::Ui, reader: Option<egui::Id>, outside: &[egui::Id]) {
        if !self.preview {
            return;
        }
        let response = ui
            .ctx()
            .memory(|m| m.focused())
            .filter(|id| Some(*id) != reader && !outside.contains(id))
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
            self.reveal(ui, &response);
        }
    }
}
