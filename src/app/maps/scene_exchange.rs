use super::scene_export_job::{SvgExportEvent, SvgExportJob};

pub(super) struct SvgExportForm {
    map_id: String,
    baseline: String,
    source: Option<String>,
    job: Option<SvgExportJob>,
    error: Option<String>,
}

impl super::super::WorldeditApp {
    pub(super) fn begin_svg_export(&mut self, ctx: &egui::Context) {
        let map_id = self.map_canvas.map_id().to_owned();
        let Some(map) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.map_index.maps.get(&map_id))
            .cloned()
        else {
            return;
        };
        let job = SvgExportJob::render(&self.project, map, ctx);
        let (job, error) = match job {
            Ok(job) => (Some(job), None),
            Err(error) => (None, Some(error)),
        };
        self.map_canvas.scene.export = Some(SvgExportForm {
            map_id,
            baseline: self.project.content_baseline(),
            source: None,
            job,
            error,
        });
    }

    pub(super) fn scene_export_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.map_canvas.scene.export.take() else {
            return;
        };
        let mut completed = form.job.as_mut().and_then(SvgExportJob::poll);
        if completed.is_some() {
            form.job = None;
        }
        if let Some(Ok(SvgExportEvent::Source(_))) = &completed {
            if let Some(Ok(SvgExportEvent::Source(source))) = completed.take() {
                form.source = Some(source);
            }
        }
        let current = form.baseline == self.project.content_baseline()
            && form.map_id == self.map_canvas.map_id();
        let mut cancel = false;
        egui::Window::new("导出矢量 SVG").id(egui::Id::new("scene-vector-export")).default_width(640.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                cancel = ui.button("关闭 / 取消").clicked();
                if ui.add_enabled(current && form.source.is_some(), egui::Button::new("复制完整 SVG")).clicked() {
                    ctx.copy_text(form.source.as_ref().expect("enabled source").clone());
                }
                if ui.add_enabled(current && form.source.is_some() && form.job.is_none(), crate::theme::primary("保存 SVG…")).clicked() {
                    #[cfg(not(target_arch = "wasm32"))]
                    match SvgExportJob::pick(format!("{}.svg", form.map_id), ctx) { Ok(job) => form.job = Some(job), Err(error) => form.error = Some(error) }
                    #[cfg(target_arch = "wasm32")]
                    if let Err(error) = crate::web::download(&format!("{}.svg", form.map_id), form.source.as_ref().expect("enabled source").as_bytes(), "image/svg+xml") { form.error = Some(error); }
                }
            });
            ui.label("导出当前文档默认可见的旧标记与原生 scene，保持每层顺序。个人临时显隐不写入此交换文件。栅格底图、资料正文与附件请用完整工程备份。");
            if self.map_canvas.has_uncommitted_work() { ui.colored_label(crate::theme::WARNING(), "包含已应用版本；未应用检查器或绘制草稿不会进入 SVG。"); }
            if !current { ui.colored_label(crate::theme::WARNING(), "工程已改变，不能保存或复制旧导出；关闭后重新生成。"); }
            if form.job.is_some() {
                ui.horizontal(|ui| { ui.spinner(); ui.label("后台生成 SVG / 等待保存目标 / 安全暂存…"); });
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            if let Some(error) = &form.error { ui.colored_label(crate::theme::ERROR(), error); }
            if let Some(source) = &form.source {
                ui.label(format!("完整 SVG：{} bytes", source.len()));
                egui::CollapsingHeader::new("查看源码前 64 KiB（复制/保存始终为完整文件）").show(ui, |ui| {
                    let mut end = source.len().min(64 * 1024);
                    while !source.is_char_boundary(end) { end -= 1; }
                    egui::ScrollArea::both().max_height(260.0).show(ui, |ui| { ui.monospace(&source[..end]); });
                });
            }
        });
        if cancel {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(Err(error)) = completed {
            form.error = Some(error);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(result) = completed {
            match result {
                Err(error) => form.error = Some(error),
                #[cfg(not(target_arch = "wasm32"))]
                Ok(SvgExportEvent::Picked(Some(path))) if current => {
                    match SvgExportJob::stage(path, form.source.clone().unwrap_or_default(), ctx) {
                        Ok(job) => form.job = Some(job),
                        Err(error) => form.error = Some(error),
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                Ok(SvgExportEvent::Staged(stage)) if current => match stage.commit() {
                    Ok(path) => self.message = Some(format!("矢量 SVG 已保存：{}", path.display())),
                    Err(error) => form.error = Some(error),
                },
                _ => {} // stale stage Drop 清理暂存；取消系统选择不触发写入。
            }
        }
        self.map_canvas.scene.export = Some(form);
    }
}
