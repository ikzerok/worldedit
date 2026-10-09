use super::protocol::*;
use worldline_core::{draft_rehearsal::DraftRehearsalSnapshot, project::Project};
use worldline_runtime::{
    draft_rehearsal::DraftRehearsal, Output, ReplayCancellation, StateInspectionQuery,
};

pub(super) struct Engine {
    snapshot: Option<DraftRehearsalSnapshot>,
    session: Option<DraftRehearsal>,
    session_id: String,
    next_request: u64,
    query: StateInspectionQuery,
    view: View,
}

impl Engine {
    pub(super) fn prepare(project: &Project, prepare: &Prepare) -> Result<Self, String> {
        identity(
            prepare.schema_version,
            &prepare.session_id,
            &prepare.request_id,
        )?;
        if prepare.request_id != "0" {
            return Err("准备请求必须从 0 开始".into());
        }
        let snapshot = project.compile_draft_rehearsal(&prepare.input)?;
        let view = View {
            scope: snapshot.scope().clone(),
            seed: None,
            outputs: vec![],
            choices: vec![],
            conditions: vec![],
            inspection: None,
            outcome: None,
            ended: false,
            node: None,
            turns: 0,
            error: None,
        };
        Ok(Self {
            snapshot: Some(snapshot),
            session: None,
            session_id: prepare.session_id.clone(),
            next_request: 1,
            query: StateInspectionQuery::default(),
            view,
        })
    }

    pub(super) fn prepared(&self) -> Response {
        self.response("0".into(), None, None)
    }

    pub(super) fn execute(&mut self, command: Command, token: &ReplayCancellation) -> Response {
        let id = command.request_id.clone();
        let result = (|| {
            command.validate()?;
            if command.session_id != self.session_id || id != self.next_request.to_string() {
                return Err("试演请求来自不同会话或顺序已失效".into());
            }
            self.next_request = self.next_request.checked_add(1).ok_or("试演请求代次耗尽")?;
            self.view.outputs.clear();
            match command.action {
                Action::Start { seed, budget } => {
                    if self.session.is_some() {
                        return Err("本次试演已经启动，不能重用启动请求".into());
                    }
                    let snapshot = self.snapshot.take().ok_or("准备快照已释放")?;
                    self.session = Some(match DraftRehearsal::new(snapshot, seed) {
                        Ok(session) => session,
                        Err(error) => {
                            let message = error.to_string();
                            // 没有真实Story时也必须标为已失败，宿主不能继续展示可推进状态。
                            self.view.error = Some(message.clone());
                            return Err(message);
                        }
                    });
                    self.advance(budget, token)?;
                }
                Action::Continue { budget } => self.advance(budget, token)?,
                Action::Choose { id, budget } => {
                    let session = self.session.as_mut().ok_or("试演尚未启动")?;
                    session.choose_id(&id).map_err(|e| e.to_string())?;
                    self.advance(budget, token)?;
                }
                Action::Inspect { query } => {
                    self.session
                        .as_ref()
                        .ok_or("试演尚未启动")?
                        .inspect_state(&query)
                        .map_err(|e| e.message)?;
                    self.query = query;
                }
                Action::EvidenceSource { source } => {
                    let session = self.session.as_ref().ok_or("试演尚未启动")?;
                    let choices = session.choice_evidence().unwrap_or_default();
                    let actual = choices.iter().any(|choice| {
                        choice.source.as_ref() == Some(&source)
                            || [&choice.condition, &choice.enable_condition]
                                .into_iter()
                                .flatten()
                                .filter_map(|condition| condition.evidence.as_ref())
                                .flat_map(|evidence| &evidence.nodes)
                                .any(|node| node.source.as_ref() == Some(&source))
                    });
                    if !actual {
                        return Err("声明已不属于本次真实条件证据".into());
                    }
                    let hit = session.snapshot().evidence_source(&source)?;
                    return self.relative_hit(hit).map(Some);
                }
                Action::DeclarationSource { source } => {
                    if !self.view.inspection.as_ref().is_some_and(|page| {
                        page.items
                            .iter()
                            .any(|item| item.source.as_ref() == Some(&source))
                    }) {
                        return Err("声明已不属于当前真实状态查询页".into());
                    }
                    let hit = self
                        .session
                        .as_ref()
                        .ok_or("试演尚未启动")?
                        .snapshot()
                        .declaration_source(&source)?;
                    return self.relative_hit(hit).map(Some);
                }
            }
            Ok(None)
        })();
        self.refresh_view();
        match result {
            Ok(source) => self.response(id, source, None),
            Err(error) => self.response(id, None, Some(error)),
        }
    }

    fn advance(
        &mut self,
        budget: worldline_runtime::ReplayBudget,
        token: &ReplayCancellation,
    ) -> Result<(), String> {
        let session = self.session.as_mut().ok_or("试演尚未启动")?;
        match session.continue_bounded(budget, token) {
            Ok(continued) => {
                self.view.outcome = Some(continued.outcome);
                self.view.outputs = continued
                    .outputs
                    .into_iter()
                    .filter_map(|output| match output {
                        Output::Text {
                            content,
                            new_line,
                            speaker,
                            ..
                        } => Some(DisplayOutput {
                            content,
                            new_line,
                            speaker: speaker.map(|speaker| speaker.id),
                        }),
                        Output::Ended => None,
                    })
                    .collect();
                self.query.expected_stamp = None;
                self.query.offset = 0;
                Ok(())
            }
            Err(error) => {
                self.view.outcome = None;
                self.view.error = Some(error.to_string());
                Err(error.to_string())
            }
        }
    }

    fn refresh_view(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        if let Err(error) = session.view_guard() {
            self.view.outcome = None;
            self.view.error = Some(error.to_string());
            return;
        }
        self.view.seed = Some(session.seed());
        self.view.ended = session.is_ended();
        self.view.node = session.current_node();
        self.view.turns = session.turns();
        self.view.choices = session
            .choice_presentations()
            .iter()
            .map(|choice| DisplayChoice {
                id: choice.id.clone(),
                label: choice.label.clone(),
                enabled: choice.enabled,
                disabled_reason: choice.disabled_reason.clone(),
            })
            .collect();
        self.view.conditions = session.choice_evidence().unwrap_or_default().to_vec();
        self.view.inspection = session.inspect_state(&self.query).ok();
    }

    fn relative_hit(
        &self,
        hit: worldline_core::search_replace::SearchMatch,
    ) -> Result<SourceHit, String> {
        let session = self.session.as_ref().ok_or("试演尚未启动")?;
        let path = session.snapshot().relative_source_path(&hit.path)?;
        Ok(SourceHit {
            path,
            range: hit.range,
            line: hit.line,
            column: hit.column,
            preview: hit.preview,
            draft: hit.draft,
        })
    }

    fn response(
        &self,
        request_id: String,
        source: Option<SourceHit>,
        error: Option<String>,
    ) -> Response {
        Response {
            schema_version: VERSION,
            session_id: self.session_id.clone(),
            request_id,
            view: Some(self.view.clone()),
            source,
            error,
        }
    }
}
