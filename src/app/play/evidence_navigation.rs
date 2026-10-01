//! 将当次缓存证据接回作者导航；不求值、不重编译、不应用草稿。
use super::super::WorldeditApp;
use std::path::Path;
use worldline_core::evidence_source::{resolve_evidence_source, EvidenceSource};
use worldline_core::search_replace::SearchMatch;
use worldline_runtime::ChoiceExplanation;

#[derive(Default)]
pub(super) struct EvidenceNavigationAccess {
    pub reason: Option<String>,
    targets: Vec<(EvidenceSource, Result<SearchMatch, String>)>,
}

impl EvidenceNavigationAccess {
    pub(super) fn source_reason(&self, source: Option<&EvidenceSource>) -> Option<&str> {
        if let Some(reason) = &self.reason {
            return Some(reason);
        }
        let Some(source) = source else {
            return Some("此证据没有可确认的作者来源");
        };
        match self
            .targets
            .iter()
            .find(|(candidate, _)| candidate == source)
        {
            Some((_, Ok(_))) => None,
            Some((_, Err(reason))) => Some(reason),
            None => Some("来源已不属于当前证据，请重新展开本次条件"),
        }
    }
}

pub(super) struct EvidenceNavigationRequest {
    pub source: EvidenceSource,
    pub cached: Vec<ChoiceExplanation>,
}

fn sources(choices: &[ChoiceExplanation]) -> Vec<EvidenceSource> {
    let mut sources = Vec::new();
    for choice in choices {
        for source in choice.source.iter().chain(
            [&choice.condition, &choice.enable_condition]
                .into_iter()
                .flatten()
                .filter_map(|condition| condition.evidence.as_ref())
                .flat_map(|evidence| {
                    evidence
                        .nodes
                        .iter()
                        .filter_map(|node| node.source.as_ref())
                }),
        ) {
            if !sources.contains(source) {
                sources.push(source.clone());
            }
        }
    }
    sources
}

impl WorldeditApp {
    fn evidence_navigation_guard(&self) -> Result<(), String> {
        let play = self.play.as_ref().ok_or("当前没有可定位的运行")?;
        if !play.scope.matches_project(&self.project, self.version) {
            return Err("实际运行快照已过期或来源内容已变化，请重新开始后定位".into());
        }
        let snapshot = self.snapshot.as_ref().ok_or("当前没有可确认的编译来源")?;
        if snapshot.result.sources != play.scope.sources {
            return Err("当前编译来源不再是此次实际运行的源码集合".into());
        }
        if self.ime_composing || self.command_palette.ime || self.command_palette.ime_frame {
            return Err("请先完成输入法组合；当前输入已保留".into());
        }
        if !self.unapplied_play_inputs().is_empty() {
            return Err("存在未应用执行草稿；请先应用或撤销这些输入，再重新开始后定位".into());
        }
        if !self.project.recovery_conflicts().is_empty() {
            return Err("工作区仍有恢复冲突，无法确认来源".into());
        }
        Ok(())
    }

    fn evidence_source_hit(&self, source: &EvidenceSource) -> Result<SearchMatch, String> {
        let snapshot = self.snapshot.as_ref().ok_or("编译来源不存在")?;
        let target = resolve_evidence_source(&snapshot.result, source)?;
        let path =
            worldline_core::file_access::within(&self.project.root, Path::new(&source.file))?;
        if path != target.path {
            return Err("证据文件身份已变化，无法确认来源".into());
        }
        let text = self.project.document(&path)?;
        if !self
            .play
            .as_ref()
            .is_some_and(|play| play.scope.source_matches(&path, text))
        {
            return Err("证据来源内容与实际运行基线不符".into());
        }
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
            replaceable: false,
            draft: false,
        })
    }

    pub(super) fn evidence_navigation_access(&self) -> EvidenceNavigationAccess {
        if let Err(reason) = self.evidence_navigation_guard() {
            return EvidenceNavigationAccess {
                reason: Some(reason),
                targets: Vec::new(),
            };
        }
        let choices = self
            .play
            .as_ref()
            .and_then(|play| play.story.as_ref())
            .and_then(|story| story.choice_evidence())
            .unwrap_or_default();
        EvidenceNavigationAccess {
            reason: None,
            targets: sources(choices)
                .into_iter()
                .map(|source| {
                    let target = self.evidence_source_hit(&source);
                    (source, target)
                })
                .collect(),
        }
    }

    pub(super) fn jump_to_evidence_source(
        &mut self,
        ctx: &egui::Context,
        request: &EvidenceNavigationRequest,
    ) {
        let result: Result<SearchMatch, String> = (|| {
            self.evidence_navigation_guard()?;
            let actual = self
                .play
                .as_ref()
                .and_then(|play| play.story.as_ref())
                .and_then(|story| story.choice_evidence())
                .ok_or("本次选择证据已失效，请重新展开条件")?;
            if actual != request.cached.as_slice() || !sources(actual).contains(&request.source) {
                return Err("本次选择证据已更新，请重新展开条件".into());
            }
            let hit = self.evidence_source_hit(&request.source)?;
            let text = self.project.document(&hit.path)?;
            self.project.verify_source_navigation(&hit.path, text)?;
            Ok(hit)
        })();
        match result {
            Ok(hit) => {
                if let Err(reason) = self.go_author_source_position(ctx, &hit, true) {
                    self.replay_debugger.notice = Some(reason);
                    return;
                }
                self.replay_debugger.notice = None;
                let fallback = self.message.take().unwrap_or_default();
                self.message = Some(format!(
                    "已定位本次证据的声明头（不是子表达式精确范围）；Alt+Left 返回。{fallback}"
                ));
            }
            Err(reason) => self.replay_debugger.notice = Some(reason),
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
