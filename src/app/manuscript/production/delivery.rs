use super::*;
#[derive(Clone, Copy)]
pub(super) enum Action {
    Preview,
    Copy,
    Save,
    #[cfg(not(target_arch = "wasm32"))]
    Browse,
}

fn options(state: &State) -> ProductionExportOptions {
    ProductionExportOptions {
        schema_version: 1,
        format: state.format,
        include_direction: state.direction,
    }
}
pub(super) fn draw(ui: &mut egui::Ui, state: &mut State, current: bool) -> Option<Action> {
    let mut action = None;
    ui.separator();
    ui.checkbox(&mut state.export_open, "准备私密交付");
    if !state.export_open {
        return None;
    }
    ui.label("材料只含当前已过滤的台本单元；复制、JSON、Markdown、CSV共用同一不可变结果。");
    let previous = options(state);
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut state.format, ProductionFormat::Json, "精确 JSON");
        ui.selectable_value(
            &mut state.format,
            ProductionFormat::Markdown,
            "阅读 Markdown",
        );
        ui.selectable_value(&mut state.format, ProductionFormat::Csv, "表格 CSV");
    });
    ui.checkbox(&mut state.direction, "明确纳入作者私密演出备注（源语言）");
    if previous != options(state) {
        state.confirmed = false;
    }
    if state.format == ProductionFormat::Csv {
        ui.colored_label(theme::WARNING(), "CSV每格含单引号显示前缀、双引号包裹与CRLF。精确值请用JSON；不保证所有表格软件或再次另存后的安全，也不是无损回导格式。");
    }
    if theme::add_enabled(ui, current, egui::Button::new("预览完整交付字节")).clicked() {
        action = Some(Action::Preview);
    }
    let reviewed = current
        && state.artifact_options.as_ref() == Some(&options(state))
        && state.artifact.as_ref().is_some_and(|artifact| {
            state
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| artifact.snapshot_key() == snapshot.key())
        });
    if let Some(artifact) = &state.artifact {
        if let Ok(text) = std::str::from_utf8(artifact.bytes()) {
            preview(ui, text, &mut state.artifact_page);
            theme::add_enabled_ui(ui, reviewed, |ui| {
                ui.checkbox(
                    &mut state.confirmed,
                    "已核对范围、语言状态和上述交付字节，确认私密材料内容",
                );
            });
        }
    }
    ui.horizontal_wrapped(|ui| {
        let enabled = reviewed && state.confirmed;
        if theme::add_enabled(ui, enabled, egui::Button::new("复制相同完整材料")).clicked()
        {
            action = Some(Action::Copy);
        }
        #[cfg(target_arch = "wasm32")]
        if theme::add_enabled(ui, enabled, egui::Button::new("下载相同材料")).clicked() {
            action = Some(Action::Save);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if theme::add_enabled(ui, enabled, egui::Button::new("选择新文件…")).clicked() {
            action = Some(Action::Browse);
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    {
        ui.label(theme::muted(
            "只写工作区外的新绝对路径文件，不覆盖已有文件。",
        ));
        crate::app::writing_workspace::register_input(
            &ui.add(
                egui::TextEdit::singleline(&mut state.destination)
                    .id_salt("production-export-path")
                    .hint_text("工作区外新文件完整路径"),
            ),
        );
        if theme::add_enabled(
            ui,
            reviewed && state.confirmed,
            egui::Button::new("写入新文件"),
        )
        .clicked()
        {
            action = Some(Action::Save);
        }
    }
    action
}
fn preview(ui: &mut egui::Ui, text: &str, page: &mut usize) {
    const BYTES: usize = 16 * 1024;
    let count = text.len().div_ceil(BYTES).max(1);
    *page = (*page).min(count - 1);
    ui.horizontal_wrapped(|ui| {
        if theme::add_enabled(ui, *page > 0, egui::Button::new("上一段交付字节")).clicked() {
            *page -= 1;
        }
        if theme::add_enabled(ui, *page + 1 < count, egui::Button::new("下一段交付字节")).clicked()
        {
            *page += 1;
        }
        ui.label(format!(
            "完整材料第 {} / {} 段 · {} UTF-8字节",
            *page + 1,
            count,
            text.len()
        ));
    });
    let boundary = |mut index: usize| {
        index = index.min(text.len());
        while !text.is_char_boundary(index) {
            index -= 1;
        }
        index
    };
    let mut segment = &text[boundary(*page * BYTES)..boundary((*page + 1) * BYTES)];
    egui::ScrollArea::both()
        .id_salt("production-artifact-bytes")
        .max_height(300.0)
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut segment)
                    .font(theme::source_font(13.0))
                    .interactive(false)
                    .desired_width(f32::INFINITY),
            );
        });
}
impl WorldeditApp {
    pub(super) fn checked_production_artifact(
        &self,
        ctx: &egui::Context,
    ) -> Result<&ProductionArtifact, String> {
        if let Some(error) = self.review_input_blocker(ctx) {
            return Err(error);
        }
        if !self.production_is_current() {
            return Err("台本或范围已变化，请重新生成和预览".into());
        }
        let state = &self.manuscript.production;
        if !state.confirmed || state.artifact_options.as_ref() != Some(&options(state)) {
            return Err("请先核对并确认当前选项的完整材料".into());
        }
        let snapshot = state.snapshot.as_ref().ok_or("尚未生成台本")?;
        self.project
            .validate_production_script(
                &self.production_buffers(),
                &self.production_drafts(),
                snapshot,
            )
            .map_err(|error| error.to_string())?;
        state
            .artifact
            .as_ref()
            .filter(|artifact| artifact.snapshot_key() == snapshot.key())
            .ok_or_else(|| "交付字节已过期，请重新预览".into())
    }
    pub(super) fn finish_production_export(&mut self, ctx: &egui::Context, action: Action) {
        if matches!(action, Action::Preview) {
            let result = (|| {
                if let Some(error) = self.review_input_blocker(ctx) {
                    return Err(error);
                }
                if !self.production_is_current() {
                    return Err("台本已过期，请重新生成".into());
                }
                let state = &self.manuscript.production;
                let snapshot = state.snapshot.as_ref().ok_or("尚未生成台本")?;
                self.project
                    .validate_production_script(
                        &self.production_buffers(),
                        &self.production_drafts(),
                        snapshot,
                    )
                    .map_err(|error| error.to_string())?;
                snapshot
                    .export(&options(state))
                    .map_err(|error| error.to_string())
            })();
            let state = &mut self.manuscript.production;
            state.confirmed = false;
            match result {
                Ok(artifact) => {
                    state.artifact = Some(artifact);
                    state.artifact_options = Some(options(state));
                    state.artifact_page = 0;
                    state.notice =
                        Some("交付字节已生成，请核对后确认。演出备注按明确选项处理。".into());
                }
                Err(error) => {
                    state.artifact = None;
                    state.artifact_options = None;
                    state.notice = Some(error);
                }
            }
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if matches!(action, Action::Browse) {
            if let Err(error) = self.checked_production_artifact(ctx) {
                self.manuscript.production.notice = Some(error);
                return;
            }
            let extension = extension(self.manuscript.production.format);
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name(format!("private-production-script.{extension}"))
                .add_filter("私密制作台本", &[extension])
                .save_file()
            {
                self.manuscript.production.destination = path.to_string_lossy().into_owned();
            }
            return;
        }
        let result = self.checked_production_artifact(ctx).and_then(|artifact| {
            if matches!(action, Action::Copy) {
                let text =
                    std::str::from_utf8(artifact.bytes()).map_err(|error| error.to_string())?;
                ctx.copy_text(text.to_owned());
                return Ok("已复制与预览相同的完整私密材料；请谨慎选择接收者".to_owned());
            }
            #[cfg(target_arch = "wasm32")]
            {
                crate::web::download(
                    &format!("private-production-script.{}", extension(artifact.format())),
                    artifact.bytes(),
                    "text/plain;charset=utf-8",
                )?;
                Ok("已请求浏览器下载与预览相同的材料；无法确认是否已保存到磁盘".into())
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                write_production_script_new(
                    &self.project.root,
                    std::path::Path::new(self.manuscript.production.destination.trim()),
                    artifact,
                    &mut || self.checked_production_artifact(ctx).map(|_| ()),
                )?;
                Ok("已写入与预览相同的新文件；未修改工程保存状态".into())
            }
        });
        self.manuscript.production.notice = Some(result.unwrap_or_else(|error| error));
    }
}
fn extension(format: ProductionFormat) -> &'static str {
    match format {
        ProductionFormat::Json => "json",
        ProductionFormat::Markdown => "md",
        ProductionFormat::Csv => "csv",
    }
}
