use super::*;
use crate::draft_rehearsal_worker::SourceHit;
use worldline_core::search_replace::SearchMatch;

impl WorldeditApp {
    pub(super) fn return_rehearsal_origin(&mut self, ctx: &egui::Context) {
        let result = (|| {
            let running = self
                .draft_rehearsal
                .running
                .as_ref()
                .ok_or("没有可返回的草稿试演")?;
            self.rehearsal_guard(&running.input, &running.key, true)?;
            if running.origin.tab == Some(crate::app::Tab::Play) {
                return Err(
                    "没有可确认的初始作者位置；可从本次证据定位来源，或回书稿继续原稿".into(),
                );
            }
            if running.origin.tab == Some(crate::app::Tab::Manuscript) {
                let session = serde_json::from_value::<crate::app::manuscript::ManuscriptSession>(
                    running.origin.manuscript.clone(),
                )
                .map_err(|_| "原稿位置不可读取")?;
                if !self.manuscript.validate_session(&self.project, &session)? {
                    return Err("原章节、光标或草稿代次不再一致；未按相似位置替代".into());
                }
            }
            Ok(running.origin.clone())
        })();
        match result {
            Ok(origin) => {
                if let Some(running) = &mut self.draft_rehearsal.running {
                    running.paused = true;
                }
                self.restore_author_location(origin, ctx);
                self.message =
                    Some("已返回同一份未应用正文与作者位置；应用正文仍需另行明确执行".into());
            }
            Err(error) => self.draft_rehearsal.notice = Some(error),
        }
    }

    pub(super) fn return_rehearsal_source(&mut self, ctx: &egui::Context, source: SourceHit) {
        let result = (|| {
            let running = self.draft_rehearsal.running.as_ref().ok_or("试演已关闭")?;
            self.rehearsal_guard(&running.input, &running.key, true)?;
            // 路径身份由core编译范围提供；先要求逐字匹配，再用core核对工作区边界。
            // 不借用仅浏览器编译的ZIP路径函数，也不另造一套源码路径语义。
            if !running
                .view
                .scope
                .sources
                .iter()
                .any(|item| item.path == source.path)
            {
                return Err("返回来源不属于这份试演的输入范围".to_owned());
            }
            let path = worldline_core::file_access::within(
                &self.project.root,
                &self.project.root.join(&source.path),
            )?;
            let draft = running
                .input
                .drafts
                .iter()
                .find(|draft| draft.path == source.path);
            if draft.is_some() != source.draft {
                return Err("返回来源的草稿/已应用身份不一致".into());
            }
            let text = match draft {
                Some(draft) => draft.source.as_str(),
                None => self.project.document(&path)?,
            };
            if text.get(source.range.clone()) != Some(source.preview.as_str()) {
                return Err("返回来源的完整文字范围不一致，证据只读保留".into());
            }
            Ok(SearchMatch {
                path,
                range: source.range,
                line: source.line,
                column: source.column,
                preview: source.preview,
                context: None,
                identity: None,
                replaceable: false,
                draft: source.draft,
            })
        })();
        match result.and_then(|hit| self.go_author_source_position(ctx, &hit, true)) {
            Ok(()) => {
                if let Some(running) = &mut self.draft_rehearsal.running {
                    running.paused = true;
                }
                self.message = Some(
                    "已定位本次草稿试演的真实声明头；未应用或保存正文。Alt+Left 返回试演".into(),
                );
            }
            Err(error) => self.draft_rehearsal.notice = Some(error),
        }
    }
}

impl WorldeditApp {
    pub(in crate::app::play) fn verify_rehearsal_localization_navigation(&self) -> bool {
        self.draft_rehearsal
            .running
            .as_ref()
            .is_some_and(|running| {
                self.rehearsal_guard(&running.input, &running.key, true)
                    .is_ok()
            })
    }
    pub(in crate::app::play) fn return_rehearsal_localization_source(
        &mut self,
        _ctx: &egui::Context,
        source: &worldline_core::localization::LocalizationSource,
    ) {
        self.send_rehearsal_action(crate::draft_rehearsal_worker::Action::LocalizationSource {
            source: source.clone(),
        });
    }
}
