//! 只适配浏览器工程 I/O，所有视图与编辑操作仍使用父模块。
use super::{Pending, WorldeditApp};
use crate::{
    archive,
    web::{self, FileAction, Files},
};
use std::path::Path;

struct BrowserSaveHost {
    checkpoint_session_id: String,
}

impl crate::save_flow::SaveHost for BrowserSaveHost {
    fn request_download(&mut self, name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
        web::download(name, bytes, mime)
    }

    fn persist(&mut self, bytes: &[u8]) -> Result<(), String> {
        web::persist(bytes, &self.checkpoint_session_id)
    }

    fn record_export_revision(&mut self, revision: u64) {
        web::record_export_revision(revision);
    }

    fn record_local_snapshot_revision(&mut self, revision: u64) {
        web::record_local_snapshot_revision(revision);
    }
}

impl WorldeditApp {
    pub(crate) fn restore_browser_save(&mut self) {
        match web::restore() {
            Ok(Some((files, checkpoint_session_id))) => {
                self.browser_open_with_mode(files, false, checkpoint_session_id);
                if self.io_error.is_none() {
                    self.browser_pending_save = false;
                    web::record_local_snapshot_revision(self.version);
                    self.message = Some("已恢复上次保存的浏览器工程".into());
                }
            }
            Ok(None) => {}
            Err(e) => self.io_error = Some(e),
        }
    }

    pub(super) fn browser_open(&mut self, files: Files) {
        self.browser_open_with_mode(files, true, None);
        if self.io_error.is_none() {
            self.browser_pending_save = true;
        }
    }

    pub(super) fn browser_apply_markdown_import(
        &mut self,
        files: Files,
        message: String,
    ) -> Result<(), String> {
        self.io_error = None;
        let checkpoint_session_id = self.project.checkpoint_session_id().to_owned();
        self.browser_open_with_mode(files, false, Some(checkpoint_session_id));
        if let Some(error) = self.io_error.take() {
            Err(error)
        } else {
            self.browser_pending_save = true;
            self.message = Some(message);
            Ok(())
        }
    }

    fn browser_open_with_mode(
        &mut self,
        mut files: Files,
        preflight: bool,
        checkpoint_session_id: Option<String>,
    ) {
        let result = (|| {
            if preflight {
                files = archive::prepare_import(files)?;
            } else if files.len() == 1 {
                let (name, bytes) = files.first_key_value().unwrap();
                if name
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
                {
                    files = archive::decode(bytes)?;
                }
            }
            let entry = archive::entry(&files)?;
            let previous = web::imported();
            web::mount(files);
            match worldline_core::project::Project::open(&Path::new("/world").join(entry)) {
                Ok(mut project) => {
                    if let Some(checkpoint_session_id) = checkpoint_session_id.as_deref() {
                        if let Err(error) = project.set_checkpoint_session_id(checkpoint_session_id)
                        {
                            web::mount(previous);
                            return Err(error);
                        }
                    }
                    Ok(project)
                }
                Err(error) => {
                    web::mount(previous);
                    Err(error)
                }
            }
        })();
        match result {
            Ok(project) => {
                self.project = project;
                self.active_file = self.project.entry.clone();
                self.saved_location = true;
                self.browser_pending_save = true;
                self.reset_views();
                self.recompile();
                self.message = Some("工程已载入浏览器；保存全部可下载工程包".into());
            }
            Err(e) => self.io_error = Some(e),
        }
    }

    pub(super) fn open_dialog(&mut self, ctx: &egui::Context, folder: bool) {
        web::select_files(
            ctx,
            folder,
            if folder { "" } else { ".wl,.zip" },
            FileAction::Open,
        );
    }

