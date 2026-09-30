use super::*;

pub(super) fn key(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    key: egui::Key,
    shift: bool,
) -> Vec<egui::WidgetInfo> {
    let mut focused = Vec::new();
    for pressed in [true, false] {
        let output = frame(
            ctx,
            app,
            vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers {
                    shift,
                    ..Default::default()
                },
            }],
            window,
        );
        for event in output.platform_output.events {
            if let egui::output::OutputEvent::FocusGained(info) = event {
                focused.push(info);
            }
        }
    }
    focused
}

// 只承认真实 FocusGained 控件，不能把包含按钮文字的可聚焦父容器误判为目标。
// 文字可见且落在该控件内才成功；不请求焦点，也不注入滚轮。
pub(super) fn tab_to(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    label: &str,
    reverse: bool,
) {
    let mut trace = Vec::new();
    for _ in 0..160 {
        let focused = key(ctx, app, window, egui::Key::Tab, reverse);
        let matches = focused.iter().any(|info| {
            info.label.as_deref() == Some(label) || info.typ == egui::WidgetType::TextEdit
        });
        if matches {
            for _ in 0..12 {
                let _ = frame(ctx, app, Vec::new(), window);
            }
        }
        let output = frame(ctx, app, Vec::new(), window);
        if let Some(response) = ctx
            .memory(|memory| memory.focused())
            .and_then(|id| ctx.read_response(id))
        {
            if trace.len() < 40 {
                trace.extend(
                    focused
                        .iter()
                        .map(|info| format!("{:?}:{:?}", info.typ, info.label)),
                );
            }
            if matches
                && output.shapes.iter().any(|shape| {
                    text_position(&shape.shape, label).is_some_and(|point| {
                        shape.clip_rect.contains(point) && response.rect.contains(point)
                    })
                })
            {
                return;
            }
        }
    }
    let output = frame(ctx, app, Vec::new(), window);
    let mut text = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut text);
    }
    panic!("键盘焦点不可达：{label}；{trace:?}；{text}");
}
