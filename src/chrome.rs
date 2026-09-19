//! 一体化窗口控制,保留原生拖动、最大化和八方向缩放。
use crate::theme::{self, BORDER, CARD, ERROR, MUTED, TEXT};
#[cfg(not(target_arch = "wasm32"))]
use egui::CursorIcon;
use egui::{Color32, Context, Pos2, Rect, Sense, Stroke, Vec2, ViewportCommand};

const CONTROL_SIZE: Vec2 = Vec2::splat(32.0);
const CONTROL_GAP: f32 = 2.0;
const TITLE_INSET: i8 = 18;
const CONTROLS_WIDTH: i8 = 100;

/// 控件固定在标题栏右侧预留区;返回关闭意图,由调用方处理未保存保护。
pub fn controls(ui: &mut egui::Ui) -> bool {
    let mut close = false;
    let (anchor, _) = ui.allocate_exact_size(Vec2::new(0.0, CONTROL_SIZE.y), Sense::hover());
    let origin = Pos2::new(
        ui.ctx().screen_rect().right() - f32::from(TITLE_INSET + CONTROLS_WIDTH),
        anchor.top(),
    );
    let (focused, maximized) = ui.input(|i| {
        (
            i.viewport().focused.unwrap_or(true),
            i.viewport().maximized.unwrap_or(false),
        )
    });
    for (index, label) in [
        "最小化",
        if maximized { "还原" } else { "最大化" },
        "关闭窗口",
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect::from_min_size(
            origin + Vec2::new(index as f32 * (CONTROL_SIZE.x + CONTROL_GAP), 0.0),
            CONTROL_SIZE,
        );
        let response = ui.interact(
            rect,
            ui.id().with(("window-control", index)),
            Sense::click(),
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        let center = rect.center();
        let highlighted = response.hovered() || response.has_focus();
        let hover = ui
            .ctx()
            .animate_bool_with_time(response.id, highlighted, 0.12);
        ui.painter().rect_filled(
            rect.shrink(1.0),
            7,
            if index == 2 {
                Color32::from_rgb(76, 42, 48)
            } else {
                CARD
            }
            .linear_multiply(hover),
        );
        let ink = Stroke::new(
            if response.is_pointer_button_down_on() {
                1.5_f32
            } else {
                1.2_f32
            },
            if highlighted {
                if index == 2 {
                    ERROR
                } else {
                    TEXT
                }
            } else if focused {
                MUTED
            } else {
                MUTED.gamma_multiply(0.6)
            },
        );
        match index {
            0 => {
                ui.painter().line_segment(
                    [center - Vec2::new(5.0, 0.0), center + Vec2::new(5.0, 0.0)],
                    ink,
                );
            }
            1 => {
                let mut icon = Rect::from_center_size(center, Vec2::splat(10.0));
                if maximized {
                    icon = icon.shrink(1.0).translate(Vec2::new(-1.0, 1.0));
                    let back = icon.translate(Vec2::new(3.0, -3.0));
                    ui.painter().line(
                        vec![
                            back.left_top() + Vec2::new(0.0, 2.0),
                            back.left_top(),
                            back.right_top(),
                            back.right_bottom(),
                            back.right_bottom() - Vec2::new(2.0, 0.0),
                        ],
                        ink,
                    );
                }
                ui.painter()
                    .rect_stroke(icon, 1, ink, egui::StrokeKind::Inside);
            }
            _ => {
                ui.painter()
                    .line_segment([center - Vec2::splat(4.0), center + Vec2::splat(4.0)], ink);
                ui.painter().line_segment(
                    [center + Vec2::new(-4.0, 4.0), center + Vec2::new(4.0, -4.0)],
                    ink,
                );
            }
        }
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.shrink(1.0),
                7,
                Stroke::new(1.0_f32, BORDER),
                egui::StrokeKind::Inside,
            );
        }
        #[cfg(not(target_arch = "wasm32"))]
        if response.clicked() {
            match index {
                0 => ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true)),
                1 => toggle_maximized(ui.ctx()),
                _ => close = true,
            }
        }
        #[cfg(target_arch = "wasm32")]
        if response.clicked() {
            match index {
                1 => crate::web::toggle_fullscreen(),
                2 => close = true,
                _ => {}
            }
        }
        #[cfg(target_arch = "wasm32")]
        let label = match index {
            0 => "请使用浏览器窗口的最小化按钮",
            1 => "切换全屏",
            _ => "结束编辑（随后可关闭标签页）",
        };
        response.on_hover_text(label);
    }
    close
}

