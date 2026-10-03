//! 端口保留既有拖拽建边，仅提交旧式人物关系，绝不改写只读上下文边。
use super::WorldeditApp;
use crate::theme;
use egui::{Rect, Sense, Stroke, Vec2};
use worldline_core::TargetRef;
impl WorldeditApp {
    pub(super) fn character_legacy_ports(
        &mut self,
        ui: &mut egui::Ui,
        canvas: Rect,
        nodes: &[(TargetRef, Rect)],
    ) -> Option<TargetRef> {
        let painter = ui.painter_at(canvas);
        for (target, rect) in nodes {
            if !canvas.intersects(*rect) {
                continue;
            }
            let port = Rect::from_center_size(rect.right_center(), Vec2::splat(20.0));
            painter.circle_filled(port.center(), 4.5, theme::ACCENT());
            let response = ui
                .interact(
                    port.intersect(canvas),
                    ui.id().with(("legacy-person-port", &target.id)),
                    Sense::click_and_drag(),
                )
                .on_hover_text("拖到另一人物：创建旧式人物关系（不改写属性、链接或参与）");
            if (response.drag_started() || response.clicked())
                && !self.prevent_replacing_draft("人物资料")
            {
                self.character_link = Some(target.id.clone());
            }
        }
        if let Some(from) = &self.character_link {
            if let (Some((_, rect)), Some(pointer)) = (
                nodes.iter().find(|(target, _)| &target.id == from),
                ui.input(|i| i.pointer.hover_pos()),
            ) {
                painter.line_segment(
                    [rect.right_center(), pointer],
                    Stroke::new(1.5_f32, theme::ACCENT()),
                );
                if ui.input(|i| i.pointer.any_released()) {
                    if let Some((target, _)) = nodes
                        .iter()
                        .find(|(target, rect)| &target.id != from && rect.contains(pointer))
                    {
                        return Some(target.clone());
                    }
                }
            }
        }
        None
    }
}
