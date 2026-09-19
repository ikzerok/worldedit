use egui::{Pos2, Vec2};

const EPSILON: f32 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalizedPoint {
    pub(super) x: f32,
    pub(super) y: f32,
}

impl NormalizedPoint {
    pub(super) const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub(super) fn as_pos2(self) -> Pos2 {
        Pos2::new(self.x, self.y)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MapGeometry {
    Point(NormalizedPoint),
    Polyline(Vec<NormalizedPoint>),
    Polygon(Vec<NormalizedPoint>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryError {
    NonFinite,
    OutOfRange,
    TooFewPoints,
    RepeatedClosingPoint,
    SelfIntersection,
    Degenerate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryHit {
    Vertex(usize),
    Edge(usize),
    Body,
}

pub(super) fn validate_geometry(geometry: &MapGeometry) -> Result<(), GeometryError> {
    match geometry {
        MapGeometry::Point(point) => validate_point(*point),
        MapGeometry::Polyline(points) => {
            validate_points(points, 2)?;
            if points.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(GeometryError::Degenerate);
            }
            Ok(())
        }
        MapGeometry::Polygon(points) => {
            validate_points(points, 3)?;
            if points.first() == points.last() {
                return Err(GeometryError::RepeatedClosingPoint);
            }
            if points.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(GeometryError::Degenerate);
            }
            if has_self_intersection(points) {
                return Err(GeometryError::SelfIntersection);
            }
            if signed_area(points).abs() <= EPSILON {
                return Err(GeometryError::Degenerate);
            }
            Ok(())
        }
    }
}

fn validate_points(points: &[NormalizedPoint], minimum: usize) -> Result<(), GeometryError> {
    if points.len() < minimum {
        return Err(GeometryError::TooFewPoints);
    }
    for point in points {
        validate_point(*point)?;
    }
    Ok(())
}

fn validate_point(point: NormalizedPoint) -> Result<(), GeometryError> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err(GeometryError::NonFinite);
    }
    if !(0.0..=1.0).contains(&point.x) || !(0.0..=1.0).contains(&point.y) {
        return Err(GeometryError::OutOfRange);
    }
    Ok(())
}

fn signed_area(points: &[NormalizedPoint]) -> f32 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum::<f32>()
        * 0.5
}

fn orientation(a: NormalizedPoint, b: NormalizedPoint, c: NormalizedPoint) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn on_segment(a: NormalizedPoint, b: NormalizedPoint, point: NormalizedPoint) -> bool {
    point.x >= a.x.min(b.x) - EPSILON
        && point.x <= a.x.max(b.x) + EPSILON
        && point.y >= a.y.min(b.y) - EPSILON
        && point.y <= a.y.max(b.y) + EPSILON
}

fn segments_intersect(
    a: NormalizedPoint,
    b: NormalizedPoint,
    c: NormalizedPoint,
    d: NormalizedPoint,
) -> bool {
    let ab_c = orientation(a, b, c);
    let ab_d = orientation(a, b, d);
    let cd_a = orientation(c, d, a);
    let cd_b = orientation(c, d, b);

    if ab_c.abs() <= EPSILON && on_segment(a, b, c) {
        return true;
    }
    if ab_d.abs() <= EPSILON && on_segment(a, b, d) {
        return true;
    }
    if cd_a.abs() <= EPSILON && on_segment(c, d, a) {
        return true;
    }
    if cd_b.abs() <= EPSILON && on_segment(c, d, b) {
        return true;
    }
    (ab_c > 0.0) != (ab_d > 0.0) && (cd_a > 0.0) != (cd_b > 0.0)
}

fn has_self_intersection(points: &[NormalizedPoint]) -> bool {
    let count = points.len();
    for first in 0..count {
        let first_end = (first + 1) % count;
        for second in (first + 1)..count {
            let second_end = (second + 1) % count;
            let adjacent = first == second
                || first_end == second
                || second_end == first
                || (first == 0 && second_end == 0);
            if adjacent {
                continue;
            }
            if segments_intersect(
                points[first],
                points[first_end],
                points[second],
                points[second_end],
            ) {
                return true;
            }
        }
    }
    false
}

