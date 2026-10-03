//! 将当次缓存证据接回作者导航；不求值、不重编译、不应用草稿。
use super::super::WorldeditApp;
use worldline_core::evidence_source::EvidenceSource;
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
        self.play_source_guard(&play.scope)
    }

    fn evidence_source_hit(&self, source: &EvidenceSource) -> Result<SearchMatch, String> {
        let play = self.play.as_ref().ok_or("当前没有可定位的运行")?;
        self.play_source_hit(&play.scope, source)
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
