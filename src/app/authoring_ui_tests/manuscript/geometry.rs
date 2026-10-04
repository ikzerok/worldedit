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