pub(super) fn triangulate_polygon(points: &[NormalizedPoint]) -> Option<Vec<[usize; 3]>> {
    if points.len() < 3 || has_self_intersection(points) || signed_area(points).abs() <= EPSILON {
        return None;
    }
    let orientation_sign = signed_area(points).signum();
    let mut remaining: Vec<usize> = (0..points.len()).collect();
    let mut triangles = Vec::with_capacity(points.len() - 2);
    let mut guard = 0;
    while remaining.len() > 3 && guard < points.len() * points.len() {
        let mut ear_found = false;
        for cursor in 0..remaining.len() {
            let previous = remaining[(cursor + remaining.len() - 1) % remaining.len()];
            let current = remaining[cursor];
            let next = remaining[(cursor + 1) % remaining.len()];
            let turn = orientation(points[previous], points[current], points[next]);
            if turn * orientation_sign <= EPSILON {
                continue;
            }
            if remaining.iter().copied().any(|candidate| {
                candidate != previous
                    && candidate != current
                    && candidate != next
                    && point_in_triangle(
                        points[candidate],
                        points[previous],
                        points[current],
                        points[next],
                    )
            }) {
                continue;
            }
            triangles.push([previous, current, next]);
            remaining.remove(cursor);
            ear_found = true;
            break;
        }
        if !ear_found {
            return None;
        }
        guard += 1;
    }
    if remaining.len() == 3 {
        triangles.push([remaining[0], remaining[1], remaining[2]]);
        Some(triangles)
    } else {
        None
    }
}

fn point_in_triangle(
    point: NormalizedPoint,
    a: NormalizedPoint,
    b: NormalizedPoint,
    c: NormalizedPoint,
) -> bool {
    let first = orientation(a, b, point);
    let second = orientation(b, c, point);
    let third = orientation(c, a, point);
    (first >= -EPSILON && second >= -EPSILON && third >= -EPSILON)
        || (first <= EPSILON && second <= EPSILON && third <= EPSILON)
}

pub(super) fn hit_test(
    geometry: &MapGeometry,
    point: NormalizedPoint,
    screen_scale: Vec2,
    tolerance_pixels: f32,
) -> Option<GeometryHit> {
    let tolerance = tolerance_pixels.max(0.0);
    let point_screen = to_screen(point, screen_scale);
    match geometry {
        MapGeometry::Point(vertex) => {
            (distance_squared_screen(to_screen(*vertex, screen_scale), point_screen)
                <= tolerance * tolerance)
                .then_some(GeometryHit::Vertex(0))
        }
        MapGeometry::Polyline(points) => hit_path(points, point, screen_scale, tolerance, false),
        MapGeometry::Polygon(points) => hit_path(points, point, screen_scale, tolerance, true)
            .or_else(|| point_in_polygon(point, points).then_some(GeometryHit::Body)),
    }
}

fn hit_path(
    points: &[NormalizedPoint],
    point: NormalizedPoint,
    screen_scale: Vec2,
    tolerance: f32,
    closed: bool,
) -> Option<GeometryHit> {
    let point_screen = to_screen(point, screen_scale);
    for (index, vertex) in points.iter().copied().enumerate() {
        if distance_squared_screen(to_screen(vertex, screen_scale), point_screen)
            <= tolerance * tolerance
        {
            return Some(GeometryHit::Vertex(index));
        }
    }
    let edge_count = if closed {
        points.len()
    } else {
        points.len().saturating_sub(1)
    };
    for index in 0..edge_count {
        let next = (index + 1) % points.len();
        if distance_to_segment_squared_screen(
            point_screen,
            to_screen(points[index], screen_scale),
            to_screen(points[next], screen_scale),
        ) <= tolerance * tolerance
        {
            return Some(GeometryHit::Edge(index));
        }
    }
    None
}