    pub(super) fn browser_events(&mut self, ctx: &egui::Context) {
        while let Some((action, result)) = web::take_event() {
            let files = match result {
                Ok(files) if files.is_empty() => continue,
                Ok(files) => files,
                Err(e) => {
                    self.io_error = Some(e);
                    continue;
                }
            };
            if matches!(&action, FileAction::MarkdownImport) {
                if let Some(wizard) = self.markdown_import_wizard.as_mut() {
                    wizard.set_source_files(files);
                }
                continue;
            }
            if matches!(action, FileAction::Open) {
                self.request_action(Pending::BrowserOpen(files), ctx);
                continue;
            }
            let paths = match web::add_files(files, !matches!(action, FileAction::Include)) {
                Ok(paths) => paths,
                Err(e) => {
                    self.io_error = Some(e);
                    continue;
                }
            };
            match action {
                FileAction::Include => {
                    self.commit("文件已引用", |p| {
                        for path in paths {
                            p.include_file(&path)?;
                        }
                        Ok(())
                    });
                }
                FileAction::Attach(target) => {
                    self.commit("文件已关联到对象", |p| {
                        for path in paths {
                            p.add_asset_reference(&target, &path)?;
                        }
                        Ok(())
                    });
                }
                FileAction::Replace(mut draft) => {
                    if let Some(path) = paths.first() {
                        // 素材声明可能在子目录，用原始声明位置计算相对路径。
                        let result = self.project.compile();
                        if let Some(asset) = result.analysis.catalog.assets.get(&draft.id) {
                            let parent = Path::new(&asset.file).parent().unwrap();
                            let depth = parent
                                .strip_prefix(&self.project.root)
                                .unwrap()
                                .components()
                                .count();
                            draft.path = format!(
                                "{}{}",
                                "../".repeat(depth),
                                path.strip_prefix(&self.project.root)
                                    .unwrap()
                                    .to_string_lossy()
                            );
                            self.commit("素材引用已更新", |p| p.write_asset(&draft));
                        }
                    }
                }
                FileAction::Open => unreachable!(),
                FileAction::MarkdownImport => unreachable!(),
            }
        }
    }

    fn browser_save(&mut self) -> Result<(), String> {
        let revision = self.version;
        let files = self.browser_package()?;
        let bytes = archive::encode(&files)?;
        let mut host = BrowserSaveHost {
            checkpoint_session_id: self.project.checkpoint_session_id().to_owned(),
        };
        let result = crate::save_flow::save_project_package(&mut host, revision, &bytes, || {
            web::mount(files);
            self.project.mark_saved();
            self.saved_location = true;
            self.browser_pending_save = false;
            self.io_error = None;
            self.message = Some("已保存到本浏览器，并请求下载完整工程包".into());
        });
        if result.is_ok() {
            let revisions = web::snapshot_revisions();
            debug_assert_eq!(revisions.last_export_revision, Some(revision));
            debug_assert_eq!(revisions.local_snapshot_revision, Some(revision));
        }
        result
    }

    /// 由 core 生成当前缓冲的完整原始工程快照；浏览器层只补充旧版入口记录。
    fn browser_package(&self) -> Result<Files, String> {
        let files = worldline_core::workspace_snapshot::snapshot_files(&self.project)?;
        self.with_browser_manifest(files)
    }

    /// 显式导出沿用 core 的严格可编译导出语义，不能被草稿保存替代。
    fn browser_export_package(&self) -> Result<Files, String> {
        let files: Files = self.project.export_files()?.into_iter().collect();
        self.with_browser_manifest(files)
    }

    fn with_browser_manifest(&self, mut files: Files) -> Result<Files, String> {
        let entry = self
            .project
            .entry
            .strip_prefix(&self.project.root)
            .map_err(|_| "总入口不在工程目录内")?
            .to_string_lossy()
            .replace('\\', "/");
        archive::ensure_legacy_manifest(&mut files, &entry)?;
        // Fail before download/local persistence if the new and legacy entry
        // records cannot reopen the same package.
        archive::entry(&files)?;
        Ok(files)
    }

    pub(super) fn save(&mut self) -> bool {
        match self.browser_save() {
            Ok(()) => true,
            Err(e) => {
                self.io_error = Some(e);
                false
            }
        }
    }

    pub(super) fn directory_dialog(&mut self, export: bool) {
        if !export {
            self.save();
            return;
        }
        let result = self
            .browser_export_package()
            .and_then(|files| archive::encode(&files))
            .and_then(|bytes| web::download("worldedit-export.zip", &bytes, "application/zip"));
        match result {
            Ok(()) => {
                web::record_export_revision(self.version);
                self.io_error = None;
                self.message = Some("完整世界工程已请求下载（保留工作区全部文件）".into());
            }
            Err(e) => self.io_error = Some(e),
        }
    }
}
