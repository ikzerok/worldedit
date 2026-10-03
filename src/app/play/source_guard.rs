//! 条件与路线动作共用的只读来源守卫；导航不得绕过作者草稿或工作区边界。
use super::super::WorldeditApp;
use super::scope::AppliedPlayScope;
use std::path::Path;
use worldline_core::evidence_source::{resolve_evidence_source, EvidenceSource};
use worldline_core::search_replace::SearchMatch;

impl WorldeditApp {
    pub(super) fn play_source_guard(&self, scope: &AppliedPlayScope) -> Result<(), String> {
        if !scope.matches_project(&self.project, self.version) {
            return Err("实际运行快照已过期或来源内容已变化，请重新运行后定位".into());
        }
        let snapshot = self.snapshot.as_ref().ok_or("当前没有可确认的编译来源")?;
        if snapshot.result.sources != scope.sources {
            return Err("当前编译来源不再是此次实际运行的源码集合".into());
        }
        if self.ime_composing || self.command_palette.ime || self.command_palette.ime_frame {
            return Err("请先完成输入法组合；当前输入已保留".into());
        }
        if !self.unapplied_play_inputs().is_empty() {
            return Err("存在未应用执行草稿；请先应用或撤销这些输入，再重新运行后定位".into());
        }
        if !self.project.recovery_conflicts().is_empty() {
            return Err("工作区仍有恢复冲突，无法确认来源".into());
        }
        Ok(())
    }

    pub(super) fn play_source_hit(
        &self,
        scope: &AppliedPlayScope,
        source: &EvidenceSource,
    ) -> Result<SearchMatch, String> {
        let snapshot = self.snapshot.as_ref().ok_or("编译来源不存在")?;
        let target = resolve_evidence_source(&snapshot.result, source)?;
        let path = worldline_core::file_access::within(&self.project.root, Path::new(&source.file))?;
        if path != target.path {
            return Err("证据文件身份已变化，无法确认来源".into());
        }
        let text = self.project.document(&path)?;
        if !scope.source_matches(&path, text) {
            return Err("证据来源内容与实际运行基线不符".into());
        }
        let preview = text.get(target.range.clone()).ok_or("声明范围已变化")?.into();
        Ok(SearchMatch {
            path,
            range: target.range,
            line: target.line,
            column: target.column,
            preview,
            replaceable: false,
            draft: false,
        })
    }
}
