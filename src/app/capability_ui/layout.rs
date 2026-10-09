//! Match egui's complete Window chrome before assigning a short-screen content budget.
pub(super) const TITLE: &str = "显式启用语言与资料能力";

pub(super) struct Budget {
    pub compact: bool,
    pub bounds: egui::Rect,
    pub inner: egui::Vec2,
    pub frame: egui::Frame,
}

impl Budget {
    pub fn new(ctx: &egui::Context) -> Self {
        let screen = ctx.screen_rect();
        let style = ctx.style();
        let frame = egui::Frame::window(&style);
        let title_font = egui::TextStyle::Heading.resolve(&style);
        let title_height = ctx
            .fonts(|fonts| fonts.row_height(&title_font))
            .max(style.spacing.interact_size.y)
            + frame.inner_margin.sum().y;
        let chrome =
            frame.total_margin().sum() + egui::vec2(0.0, title_height + frame.stroke.width);
        let bounds = screen.shrink(8.0);
        Self {
            compact: screen.width() < 760.0 || screen.height() < 640.0,
            bounds,
            inner: (bounds.size() - chrome).max(egui::Vec2::splat(1.0)),
            frame,
        }
    }
}

/// Tab 的后退焦点下一帧才交付；仅短暂滚入，新的指针或输入意图优先。
#[derive(Clone, Copy)]
pub(super) struct FocusReveal {
    enabled: bool,
    restore_escaped_focus: bool,
}

impl FocusReveal {
    pub fn for_frame(ctx: &egui::Context, opening: bool, restore_escaped_focus: bool) -> Self {
        let id = egui::Id::new("capability-keyboard-focus-frame");
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
            // raw 保留已被表单 guard 消费的键，不能让旧 Tab 意图跨过 Escape。
            let newer = input.raw.events.iter().any(|event| {
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
        if opening || restore_escaped_focus {
            ctx.data_mut(|data| data.insert_temp(id, frame));
        } else if newer_intent {
            ctx.data_mut(|data| data.remove::<u64>(id));
        } else if tab {
            ctx.data_mut(|data| data.insert_temp(id, frame));
        }
        let enabled = ctx
            .data(|data| data.get_temp::<u64>(id))
            .is_some_and(|last| last <= frame && frame - last <= 1);
        Self {
            enabled,
            restore_escaped_focus,
        }
    }

    pub fn request(self, ui: &egui::Ui, response: &egui::Response) {
        response.request_focus();
        reveal(ui, response);
    }

    pub fn reveal(self, ui: &egui::Ui, response: &egui::Response) {
        if self.restore_escaped_focus
            && ui.ctx().memory(|memory| {
                memory.focused().is_none() && memory.had_focus_last_frame(response.id)
            })
        {
            response.request_focus();
        }
        if self.enabled && response.has_focus() {
            reveal(ui, response);
        }
    }
}

fn reveal(ui: &egui::Ui, response: &egui::Response) {
    if !ui.clip_rect().contains_rect(response.rect) {
        ui.scroll_to_rect_animation(response.rect, None, egui::style::ScrollAnimation::none());
    }
}
