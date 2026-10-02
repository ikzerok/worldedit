//! 按当前 zoom × DPI 的派生纹理；所有实例共享工作/缓存预算。
use super::camera::Camera2D;
use super::render_budget::{AdmissionError, RenderBudget, RenderPermit, TextureLease};
use super::scene_render_job::RenderJob;
use crate::scene_raster::RasterSpec;
use egui::{Color32, ColorImage, Pos2, Rect, Vec2};
use std::sync::Arc;
use worldline_core::vector_scene::MapScene;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RenderKey {
    pub(super) generation: u64,
    pub(super) extent: [f64; 2],
    pub(super) spec: RasterSpec,
}

pub(super) struct SceneView<'a> {
    pub(super) camera: &'a Camera2D,
    pub(super) viewport: Rect,
    pub(super) extent: [f64; 2],
}

pub(super) struct RasterReady {
    pub(super) key: RenderKey,
    pub(super) pixels: Result<Vec<u8>, String>,
    pub(super) permit: Arc<RenderPermit>,
}

pub(super) enum RenderStatus {
    Idle,
    Queued(AdmissionError),
    Blocked(AdmissionError, u64),
    Rendering,
    Ready,
    Failed(String),
}

struct CachedScene {
    key: RenderKey,
    texture: Option<egui::TextureHandle>,
    offset: [u32; 2],
    size: [u32; 2],
    _lease: TextureLease,
}

pub(super) struct SceneRenderer {
    budget: RenderBudget,
    cache: Option<CachedScene>,
    pending: Option<RenderJob>,
    requested: Option<RenderKey>,
    pub(super) status: RenderStatus,
}

impl Default for SceneRenderer {
    fn default() -> Self {
        Self {
            budget: RenderBudget::shared(),
            cache: None,
            pending: None,
            requested: None,
            status: RenderStatus::Idle,
        }
    }
}

impl SceneRenderer {
    pub(super) fn clear(&mut self) {
        // RenderJob::drop 使旧结果失效；native 执行 guard 仍留在线程内。
        self.pending = None;
        self.cache = None;
        self.requested = None;
        self.status = RenderStatus::Idle;
    }

    pub(super) fn busy(&self) -> bool {
        self.pending.is_some() || matches!(self.status, RenderStatus::Queued(_))
    }

    pub(super) fn retry(&mut self) {
        if self.pending.is_none() {
            self.requested = None;
            self.status = RenderStatus::Idle;
        }
    }

    pub(super) fn message(&self, layer: &str) -> Option<String> {
        match &self.status {
            RenderStatus::Idle | RenderStatus::Ready => None,
            RenderStatus::Rendering => {
                Some(format!("图层「{layer}」正在后台渲染{}", self.stale_label()))
            }
            RenderStatus::Queued(error) => {
                Some(format!("{}{}", error.message(layer), self.stale_label()))
            }
            RenderStatus::Blocked(error, _) => {
                Some(format!("{}{}", error.message(layer), self.stale_label()))
            }
            RenderStatus::Failed(error) => Some(format!(
                "图层「{layer}」当前结果尚未显示：{error}{}",
                self.stale_label()
            )),
        }
    }

    fn stale_label(&self) -> &'static str {
        if self
            .cache
            .as_ref()
            .is_some_and(|cache| self.requested.as_ref() != Some(&cache.key))
        {
            " · 暂显旧视图，当前 DPI 图像尚未就绪"
        } else {
            ""
        }
    }

    pub(super) fn show(
        &mut self,
        painter: &egui::Painter,
        scene: &MapScene,
        generation: u64,
        view: SceneView<'_>,
    ) {
        let ctx = painter.ctx();
        let dpi = ctx.pixels_per_point();
        let physical = view.viewport.size() * dpi;
        if !physical.is_finite()
            || physical.x <= 0.0
            || physical.y <= 0.0
            || physical.x.ceil() > u32::MAX as f32
            || physical.y.ceil() > u32::MAX as f32
        {
            self.clear();
            self.status = RenderStatus::Failed("视口尺寸无效".into());
            return;
        }
        let (zoom, pan) = view.camera.state();
        let key = RenderKey {
            generation,
            extent: view.extent,
            spec: RasterSpec {
                width: physical.x.ceil() as u32,
                height: physical.y.ceil() as u32,
                zoom,
                pan,
                dpi,
            },
        };
        if self
            .requested
            .as_ref()
            .is_some_and(|previous| previous != &key)
        {
            self.pending = None;
            self.status = RenderStatus::Idle;
            if self
                .cache
                .as_ref()
                .is_some_and(|cache| cache.key.extent != key.extent)
            {
                self.cache = None;
            }
            self.requested = Some(key.clone());
        }
        if let Some(result) = self.pending.as_mut().and_then(RenderJob::poll) {
            self.pending = None;
            if result.key == key {
                match cache_result(ctx, result) {
                    Ok(cache) => {
                        self.cache = Some(cache);
                        self.status = RenderStatus::Ready;
                    }
                    Err(error) => self.status = RenderStatus::Failed(error),
                }
            }
        }
        if self.cache.as_ref().is_some_and(|cache| cache.key == key) {
            if let Some(cache) = &self.cache {
                draw_cache(painter, view.viewport, cache, &key);
            }
            return;
        }
        if let RenderStatus::Blocked(_, generation) = self.status {
            if self.budget.stats().generation != generation {
                self.status = RenderStatus::Idle;
            }
        }
        if self.pending.is_none()
            && !matches!(
                self.status,
                RenderStatus::Failed(_) | RenderStatus::Blocked(_, _)
            )
        {
            let mut admission = self.budget.start([key.spec.width, key.spec.height]);
            if matches!(admission, Err(AdmissionError::Capacity { .. }))
                && self.cache.as_ref().is_some_and(|cache| cache.key != key)
            {
                self.cache = None;
                admission = self.budget.start([key.spec.width, key.spec.height]);
            }
            match admission {
                Ok((permit, execution)) => {
                    // 准入成功后才复制 typed scene；不为排队的 5000 层各造副本。
                    match RenderJob::start(ctx, scene.clone(), key.clone(), permit, execution) {
                        Ok(job) => {
                            self.pending = Some(job);
                            self.status = RenderStatus::Rendering;
                        }
                        Err(error) => self.status = RenderStatus::Failed(error),
                    }
                }
                Err(AdmissionError::InvalidDimensions) => {
                    self.status = RenderStatus::Failed("视口字节数无效".into());
                }
                Err(error @ AdmissionError::Waiting { .. }) => {
                    self.status = RenderStatus::Queued(error)
                }
                Err(error) => {
                    self.status = RenderStatus::Blocked(error, self.budget.stats().generation)
                }
            }
            self.requested = Some(key.clone());
        }
        if let Some(cache) = &self.cache {
            draw_cache(painter, view.viewport, cache, &key);
        }
        if self.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(25));
        }
    }
}

