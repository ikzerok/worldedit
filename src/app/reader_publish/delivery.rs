use super::super::WorldeditApp;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

impl WorldeditApp {
    pub(super) fn publish_reader_package(&mut self) {
        let Some(reviewed) = self.reader_publish.reviewed.as_ref() else {
            self.reader_publish.status = Some("请先生成并核对预览。".into());
            return;
        };
        if !self.reader_publish.confirmed {
            self.reader_publish.status = Some("发布前必须明确确认预览范围。".into());
            return;
        }
        let selection = reviewed.selection.clone();
        let expected = reviewed.preview.plan_digest.clone();
        let zip = reviewed.zip.clone();
        let fresh = match self.project.preview_reader_export(&selection) {
            Ok(preview) => preview,
            Err(error) => {
                self.reader_publish.status = Some(format!("发布前复核失败：{error}"));
                self.reader_publish.confirmed = false;
                return;
            }
        };
        if fresh.plan_digest != expected {
            self.reader_publish.invalidate_review();
            self.reader_publish.status = Some("预览已过期；内容变化后请重新预览并确认。".into());
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let target = PathBuf::from(self.reader_publish.destination.trim());
            if target
                .extension()
                .is_none_or(|extension| !extension.eq_ignore_ascii_case("zip"))
            {
                self.reader_publish.status = Some("目标文件扩展名必须为 .zip。".into());
                return;
            }
            match super::super::package::write_package_file(&self.project.root, &target, &zip) {
                Ok(()) => {
                    self.reader_publish.status = Some(format!(
                        "阅读包已写入 {}（{} B；工程保存状态未改变）。",
                        target.display(),
                        zip.len()
                    ));
                    self.message = Some("静态阅读 ZIP 已发布；作者工程未改变".into());
                }
                Err(error) => self.reader_publish.status = Some(format!("发布失败：{error}")),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            match crate::web::download("worldedit-reader-site.zip", &zip, "application/zip") {
                Ok(()) => {
                    self.reader_publish.status = Some(
                        "浏览器下载已启动；文件名 worldedit-reader-site.zip。工程保存状态未改变。"
                            .into(),
                    );
                    self.message = Some("静态阅读 ZIP 下载已启动；作者工程未改变".into());
                }
                Err(error) => self.reader_publish.status = Some(format!("下载失败：{error}")),
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn choose_reader_package_destination(&mut self) {
        let initial = PathBuf::from(self.reader_publish.destination.trim());
        let parent = initial
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| self.project.root.parent().unwrap_or(&self.project.root));
        let file_name = initial
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("reader-site.zip");
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(parent)
            .set_file_name(file_name)
            .add_filter("离线阅读 ZIP", &["zip"])
            .save_file()
        {
            self.reader_publish.destination = path.to_string_lossy().into_owned();
            self.reader_publish.confirmed = false;
        }
    }
}
