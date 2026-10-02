use worldline_core::vector_scene::{PathSegment, SceneGeometry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HandleKind {
    Point(usize),
    RectMin,
    RectMax,
    Center,
    RadiusX,
    RadiusY,
    Text,
    PathEnd(usize),
    Control1(usize),
    Control2(usize),
    Quadratic(usize),
}

#[derive(Clone, Debug)]
pub(super) struct SceneHandle {
    pub(super) kind: HandleKind,
    pub(super) point: [f64; 2],
    pub(super) control: bool,
}

pub(super) fn handles(geometry: &SceneGeometry) -> Vec<SceneHandle> {
    let mut result = Vec::new();
    let mut push = |kind, point, control| {
        result.push(SceneHandle {
            kind,
            point,
            control,
        })
    };
    match geometry {
        SceneGeometry::Point { position } => push(HandleKind::Point(0), *position, false),
        SceneGeometry::Polyline { points } | SceneGeometry::Polygon { points } => {
            for (index, point) in points.iter().enumerate() {
                push(HandleKind::Point(index), *point, false);
            }
        }
        SceneGeometry::Rect {
            x,
            y,
            width,
            height,
            ..
        } => {
            push(HandleKind::RectMin, [*x, *y], false);
            push(HandleKind::RectMax, [x + width, y + height], false);
        }
        SceneGeometry::Ellipse { cx, cy, rx, ry } => {
            push(HandleKind::Center, [*cx, *cy], false);
            push(HandleKind::RadiusX, [cx + rx, *cy], true);
            push(HandleKind::RadiusY, [*cx, cy + ry], true);
        }
        SceneGeometry::Text { x, y, .. } => push(HandleKind::Text, [*x, *y], false),
        SceneGeometry::Path { segments } => {
            for (index, segment) in segments.iter().enumerate() {
                match segment {
                    PathSegment::Move { to }
                    | PathSegment::Line { to }
                    | PathSegment::Arc { to, .. } => push(HandleKind::PathEnd(index), *to, false),
                    PathSegment::Cubic {
                        control1,
                        control2,
                        to,
                    } => {
                        push(HandleKind::Control1(index), *control1, true);
                        push(HandleKind::Control2(index), *control2, true);
                        push(HandleKind::PathEnd(index), *to, false);
                    }
                    PathSegment::Quadratic { control, to } => {
                        push(HandleKind::Quadratic(index), *control, true);
                        push(HandleKind::PathEnd(index), *to, false);
                    }
                    PathSegment::Close => {}
                }
            }
        }
        SceneGeometry::Group { .. } => {}
    }
    result
}

pub(super) fn move_handle(geometry: &mut SceneGeometry, kind: HandleKind, to: [f64; 2]) -> bool {
    match (geometry, kind) {
        (SceneGeometry::Point { position }, HandleKind::Point(0)) => *position = to,
        (
            SceneGeometry::Polyline { points } | SceneGeometry::Polygon { points },
            HandleKind::Point(index),
        ) => {
            let Some(point) = points.get_mut(index) else {
                return false;
            };
            *point = to;
        }
        (
            SceneGeometry::Rect {
                x,
                y,
                width,
                height,
                ..
            },
            HandleKind::RectMin,
        ) => {
            let end = [*x + *width, *y + *height];
            *x = to[0].min(end[0]);
            *y = to[1].min(end[1]);
            *width = (to[0] - end[0]).abs();
            *height = (to[1] - end[1]).abs();
        }
        (
            SceneGeometry::Rect {
                x,
                y,
                width,
                height,
                ..
            },
            HandleKind::RectMax,
        ) => {
            *width = (to[0] - *x).abs();
            *height = (to[1] - *y).abs();
            *x = x.min(to[0]);
            *y = y.min(to[1]);
        }
        (SceneGeometry::Ellipse { cx, cy, .. }, HandleKind::Center) => {
            *cx = to[0];
            *cy = to[1];
        }
        (SceneGeometry::Ellipse { cx, rx, .. }, HandleKind::RadiusX) => *rx = (to[0] - *cx).abs(),
        (SceneGeometry::Ellipse { cy, ry, .. }, HandleKind::RadiusY) => *ry = (to[1] - *cy).abs(),
        (SceneGeometry::Text { x, y, .. }, HandleKind::Text) => {
            *x = to[0];
            *y = to[1];
        }
        (SceneGeometry::Path { segments }, kind) => {
            let index = match kind {
                HandleKind::PathEnd(i)
                | HandleKind::Control1(i)
                | HandleKind::Control2(i)
                | HandleKind::Quadratic(i) => i,
                _ => return false,
            };
            let Some(segment) = segments.get_mut(index) else {
                return false;
            };
            let point = match (segment, kind) {
                (
                    PathSegment::Move { to }
                    | PathSegment::Line { to }
                    | PathSegment::Arc { to, .. }
                    | PathSegment::Cubic { to, .. }
                    | PathSegment::Quadratic { to, .. },
                    HandleKind::PathEnd(_),
                ) => to,
                (PathSegment::Cubic { control1, .. }, HandleKind::Control1(_)) => control1,
                (PathSegment::Cubic { control2, .. }, HandleKind::Control2(_)) => control2,
                (PathSegment::Quadratic { control, .. }, HandleKind::Quadratic(_)) => control,
                _ => return false,
            };
            *point = to;
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editing_a_control_handle_keeps_native_segments_and_close() {
        let mut path = SceneGeometry::Path {
            segments: vec![
                PathSegment::Move { to: [0.0, 0.0] },
                PathSegment::Cubic {
                    control1: [1.0, 2.0],
                    control2: [3.0, 4.0],
                    to: [5.0, 6.0],
                },
                PathSegment::Close,
            ],
        };
        assert_eq!(handles(&path).len(), 4);
        assert!(move_handle(&mut path, HandleKind::Control1(1), [9.0, 8.0]));
        let SceneGeometry::Path { segments } = path else {
            panic!("native path lost");
        };
        assert!(matches!(
            segments[1],
            PathSegment::Cubic {
                control1: [9.0, 8.0],
                ..
            }
        ));
        assert!(matches!(segments[2], PathSegment::Close));
    }
}
