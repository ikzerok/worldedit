use crate::app::WorldeditApp;
use worldline_core::manuscript::ManuscriptQueryDraft;

impl WorldeditApp {
    /// Workbench必须先放回LocalBook，交付守卫才能核对完整编排草稿。
    pub(in crate::app) fn finish_manuscript_delivery_actions(&mut self, ctx: &egui::Context) {
        match self.manuscript.preview_cache.delivery.action.take() {
            Some(super::delivery::Action::Copy) => self.copy_manuscript_markdown(ctx),
            Some(super::delivery::Action::Save) => self.save_manuscript_markdown(ctx),
            #[cfg(not(target_arch = "wasm32"))]
            Some(super::delivery::Action::Browse) => self.choose_manuscript_markdown_destination(),
            None => {}
        }
    }
    pub(super) fn checked_manuscript_markdown(&self, ctx: &egui::Context) -> Result<&str, String> {
        if let Some(error) = self.review_input_blocker(ctx) {
            return Err(error);
        }
        let state = &self.manuscript.preview_cache.delivery;
        if state.job.is_some() || !self.manuscript_delivery_is_current() {
            return Err("审稿已过期或仍在生成；请重新生成同范围材料".into());
        }
        if !state.confirmed {
            return Err("请先确认作者私密材料的范围和完整Markdown预览".into());
        }
        let report = state.reviewed.as_ref().ok_or("请先生成同范围审稿")?;
        self.project.verify_review_navigation()?;
        let id = &report.scope().request.query.manuscript_id;
        let drafts: Vec<_> = self
            .manuscript
            .books
            .get(id)
            .filter(|book| book.changed)
            .map(|book| ManuscriptQueryDraft {
                expected_baseline: book.baseline.clone(),
                draft: book.draft.clone(),
            })
            .into_iter()
            .collect();
        self.project
            .validate_manuscript_delivery(&self.manuscript.writing_buffers(), &drafts, report)
            .map_err(|error| error.to_string())?;
        report
            .markdown()
            .ok_or_else(|| "范围或章节不完整，没有可交付Markdown".into())
    }

    pub(in crate::app) fn copy_manuscript_markdown(&mut self, ctx: &egui::Context) {
        let result = self
            .checked_manuscript_markdown(ctx)
            .map(|text| ctx.copy_text(text.to_owned()));
        self.manuscript.preview_cache.delivery.notice = Some(match result {
            Ok(()) => "已复制与预览逐字相同的作者Markdown；请谨慎选择接收者".into(),
            Err(error) => error,
        });
    }

    pub(in crate::app) fn save_manuscript_markdown(&mut self, ctx: &egui::Context) {
        let result = self.checked_manuscript_markdown(ctx).and_then(|markdown| {
            #[cfg(target_arch = "wasm32")]
            {
                crate::web::download(
                    "worldedit-manuscript-review.md",
                    markdown.as_bytes(),
                    "text/markdown;charset=utf-8",
                )
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = markdown;
                let state = &self.manuscript.preview_cache.delivery;
                worldline_core::manuscript::write_manuscript_markdown_new(
                    &self.project.root,
                    std::path::Path::new(state.destination.trim()),
                    state.reviewed.as_ref().expect("交付已核对"),
                    &mut || self.checked_manuscript_markdown(ctx).map(|_| ()),
                )
            }
        });
        self.manuscript.preview_cache.delivery.notice = Some(match result {
            #[cfg(not(target_arch = "wasm32"))]
            Ok(()) => "已原子保存新Markdown文件；内容与预览完全相同，工程保存状态未改变".into(),
            #[cfg(target_arch = "wasm32")]
            Ok(()) => "已请求浏览器下载与预览相同的Markdown；无法确认是否已落盘".into(),
            Err(error) => error,
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn choose_manuscript_markdown_destination(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("manuscript-review.md")
            .add_filter("作者Markdown审稿本", &["md"])
            .save_file()
        {
            self.manuscript.preview_cache.delivery.destination =
                path.to_string_lossy().into_owned();
        }
    }
}
