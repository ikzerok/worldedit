//! 临时范围标注仅绘制轮廓；不进入选择、图层显隐或任何展示命令。
use super::*;
use worldline_core::catalog_scope::ScopePlacementKind;

impl crate::app::WorldeditApp {
    pub(in crate::app) fn draw_query_scope_overlay(&mut self, ui: &egui::Ui) {
        #[cfg(test)]
        ui.ctx().data_mut(|data| {
            data.insert_temp(
                egui::Id::new("catalog-scope-map-clip"),
                self.map_canvas.viewport.intersect(ui.clip_rect()),
            )
        });
        let (placements, total) = self.query_scope_map_overlays();
        if total == 0 {
            return;
        }
        let canvas = &self.map_canvas;
        let painter = ui.painter().with_clip_rect(canvas.viewport);
        for placement in placements.iter() {
            if !canvas
                .snapshot
                .layers
                .iter()
                .any(|layer| layer.id == placement.layer_id && layer.visible)
            {
                continue;
            }
            if !placement.node_visible && !canvas.scene.revealed.contains(&placement.placement_id) {
                continue;
            }
            let bounds = match placement.kind {
                ScopePlacementKind::SceneNode => canvas.node_screen_bounds(&placement.placement_id),
                ScopePlacementKind::Placement => placement.bounds.map(|[x0, y0, x1, y1]| {
                    Rect::from_two_pos(
                        canvas
                            .camera
                            .normalized_to_screen(Pos2::new(x0 as f32, y0 as f32), canvas.viewport),
                        canvas
                            .camera
                            .normalized_to_screen(Pos2::new(x1 as f32, y1 as f32), canvas.viewport),
                    )
                }),
            };
            let Some(bounds) = bounds.filter(|bounds| {
                bounds.is_finite() && canvas.viewport.intersects(bounds.expand(6.0))
            }) else {
                continue;
            };
            painter.rect_stroke(
                bounds.expand(5.0),
                3.0,
                egui::Stroke::new(1.5_f32, crate::theme::ACCENT()),
                egui::StrokeKind::Outside,
            );
        }
        painter.text(
            canvas.viewport.left_bottom() + Vec2::new(12.0, -12.0),
            egui::Align2::LEFT_BOTTOM,
            format!(
                "查询匹配 · 此图 {total} 处{}",
                if total > 500 {
                    " · 轮廓预算前 500 处，全部位置见范围列表"
                } else {
                    " · 仅标注当前可见位置"
                }
            ),
            egui::FontId::proportional(12.0),
            crate::theme::TEXT(),
        );
    }
}
