use serde::{Deserialize, Serialize};
use std::{ops::Range, path::PathBuf};
use worldline_core::{
    draft_rehearsal::{DraftRehearsalRequest, DraftRehearsalScope},
    evidence_source::EvidenceSource,
    project::SnapshotState,
    state_inspection_source::DeclarationSource,
};
use worldline_runtime::{
    ChoiceExplanation, ContinuationOutcome, ReplayBudget, StateInspectionPage, StateInspectionQuery,
};

pub(super) const VERSION: u32 = 1;
#[cfg(target_arch = "wasm32")]
pub(super) const MAX_REQUEST: usize = 32 * 1024 * 1024;
pub(super) const MAX_RESPONSE: usize = 8 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Prepare {
    pub schema_version: u32,
    pub session_id: String,
    pub request_id: String,
    pub input: DraftRehearsalRequest,
    pub entry: PathBuf,
    pub snapshot_state: SnapshotState,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Command {
    pub schema_version: u32,
    pub session_id: String,
    pub request_id: String,
    pub action: Action,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Action {
    Start { seed: u64, budget: ReplayBudget },
    Continue { budget: ReplayBudget },
    Choose { id: String, budget: ReplayBudget },
    Inspect { query: StateInspectionQuery },
    EvidenceSource { source: EvidenceSource },
    DeclarationSource { source: DeclarationSource },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceHit {
    pub path: PathBuf,
    pub range: Range<usize>,
    pub line: u32,
    pub column: u32,
    pub preview: String,
    pub draft: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DisplayOutput {
    pub content: String,
    pub new_line: bool,
    pub speaker: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DisplayChoice {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct View {
    pub scope: DraftRehearsalScope,
    pub seed: Option<u64>,
    pub outputs: Vec<DisplayOutput>,
    pub choices: Vec<DisplayChoice>,
    pub conditions: Vec<ChoiceExplanation>,
    pub inspection: Option<StateInspectionPage>,
    pub outcome: Option<ContinuationOutcome>,
    pub ended: bool,
    pub node: Option<String>,
    pub turns: u32,
    pub error: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Response {
    pub schema_version: u32,
    pub session_id: String,
    pub request_id: String,
    pub view: Option<View>,
    pub source: Option<SourceHit>,
    pub error: Option<String>,
}

impl Command {
    pub(super) fn validate(&self) -> Result<(), String> {
        identity(self.schema_version, &self.session_id, &self.request_id)?;
        let budget = match &self.action {
            Action::Start { seed, budget } => {
                if *seed > 9_007_199_254_740_991 {
                    return Err("seed 超出安全整数范围".into());
                }
                Some(budget)
            }
            Action::Choose { id, budget } => {
                if id.len() > 1024 {
                    return Err("选择 ID 超过预算".into());
                }
                Some(budget)
            }
            Action::Continue { budget } => Some(budget),
            Action::Inspect { query }
                if query.limit == 0 || query.limit > 100 || query.text.chars().count() > 256 =>
            {
                return Err("状态查询超过预算".into())
            }
            _ => None,
        };
        if budget.is_some_and(|budget| budget.max_steps > 100_000 || budget.time_budget_ms > 250) {
            return Err("单次后台推进最多 100000 步 / 250 毫秒".into());
        }
        Ok(())
    }
}

pub(super) fn identity(version: u32, session: &str, request: &str) -> Result<(), String> {
    if version != VERSION
        || session.is_empty()
        || session.len() > 160
        || request.is_empty()
        || request.len() > 80
        || !request.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("隔离试演协议版本或会话身份无效".into());
    }
    Ok(())
}

#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn decode<T: for<'de> Deserialize<'de>>(json: &str, limit: usize) -> Result<T, String> {
    if json.len() > limit {
        return Err("隔离试演 JSON 超过预算".into());
    }
    serde_json::from_value(worldline_core::parse_unique_json(json.as_bytes())?)
        .map_err(|error| error.to_string())
}

pub(super) fn encode(response: &Response) -> Result<String, String> {
    if !crate::json_budget::serialized_within(response, MAX_RESPONSE) {
        return Err("试演结果超过 8 MiB；已停止接收，先前证据保留但本次输出不完整".into());
    }
    serde_json::to_string(response).map_err(|error| error.to_string())
}
