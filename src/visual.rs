//! 关系图与时间线共用的几何和显示辅助函数。

use egui::{Color32, Pos2, Stroke, Vec2};

/// 三次贝塞尔采样(漂流弧线用)。
pub(super) fn bezier_points(p0: Pos2, c0: Pos2, c1: Pos2, p1: Pos2, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32;
            let u = 1.0 - t;
            Pos2::new(
                u * u * u * p0.x
                    + 3.0 * u * u * t * c0.x
                    + 3.0 * u * t * t * c1.x
                    + t * t * t * p1.x,
                u * u * u * p0.y
                    + 3.0 * u * u * t * c0.y
                    + 3.0 * u * t * t * c1.y
                    + t * t * t * p1.y,
            )
        })
        .collect()
}

/// 从 `from` 指向 `to` 方向的小箭头。
pub(super) fn draw_arrow(painter: &egui::Painter, from: Pos2, to: Pos2, color: Color32) {
    let dir = (to - from).normalized();
    let tip = to - dir * 9.0;
    let side = Vec2::new(-dir.y, dir.x) * 4.0;
    painter.add(egui::Shape::convex_polygon(
        vec![to, tip + side, tip - side],
        color,
        Stroke::NONE,
    ));
}

/// 按字符数截断。
pub(super) fn truncated(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    } else {
        s.to_string()
    }
}

pub(super) fn lighten(c: Color32) -> Color32 {
    Color32::from_rgb(
        c.r().saturating_add(25),
        c.g().saturating_add(25),
        c.b().saturating_add(25),
    )
}

#[cfg(test)]
mod tests {
    use super::{lighten, truncated};
    use egui::Color32;

    #[test]
    fn truncation_preserves_short_text_and_marks_long_text() {
        assert_eq!(truncated("短文本", 4), "短文本");
        assert_eq!(truncated("一二三四五", 4), "一二三…");
    }

    #[test]
    fn geometry_and_color_helpers_are_bounded() {
        assert_eq!(
            lighten(Color32::from_rgb(250, 10, 20)),
            Color32::from_rgb(255, 35, 45)
        );
    }
}
