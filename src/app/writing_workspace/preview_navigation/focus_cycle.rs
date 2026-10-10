//! Explicit host opt-in only: a current real-widget roster, never global Tab routing.
#[derive(Clone)]
pub(in crate::app) struct FocusCycle {
    first: bool,
    owners: Vec<egui::Id>,
    frame: u64,
}
impl Default for FocusCycle {
    fn default() -> Self {
        Self {
            first: true,
            owners: Vec::new(),
            frame: 0,
        }
    }
}
impl FocusCycle {
    pub fn load(ctx: &egui::Context, key: egui::Id) -> Self {
        ctx.data(|data| data.get_temp::<Self>(key))
            .filter(|cycle| cycle.frame.saturating_add(1) >= ctx.cumulative_frame_nr())
            .unwrap_or_default()
    }
    pub fn store(self, ctx: &egui::Context, key: egui::Id) {
        ctx.data_mut(|data| data.insert_temp(key, self));
    }
    pub fn clear(ctx: &egui::Context, key: egui::Id) {
        ctx.data_mut(|data| data.remove::<Self>(key));
    }
    pub fn advance(&self, ctx: &egui::Context, active: bool) {
        if !active
            || self.frame.saturating_add(1) < ctx.cumulative_frame_nr()
            || egui::Popup::is_any_open(ctx)
            || ctx.memory(|m| m.top_modal_layer().is_some())
        {
            return;
        }
        let Some(at) = ctx
            .memory(|m| m.focused())
            .and_then(|id| self.owners.iter().position(|i| *i == id))
        else {
            return;
        };
        let modifiers = ctx.input(|i| i.modifiers);
        if modifiers != egui::Modifiers::NONE && modifiers != egui::Modifiers::SHIFT {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(modifiers, egui::Key::Tab)) {
            let step = if modifiers.shift {
                self.owners.len() - 1
            } else {
                1
            };
            let next = self.owners[(at + step) % self.owners.len()];
            ctx.memory_mut(|m| m.request_focus(next));
        }
    }
    pub fn initial(&mut self, response: &egui::Response, active: bool) {
        if self.first && active && response.enabled() {
            response.request_focus();
            self.first = false;
        }
    }
    /// Opt in only before the caller's navigation actions. A primary pointer
    /// click may surrender the previous owner without focusing its button.
    /// Keyboard activation and intentional background clicks do not bind here.
    pub fn bind_primary_click(ctx: &egui::Context, layer: egui::LayerId, active: bool) {
        if !active {
            return;
        }
        let owners: Vec<_> = ctx.viewport(|v| {
            v.this_pass
                .widgets
                .get_layer(layer)
                .filter(|widget| {
                    widget.enabled && widget.sense.senses_click() && widget.sense.is_focusable()
                })
                .map(|widget| widget.id)
                .collect()
        });
        let clicked = owners.into_iter().find_map(|id| {
            ctx.read_response(id)
                .filter(|response| response.clicked_by(egui::PointerButton::Primary))
        });
        if let Some(response) = clicked {
            response.request_focus();
            ctx.request_repaint();
        }
    }
    /// Explicit host opt-in for a control removed/disabled by its own action.
    /// This uses only the real preceding pass's owner and the current widgets;
    /// an external owner or a new pointer intent is never pulled back here.
    pub fn restore_invalidated(&self, ctx: &egui::Context, active: bool, safe: &egui::Response) {
        if !active
            || self.frame.saturating_add(1) < ctx.cumulative_frame_nr()
            || !safe.enabled()
            || !safe.sense.is_focusable()
            || ctx.input(|input| {
                input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::PointerButton { pressed: true, .. }
                            | egui::Event::Touch { .. }
                            | egui::Event::MouseWheel { .. }
                    )
                })
            })
        {
            return;
        }
        let Some(previous) = self
            .owners
            .iter()
            .copied()
            .find(|id| ctx.memory(|memory| memory.had_focus_last_frame(*id)))
        else {
            return;
        };
        if ctx
            .memory(|memory| memory.focused())
            .is_some_and(|id| id != previous)
        {
            return;
        }
        let still_eligible = ctx.viewport(|v| {
            v.this_pass.widgets.get(previous).is_some_and(|widget| {
                widget.layer_id == safe.layer_id
                    && widget.enabled
                    && widget.sense.senses_click()
                    && widget.sense.is_focusable()
            })
        });
        if !still_eligible {
            safe.request_focus();
            ctx.request_repaint();
        }
    }
    pub fn finish(
        &mut self,
        ctx: &egui::Context,
        layer: egui::LayerId,
        active: bool,
        reader: Option<egui::Id>,
    ) {
        self.owners = ctx.viewport(|v| {
            v.this_pass
                .widgets
                .get_layer(layer)
                .filter(|widget| {
                    widget.enabled && widget.sense.senses_click() && widget.sense.is_focusable()
                })
                .map(|widget| widget.id)
                .collect()
        });
        self.frame = ctx.cumulative_frame_nr();
        if active {
            if let Some(id) = ctx
                .memory(|m| m.focused())
                .filter(|id| self.owners.contains(id))
            {
                let text = egui::TextEdit::load_state(ctx, id).is_some();
                ctx.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        id,
                        egui::EventFilter {
                            tab: true,
                            escape: true,
                            horizontal_arrows: text,
                            vertical_arrows: text || reader == Some(id),
                        },
                    )
                });
            }
        }
    }
}

/// Window-owned title controls (including X) are outside the content scroll clip.
/// Paint their real focus rectangle in the actual window, never a clamped proxy.
pub(in crate::app) fn outline_shell(
    ctx: &egui::Context,
    layer: egui::LayerId,
    window: egui::Rect,
    content: &[egui::Id],
) {
    if !ctx.input(|i| i.focused)
        || egui::Popup::is_any_open(ctx)
        || ctx.memory(|m| m.top_modal_layer().is_some())
    {
        return;
    }
    let Some(id) = ctx
        .memory(|m| m.focused())
        .filter(|id| !content.contains(id))
    else {
        return;
    };
    let current = ctx.viewport(|v| {
        v.this_pass
            .widgets
            .get(id)
            .is_some_and(|w| w.layer_id == layer && w.enabled)
    });
    if !current {
        return;
    }
    let Some(response) = ctx.read_response(id) else {
        return;
    };
    let theme = crate::theme::resolved(ctx);
    ctx.layer_painter(layer)
        .with_clip_rect(window.intersect(ctx.screen_rect()))
        .rect_stroke(
            response.rect.expand(2.0),
            2.0,
            egui::Stroke::new(theme.focus_width, theme.colors.focus),
            egui::StrokeKind::Inside,
        );
}
