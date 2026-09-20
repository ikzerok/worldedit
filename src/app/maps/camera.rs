use egui::{pos2, Pos2, Rect, Vec2};

const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera2D {
    map_extent: Vec2,
    zoom: f32,
    pan: Vec2,
}

impl Camera2D {
    pub(super) fn new(map_extent: Vec2) -> Self {
        let map_extent = Vec2::new(
            if map_extent.x.is_finite() && map_extent.x > 0.0 {
                map_extent.x
            } else {
                1.0
            },
            if map_extent.y.is_finite() && map_extent.y > 0.0 {
                map_extent.y
            } else {
                1.0
            },
        );
        Self {
            map_extent,
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }

    pub(super) fn zoom(&self) -> f32 {
        self.zoom
    }

    pub(super) fn map_scale(&self) -> Vec2 {
        self.map_extent * self.zoom
    }

    pub(super) fn state(&self) -> (f32, [f32; 2]) {
        (self.zoom, [self.pan.x, self.pan.y])
    }

    pub(super) fn restore(&mut self, zoom: f32, pan: [f32; 2]) {
        if zoom.is_finite() {
            self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        }
        if pan.iter().all(|value| value.is_finite()) {
            self.pan = Vec2::new(pan[0], pan[1]);
        }
    }

    #[cfg(test)]
    pub fn pan(&self) -> Vec2 {
        self.pan
    }

    pub(super) fn pan_by(&mut self, delta: Vec2) {
        self.pan += delta;
    }

    pub(super) fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }

    pub(super) fn fit(&mut self, viewport: Rect) {
        self.zoom = (viewport.width() / self.map_extent.x)
            .min(viewport.height() / self.map_extent.y)
            .clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = Vec2::new(
            (viewport.width() - self.map_extent.x * self.zoom) * 0.5,
            (viewport.height() - self.map_extent.y * self.zoom) * 0.5,
        );
    }

    pub(super) fn normalized_to_screen(&self, point: Pos2, viewport: Rect) -> Pos2 {
        viewport.min
            + self.pan
            + Vec2::new(point.x * self.map_extent.x, point.y * self.map_extent.y) * self.zoom
    }

    pub(super) fn screen_to_normalized(&self, point: Pos2, viewport: Rect) -> Pos2 {
        let map_point = (point - viewport.min - self.pan) / self.zoom;
        pos2(
            map_point.x / self.map_extent.x,
            map_point.y / self.map_extent.y,
        )
    }

    pub(super) fn zoom_at(&mut self, pointer: Pos2, factor: f32, viewport: Rect) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let map_point = self.screen_to_normalized(pointer, viewport);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let map_pixels = Vec2::new(
            map_point.x * self.map_extent.x,
            map_point.y * self.map_extent.y,
        ) * self.zoom;
        self.pan = pointer - viewport.min - map_pixels;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_to_screen_matches_presentation_reference_formula() {
        let viewport = Rect::from_min_size(pos2(100.0, 80.0), Vec2::new(800.0, 600.0));
        let mut camera = Camera2D::new(Vec2::new(400.0, 200.0));
        camera.pan = Vec2::new(10.0, 20.0);
        camera.zoom = 2.0;

        // viewport.min + pan + zoom * (u * width, v * height)
        assert_eq!(
            camera.normalized_to_screen(pos2(0.25, 0.5), viewport),
            pos2(310.0, 300.0)
        );
    }

    #[test]
    fn fit_centers_the_map_without_changing_coordinate_semantics() {
        let viewport = Rect::from_min_size(pos2(100.0, 80.0), Vec2::new(800.0, 600.0));
        let mut camera = Camera2D::new(Vec2::new(400.0, 200.0));
        camera.fit(viewport);
        assert_eq!(camera.zoom(), 2.0);
        assert_eq!(camera.pan(), Vec2::new(0.0, 100.0));
        assert_eq!(
            camera.normalized_to_screen(pos2(0.0, 0.0), viewport),
            pos2(100.0, 180.0)
        );
    }
}
