use super::*;

// 同值可能同时绘制在下层旧表单与新工作表，必须在当前真实控件内匹配。
pub(super) fn response_text(output: &egui::FullOutput, label: &str, response: Rect) -> bool {
    fn contains(shape: &egui::Shape, label: &str, visible: Rect) -> bool {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                visible.contains_rect(text.galley.rect.translate(text.pos.to_vec2()))
            }
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| contains(shape, label, visible)),
            _ => false,
        }
    }
    output
        .shapes
        .iter()
        .any(|shape| contains(&shape.shape, label, response.intersect(shape.clip_rect)))
}
