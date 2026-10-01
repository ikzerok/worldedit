//! 协作审阅 UI：只编排 core 的批注/提案事务，不实现第二套合并逻辑。
mod comment_editor;
pub(in crate::app) mod comment_lifecycle;
mod proposal_resolution;
mod review_list;
mod review_tab;
mod selection;
use super::{Tab, WorldeditApp};
use proposal_resolution::{
    proposal_conflicts_resolved, proposal_resolutions, show_conflict_resolution,
    update_resolution_context,
};
use std::collections::BTreeMap;
use worldline_core::collaboration::{
    self, ApplyProposalCommand, CommentAnchor, CommentCommand, CommentDraft, ProposalCommand,
    ProposalPreview,
};

fn change_marker(base: Option<&str>, value: Option<&str>) -> &'static str {
    match (base, value) {
        (None, None) => "未改",
        (None, Some(_)) => "新增",
        (Some(_), None) => "删除",
        (Some(base), Some(value)) if base == value => "未改",
        (Some(_), Some(_)) => "修改",
    }
}

#[derive(Clone)]
pub(super) struct ProposalPreviewState {
    proposal_id: String,
    baseline: String,
    revision: worldline_core::presentation_commands::Revision,
    result: Result<ProposalPreview, String>,
}

#[derive(Clone)]
pub(super) struct CommentEditor {
    pub original: Option<String>,
    pub draft: CommentDraft,
    pub initial: CommentDraft,
    pub baseline: String,
    pub version: u64,
}

#[derive(Clone, Default)]
pub(super) struct ProposalResolutionDraft {
    value: String,
    deletion: bool,
    resolved: bool,
}
pub(super) type ProposalResolutionDrafts =
    BTreeMap<String, BTreeMap<String, BTreeMap<String, ProposalResolutionDraft>>>;

pub(super) struct ReviewState {
    pub author: String,
    pub reason: String,
    pub proposal_id: String,
    pub selected_proposal: Option<String>,
    pub preview: Option<ProposalPreviewState>,
    pub conflict_resolutions: ProposalResolutionDrafts,
    pub resolution_context: Option<(String, bool)>,
    pub preview_side: usize,
    pub comment_editor: Option<CommentEditor>,
    pub text_path: String,
    pub text_start: String,
    pub text_end: String,
    pub filter: collaboration::CommentReviewFilter,
    pub selected_comment: Option<String>,
    pub scroll_selection: bool,
    pub pending_comment_action: Option<comment_lifecycle::ReviewAction>,
    pub last_tab: Option<Tab>,
    pub composition_frame: bool,
    pub pending_source_selection: Option<(std::path::PathBuf, String, std::ops::Range<usize>)>,
}

impl Default for ReviewState {
    fn default() -> Self {
        Self {
            author: "作者".into(),
            reason: String::new(),
            proposal_id: String::new(),
            selected_proposal: None,
            preview: None,
            conflict_resolutions: BTreeMap::new(),
            resolution_context: None,
            preview_side: 1,
            comment_editor: None,
            text_path: "world.wl".into(),
            text_start: "1".into(),
            text_end: "1".into(),
            filter: Default::default(),
            selected_comment: None,
            scroll_selection: false,
            pending_comment_action: None,
            last_tab: None,
            composition_frame: false,
            pending_source_selection: None,
        }
    }
}