pub fn title_drag(ui: &mut egui::Ui) {
    let rect = ui.max_rect();
    let response = ui.interact(rect, ui.id().with("window-drag"), Sense::click_and_drag());
    if response.double_clicked() {
        toggle_maximized(ui.ctx());
    } else if !cfg!(target_arch = "wasm32")
        && response.drag_started_by(egui::PointerButton::Primary)
    {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

fn toggle_maximized(ctx: &Context) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ctx;
        crate::web::toggle_fullscreen();
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn resize_edges(ctx: &Context) {
    if ctx.input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    }) {
        return;
    }
    use egui::ResizeDirection::*;
    let rect = ctx.screen_rect();
    let min = rect.min;
    let max = rect.max;
    let edge = 5.0;
    let corner = 12.0;
    let regions = [
        (
            Rect::from_min_max(
                Pos2::new(min.x + corner, min.y),
                Pos2::new(max.x - corner, min.y + edge),
            ),
            North,
            CursorIcon::ResizeVertical,
        ),
        (
            Rect::from_min_max(
                Pos2::new(min.x + corner, max.y - edge),
                Pos2::new(max.x - corner, max.y),
            ),
            South,
            CursorIcon::ResizeVertical,
        ),
        (
            Rect::from_min_max(
                Pos2::new(min.x, min.y + corner),
                Pos2::new(min.x + edge, max.y - corner),
            ),
            West,
            CursorIcon::ResizeHorizontal,
        ),
        (
            Rect::from_min_max(
                Pos2::new(max.x - edge, min.y + corner),
                Pos2::new(max.x, max.y - corner),
            ),
            East,
            CursorIcon::ResizeHorizontal,
        ),
        (
            Rect::from_min_size(min, Vec2::splat(corner)),
            NorthWest,
            CursorIcon::ResizeNwSe,
        ),
        (
            Rect::from_min_size(Pos2::new(max.x - corner, min.y), Vec2::splat(corner)),
            NorthEast,
            CursorIcon::ResizeNeSw,
        ),
        (
            Rect::from_min_size(Pos2::new(min.x, max.y - corner), Vec2::splat(corner)),
            SouthWest,
            CursorIcon::ResizeNeSw,
        ),
        (
            Rect::from_min_size(max - Vec2::splat(corner), Vec2::splat(corner)),
            SouthEast,
            CursorIcon::ResizeNwSe,
        ),
    ];
    for (index, (rect, direction, cursor)) in regions.into_iter().enumerate() {
        egui::Area::new(egui::Id::new(("resize-edge", index)))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                let (_, response) = ui.allocate_exact_size(rect.size(), Sense::drag());
                if response.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
                }
                response.on_hover_cursor(cursor);
            });
    }
}

pub fn title_frame(ctx: &Context) -> egui::Frame {
    let radius = if ctx.input(|i| {
        i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
    }) {
        0
    } else {
        14
    };
    theme::panel()
        // 预留右侧控件空间,让原有标题/工具栏调用顺序保持兼容。
        .inner_margin(egui::Margin {
            left: TITLE_INSET,
            right: TITLE_INSET + CONTROLS_WIDTH + 8,
            top: 12,
            bottom: 12,
        })
        .corner_radius(egui::CornerRadius {
            nw: radius,
            ne: radius,
            sw: 0,
            se: 0,
        })
}

pub fn subtitle(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(MUTED).size(12.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_controls_preserve_window_actions_and_delegate_close() {
        for width in [1040.0, 1280.0] {
            for maximized in [false, true] {
                for index in 0..3 {
                    let ctx = Context::default();
                    theme::install(&ctx);
                    // 从窗口右边缘定位,不依赖调用方把 controls 放在工具栏的哪一端。
                    let pointer = Pos2::new(width - 102.0 + index as f32 * 34.0, 28.0);
                    for phase in 0..4 {
                        let mut input = egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(width, 760.0),
                            )),
                            ..Default::default()
                        };
                        input
                            .viewports
                            .get_mut(&egui::ViewportId::ROOT)
                            .unwrap()
                            .maximized = Some(maximized);
                        if phase >= 2 {
                            input.events = vec![
                                egui::Event::PointerMoved(pointer),
                                egui::Event::PointerButton {
                                    pos: pointer,
                                    button: egui::PointerButton::Primary,
                                    pressed: phase == 2,
                                    modifiers: egui::Modifiers::NONE,
                                },
                            ];
                        }
                        let mut close = false;
                        let output = ctx.run(input, |ctx| {
                            egui::TopBottomPanel::top("top")
                                .frame(title_frame(ctx))
                                .show(ctx, |ui| {
                                    title_drag(ui);
                                    ui.horizontal(|ui| {
                                        close = ui.horizontal(controls).inner;
                                        let title = ui.label("worldedit");
                                        assert!(title.rect.right() < width - 126.0);
                                    });
                                });
                            #[cfg(not(target_arch = "wasm32"))]
                            resize_edges(ctx);
                        });
                        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
                        assert!(!commands
                            .iter()
                            .any(|cmd| matches!(cmd, ViewportCommand::Close)));
                        assert_eq!(close, phase == 3 && index == 2);
                        if phase == 3 {
                            match index {
                                0 => assert!(commands.iter().any(|cmd| matches!(cmd, ViewportCommand::Minimized(true)))),
                                1 => assert!(commands.iter().any(|cmd| matches!(cmd, ViewportCommand::Maximized(value) if *value != maximized))),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
}
