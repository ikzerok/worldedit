//! SVG 安全预检窗口；typed core scene 是唯一编辑真源。
use super::scene_renderer::{SceneRenderer, SceneView};
use super::svg_import_job::SvgJob;
use super::*;
use worldline_core::vector_scene::{import_view_transform, MapScene, SceneOp, SvgScenePreview};

#[derive(Default)]
pub(super) struct SvgImportForm {
    pub(super) open: bool,
    map_id: String,
    baseline: Option<MapCommandBaseline>,
    source: String,
    preview: Option<SvgScenePreview>,
    display: Option<MapScene>,
    renderer: SceneRenderer,
    generation: u64,
    job: Option<SvgJob>,
    error: Option<String>,
}

impl super::super::WorldeditApp {
    pub(super) fn svg_import_panel(&mut self, ui: &mut egui::Ui) {
        if self.map_canvas.is_edit_mode()
            && !self.map_canvas.svg_import.open
            && ui.button("导入 SVG：预检与原生编辑…").clicked()
        {
            if self.map_canvas.has_uncommitted_work() {
                self.message = Some("请先完成或取消当前地图草稿".into());
                return;
            }
            let map_id = self.map_canvas.map_id().to_owned();
            self.map_canvas.svg_import = SvgImportForm {
                open: true,
                baseline: self.map_command_baseline(&map_id),
                map_id,
                ..Default::default()
            };
        }
    }

    pub(super) fn svg_import_window(&mut self, ctx: &egui::Context) {
        if !self.map_canvas.svg_import.open {
            return;
        }
        let mut form = std::mem::take(&mut self.map_canvas.svg_import);
        if let Some(result) = form.job.as_mut().and_then(|job| job.poll(ctx)) {
            form.job = None;
            match result {
                Ok(result) => {
                    form.source = result.source;
                    match result.preview {
                        Ok(preview) => {
                            let target =
                                self.map_canvas.scene.source.clone().unwrap_or_else(|| {
                                    MapScene::new(
                                        self.map_canvas.snapshot.canvas.width as f64,
                                        self.map_canvas.snapshot.canvas.height as f64,
                                    )
                                });
                            match preview_display(&preview, &target) {
                                Ok(display) => {
                                    form.display = Some(display);
                                    form.preview = Some(preview);
                                    form.error = None;
                                }
                                Err(error) => form.error = Some(error),
                            }
                            form.generation = form.generation.wrapping_add(1);
                        }
                        Err(error) => {
                            form.preview = None;
                            form.display = None;
                            form.error = Some(error);
                        }
                    }
                }
                Err(error) => form.error = Some(error),
            }
        }
        let mut open = true;
        let mut confirm = false;
        let mut cancel = false;
        egui::Window::new("导入 SVG · 安全预检").id(egui::Id::new("scene-svg-import"))
            .open(&mut open).default_width(660.0).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    confirm = crate::theme::add_enabled(ui, form.preview.is_some() && form.job.is_none(), crate::theme::primary("确认添加矢量图层")).clicked();
                    cancel = ui.button("取消导入").clicked();
                    ui.label("完整预检后一次应用，可撤销");
                });
                if let Some(job) = &form.job {
                    ui.horizontal(|ui| { ui.spinner(); ui.label(job.label()); });
                    if ui.button("取消本次预检（保留输入）").clicked() { form.job = None; }
                    ctx.request_repaint_after(std::time::Duration::from_millis(50));
                }
                ui.horizontal(|ui| {
                    if ui.button("选择 SVG 文件…").clicked() {
                        form.clear_preview();
                        match SvgJob::pick(ctx) { Ok(job) => form.job = Some(job), Err(error) => form.error = Some(error) }
                    }
                    if crate::theme::add_enabled(ui, !form.source.is_empty(), egui::Button::new("检查并预览当前源码")).clicked() {
                        form.clear_preview();
                        match SvgJob::check(form.source.clone(), ctx) { Ok(job) => form.job = Some(job), Err(error) => form.error = Some(error) }
                    }
                });
                ui.label("保留曲线控制柄、多子路径、基本文字、组与 Affine；不支持项整批拒绝，原输入保留。");
                egui::CollapsingHeader::new(format!("SVG 源码 · {} bytes", form.source.len()))
                    .id_salt("svg-complete-source").default_open(form.source.len() <= 32 * 1024).show(ui, |ui| {
                        if ui.add(egui::TextEdit::multiline(&mut form.source).desired_rows(5).desired_width(f32::INFINITY)
                            .char_limit(2 * 1024 * 1024)).changed() { form.clear_preview(); }
                        if ui.button("复制完整源码").clicked() { ctx.copy_text(form.source.clone()); }
                    });
                if let Some(error) = &form.error { ui.colored_label(crate::theme::ERROR(), error); }
                if let Some(preview) = &form.preview {
                    ui.label(format!("{} 个节点 · 源 viewport {} × {}", preview.scene.nodes.len(), preview.width, preview.height));
                    for diagnostic in &preview.diagnostics { ui.colored_label(crate::theme::WARNING(), diagnostic.to_string()); }
                }
                if let Some(scene) = &form.display {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 240.0), egui::Sense::hover());
                    let painter = ui.painter().with_clip_rect(rect);
                    painter.rect_filled(rect, 0.0, crate::theme::canvas_background());
                    let extent = [self.map_canvas.snapshot.canvas.width as f64, self.map_canvas.snapshot.canvas.height as f64];
                    let mut camera = Camera2D::new(Vec2::new(extent[0] as f32, extent[1] as f32)); camera.fit(rect);
                    let document = Rect::from_two_pos(camera.normalized_to_screen(Pos2::ZERO, rect), camera.normalized_to_screen(Pos2::new(1.0, 1.0), rect));
                    painter.rect_filled(document, 0.0, crate::theme::document_background());
                    form.renderer.show(&painter, scene, form.generation, SceneView { camera: &camera, viewport: rect, extent });
                    if let Some(message) = form.renderer.message("导入预览") { ui.colored_label(crate::theme::WARNING(), message); }
                }
            });
        if confirm {
            if form.map_id != self.map_canvas.map_id() {
                form.error = Some("地图已切换，请重新预检".into());
            } else if let (Some(preview), Some(baseline)) =
                (form.preview.take(), form.baseline.clone())
            {
                let mut index = 1;
                while self
                    .map_canvas
                    .snapshot
                    .layers
                    .iter()
                    .any(|layer| layer.id == format!("svg_{index}"))
                {
                    index += 1;
                }
                let mut operations = Vec::new();
                if self.map_canvas.scene.source.is_none() {
                    operations.push(SceneOp::EnableScene);
                }
                operations.push(SceneOp::ImportScene {
                    layer_id: format!("svg_{index}"),
                    title: format!("SVG {index}"),
                    scene: preview.scene,
                    width: preview.width,
                    height: preview.height,
                });
                self.map_canvas.scene.intent_baseline = Some(baseline);
                self.map_canvas.scene_queue(operations);
                return;
            } else {
                form.error = Some("地图预检基线不可用，请关闭后重新打开".into());
            }
        }
        if open && !cancel {
            self.map_canvas.svg_import = form;
        }
    }
}

