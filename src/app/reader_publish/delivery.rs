use super::super::WorldeditApp;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

impl WorldeditApp {
    pub(super) fn publish_reader_package(&mut self, _ctx: &egui::Context) {
        let Some(reviewed) = self.reader_publish.reviewed.as_ref() else {
            self.reader_publish.status = Some("请先生成并核对预览。".into());
            return;
        };
        if !self.reader_publish.confirmed {
            self.reader_publish.status = Some("发布前必须明确确认预览范围。".into());
            return;
        }
        if !self
            .reader_publish
            .matches_review(reviewed, &self.project.content_baseline())
        {
            self.reader_publish.invalidate_review();
            self.reader_publish.status =
                Some("审核已过期；内容或选择变化后请重新审核并确认。".into());
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.start_native_reader_delivery();
        #[cfg(target_arch = "wasm32")]
        {
            self.start_web_reader_job(
                reviewed.selection.clone(),
                Some(reviewed.preview.plan_digest.clone()),
                _ctx,
            );
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
