use super::SourceRequest;
use crate::app::WorldeditApp;
use worldline_core::search_replace::SearchMatch;

impl WorldeditApp {
    pub(in crate::app::play) fn inspection_source_hit(
        &self,
        request: &SourceRequest,
    ) -> Result<SearchMatch, String> {
        let play = self.play.as_ref().ok_or("当前没有真实运行")?;
        self.play_source_navigation_guard(&play.scope)?;
        let story = play.story.as_ref().ok_or("当前没有真实Story")?;
        if story.inspection_stamp() != request.stamp {
            return Err("运行观察已变化，请重新查询".into());
        }
        let page = self
            .replay_debugger
            .inspection
            .page
            .as_ref()
            .filter(|page| page.stamp == request.stamp)
            .ok_or("检查页已失效，请重新查询")?;
        // Exact key and source membership, including local call identity; no textual stand-in.
        let actual = page
            .items
            .iter()
            .find(|item| item.key == request.key)
            .ok_or("检查项已失效，请重新查询")?;
        if actual.source.as_ref() != Some(&request.source) {
            return Err("声明来源不属于本次检查项".into());
        }
        let snapshot = self.snapshot.as_ref().ok_or("当前编译来源不可用")?;
        let target = worldline_core::state_inspection_source::resolve_state_inspection_source(
            &snapshot.result,
            &request.source,
        )?;
        let path = worldline_core::file_access::within(
            &self.project.root,
            std::path::Path::new(&request.source.file),
        )?;
        if path != target.path {
            return Err("声明文件身份已变化".into());
        }
        let text = self.project.document(&path)?;
        if !play.scope.source_matches(&path, text) {
            return Err("声明来源已不是实际运行稿".into());
        }
        self.project.verify_source_navigation(&path, text)?;
        let preview = text
            .get(target.range.clone())
            .ok_or("声明范围已变化")?
            .into();
        Ok(SearchMatch {
            path,
            range: target.range,
            line: target.line,
            column: target.column,
            preview,
            context: None,
            identity: None,
            replaceable: false,
            draft: false,
        })
    }
    pub(in crate::app::play) fn jump_to_inspection_source(
        &mut self,
        ctx: &egui::Context,
        request: &SourceRequest,
    ) {
        let result = self
            .inspection_source_hit(request)
            .and_then(|hit| self.go_author_source_position(ctx, &hit, true));
        match result {
            Ok(()) => {
                self.replay_debugger.notice = None;
                self.message =
                    Some("已定位真实声明头；这不是最后写入。Alt+Left 返回同一试玩检查位置".into());
            }
            Err(reason) => {
                self.replay_debugger.inspection.error = Some(reason);
            }
        }
    }
}