fn point_in_polygon(point: NormalizedPoint, points: &[NormalizedPoint]) -> bool {
    let mut inside = false;
    for (a, b) in points
        .iter()
        .copied()
        .zip(points.iter().copied().cycle().skip(1))
        .take(points.len())
    {
        let crosses = (a.y > point.y) != (b.y > point.y);
        if crosses {
            let x = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if point.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

fn to_screen(point: NormalizedPoint, scale: Vec2) -> Pos2 {
    Pos2::new(point.x * scale.x, point.y * scale.y)
}

fn distance_squared_screen(a: Pos2, b: Pos2) -> f32 {
    a.distance_sq(b)
}

fn distance_to_segment_squared_screen(point: Pos2, start: Pos2, end: Pos2) -> f32 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx.mul_add(dx, dy * dy);
    if length_squared <= EPSILON {
        return point.distance_sq(start);
    }
    let t =
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    let closest = Pos2::new(start.x + t * dx, start.y + t * dy);
    point.distance_sq(closest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concave_polygon_triangulates_into_full_fill() {
        let polygon = [
            NormalizedPoint::new(0.05, 0.05),
            NormalizedPoint::new(0.95, 0.05),
            NormalizedPoint::new(0.95, 0.4),
            NormalizedPoint::new(0.55, 0.4),
            NormalizedPoint::new(0.55, 0.95),
            NormalizedPoint::new(0.05, 0.95),
        ];
        let triangles = triangulate_polygon(&polygon).expect("simple concave polygon is drawable");
        assert_eq!(triangles.len(), polygon.len() - 2);
        assert!(triangles
            .iter()
            .all(|triangle| { triangle.iter().all(|index| *index < polygon.len()) }));
        let polygon_area = signed_area(&polygon).abs();
        let triangle_area = triangles
            .iter()
            .map(|[a, b, c]| orientation(polygon[*a], polygon[*b], polygon[*c]).abs() * 0.5)
            .sum::<f32>();
        assert!((triangle_area - polygon_area).abs() < 1e-5);
        assert!(!triangles.iter().any(|[a, b, c]| {
            point_in_triangle(
                NormalizedPoint::new(0.8, 0.8),
                polygon[*a],
                polygon[*b],
                polygon[*c],
            )
        }));
    }

    #[test]
    fn polygon_validation_rejects_crossing_edges() {
        let polygon = MapGeometry::Polygon(vec![
            NormalizedPoint::new(0.1, 0.1),
            NormalizedPoint::new(0.9, 0.9),
            NormalizedPoint::new(0.1, 0.9),
            NormalizedPoint::new(0.9, 0.1),
        ]);
        assert_eq!(
            validate_geometry(&polygon),
            Err(GeometryError::SelfIntersection)
        );
    }

    #[test]
    fn geometry_validation_rejects_invalid_coordinate_and_shape_cases() {
        assert_eq!(
            validate_geometry(&MapGeometry::Point(NormalizedPoint::new(f32::NAN, 0.5))),
            Err(GeometryError::NonFinite)
        );
        assert_eq!(
            validate_geometry(&MapGeometry::Polyline(vec![
                NormalizedPoint::new(0.1, 0.1),
                NormalizedPoint::new(1.1, 0.1),
            ])),
            Err(GeometryError::OutOfRange)
        );
        assert_eq!(
            validate_geometry(&MapGeometry::Polygon(vec![
                NormalizedPoint::new(0.1, 0.1),
                NormalizedPoint::new(0.2, 0.2),
            ])),
            Err(GeometryError::TooFewPoints)
        );
        assert_eq!(
            validate_geometry(&MapGeometry::Polygon(vec![
                NormalizedPoint::new(0.1, 0.1),
                NormalizedPoint::new(0.9, 0.1),
                NormalizedPoint::new(0.9, 0.9),
                NormalizedPoint::new(0.1, 0.1),
            ])),
            Err(GeometryError::RepeatedClosingPoint)
        );
    }

    #[test]
    fn polygon_body_hit_works_for_concave_shape() {
        let polygon = MapGeometry::Polygon(vec![
            NormalizedPoint::new(0.05, 0.05),
            NormalizedPoint::new(0.95, 0.05),
            NormalizedPoint::new(0.95, 0.4),
            NormalizedPoint::new(0.55, 0.4),
            NormalizedPoint::new(0.55, 0.95),
            NormalizedPoint::new(0.05, 0.95),
        ]);
        assert_eq!(
            hit_test(
                &polygon,
                NormalizedPoint::new(0.2, 0.8),
                Vec2::splat(1.0),
                0.01
            ),
            Some(GeometryHit::Body)
        );
        assert_eq!(
            hit_test(
                &polygon,
                NormalizedPoint::new(0.8, 0.8),
                Vec2::splat(1.0),
                0.01
            ),
            None
        );
    }
}
