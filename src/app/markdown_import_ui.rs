//! 桌面 Markdown 迁移向导；解析、冲突与写入都由 worldline-core 承担。
use super::WorldeditApp;
use egui::Id;
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
use worldline_core::markdown_import::MarkdownImportRequest;
use worldline_core::markdown_import::{
    MarkdownImportOptions, MarkdownImportPlan, MarkdownImportSourceSnapshot,
};
use worldline_core::project::Project;
use worldline_core::workspace_snapshot::Files;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetMode {
    NewProject,
    CurrentProject,
}

pub(super) struct Wizard {
    source_root: String,
    #[cfg(not(target_arch = "wasm32"))]
    target_root: String,
    target_mode: TargetMode,
    namespace: String,
    id_overrides: BTreeMap<String, String>,
    source_files: Option<Files>,
    accept_losses: bool,
    allow_language_upgrade: bool,
    #[cfg(not(target_arch = "wasm32"))]
    request: Option<MarkdownImportRequest>,
    plan: Option<MarkdownImportPlan>,
    error: Option<String>,
    stale: bool,
    closed: bool,
    page_offset: usize,
    link_offset: usize,
    attachment_offset: usize,
    loss_offset: usize,
    conflict_offset: usize,
    name_conflict_offset: usize,
    file_offset: usize,
    preview_generation: u64,
}

impl Default for Wizard {
    fn default() -> Self {
        Self {
            source_root: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            target_root: String::new(),
            target_mode: TargetMode::NewProject,
            namespace: String::new(),
            id_overrides: BTreeMap::new(),
            source_files: None,
            accept_losses: false,
            allow_language_upgrade: false,
            #[cfg(not(target_arch = "wasm32"))]
            request: None,
            plan: None,
            error: None,
            stale: false,
            closed: false,
            page_offset: 0,
            link_offset: 0,
            attachment_offset: 0,
            loss_offset: 0,
            conflict_offset: 0,
            name_conflict_offset: 0,
            file_offset: 0,
            preview_generation: 0,
        }
    }
}

impl Wizard {
    pub(super) fn show(&mut self, ctx: &egui::Context, app: &mut WorldeditApp) -> bool {
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.closed = true;
        }
        if self.closed {
            return false;
        }

