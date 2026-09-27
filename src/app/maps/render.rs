use super::geometry::{triangulate_polygon, GeometryError};
use super::*;
use egui::{Shape, Stroke, StrokeKind};
use std::path::Path;
impl MapCanvas {
    pub(super) fn draw_rasters(
        &mut self,
        painter: &egui::Painter,
        _ctx: &egui::Context,
        viewport: Rect,
    ) {
        for raster in &self.snapshot.raster_layers {
            let rect = raster.rect.to_screen(&self.camera, viewport);
            let key = raster.cache_key();
            if let Some(texture) = self.textures.get(&key) {
                painter.image(
                    texture.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                painter.rect_filled(rect, 0.0, Color32::from_gray(38));
                painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(1.0_f32, Color32::from_gray(85)),
                    StrokeKind::Inside,
                );
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    self.raster_errors
                        .get(&key)
                        .map(String::as_str)
                        .unwrap_or("栅格资源不可用"),
                    egui::FontId::proportional(12.0),
                    Color32::from_gray(150),
                );
            }
        }
    }

    pub(super) fn draw_vectors(&self, painter: &egui::Painter, viewport: Rect) {
        for layer in &self.snapshot.layers {
            if !layer.visible {
                continue;
            }
            for placement in &layer.placements {
                draw_geometry(
                    painter,
                    &placement.geometry,
                    &placement.style,
                    &self.camera,
                    viewport,
                );
                if self
                    .selected
                    .as_ref()
                    .is_some_and(|(id, _)| id == &placement.id)
                {
                    draw_control_points(painter, &placement.geometry, &self.camera, viewport);
                }
            }
        }
    }
}
pub(super) fn read_raster(
    root: &Path,
    path: &Path,
    config: raster::RasterDecodeConfig,
) -> Result<raster::DecodedRaster, String> {
    let path = worldline_core::file_access::within(root, path)?;
    let max_encoded_bytes = max_encoded_bytes();
    preflight_raster_size(&path, max_encoded_bytes)?;
    let bytes = worldline_core::file_access::read(&path).map_err(|error| error.to_string())?;
    if bytes.len() > max_encoded_bytes {
        return Err(format!(
            "素材文件超过读取上限（{} MiB）",
            max_encoded_bytes / (1024 * 1024)
        ));
    }
    raster::decode_rgba(&bytes, config).map_err(raster_error_message)
}

fn preflight_raster_size(path: &Path, max_encoded_bytes: usize) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let size = std::fs::metadata(path)
            .map_err(|error| format!("无法读取素材大小：{error}"))?
            .len();
        if size > max_encoded_bytes as u64 {
            return Err(format!(
                "素材文件超过读取上限（{} MiB）",
                max_encoded_bytes / (1024 * 1024)
            ));
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let size = crate::web::imported_size(path)
            .ok_or_else(|| "素材尚未导入当前浏览器工作区".to_string())?;
        if size > max_encoded_bytes {
            return Err(format!(
                "素材文件超过读取上限（{} MiB）",
                max_encoded_bytes / (1024 * 1024)
            ));
        }
    }
    Ok(())
}

fn max_encoded_bytes() -> usize {
    #[cfg(target_arch = "wasm32")]
    {
        64 * 1024 * 1024
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        128 * 1024 * 1024
    }
}

fn raster_error_message(error: raster::RasterError) -> String {
    match error {
        raster::RasterError::UnsupportedFormat => "仅支持 PNG 或 JPEG 素材".into(),
        raster::RasterError::InvalidImage(message) => format!("图片无法解码：{message}"),
        raster::RasterError::DimensionsExceeded { width, height } => {
            format!("图片尺寸 {width}×{height} 超出显示上限")
        }
        raster::RasterError::PixelsExceeded { width, height } => {
            format!("图片像素数 {width}×{height} 超出显示上限")
        }
        raster::RasterError::AllocationExceeded { bytes } => {
            format!("图片解码内存 {} MiB 超出显示上限", bytes / (1024 * 1024))
        }
    }
}

pub(super) fn geometry_error_message(error: GeometryError) -> String {
    match error {
        GeometryError::NonFinite => "点坐标必须是有限数字".into(),
        GeometryError::OutOfRange => "点必须位于地图范围内".into(),
        GeometryError::TooFewPoints => "线至少需要两个点，面至少需要三个点".into(),
        GeometryError::RepeatedClosingPoint => "面不应重复首个点作为末点".into(),
        GeometryError::SelfIntersection => "面边界不能自相交".into(),
        GeometryError::Degenerate => "几何形状不能退化".into(),
    }
}

pub(super) fn geometry_vertex_mut(
    geometry: &mut MapGeometry,
    index: usize,
) -> Option<&mut NormalizedPoint> {
    match geometry {
        MapGeometry::Point(point) if index == 0 => Some(point),
        MapGeometry::Polyline(points) | MapGeometry::Polygon(points) => points.get_mut(index),
        _ => None,
    }
}

pub(super) fn geometry_vertex(geometry: &MapGeometry, index: usize) -> Option<NormalizedPoint> {
    match geometry {
        MapGeometry::Point(point) if index == 0 => Some(*point),
        MapGeometry::Polyline(points) | MapGeometry::Polygon(points) => points.get(index).copied(),
        _ => None,
    }
}

pub(super) fn draw_geometry(
    painter: &egui::Painter,
    geometry: &MapGeometry,
    style: &MapStyle,
    camera: &Camera2D,
    viewport: Rect,
) {
    match geometry {
        MapGeometry::Point(point) => {
            let screen = camera.normalized_to_screen(point.as_pos2(), viewport);
            painter.circle_filled(screen, 5.0, style.stroke);
            painter.circle_stroke(screen, 7.0, Stroke::new(style.width, style.stroke));
        }
        MapGeometry::Polyline(points) => {
            let screen_points = points
                .iter()
                .map(|point| camera.normalized_to_screen(point.as_pos2(), viewport))
                .collect::<Vec<_>>();
            if screen_points.len() >= 2 {
                painter.add(Shape::line(
                    screen_points,
                    Stroke::new(style.width, style.stroke),
                ));
            }
        }
        MapGeometry::Polygon(points) => {
            let screen_points = points
                .iter()
                .map(|point| camera.normalized_to_screen(point.as_pos2(), viewport))
                .collect::<Vec<_>>();
            if let Some(triangles) = triangulate_polygon(points) {
                for triangle in triangles {
                    painter.add(Shape::convex_polygon(
                        triangle
                            .into_iter()
                            .map(|index| screen_points[index])
                            .collect(),
                        style.fill,
                        Stroke::NONE,
                    ));
                }
            }
            if screen_points.len() >= 3 {
                painter.add(Shape::closed_line(
                    screen_points,
                    Stroke::new(style.width, style.stroke),
                ));
            }
        }
    }
}

pub(super) fn draw_control_points(
    painter: &egui::Painter,
    geometry: &MapGeometry,
    camera: &Camera2D,
    viewport: Rect,
) {
    let draw = |painter: &egui::Painter, point: &NormalizedPoint| {
        painter.circle_filled(
            camera.normalized_to_screen(point.as_pos2(), viewport),
            4.0,
            Color32::from_rgb(255, 210, 90),
        );
    };
    match geometry {
        MapGeometry::Point(point) => draw(painter, point),
        MapGeometry::Polyline(points) | MapGeometry::Polygon(points) => {
            for point in points {
                draw(painter, point);
            }
        }
    }
}
