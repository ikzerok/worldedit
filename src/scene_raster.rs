//! 核心安全 SVG 的纯像素投影；native 与同源 Worker 共用本入口。
mod surfaces;

use resvg::{tiny_skia, usvg};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use worldline_core::vector_scene::{scene_to_safe_svg, MapScene};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct RasterSpec {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) zoom: f32,
    pub(crate) pan: [f32; 2],
    pub(crate) dpi: f32,
}

impl RasterSpec {
    fn transform(&self) -> Result<tiny_skia::Transform, String> {
        let pixels = u64::from(self.width) * u64::from(self.height);
        if pixels == 0 || pixels > 16_777_216 {
            return Err("scene 视口须为正尺寸且不超过 16,777,216 像素".into());
        }
        let scale = self.zoom * self.dpi;
        let x = self.pan[0] * self.dpi;
        let y = self.pan[1] * self.dpi;
        if !self.zoom.is_finite()
            || self.zoom <= 0.0
            || !self.dpi.is_finite()
            || self.dpi <= 0.0
            || !scale.is_finite()
            || scale <= 0.0
            || !x.is_finite()
            || !y.is_finite()
        {
            return Err("scene 镜头与 DPI 必须为有限有效数值".into());
        }
        Ok(tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, x, y))
    }
}

pub(crate) fn render_scene(
    scene: &MapScene,
    extent: [f64; 2],
    spec: &RasterSpec,
) -> Result<Vec<u8>, String> {
    let transform = spec.transform()?;
    if extent
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err("scene 逻辑画布尺寸无效".into());
    }
    let source =
        scene_to_safe_svg(scene, extent[0], extent[1]).map_err(|error| error.to_string())?;
    raster_safe_source(&source, spec, transform)
}

fn raster_safe_source(
    source: &str,
    spec: &RasterSpec,
    transform: tiny_skia::Transform,
) -> Result<Vec<u8>, String> {
    let tree = usvg::Tree::from_str(source, &options())
        .map_err(|error| format!("安全 SVG 无法生成渲染树：{error}"))?;
    // 必须在分配主 viewport 及进入 resvg 前完成；Worker 同样经过此处。
    surfaces::preflight(&tree, transform, [spec.width, spec.height])?;
    let mut pixmap = tiny_skia::Pixmap::new(spec.width, spec.height)
        .ok_or_else(|| "无法为 scene 分配视口像素".to_owned())?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(pixmap.take())
}

fn options() -> usvg::Options<'static> {
    static FONT: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    let fontdb = FONT
        .get_or_init(|| {
            let mut database = usvg::fontdb::Database::new();
            database.load_font_data(include_bytes!("../assets/fonts/NotoSansSC.ttf").to_vec());
            database.set_sans_serif_family("Noto Sans SC");
            database.set_serif_family("Noto Sans SC");
            database.set_monospace_family("Noto Sans SC");
            Arc::new(database)
        })
        .clone();
    usvg::Options {
        font_family: "Noto Sans SC".into(),
        fontdb,
        resources_dir: None,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    }
}

#[cfg(test)]
mod dash_tests;
#[cfg(test)]
mod tests;
