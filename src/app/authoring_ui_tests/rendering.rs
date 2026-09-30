use super::{
    click, collect_text, frame, text_position_contains, vec2, Event, PointerButton, WorldeditApp,
};

pub(super) fn enter_text_at_placeholder(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    placeholder: &str,
    text: &str,
) {
    enter_text_at_placeholder_in_window(ctx, app, 13, placeholder, text);
}

pub(super) fn enter_text_at_placeholder_in_window(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    placeholder: &str,
    text: &str,
) {
    for _ in 0..3 {
        let _ = frame(ctx, app, Vec::new(), window);
    }
    let output = frame(ctx, app, Vec::new(), window);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, placeholder))
        .unwrap_or_else(|| panic!("未显示输入提示：{placeholder}"));
    for pressed in [true, false] {
        let _ = frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            window,
        );
    }
    let _ = frame(ctx, app, vec![Event::Text(text.into())], window);
}

pub(super) fn rendered_text_in_window(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    needle: &str,
) -> String {
    let mut rendered = String::new();
    for _ in 0..80 {
        let output = frame(ctx, app, Vec::new(), window);
        rendered.clear();
        for shape in &output.shapes {
            collect_text(&shape.shape, &mut rendered);
        }
        if rendered.contains(needle) {
            break;
        }
    }
    rendered
}

pub(super) fn scroll_rendered_text(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    needle: &str,
) -> String {
    scroll_to_visible(ctx, app, window, needle, -90.0)
}

pub(super) fn scroll_window_to_top(ctx: &egui::Context, app: &mut WorldeditApp, window: u8) {
    for _ in 0..30 {
        let output = frame(ctx, app, Vec::new(), window);
        let Some(point) = detail_anchor_position(&output) else {
            continue;
        };
        let _ = frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, 360.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            window,
        );
    }
}

pub(super) fn scroll_to_visible(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    needle: &str,
    delta_y: f32,
) -> String {
    let mut rendered = String::new();
    for direction in [delta_y, -delta_y] {
        for _ in 0..40 {
            let output = frame(ctx, app, Vec::new(), window);
            rendered.clear();
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut rendered);
            }
            if visible_text_position(&output, needle).is_some() {
                return rendered;
            }
            let Some(point) = detail_anchor_position(&output) else {
                continue;
            };
            let _ = frame(
                ctx,
                app,
                vec![
                    Event::PointerMoved(point),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, direction),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                window,
            );
        }
    }
    rendered
}

pub(super) fn detail_anchor_position(output: &egui::FullOutput) -> Option<egui::Pos2> {
    [
        "页面映射 ·",
        "同名资料提示 ·",
        "来源链接 ·",
        "附件映射 ·",
        "损失预览 ·",
        "阻塞冲突 ·",
        "写入文件预览 ·",
    ]
    .into_iter()
    .find_map(|fragment| visible_text_position(output, fragment))
}

pub(super) fn visible_text_position(
    output: &egui::FullOutput,
    fragment: &str,
) -> Option<egui::Pos2> {
    output.shapes.iter().find_map(|clipped| {
        let point = text_position_contains(&clipped.shape, fragment)?;
        clipped.clip_rect.contains(point).then_some(point)
    })
}

pub(super) fn replace_manuscript_source(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    replacement: &str,
) {
    click(ctx, app, 13, "编辑来源文件");
    for _ in 0..3 {
        let _ = frame(ctx, app, Vec::new(), 13);
    }
    let output = frame(ctx, app, Vec::new(), 13);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, "event arrival as"))
        .expect("源码编辑器应显示原始事件声明");
    for pressed in [true, false] {
        let _ = frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            13,
        );
    }
    let _ = frame(
        ctx,
        app,
        vec![
            Event::Key {
                key: egui::Key::A,
                physical_key: Some(egui::Key::A),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
            Event::Key {
                key: egui::Key::A,
                physical_key: Some(egui::Key::A),
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
            Event::Text(replacement.into()),
        ],
        13,
    );
}

/// 通过真实滚动区域找到可见控件，避免依赖特定字体或视口恰好把整张表单放在首屏。
pub(super) fn scroll_from_visible_anchor_to(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    anchor: &str,
    needle: &str,
) -> String {
    for _ in 0..3 {
        let _ = frame(ctx, app, Vec::new(), window);
    }
    let output = frame(ctx, app, Vec::new(), window);
    let point = visible_text_position(&output, anchor)
        .unwrap_or_else(|| panic!("未显示滚动锚点：{anchor}"));
    let mut rendered = String::new();
    for delta in [-60.0, 60.0] {
        for _ in 0..80 {
            let output = frame(ctx, app, Vec::new(), window);
            rendered.clear();
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut rendered);
            }
            if visible_text_position(&output, needle).is_some() {
                return rendered;
            }
            let _ = frame(
                ctx,
                app,
                vec![
                    Event::PointerMoved(point),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, delta),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                window,
            );
        }
    }
    panic!("滚动后仍不可达：{needle}；当前文字：{rendered}");
}

/// 在指定滚动区域等待惯性稳定后重新查找可见文字，避免路径换行改变滚动锚点。
pub(super) fn scroll_at_to_visible(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    point: egui::Pos2,
    needle: &str,
) -> String {
    let mut rendered = String::new();
    for delta in [-60.0, 60.0] {
        for _ in 0..100 {
            for _ in 0..12 {
                let _ = frame(ctx, app, Vec::new(), window);
            }
            let output = frame(ctx, app, Vec::new(), window);
            rendered.clear();
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut rendered);
            }
            if visible_text_position(&output, needle).is_some() {
                return rendered;
            }
            let _ = frame(
                ctx,
                app,
                vec![
                    Event::PointerMoved(point),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, delta),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                window,
            );
        }
    }
    panic!("滚动稳定后控件仍不可达：{needle}；{rendered}");
}
