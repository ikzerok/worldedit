//! SVG 安全导入与基础图形制作；预览/取消只修改本地表单。
use super::*;
#[derive(Default)]
pub(super) struct SvgImportForm {
    pub(super) open: bool,
    map_id: String,
    baseline: Option<MapCommandBaseline>,
    source: String,
    preview: Option<worldline_core::svg_import::SvgPreview>,
    error: Option<String>,
    ellipse: bool,
    bounds: [f32; 4],
    fill: String,
    stroke: String,
    width: f32,
    opacity: f32,
}
impl super::super::WorldeditApp {
    pub(super) fn svg_import_panel(&mut self, ui: &mut egui::Ui) {
        if !self.map_canvas.is_edit_mode() {
            return;
        }
        ui.separator();
        ui.strong("添加矢量内容");
        if !self.map_canvas.svg_import.open {
            let open = egui::CollapsingHeader::new("图形与导入")
                .id_salt("map-add-vector-content")
                .show(ui, |ui| ui.button("绘制矩形、椭圆 / 导入 SVG").clicked())
                .body_returned
                .unwrap_or(false);
            if open {
                if self.map_canvas.has_uncommitted_work() {
                    self.message = Some("请先完成或取消当前绘图".into());
                    return;
                }
                let map_id = self.map_selection.clone().unwrap_or_default();
                let baseline = self.map_command_baseline(&map_id);
                self.map_canvas.svg_import = SvgImportForm {
                    open: true,
                    map_id,
                    baseline,
                    bounds: [10., 10., 60., 40.],
                    fill: "#356f99".into(),
                    stroke: "#65b4ff".into(),
                    width: 2.,
                    opacity: 1.,
                    ..Default::default()
                };
            }
            return;
        }
        let mut form = std::mem::take(&mut self.map_canvas.svg_import);
        let mut cancel = false;
        let mut apply = false;
        ui.label(crate::theme::muted(
            "确认后生成可编辑图层，可整体撤销。椭圆以 64 个顶点近似。",
        ));
        egui::CollapsingHeader::new("基础图形与样式").default_open(true).show(ui,|ui|{
            ui.horizontal(|ui|{ui.selectable_value(&mut form.ellipse,false,"矩形");ui.selectable_value(&mut form.ellipse,true,"椭圆");});
            for (i,label) in ["左侧 %","顶部 %","宽度 %","高度 %"].iter().enumerate(){ui.add(egui::Slider::new(&mut form.bounds[i],0.0..=100.0).text(*label));}
            ui.horizontal(|ui|{ui.label("填充");ui.text_edit_singleline(&mut form.fill);});
            ui.horizontal(|ui|{ui.label("描边");ui.text_edit_singleline(&mut form.stroke);});
            ui.label(crate::theme::muted("颜色：#RGB / #RRGGBB；none 表示透明"));
            ui.add(egui::Slider::new(&mut form.width,0.0..=20.0).text("描边宽度"));
            ui.add(egui::Slider::new(&mut form.opacity,0.0..=1.0).text("填充不透明度"));
            if ui.button("生成图形预览").clicked(){
                let [x,y,w,h]=form.bounds;
                let geometry=if form.ellipse{format!("<ellipse cx='{}' cy='{}' rx='{}' ry='{}'",x+w/2.,y+h/2.,w/2.,h/2.)}else{format!("<rect x='{x}' y='{y}' width='{w}' height='{h}'")};
                form.source=format!("<svg viewBox='0 0 100 100'>{geometry} fill='{}' stroke='{}' stroke-width='{}' fill-opacity='{}'/></svg>",form.fill,form.stroke,form.width,form.opacity);
                form.check();
            }
        });
        #[cfg(not(target_arch = "wasm32"))]
        if ui.button("选择 SVG 文件…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SVG 矢量图", &["svg"])
                .pick_file()
            {
                let result = std::fs::metadata(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|m| {
                        if m.len() > 2 * 1024 * 1024 {
                            Err("SVG 超过 2 MiB 上限".into())
                        } else {
                            std::fs::read_to_string(path).map_err(|e| e.to_string())
                        }
                    });
                match result {
                    Ok(source) => {
                        form.source = source;
                        form.check();
                    }
                    Err(error) => form.error = Some(error),
                }
            }
        }
        ui.label("或粘贴 SVG 源码");
        if ui
            .add(
                egui::TextEdit::multiline(&mut form.source)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY)
                    .char_limit(2 * 1024 * 1024),
            )
            .changed()
        {
            form.preview = None;
            form.error = None;
        }
        ui.label(crate::theme::muted("支持基础图形和 M/L/H/V/Z、C/Q 路径（含相对指令与重复参数）；曲线转换为可编辑折线。支持平移、旋转、缩放；带描边的非均匀缩放、matrix/skew、圆弧、CSS、文字和外链将明确拒绝。图形按 viewBox 映射到整张地图。"));
        if ui.button("检查并预览 SVG").clicked() {
            form.check();
        }
        if let Some(error) = &form.error {
            ui.colored_label(crate::theme::GOLD(), error);
        }
        if let Some(preview) = &form.preview {
            ui.label(format!(
                "{} 个图形 · {} × {}",
                preview.shapes.len(),
                preview.width,
                preview.height
            ));
            let (rect, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 130.), egui::Sense::hover());
            ui.painter().rect_filled(rect, 4., Color32::from_gray(22));
            let camera = Camera2D::new(rect.size());
            for shape in &preview.shapes {
                render::draw_geometry(
                    ui.painter(),
                    &geometry_from_core(&shape.geometry),
                    &map_style(Some(&shape.style)),
                    &camera,
                    rect,
                );
            }
        }
        ui.horizontal(|ui| {
            apply = ui
                .add_enabled(
                    form.preview.is_some(),
                    crate::theme::primary("确认添加图层"),
                )
                .clicked();
            cancel = ui.button("取消").clicked();
        });
        if apply {
            let Some(baseline) = form.baseline.clone() else {
                form.error = Some("无法读取地图基线，请取消后重开".into());
                self.map_canvas.svg_import = form;
                return;
            };
            let before = self.project.clone();
            let mut i = 1;
            let layers = &self.map_canvas.snapshot.layers;
            while layers.iter().any(|l| l.id == format!("svg_{i}")) {
                i += 1;
            }
            match worldline_core::svg_import::apply(
                &mut self.project,
                &mut self.map_revision,
                &form.map_id,
                &format!("svg_{i}"),
                &form.source,
                baseline.revision,
                baseline.expected_documents,
            ) {
                Ok(count) => {
                    self.remember(before);
                    self.map_canvas.reset_local_preview();
                    self.refresh_presentation_after_map_command();
                    self.message = Some(format!("已添加 {count} 个可编辑矢量图形（可撤销）"));
                    self.io_error = None;
                    return;
                }
                Err(error) => form.error = Some(error.to_string()),
            }
        }
        if !cancel {
            self.map_canvas.svg_import = form;
        }
    }
}
impl SvgImportForm {
    fn check(&mut self) {
        match worldline_core::svg_import::preview(&self.source) {
            Ok(preview) => {
                self.preview = Some(preview);
                self.error = None;
            }
            Err(error) => {
                self.preview = None;
                self.error = Some(error);
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
