//! 只适配浏览器工程 I/O，所有视图与编辑操作仍使用父模块。
use super::{Pending, WorldeditApp};
use crate::{
    archive,
    web::{self, FileAction, Files},
};
use std::path::Path;

impl WorldeditApp {
    pub(crate) fn restore_browser_save(&mut self) {
        match web::restore() {
            Ok(Some(files)) => {
                self.browser_open(files);
                if self.io_error.is_none() {
                    self.message = Some("已恢复上次保存的浏览器工程".into());
                }
            }
            Ok(None) => {}
            Err(e) => self.io_error = Some(e),
        }
    }

    pub(super) fn browser_open(&mut self, mut files: Files) {
        let result = (|| {
            if files.len() == 1 {
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
                Ok(project) => Ok(project),
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
            }
        }
    }

    fn browser_save(&mut self) -> Result<(), String> {
        let mut files = web::imported();
        for (path, text) in self.project.sources() {
            let relative = path
                .strip_prefix(&self.project.root)
                .map_err(|_| "引用文件不在工程目录内")?;
            files.insert(relative.to_path_buf(), text.into_bytes());
        }
        let entry = self
            .project
            .entry
            .strip_prefix(&self.project.root)
            .map_err(|_| "总入口不在工程目录内")?
            .to_string_lossy();
        files.insert(
            archive::MANIFEST.into(),
            serde_json::to_vec(&serde_json::json!({"entry": entry})).map_err(|e| e.to_string())?,
        );
        let bytes = archive::encode(&files)?;
        web::download("worldedit-project.zip", &bytes, "application/zip")?;
        web::persist(&bytes)?;
        web::mount(files);
        self.project.mark_saved();
        self.saved_location = true;
        self.recompile();
        self.io_error = None;
        self.message = Some("已保存到本浏览器，并请求下载完整工程包".into());
        Ok(())
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
            .project
            .export_files()
            .and_then(|files| archive::encode(&files))
            .and_then(|bytes| web::download("worldedit-export.zip", &bytes, "application/zip"));
        match result {
            Ok(()) => {
                self.io_error = None;
                self.message = Some("完整世界工程已请求下载（保留工作区全部文件）".into());
            }
            Err(e) => self.io_error = Some(e),
        }
    }
}
