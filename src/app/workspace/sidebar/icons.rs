//! 自绘单色图标只提供方向线索；完整中文标签保持可见。
use crate::app::Tab;
use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Stroke, StrokeKind};

pub(super) fn paint(p: &Painter, center: Pos2, tab: Tab, color: Color32) {
    let stroke = Stroke::new(1.4_f32, color);
    let point = |x, y| center + vec2(x, y);
    let line = |a: [f32; 2], b: [f32; 2]| {
        p.line_segment([point(a[0], a[1]), point(b[0], b[1])], stroke);
    };
    let box_at = |x: f32, y: f32, w: f32, h: f32| {
        p.rect_stroke(
            Rect::from_center_size(point(x, y), vec2(w, h)),
            0,
            stroke,
            StrokeKind::Inside,
        );
    };
    match tab {
        Tab::Manuscript => {
            p.line(
                vec![
                    point(-8., -6.),
                    point(-2., -7.),
                    point(0., -5.),
                    point(2., -7.),
                    point(8., -6.),
                    point(8., 7.),
                    point(2., 6.),
                    point(0., 8.),
                    point(-2., 6.),
                    point(-8., 7.),
                    point(-8., -6.),
                ],
                stroke,
            );
            line([0., -5.], [0., 8.]);
        }
        Tab::Overview | Tab::Templates | Tab::Wiki => {
            box_at(0., 0., 13., 16.);
            for y in [-3., 1., 5.] {
                line([-3., y], [3., y]);
            }
            if tab == Tab::Overview {
                line([-9., -5.], [-9., 10.]);
            }
        }
        Tab::Edit => {
            line([-3., -5.], [-8., 0.]);
            line([-8., 0.], [-3., 5.]);
            line([3., -5.], [8., 0.]);
            line([8., 0.], [3., 5.]);
            line([1., -8.], [-1., 8.]);
        }
        Tab::Characters => {
            p.circle_stroke(point(0., -4.), 3.5, stroke);
            p.line(
                vec![
                    point(-7., 8.),
                    point(-6., 3.),
                    point(0., 1.),
                    point(6., 3.),
                    point(7., 8.),
                ],
                stroke,
            );
        }
        Tab::World | Tab::Localization => {
            p.circle_stroke(center, 8., stroke);
            line([-8., 0.], [8., 0.]);
            p.line(
                vec![
                    point(0., -8.),
                    point(-3., -3.),
                    point(-3., 3.),
                    point(0., 8.),
                ],
                stroke,
            );
            p.line(
                vec![point(0., -8.), point(3., -3.), point(3., 3.), point(0., 8.)],
                stroke,
            );
        }
        Tab::Catalog => {
            for x in [-5., 0., 5.] {
                box_at(x, 0., 4., 15.);
            }
            line([-7., 4.], [7., 4.]);
        }
        Tab::CatalogImport => {
            line([0., -8.], [0., 3.]);
            line([-4., -1.], [0., 3.]);
            line([0., 3.], [4., -1.]);
            p.line(
                vec![point(-8., 2.), point(-8., 8.), point(8., 8.), point(8., 2.)],
                stroke,
            );
        }
        Tab::Map => {
            p.line(
                vec![
                    point(-8., -5.),
                    point(-3., -8.),
                    point(3., -5.),
                    point(8., -8.),
                    point(8., 5.),
                    point(3., 8.),
                    point(-3., 5.),
                    point(-8., 8.),
                    point(-8., -5.),
                ],
                stroke,
            );
            line([-3., -8.], [-3., 5.]);
            line([3., -5.], [3., 8.]);
        }
        Tab::Network | Tab::Graph => {
            line([-6., -5.], [6., 0.]);
            line([-6., 6.], [6., 0.]);
            for offset in [pos2(-6., -5.), pos2(-6., 6.), pos2(6., 0.)] {
                p.circle_filled(center + offset.to_vec2(), 2.6, color);
            }
        }
        Tab::Timeline => {
            line([-8., 0.], [8., 0.]);
            for (x, y) in [(-6., -5.), (0., 5.), (6., -5.)] {
                line([x, 0.], [x, y]);
                p.circle_filled(point(x, y), 2., color);
            }
        }
        Tab::Review => {
            p.line(
                vec![
                    point(-8., -6.),
                    point(8., -6.),
                    point(8., 4.),
                    point(0., 4.),
                    point(-5., 8.),
                    point(-5., 4.),
                    point(-8., 4.),
                    point(-8., -6.),
                ],
                stroke,
            );
            line([-4., -2.], [4., -2.]);
        }
        Tab::CheckpointHistory => {
            p.circle_stroke(center, 8., stroke);
            line([0., -5.], [0., 0.]);
            line([0., 0.], [4., 2.]);
        }
        Tab::Play => {
            p.add(egui::Shape::closed_line(
                vec![point(-5., -8.), point(8., 0.), point(-5., 8.)],
                stroke,
            ));
        }
    }
}
