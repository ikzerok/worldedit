use super::*;
use std::path::{Path, PathBuf};
use worldline_core::markdown_import::{MarkdownImportOptions, MarkdownImportSourceSnapshot};
use worldline_core::project::Project;

impl Wizard {
    pub(super) fn preview(&mut self, app: &WorldeditApp) {
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
    pub(super) fn apply(&mut self, app: &mut WorldeditApp) {
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
                            app.tab = super::super::Tab::Timeline;
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
