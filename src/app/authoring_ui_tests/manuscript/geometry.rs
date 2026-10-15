//! 先确认折叠动画结束且目标真实可见，再执行已有业务操作。
use super::*;

fn label_rect(shape: &egui::Shape, label: &str) -> Option<egui::Rect> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            Some(text.galley.rect.translate(text.pos.to_vec2()))
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| label_rect(shape, label)),
        _ => None,
    }
}

/// 从实际绘制的元数据标签取滚动区，避免工具条收拢后沿用旧屏幕坐标。
pub(super) fn label_clip(output: &egui::FullOutput, label: &str) -> Option<egui::Rect> {
    output.shapes.iter().find_map(|shape| {
        label_rect(&shape.shape, label)
            .filter(|rect| shape.clip_rect.contains_rect(*rect))
            .map(|_| shape.clip_rect)
    })
}

pub(super) fn settle_scroll_clip(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    label: &str,
) -> egui::Rect {
    for _ in 0..16 {
        let output = frame(ctx, app, Vec::new(), window);
        if let Some(clip) = label_clip(&output, label) {
            assert!(ctx.screen_rect().contains_rect(clip));
            return clip;
        }
    }
    panic!("元数据滚动区必须真实可见：{label}");
}

pub(super) fn label_fully_visible(output: &egui::FullOutput, label: &str) -> bool {
    output.shapes.iter().any(|shape| {
        label_rect(&shape.shape, label).is_some_and(|rect| shape.clip_rect.contains_rect(rect))
    })
}

pub(super) fn first_line_fully_visible(output: &egui::FullOutput, fragment: &str) -> bool {
    fn first_row(shape: &egui::Shape, fragment: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.text().starts_with(fragment) => text
                .galley
                .rows
                .first()
                .map(|row| row.rect().translate(text.pos.to_vec2())),
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| first_row(shape, fragment)),
            _ => None,
        }
    }
    output.shapes.iter().any(|shape| {
        first_row(&shape.shape, fragment).is_some_and(|rect| shape.clip_rect.contains_rect(rect))
    })
}

pub(super) fn text_geometry(output: &egui::FullOutput) -> String {
    fn collect(shape: &egui::Shape, clip: egui::Rect, trace: &mut String) {
        match shape {
            egui::Shape::Text(text) => trace.push_str(&format!(
                "{:?}: rect={:?}, first_row={:?}, clip={clip:?}\n",
                text.galley.text(),
                text.galley.rect.translate(text.pos.to_vec2()),
                text.galley
                    .rows
                    .first()
                    .map(|row| row.rect().translate(text.pos.to_vec2())),
            )),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, clip, trace)),
            _ => {}
        }
    }
    let mut trace = String::new();
    for shape in &output.shapes {
        collect(&shape.shape, shape.clip_rect, &mut trace);
    }
    trace
}

pub(super) fn settle_label(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let mut previous = None;
    let mut stable = 0;
    for _ in 0..32 {
        let output = frame(ctx, app, Vec::new(), 13);
        let geometry = output.shapes.iter().find_map(|clipped| {
            label_rect(&clipped.shape, label).map(|rect| (rect, clipped.clip_rect))
        });
        if geometry.is_some_and(|(rect, clip)| clip.contains_rect(rect)) && geometry == previous {
            stable += 1;
            if stable >= 2 {
                return;
            }
        } else {
            stable = 0;
        }
        previous = geometry;
    }
    panic!("控件未达到完整可见且稳定状态：{label}；最后几何：{previous:?}");
}