fn preview_display(preview: &SvgScenePreview, target: &MapScene) -> Result<MapScene, String> {
    let transform = import_view_transform(&preview.scene, preview.width, preview.height, target)
        .map_err(|error| error.to_string())?;
    let mut display = preview.scene.clone();
    for id in display.root_order.values().flatten() {
        if let Some(root) = display.nodes.get_mut(id) {
            root.transform = transform.then(root.transform);
        }
    }
    display.view_box = target.view_box;
    display.preserve_aspect_ratio = target.preserve_aspect_ratio.clone();
    Ok(display)
}

impl SvgImportForm {
    fn clear_preview(&mut self) {
        self.preview = None;
        self.display = None;
        self.renderer.clear();
        self.job = None;
        self.error = None;
    }

    #[cfg(test)]
    fn check(&mut self) {
        match worldline_core::svg_import::preview_scene(&self.source) {
            Ok(preview) => {
                self.preview = Some(preview);
                self.error = None;
            }
            Err(error) => {
                self.preview = None;
                self.error = Some(error.to_string());
            }
        }
    }
}
pub(super) fn map_style(style: Option<&serde_json::Map<String, serde_json::Value>>) -> MapStyle {
    let Some(style) = style else {
        return MapStyle::default();
    };
    let color = |key: &str, fallback: Color32| {
        let Some(text) = style.get(key).and_then(|v| v.as_str()) else {
            return fallback;
        };
        if text == "none" {
            return Color32::TRANSPARENT;
        }
        let Some(hex) = text.strip_prefix('#').filter(|h| h.len() == 6) else {
            return fallback;
        };
        u32::from_str_radix(hex, 16)
            .map(|n| {
                let alpha = style
                    .get(&format!("{key}_opacity"))
                    .and_then(|v| v.as_f64())
                    .filter(|v| v.is_finite())
                    .unwrap_or(1.)
                    .clamp(0., 1.);
                Color32::from_rgba_unmultiplied(
                    (n >> 16) as u8,
                    (n >> 8) as u8,
                    n as u8,
                    (alpha * 255.) as u8,
                )
            })
            .unwrap_or(fallback)
    };
    let default = MapStyle::default();
    MapStyle {
        stroke: color("stroke", default.stroke),
        fill: color("fill", default.fill),
        width: style
            .get("stroke_width")
            .and_then(|v| v.as_f64())
            .filter(|v| v.is_finite() && (0.0..=100.0).contains(v))
            .unwrap_or(default.width as f64) as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn svg_preview_keeps_source_and_reports_unsupported_features() {
        let mut form = SvgImportForm {
            source: "<svg viewBox='0 0 10 10'><rect width='4' height='3'/></svg>".into(),
            ..Default::default()
        };
        let source = form.source.clone();
        form.check();
        assert!(form.preview.is_some());
        assert_eq!(form.source, source);
        form.source = "<svg><script/></svg>".into();
        form.check();
        assert!(form.preview.is_none());
        assert!(form.error.is_some());
    }
    #[test]
    fn svg_style_projects_safe_colors_width_and_alpha() {
        let style = serde_json::json!({"fill":"#112233","fill_opacity":0.5,"stroke":"none","stroke_width":3});
        let style = map_style(style.as_object());
        assert_eq!(style.fill.a(), 127);
        assert_eq!(style.stroke, Color32::TRANSPARENT);
        assert_eq!(style.width, 3.);
    }
    #[test]
    fn svg_cancel_is_local_and_does_not_queue_commands() {
        let mut canvas = MapCanvas::new(MapRenderSnapshot::empty(egui::vec2(100., 100.)));
        canvas.svg_import.open = true;
        assert!(canvas.has_uncommitted_work());
        canvas.set_mode(CanvasMode::Browse);
        assert!(!canvas.has_uncommitted_work());
        assert!(canvas.edit_intents.is_empty());
    }
}