        let mut open = true;
        egui::Window::new("导入 Markdown")
            .id(Id::new("markdown-import-wizard"))
            .open(&mut open)
            .resizable(true)
            .default_size([680.0, 600.0])
            .show(ctx, |ui| self.contents(ui, app, ctx));
        open && !self.closed
    }

    fn contents(&mut self, ui: &mut egui::Ui, app: &mut WorldeditApp, _ctx: &egui::Context) {
        ui.label("步骤 1 来源　→　步骤 2 只读预检　→　步骤 3 确认并应用");
        ui.label("预检不写入文件；应用时会重新检查来源与工程基线。取消不会创建半成品工程。");
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if ui
                .radio_value(
                    &mut self.target_mode,
                    TargetMode::NewProject,
                    "导入到新工程",
                )
                .changed()
            {
                self.invalidate_preview();
            }
            if ui
                .radio_value(
                    &mut self.target_mode,
                    TargetMode::CurrentProject,
                    "导入到当前工程",
                )
                .changed()
            {
                self.invalidate_preview();
            }
        });

        ui.label("Markdown 来源目录");
        #[cfg(not(target_arch = "wasm32"))]
        ui.horizontal(|ui| {
            let changed = ui
                .add(
                    egui::TextEdit::singleline(&mut self.source_root)
                        .hint_text("选择 Markdown 来源目录…")
                        .desired_width(ui.available_width() - 105.0),
                )
                .changed();
            if changed {
                self.invalidate_preview();
            }
            if ui.button("选择目录…").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.source_root = path.display().to_string();
                    self.source_files = None;
                    self.invalidate_preview();
                }
            }
        });
        #[cfg(target_arch = "wasm32")]
        ui.horizontal(|ui| {
            ui.label(if self.source_root.is_empty() {
                "尚未选择文件夹".to_string()
            } else {
                self.source_root.clone()
            });
            if ui.button("选择 Markdown 文件夹…").clicked() {
                self.invalidate_preview();
                crate::web::select_files(_ctx, true, "", crate::web::FileAction::MarkdownImport);
            }
        });

        match self.target_mode {
            TargetMode::NewProject => {
                #[cfg(not(target_arch = "wasm32"))]
                ui.label("新工程目录（须已存在且为空）");
                #[cfg(target_arch = "wasm32")]
                ui.label("应用后切换到新浏览器工程；当前工程不会在预检或取消时改变。");
                #[cfg(not(target_arch = "wasm32"))]
                ui.horizontal(|ui| {
                    let changed = ui
                        .add(
                            egui::TextEdit::singleline(&mut self.target_root)
                                .hint_text("选择空工程目录…")
                                .desired_width(ui.available_width() - 105.0),
                        )
                        .changed();
                    if changed {
                        self.invalidate_preview();
                    }
                    if ui.button("选择目录…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.target_root = path.display().to_string();
                            self.invalidate_preview();
                        }
                    }
                });
            }
            TargetMode::CurrentProject => {
                #[cfg(not(target_arch = "wasm32"))]
                ui.label(format!("目标工程：{}", app.project.root.display()));
                #[cfg(target_arch = "wasm32")]
                ui.label("目标工程：当前浏览器工程");
                if !app.saved_location {
                    ui.colored_label(
                        egui::Color32::LIGHT_RED,
                        "当前工程尚未保存到工作区；请先另存工程，或选择导入到新工程。",
                    );
                } else if app.project.is_dirty() || app.has_open_authoring_form() {
                    ui.colored_label(
                        egui::Color32::LIGHT_RED,
                        "当前工程有未保存的草稿；先保存或完成草稿，再开始迁移。",
                    );
                }
            }
        }

        ui.horizontal_wrapped(|ui| {
            ui.label("迁移命名空间");
            let changed = ui
                .add(
                    egui::TextEdit::singleline(&mut self.namespace)
                        .hint_text("留空时由来源路径稳定生成")
                        .desired_width(190.0),
                )
                .changed();
            if changed {
                self.invalidate_preview();
            }
            if ui.button("预检导入").clicked() {
                self.preview(app);
            }
            if ui.button("取消").clicked() {
                self.closed = true;
            }
        });

        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        let Some(plan) = self.plan.take() else {
            return;
        };

        ui.separator();
        ui.label(format!(
            "预检完成：{} 个页面、{} 个页面链接、{} 个附件、{} 个损失、{} 个阻塞冲突。",
            plan.pages.len(),
            plan.links.len(),
            plan.attachments.len(),
            plan.losses.len(),
            plan.conflicts.len()
        ));
        ui.label(format!(
            "命名空间：{}　·　输出文件：{}　·　来源指纹：{}",
            plan.namespace,
            plan.files.len(),
            plan.source_fingerprint
        ));
        if self.stale {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                "预检已过期，应用已锁定。请重新预检后再确认。",
            );
        }
        if plan.requires_language_upgrade {
            ui.checkbox(
                &mut self.allow_language_upgrade,
                "我已检查目标语言版本升级及其影响",
            );
        }
        if !plan.losses.is_empty() {
            ui.checkbox(&mut self.accept_losses, "我已检查并接受预览中的损失");
        }

        let can_apply = !self.stale
            && plan.conflicts.is_empty()
            && (plan.losses.is_empty() || self.accept_losses)
            && (!plan.requires_language_upgrade || self.allow_language_upgrade);
        if can_apply {
            ui.label("必需确认已完成，可以应用。");
        } else {
            let mut blockers = Vec::new();
            if self.stale {
                blockers.push("预检已过期，请重新预检".to_string());
            }
            if !plan.conflicts.is_empty() {
                blockers.push(format!("仍有 {} 项阻塞冲突", plan.conflicts.len()));
            }
            if !plan.losses.is_empty() && !self.accept_losses {
                blockers.push("尚未确认损失".into());
            }
            if plan.requires_language_upgrade && !self.allow_language_upgrade {
                blockers.push("尚未确认语言升级".into());
            }
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                format!("暂不能应用：{}。", blockers.join("；")),
            );
        }
        let show_refresh = self.stale;
        let mut should_apply = false;
        let mut should_preview = false;
        let mut should_close = false;
        ui.horizontal_wrapped(|ui| {
            should_apply = ui
                .add_enabled(can_apply, egui::Button::new("应用导入"))
                .clicked();
            if show_refresh {
                should_preview = ui.button("重新预检").clicked();
            }
            should_close = ui.button("取消").clicked();
        });

        egui::ScrollArea::vertical()
            .id_salt(("markdown-import-details", self.preview_generation))
            .auto_shrink([false, false])
            .max_height(ui.available_height().max(180.0))
            .show(ui, |ui| self.show_plan_sections(ui, &plan));
        self.plan = Some(plan);
        if should_apply {
            self.apply(app);
        } else if should_preview {
            self.preview(app);
        }
        if should_close {
            self.closed = true;
        }
    }

    fn show_plan_sections(&mut self, ui: &mut egui::Ui, plan: &MarkdownImportPlan) {
        ui.collapsing(format!("页面映射 · {} 项", plan.pages.len()), |ui| {
            let range = paged_range(plan.pages.len(), &mut self.page_offset, 30);
            show_pager(
                ui,
                plan.pages.len(),
                self.page_offset,
                30,
                &mut self.page_offset,
            );
            for page in &plan.pages[range] {
                ui.label(format!(
                    "{} → entity:{} · {} · {}",
                    page.source, page.id, page.title, page.entity_type
                ));
            }
        });
        ui.collapsing(
            format!("同名资料提示 · {} 项", plan.name_conflicts.len()),
            |ui| {
                let range = paged_range(
                    plan.name_conflicts.len(),
                    &mut self.name_conflict_offset,
                    30,
                );
                show_pager(
                    ui,
                    plan.name_conflicts.len(),
                    self.name_conflict_offset,
                    30,
                    &mut self.name_conflict_offset,
                );
                for conflict in &plan.name_conflicts[range] {
                    ui.label(format!(
                        "{}：{}；现有目标：{}；来源：{}",
                        conflict.title,
                        conflict.message,
                        conflict
                            .existing_targets
                            .iter()
                            .map(|target| format!("{}:{}", target.kind, target.id))
                            .collect::<Vec<_>>()
                            .join("、"),
                        conflict.sources.join("、")
                    ));
                }
            },
        );
        ui.collapsing(format!("来源链接 · {} 项", plan.links.len()), |ui| {
            let range = paged_range(plan.links.len(), &mut self.link_offset, 30);
            show_pager(
                ui,
                plan.links.len(),
                self.link_offset,
                30,
                &mut self.link_offset,
            );
            for link in &plan.links[range] {
                ui.label(format!(
                    "{}:{} · {} → {}:{} · {}",
                    link.source, link.line, link.href, link.target.kind, link.target.id, link.label
                ));
            }
        });
        ui.collapsing(
            format!("附件映射 · {} 项", plan.attachments.len()),
            |ui| {
                let range = paged_range(plan.attachments.len(), &mut self.attachment_offset, 30);
                show_pager(
                    ui,
                    plan.attachments.len(),
                    self.attachment_offset,
                    30,
                    &mut self.attachment_offset,
                );
                for attachment in &plan.attachments[range] {
                    ui.label(format!(
                        "{}:{} · {} → {} · {}",
                        attachment.source,
                        attachment.line,
                        attachment.href,
                        attachment.output_path,
                        attachment.alt
                    ));
                }
            },
        );
        ui.collapsing(format!("损失预览 · {} 项", plan.losses.len()), |ui| {
            let range = paged_range(plan.losses.len(), &mut self.loss_offset, 30);
            show_pager(
                ui,
                plan.losses.len(),
                self.loss_offset,
                30,
                &mut self.loss_offset,
            );
            for loss in &plan.losses[range] {
                ui.label(format!(
                    "{}:{} · {} · {}{}",
                    loss.source,
                    loss.line,
                    loss.code,
                    loss.message,
                    loss.preserved_at
                        .as_ref()
                        .map(|path| format!("；原文保留在 {path}"))
                        .unwrap_or_default()
                ));
            }
        });
        ui.collapsing(format!("阻塞冲突 · {} 项", plan.conflicts.len()), |ui| {
            let range = paged_range(plan.conflicts.len(), &mut self.conflict_offset, 20);
            show_pager(
                ui,
                plan.conflicts.len(),
                self.conflict_offset,
                20,
                &mut self.conflict_offset,
            );
            for conflict in &plan.conflicts[range] {
                ui.separator();
                ui.label(format!(
                    "{}{} · {}",
                    conflict
                        .source
                        .as_ref()
                        .map(|source| format!("{source} · "))
                        .unwrap_or_default(),
                    conflict.code,
                    conflict.message
                ));
                if let Some(source) = &conflict.source {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{source} 的目标 ID"));
                        let value = self
                            .id_overrides
                            .entry(source.clone())
                            .or_insert_with(|| conflict.preferred_id.clone().unwrap_or_default());
                        let changed = ui.add(
                            egui::TextEdit::singleline(value)
                                .hint_text("输入目标 ID")
                                .desired_width(180.0),
                        ).changed();
                        if changed {
                            self.stale = true;
                        }
                    });
                }
                if !conflict.candidates.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("可选 ID");
                        for candidate in &conflict.candidates {
                            if ui.small_button(candidate).clicked() {
                                if let Some(source) = &conflict.source {
                                    self.id_overrides
                                        .insert(source.clone(), candidate.clone());
                                    self.stale = true;
                                } else {
                                    self.namespace = candidate.clone();
                                }
                            }
                        }
                    });
                }
            }
            if !plan.conflicts.is_empty() {
                ui.label("修改映射或命名空间后，点击上方「预检导入」重新计算候选。冲突未解决时应用会保持禁用。");
            }
        });
        ui.collapsing(
            format!("写入文件预览 · {} 项", plan.files.len()),
            |ui| {
                let range = paged_range(plan.files.len(), &mut self.file_offset, 30);
                show_pager(
                    ui,
                    plan.files.len(),
                    self.file_offset,
                    30,
                    &mut self.file_offset,
                );
                for file in &plan.files[range] {
                    ui.label(format!(
                        "{} · {} · {} 字节{}",
                        file.kind,
                        file.path,
                        file.bytes,
                        file.source
                            .as_ref()
                            .map(|source| format!(" · 来源 {source}"))
                            .unwrap_or_default()
                    ));
                }
            },
        );
    }

    fn preview(&mut self, app: &WorldeditApp) {
        self.preview_generation = self.preview_generation.wrapping_add(1);
        self.plan = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.request = None;
        }
        self.error = None;
        self.stale = false;
        let source_root = PathBuf::from(self.source_root.trim());
        if self.source_root.trim().is_empty() {
            self.error = Some("请选择 Markdown 来源目录。".into());
            return;
        }
        let target = match self.target_project(app, &source_root) {
            Ok(project) => project,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let options = MarkdownImportOptions {
            expected_baseline: target.content_baseline(),
            id_overrides: self.id_overrides.clone(),
            namespace: (!self.namespace.trim().is_empty()).then(|| self.namespace.trim().into()),
            accept_losses: self.accept_losses,
            allow_language_upgrade: self.allow_language_upgrade,
        };
        #[cfg(not(target_arch = "wasm32"))]
        let result = if let Some(files) = &self.source_files {
            target.preview_markdown_import_snapshot(
                &MarkdownImportSourceSnapshot {
                    label: self.source_root.trim(),
                    files,
                },
                &options,
            )
        } else {
            let request = MarkdownImportRequest {
                source_root,
                expected_baseline: options.expected_baseline.clone(),
                id_overrides: options.id_overrides.clone(),
                namespace: options.namespace.clone(),
                accept_losses: options.accept_losses,
                allow_language_upgrade: options.allow_language_upgrade,
            };
            let result = target.preview_markdown_import(&request);
            if result.is_ok() {
                self.request = Some(request);
            }
            result
        };
        #[cfg(target_arch = "wasm32")]
        let result = match self.source_files.as_ref() {
            Some(files) => target.preview_markdown_import_snapshot(
                &MarkdownImportSourceSnapshot {
                    label: self.source_root.trim(),
                    files,
                },
                &options,
            ),
            None => Err("请选择 Markdown 来源文件夹。".into()),
        };
        match result {
            Ok(plan) => {
                if self.namespace.trim().is_empty() {
                    self.namespace = plan.namespace.clone();
                }
                self.plan = Some(plan);
                self.page_offset = 0;
                self.link_offset = 0;
                self.attachment_offset = 0;
                self.loss_offset = 0;
                self.conflict_offset = 0;
                self.name_conflict_offset = 0;
                self.file_offset = 0;
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn apply(&mut self, app: &mut WorldeditApp) {
        let Some(plan) = self.plan.as_ref() else {
            self.error = Some("请先完成预检。".into());
            return;
        };
        let digest = plan.plan_digest.clone();
        let source_root = PathBuf::from(self.source_root.trim());
        let target = match self.target_project(app, &source_root) {
            Ok(project) => project,
            Err(error) => {
                self.error = Some(error);
                self.stale = true;
                return;
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        let mut target = target;
        if let Some(files) = self.source_files.as_ref() {
            let options = MarkdownImportOptions {
                expected_baseline: target.content_baseline(),
                id_overrides: self.id_overrides.clone(),
                namespace: (!self.namespace.trim().is_empty())
                    .then(|| self.namespace.trim().into()),
                accept_losses: self.accept_losses,
                allow_language_upgrade: self.allow_language_upgrade,
            };
            let source = MarkdownImportSourceSnapshot {
                label: self.source_root.trim(),
                files,
            };
            match target.apply_markdown_import_snapshot(&source, &options, &digest) {
                Ok(snapshot) => {
                    #[cfg(target_arch = "wasm32")]
                    {
                        let message = import_success_message(&snapshot.result.plan);
                        match app.browser_apply_markdown_import(snapshot.workspace_files, message) {
                            Ok(()) => self.closed = true,
                            Err(error) => {
                                self.error = Some(error);
                                self.stale = true;
                            }
                        }
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let _ = snapshot;
                        self.error = Some("Files 快照应用只能由浏览器宿主接收。".into());
                    }
                }
                Err(error) => {
                    self.error = Some(error);
                    self.stale = true;
                }
            }
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(mut request) = self.request.clone() else {
                self.error = Some("请重新预检来源目录。".into());
                self.stale = true;
                return;
            };
            request.accept_losses = self.accept_losses;
            request.allow_language_upgrade = self.allow_language_upgrade;
            match target.apply_markdown_import(&request, &digest) {
                Ok(result) => {
                    let counts = import_success_message(&result.plan);
                    match self.target_mode {
                        TargetMode::NewProject => {
                            app.project = target;
                            app.active_file = app.project.entry.clone();
                            app.saved_location = true;
                            app.reset_views();
                            app.recompile();
                            app.tab = super::Tab::Timeline;
                        }
                        TargetMode::CurrentProject => {
                            app.project = target;
                            app.active_file = app.project.entry.clone();
                            app.history.clear();
                            app.redo.clear();
                            app.recompile();
                        }
                    }
                    app.io_error = None;
                    app.message = Some(counts);
                    self.closed = true;
                }
                Err(error) => {
                    self.error = Some(error);
                    self.stale = true;
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.error = Some("请重新选择 Markdown 来源文件夹。".into());
            self.stale = true;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn target_project(&self, app: &WorldeditApp, source_root: &Path) -> Result<Project, String> {
        match self.target_mode {
            TargetMode::CurrentProject => {
                if !app.saved_location {
                    return Err(
                        "当前工程尚未保存到工作区；请先另存工程，或选择导入到新工程。".into(),
                    );
                }
                if app.project.is_dirty() || app.has_open_authoring_form() {
                    return Err("当前工程有未保存的草稿；先保存或完成草稿，再开始迁移。".into());
                }
                if self.source_files.is_none() {
                    ensure_disjoint_roots(source_root, &app.project.root)?;
                }
                Ok(app.project.clone())
            }
            TargetMode::NewProject => {
                let root = PathBuf::from(self.target_root.trim());
                if self.target_root.trim().is_empty() {
                    return Err("请选择一个已存在的空工程目录。".into());
                }
                let entries = std::fs::read_dir(&root)
                    .map_err(|error| format!("无法读取新工程目录：{error}"))?;
                if entries.into_iter().next().is_some() {
                    return Err("新工程目录必须为空；预检和取消不会创建或覆盖文件。".into());
                }
                if self.source_files.is_none() {
                    ensure_disjoint_roots(source_root, &root)?;
                }
                blank_project(&root)
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn target_project(&self, app: &WorldeditApp, _source_root: &Path) -> Result<Project, String> {
        match self.target_mode {
            TargetMode::CurrentProject => {
                if !app.saved_location {
                    return Err(
                        "当前工程尚未保存到浏览器存档；请先保存，或选择新浏览器工程。".into(),
                    );
                }
                if app.project.is_dirty() || app.has_open_authoring_form() {
                    return Err("当前工程有未保存的草稿；先保存或完成草稿，再开始迁移。".into());
                }
                Ok(app.project.clone())
            }
            TargetMode::NewProject => {
                if app.saved_location && (app.project.is_dirty() || app.has_open_authoring_form()) {
                    return Err("当前浏览器工程有未保存的内容；请先保存再导入新工程。".into());
                }
                blank_project(&app.project.root)
            }
        }
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn set_source_files(&mut self, files: Files) {
        self.source_root = "浏览器所选文件夹".into();
        self.source_files = Some(files);
        self.invalidate_preview();
    }

    fn invalidate_preview(&mut self) {
        self.plan = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.request = None;
        }
        self.error = None;
        self.stale = false;
    }
}

fn blank_project(root: &Path) -> Result<Project, String> {
    let mut project = Project::new(root);
    let entry = project.entry.clone();
    project.documents.retain(|path, _| path == &entry);
    project.set_text(
        &entry,
        "world markdown_import as \"Markdown import\"\nevent start as \"Start\"\n  -> END\n".into(),
    )?;
    Ok(project)
}

#[cfg(not(target_arch = "wasm32"))]
fn ensure_disjoint_roots(source: &Path, target: &Path) -> Result<(), String> {
    let source = source
        .canonicalize()
        .map_err(|error| format!("无法解析 Markdown 来源目录：{error}"))?;
    let target = target
        .canonicalize()
        .map_err(|error| format!("无法解析目标工程目录：{error}"))?;
    if source.starts_with(&target) || target.starts_with(&source) {
        return Err("来源目录与目标工程目录不能相同或互相嵌套。".into());
    }
    Ok(())
}

fn import_success_message(plan: &MarkdownImportPlan) -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        format!(
            "Markdown 导入已保存：{} 个页面、{} 个链接、{} 个附件，{} 项损失已确认。",
            plan.pages.len(),
            plan.links.len(),
            plan.attachments.len(),
            plan.losses.len()
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        format!(
            "Markdown 导入已应用到浏览器工作区：{} 个页面、{} 个链接、{} 个附件，{} 项损失已确认；点击「保存全部」写入浏览器存档。",
            plan.pages.len(),
            plan.links.len(),
            plan.attachments.len(),
            plan.losses.len()
        )
    }
}

fn paged_range(total: usize, offset: &mut usize, page_size: usize) -> Range<usize> {
    if total == 0 {
        *offset = 0;
        return 0..0;
    }
    *offset = (*offset / page_size) * page_size;
    *offset = (*offset).min((total - 1) / page_size * page_size);
    *offset..(*offset + page_size).min(total)
}

fn show_pager(
    ui: &mut egui::Ui,
    total: usize,
    offset: usize,
    page_size: usize,
    next_offset: &mut usize,
) {
    if total <= page_size {
        return;
    }
    ui.horizontal(|ui| {
        let end = (offset + page_size).min(total);
        ui.label(format!("显示 {}–{} / {total}", offset + 1, end));
        if ui
            .add_enabled(offset > 0, egui::Button::new("上一页"))
            .clicked()
        {
            *next_offset = offset.saturating_sub(page_size);
        }
        if ui
            .add_enabled(end < total, egui::Button::new("下一页"))
            .clicked()
        {
            *next_offset = end;
        }
    });
}

impl WorldeditApp {
    pub(super) fn markdown_import_window(&mut self, ctx: &egui::Context) {
        let Some(mut wizard) = self.markdown_import_wizard.take() else {
            return;
        };
        if wizard.show(ctx, self) {
            self.markdown_import_wizard = Some(wizard);
        }
    }
}
