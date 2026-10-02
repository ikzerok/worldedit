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

    pub(super) fn draw_vectors(&mut self, painter: &egui::Painter, viewport: Rect) {
        self.release_hidden_scene_layers();
        for layer in &self.snapshot.layers {
            if !layer.visible {
                continue;
            }
            for placement in &layer.placements {
                if self
                    .drag
                    .as_ref()
                    .is_some_and(|drag| drag.active && drag.placement == placement.id)
                    && matches!(placement.geometry, MapGeometry::Text { .. })
                {
                    continue;
                }
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
                    if let MapGeometry::Text {
                        position,
                        text,
                        font_size,
                        color,
                    } = &placement.geometry
                    {
                        let galley = text_labels::text_layout(
                            painter,
                            &self.camera,
                            text,
                            *font_size,
                            color,
                        );
                        let origin = self
                            .camera
                            .normalized_to_screen(position.as_pos2(), viewport);
                        painter.rect_stroke(
                            Rect::from_min_size(origin, galley.size()).expand(4.0),
                            2.0,
                            Stroke::new(1.0_f32, crate::theme::GOLD()),
                            StrokeKind::Outside,
                        );
                    }
                    draw_control_points(painter, &placement.geometry, &self.camera, viewport);
                }
            }
            // 逐地图层保持 legacy → scene；不能将所有 scene 放在全体旧图元顶层。
            if let Some(scene_layer) = self.scene.layers.get_mut(&layer.id) {
                scene_layer.renderer.show(
                    painter,
                    &scene_layer.scene,
                    self.scene.generation,
                    super::scene_renderer::SceneView {
                        camera: &self.camera,
                        viewport,
                        extent: [
                            self.snapshot.canvas.width as f64,
                            self.snapshot.canvas.height as f64,
                        ],
                    },
                );
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
        MapGeometry::Point(point)
        | MapGeometry::Text {
            position: point, ..
        } if index == 0 => Some(point),
        MapGeometry::Polyline(points) | MapGeometry::Polygon(points) => points.get_mut(index),
        _ => None,
    }
}

pub(super) fn geometry_vertex(geometry: &MapGeometry, index: usize) -> Option<NormalizedPoint> {
    match geometry {
        MapGeometry::Point(point)
        | MapGeometry::Text {
            position: point, ..
        } if index == 0 => Some(*point),
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
        MapGeometry::Text {
            position,
            text,
            font_size,
            color,
        } => {
            super::text_labels::paint_text(
                painter, camera, viewport, *position, text, *font_size, color,
            );
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
                // 三角形共享同一网格，避免每片单独抗锯齿造成半透明接缝。
                painter.add(Shape::mesh(polygon_fill_mesh(
                    &screen_points,
                    &triangles,
                    style.fill,
                )));
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
        MapGeometry::Point(point)
        | MapGeometry::Text {
            position: point, ..
        } => draw(painter, point),
        MapGeometry::Polyline(points) | MapGeometry::Polygon(points) => {
            for point in points {
                draw(painter, point);
            }
        }
    }
}

fn polygon_fill_mesh(points: &[Pos2], triangles: &[[usize; 3]], color: Color32) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    for point in points {
        mesh.colored_vertex(*point, color);
    }
    for &[a, b, c] in triangles {
        mesh.add_triangle(a as u32, b as u32, c as u32);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn polygon_fill_uses_shared_vertices_without_internal_aa_edges() {
        let color = Color32::from_rgba_unmultiplied(10, 50, 90, 128);
        let points = [
            Pos2::ZERO,
            egui::pos2(10., 0.),
            egui::pos2(10., 10.),
            egui::pos2(0., 10.),
        ];
        let mesh = polygon_fill_mesh(&points, &[[0, 1, 2], [0, 2, 3]], color);
        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices, [0, 1, 2, 0, 2, 3]);
        assert!(mesh.vertices.iter().all(|vertex| vertex.color == color));
    }
}
