use super::{ProposalResolutionDraft, ProposalResolutionDrafts, ReviewState};
use crate::theme::{self, *};
use std::collections::BTreeMap;
use worldline_core::collaboration::{ProposalConflict, ProposalFilePreview, ProposalResolution};
fn proposal_resolution_draft<'a>(
    proposal_id: &str,
    conflict: &ProposalConflict,
    drafts: &'a mut ProposalResolutionDrafts,
) -> &'a mut ProposalResolutionDraft {
    if !drafts.contains_key(proposal_id) {
        drafts.insert(proposal_id.to_owned(), BTreeMap::new());
    }
    let proposal_drafts = drafts.get_mut(proposal_id).unwrap();
    if !proposal_drafts.contains_key(conflict.path.as_str()) {
        proposal_drafts.insert(conflict.path.clone(), BTreeMap::new());
    }
    let file_drafts = proposal_drafts.get_mut(conflict.path.as_str()).unwrap();
    if !file_drafts.contains_key(conflict.location.as_str()) {
        file_drafts.insert(
            conflict.location.clone(),
            ProposalResolutionDraft::default(),
        );
    }
    file_drafts.get_mut(conflict.location.as_str()).unwrap()
}

pub(super) fn proposal_conflicts_resolved(
    proposal_id: &str,
    conflicts: &[ProposalConflict],
    drafts: &ProposalResolutionDrafts,
) -> bool {
    conflicts.iter().all(|conflict| {
        drafts
            .get(proposal_id)
            .and_then(|files| files.get(conflict.path.as_str()))
            .and_then(|locations| locations.get(conflict.location.as_str()))
            .is_some_and(|draft| draft.resolved)
    })
}

pub(super) fn proposal_resolutions(
    proposal_id: &str,
    conflicts: &[ProposalConflict],
    drafts: &ProposalResolutionDrafts,
) -> Vec<ProposalResolution> {
    conflicts
        .iter()
        .filter_map(|conflict| {
            let draft = drafts
                .get(proposal_id)?
                .get(conflict.path.as_str())?
                .get(conflict.location.as_str())?;
            draft.resolved.then(|| ProposalResolution {
                path: conflict.path.clone(),
                location: conflict.location.clone(),
                value: (!draft.deletion).then(|| draft.value.clone()),
            })
        })
        .collect()
}

pub(super) fn update_resolution_context(review: &mut ReviewState, proposal_id: &str, open: bool) {
    if review
        .resolution_context
        .as_ref()
        .is_some_and(|(current_id, current_open)| {
            current_id == proposal_id && *current_open == open
        })
    {
        return;
    }
    if let Some((previous_id, _)) = review.resolution_context.take() {
        review.conflict_resolutions.remove(&previous_id);
    }
    review.conflict_resolutions.remove(proposal_id);
    review.resolution_context = Some((proposal_id.to_owned(), open));
}

pub(super) fn show_conflict_resolution(
    ui: &mut egui::Ui,
    proposal_id: &str,
    file: &ProposalFilePreview,
    conflict: &ProposalConflict,
    drafts: &mut ProposalResolutionDrafts,
) {
    let draft = proposal_resolution_draft(proposal_id, conflict, drafts);
    let sides = file
        .differences
        .iter()
        .find(|difference| difference.path == conflict.location)
        .map(|difference| {
            [
                ("基底", difference.base.as_deref()),
                ("当前", difference.current.as_deref()),
                ("提议", difference.proposed.as_deref()),
            ]
        })
        .unwrap_or([
            ("基底", file.raw.base.as_deref()),
            ("当前", file.raw.current.as_deref()),
            ("提议", file.raw.proposed.as_deref()),
        ]);

    ui.group(|ui| {
        ui.colored_label(
            ERROR(),
            format!(
                "冲突 {}{} · {}",
                conflict.path, conflict.location, conflict.message
            ),
        );
        if file.truncated {
            ui.colored_label(
                theme::GOLD(),
                "预览已截断；请先打开原文，并在下方输入完整解决内容",
            );
        }
        ui.horizontal_wrapped(|ui| {
            for (side, value) in sides {
                let label = if value.is_some() {
                    format!("采用{side}")
                } else {
                    format!("采用{side}（删除）")
                };
                if ui
                    .add_enabled(!file.truncated, egui::Button::new(label))
                    .clicked()
                {
                    draft.value = value.unwrap_or_default().to_owned();
                    draft.deletion = value.is_none();
                    draft.resolved = true;
                }
            }
        });
        if draft.deletion {
            ui.label(theme::muted("解决方案：删除对应字段或文件"));
            if ui.small_button("改为编辑解决方案").clicked() {
                draft.deletion = false;
                draft.resolved = true;
            }
        } else {
            let hint = if file.domain == "presentation" {
                if conflict.location.is_empty() {
                    "编辑完整 JSON 文档"
                } else {
                    "编辑 JSON 值"
                }
            } else {
                "编辑完整文件文本"
            };
            let response = ui.add(
                egui::TextEdit::multiline(&mut draft.value)
                    .desired_rows(if conflict.location.is_empty() { 6 } else { 2 })
                    .desired_width(f32::INFINITY)
                    .hint_text(hint),
            );
            if response.changed() {
                draft.resolved = true;
            }
        }
        ui.label(theme::muted(if draft.resolved {
            "已选择；采纳时 core 会重新验证"
        } else {
            "尚未解决"
        }));
    });
}