impl WorldeditApp {
    fn refresh_selected_proposal_preview(&mut self, id: &str) {
        let result = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.proposal_index.proposals.get(id))
            .map(|proposal| collaboration::preview_proposal(&self.project, &proposal.draft))
            .unwrap_or_else(|| Err("提案已不存在，请刷新后重试".into()));
        let baseline = result.as_ref().map_or_else(
            |_| self.project.content_baseline(),
            |preview| preview.expected_baseline.clone(),
        );
        self.review.preview = Some(ProposalPreviewState {
            proposal_id: id.into(),
            baseline,
            revision: self.map_revision,
            result,
        });
    }

    fn next_comment_id(&self) -> String {
        let existing = self
            .snapshot
            .as_ref()
            .map(|snapshot| &snapshot.comment_index.comments);
        (1..)
            .map(|index| format!("comment_{index}"))
            .find(|id| existing.is_none_or(|comments| !comments.contains_key(id)))
            .unwrap()
    }

    fn next_proposal_id(&self) -> String {
        let existing = self
            .snapshot
            .as_ref()
            .map(|snapshot| &snapshot.proposal_index.proposals);
        (1..)
            .map(|index| format!("proposal_{index}"))
            .find(|id| existing.is_none_or(|proposals| !proposals.contains_key(id)))
            .unwrap()
    }

    pub(super) fn new_comment_for_anchor(&mut self, anchor: CommentAnchor) {
        self.request_review_action(comment_lifecycle::ReviewAction::New(anchor));
    }

    pub(super) fn edit_comment(&mut self, id: &str) {
        self.request_review_action(comment_lifecycle::ReviewAction::Edit(id.into()));
    }

    fn save_comment_editor(&mut self, editor: &CommentEditor) -> bool {
        if editor.version != self.version || editor.baseline != self.project.content_baseline() {
            self.io_error = Some("批注表单打开后工程已变化，请重新打开后合并。".into());
            return false;
        }
        let before = self.project.clone();
        let command = CommentCommand {
            expected_revision: self.map_revision,
            expected_baseline: editor.baseline.clone(),
            original: editor.original.clone(),
            draft: editor.draft.clone(),
        };
        match collaboration::write_comment(&mut self.project, &mut self.map_revision, command) {
            Ok(_) => {
                self.remember(before);
                self.refresh_presentation_after_map_command();
                self.io_error = None;
                self.message = Some("批注已更新；保存全部可写入作品目录".into());
                true
            }
            Err(error) => {
                self.io_error = Some(error);
                false
            }
        }
    }

    fn capture_text_comment(&mut self) {
        let Ok(start_line) = self.review.text_start.parse::<u32>() else {
            self.io_error = Some("正文批注起始行必须是正整数".into());
            return;
        };
        let Ok(end_line) = self.review.text_end.parse::<u32>() else {
            self.io_error = Some("正文批注结束行必须是正整数".into());
            return;
        };
        let path = self.project.root.join(&self.review.text_path);
        match self.review_text_anchor(&path, start_line, end_line) {
            Ok(anchor) => self.new_comment_for_anchor(anchor),
            Err(error) => self.io_error = Some(error),
        }
    }

    fn capture_current_proposal(&mut self) {
        let id = if self.review.proposal_id.trim().is_empty() {
            self.next_proposal_id()
        } else {
            self.review.proposal_id.trim().to_owned()
        };
        let draft = match collaboration::capture_dirty_proposal(
            &self.project,
            &id,
            self.review.author.trim(),
            self.review.reason.trim(),
        ) {
            Ok(draft) => draft,
            Err(error) => {
                self.io_error = Some(error);
                return;
            }
        };
        let baseline = self.project.content_baseline();
        let before = self.project.clone();
        let command = ProposalCommand {
            expected_revision: self.map_revision,
            expected_baseline: baseline,
            draft,
        };
        match collaboration::write_proposal(&mut self.project, &mut self.map_revision, command) {
            Ok(_) => {
                self.remember(before);
                self.refresh_presentation_after_map_command();
                self.review.selected_proposal = Some(id);
                self.review.proposal_id.clear();
                self.review.reason.clear();
                self.io_error = None;
                self.message = Some("提案已保存；当前未保存修改仍保留在工作区".into());
            }
            Err(error) => self.io_error = Some(error),
        }
    }

    fn apply_selected_proposal(&mut self, id: &str) {
        let Some(preview_state) = self.review.preview.as_ref() else {
            self.io_error = Some("请先比较提案再采纳".into());
            return;
        };
        if preview_state.proposal_id != id
            || preview_state.revision != self.map_revision
            || preview_state.baseline != self.project.content_baseline()
        {
            self.io_error = Some("审阅预览已过期，请重新比较后采纳".into());
            return;
        }
        let (baseline, resolutions, had_conflicts) = match &preview_state.result {
            Ok(preview) => {
                let resolutions =
                    proposal_resolutions(id, &preview.conflicts, &self.review.conflict_resolutions);
                if resolutions.len() != preview.conflicts.len() {
                    self.io_error = Some("请逐项选择或编辑所有冲突解决方案".into());
                    return;
                }
                (
                    preview_state.baseline.clone(),
                    resolutions,
                    !preview.conflicts.is_empty(),
                )
            }
            Err(error) => {
                self.io_error = Some(error.clone());
                return;
            }
        };
        let before = self.project.clone();
        let command = ApplyProposalCommand {
            expected_revision: self.map_revision,
            expected_baseline: baseline,
            proposal_id: id.into(),
        };
        match collaboration::apply_proposal_with_resolutions(
            &mut self.project,
            &mut self.map_revision,
            command,
            &resolutions,
        ) {
            Ok(_) => {
                self.review.conflict_resolutions.remove(id);
                self.remember(before);
                self.recompile();
                self.review.preview = None;
                self.io_error = None;
                self.message = Some(if had_conflicts {
                    "逐项解决方案已由 core 复验；提案与解决后的内容已一并提交".into()
                } else {
                    "提案已采纳；内容与版式差异已按三方规则提交".into()
                });
            }
            Err(error) => self.io_error = Some(error),
        }
    }
}