fn cache_result(ctx: &egui::Context, ready: RasterReady) -> Result<CachedScene, String> {
    let RasterReady {
        key,
        pixels,
        permit,
    } = ready;
    let bytes = pixels?;
    let size = [key.spec.width, key.spec.height];
    let expected =
        super::render_budget::rgba_bytes(size).ok_or_else(|| "渲染结果尺寸溢出".to_owned())?;
    if bytes.len() != expected {
        return Err("渲染返回的像素长度与视口不符".into());
    }
    let (offset, cropped_size, cropped) = crop_transparent(&bytes, size);
    let image = (!cropped.is_empty()).then(|| {
        ColorImage::from_rgba_premultiplied(
            [cropped_size[0] as usize, cropped_size[1] as usize],
            &cropped,
        )
    });
    let rgba_bytes = cropped.len();
    // 转 cache lease 前释放主 RGBA 和裁切 Vec，留下唯一 ColorImage 上传值。
    drop(bytes);
    drop(cropped);
    let texture =
        image.map(|image| ctx.load_texture("scene-vector", image, egui::TextureOptions::LINEAR));
    let lease = permit.cache(rgba_bytes)?;
    Ok(CachedScene {
        key,
        texture,
        offset,
        size: cropped_size,
        _lease: lease,
    })
}

fn draw_cache(painter: &egui::Painter, viewport: Rect, cache: &CachedScene, current: &RenderKey) {
    let Some(texture) = &cache.texture else {
        return;
    };
    let dpi = cache.key.spec.dpi;
    let ratio = current.spec.zoom / cache.key.spec.zoom;
    let origin = viewport.min
        + Vec2::from(current.spec.pan)
        + (Vec2::new(cache.offset[0] as f32, cache.offset[1] as f32) / dpi
            - Vec2::from(cache.key.spec.pan))
            * ratio;
    let size = Vec2::new(cache.size[0] as f32, cache.size[1] as f32) / dpi * ratio;
    painter.with_clip_rect(viewport).image(
        texture.id(),
        Rect::from_min_size(origin, size),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
}

fn crop_transparent(bytes: &[u8], size: [u32; 2]) -> ([u32; 2], [u32; 2], Vec<u8>) {
    let width = size[0] as usize;
    let mut left = size[0];
    let mut top = size[1];
    let mut right = 0;
    let mut bottom = 0;
    for (index, rgba) in bytes.as_chunks::<4>().0.iter().enumerate() {
        if rgba[3] != 0 {
            let x = (index % width) as u32;
            let y = (index / width) as u32;
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if left >= right || top >= bottom {
        return ([0, 0], [0, 0], Vec::new());
    }
    let crop_size = [right - left, bottom - top];
    let mut cropped = Vec::with_capacity(crop_size[0] as usize * crop_size[1] as usize * 4);
    for y in top..bottom {
        let begin = (y as usize * width + left as usize) * 4;
        let end = (y as usize * width + right as usize) * 4;
        cropped.extend_from_slice(&bytes[begin..end]);
    }
    ([left, top], crop_size, cropped)
}

#[cfg(test)]
#[path = "tests/scene_renderer.rs"]
mod tests;
