//! 协作审阅 UI：只编排 core 的批注/提案事务，不实现第二套合并逻辑。
use super::{Tab, WorldeditApp};
use crate::theme::{self, *};
use std::collections::BTreeMap;
use worldline_core::collaboration::{
    self, AnchorStatus, ApplyProposalCommand, CommentAnchor, CommentCommand, CommentDraft,
    ProposalCommand, ProposalConflict, ProposalFilePreview, ProposalPreview, ProposalResolution,
    ProposalStatus,
};

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
    pub preview_side: usize,
    pub comment_editor: Option<CommentEditor>,
    pub text_path: String,
    pub text_start: String,
    pub text_end: String,
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
            preview_side: 1,
            comment_editor: None,
            text_path: "world.wl".into(),
            text_start: "1".into(),
            text_end: "1".into(),
        }
    }
}

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

fn proposal_conflicts_resolved(
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

fn proposal_resolutions(
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

fn show_conflict_resolution(
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
            ERROR,
            format!(
                "冲突 {}{} · {}",
                conflict.path, conflict.location, conflict.message
            ),
        );
        if file.truncated {
            ui.colored_label(
                theme::GOLD,
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
        let id = self.next_comment_id();
        self.review.comment_editor = Some(CommentEditor {
            original: None,
            draft: CommentDraft {
                id,
                author: self.review.author.clone(),
                body: String::new(),
                anchor,
                resolved: false,
            },
            baseline: self.project.content_baseline(),
            version: self.version,
        });
        self.tab = Tab::Review;
    }

    pub(super) fn edit_comment(&mut self, id: &str) {
        let Some(comment) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.comment_index.comments.get(id))
        else {
            self.io_error = Some("批注已不存在，请刷新后重试".into());
            return;
        };
        self.review.comment_editor = Some(CommentEditor {
            original: Some(id.into()),
            draft: comment.draft.clone(),
            baseline: self.project.content_baseline(),
            version: self.version,
        });
        self.tab = Tab::Review;
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
        match collaboration::capture_text_anchor(&self.project, &path, start_line, end_line) {
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
                let resolutions = proposal_resolutions(
                    id,
                    &preview.conflicts,
                    &self.review.conflict_resolutions,
                );
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

    fn render_comment_anchor(ui: &mut egui::Ui, anchor: &mut CommentAnchor) {
        match anchor {
            CommentAnchor::Object { target } => {
                ui.label(theme::muted("对象 / 关系锚点"));
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut target.kind);
                    ui.text_edit_singleline(&mut target.id);
                });
            }
            CommentAnchor::MapPlacement {
                map_id,
                placement_id,
            } => {
                ui.label(theme::muted("地图标记锚点"));
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(map_id);
                    ui.text_edit_singleline(placement_id);
                });
            }
            CommentAnchor::TextRange {
                path,
                start_line,
                end_line,
                quote,
                ..
            } => {
                ui.label(theme::muted(format!(
                    "正文范围 · {path}:{start_line}-{end_line}"
                )));
                ui.label(theme::muted(format!(
                    "原引用：{}",
                    quote.replace('\n', " / ")
                )));
            }
        }
    }

    pub(super) fn review_tab(&mut self, ctx: &egui::Context) {
        let comments = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .comment_index
                    .comments
                    .values()
                    .map(|comment| {
                        (
                            comment.draft.id.clone(),
                            comment.draft.author.clone(),
                            comment.draft.body.clone(),
                            comment.anchor_status,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let proposals = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .proposal_index
                    .proposals
                    .values()
                    .map(|proposal| {
                        (
                            proposal.draft.id.clone(),
                            proposal.draft.author.clone(),
                            proposal.draft.reason.clone(),
                            proposal.draft.status.clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        egui::SidePanel::right("collaboration-index")
            .default_width(300.0)
            .width_range(240.0..=420.0)
            .frame(theme::panel())
            .show(ctx, |ui| {
                ui.heading("批注与提案");
                ui.label(theme::muted(format!(
                    "{} 条批注 · {} 个提案",
                    comments.len(),
                    proposals.len()
                )));
                ui.separator();
                ui.label(egui::RichText::new("批注").strong());
                for (id, author, body, status) in &comments {
                    let marker = if *status == AnchorStatus::Attached {
                        "已锚定"
                    } else {
                        "失锚"
                    };
                    if ui.button(format!("{marker} · {author} · {body}")).clicked() {
                        self.edit_comment(id);
                    }
                }
                if comments.is_empty() {
                    ui.label(theme::muted("尚无批注"));
                }
                ui.separator();
                ui.label(egui::RichText::new("提案").strong());
                for (id, author, reason, status) in &proposals {
                    let status = match status {
                        ProposalStatus::Open => "待审阅",
                        ProposalStatus::Accepted => "已采纳",
                    };
                    if ui
                        .selectable_label(
                            self.review.selected_proposal.as_deref() == Some(id),
                            format!("{status} · {author} · {reason}"),
                        )
                        .clicked()
                    {
                        self.review.selected_proposal = Some(id.clone());
                    }
                }
                if proposals.is_empty() {
                    ui.label(theme::muted("尚无提案"));
                }
            });

        egui::CentralPanel::default()
            .frame(theme::panel().fill(BG))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("collaboration-review")
                    .show(ui, |ui| {
                        ui.heading("协作审阅");
                        ui.label(theme::muted(
                            "作者署名与已接受状态是团队记录，不是身份认证或权限控制。",
                        ));
                        ui.separator();
                        ui.label(egui::RichText::new("新建正文批注").strong());
                        ui.horizontal_wrapped(|ui| {
                            ui.label("相对路径");
                            ui.text_edit_singleline(&mut self.review.text_path);
                            ui.label("行");
                            ui.text_edit_singleline(&mut self.review.text_start);
                            ui.label("到");
                            ui.text_edit_singleline(&mut self.review.text_end);
                            if ui.button("锚定当前正文范围").clicked() {
                                self.capture_text_comment();
                            }
                        });

                        let mut editor = self.review.comment_editor.take();
                        let mut comment_saved = false;
                        if let Some(comment) = editor.as_mut() {
                            ui.add_space(12.0);
                            theme::card().show(ui, |ui| {
                                ui.label(egui::RichText::new("批注编辑").strong());
                                let status = comment
                                    .original
                                    .as_ref()
                                    .and_then(|id| {
                                        self.snapshot.as_ref()?.comment_index.comments.get(id)
                                    })
                                    .map(|comment| comment.anchor_status)
                                    .unwrap_or(AnchorStatus::Attached);
                                if status == AnchorStatus::Detached {
                                    ui.colored_label(
                                        GOLD,
                                        "原锚点已失效；不会自动绑定到相邻对象或段落。",
                                    );
                                }
                                ui.label("作者");
                                ui.text_edit_singleline(&mut comment.draft.author);
                                ui.label("正文");
                                ui.add(
                                    egui::TextEdit::multiline(&mut comment.draft.body)
                                        .desired_rows(5)
                                        .desired_width(f32::INFINITY),
                                );
                                Self::render_comment_anchor(ui, &mut comment.draft.anchor);
                                ui.checkbox(&mut comment.draft.resolved, "已解决");
                                if matches!(comment.draft.anchor, CommentAnchor::TextRange { .. })
                                    && status == AnchorStatus::Detached
                                {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.text_edit_singleline(&mut self.review.text_path);
                                        ui.text_edit_singleline(&mut self.review.text_start);
                                        ui.text_edit_singleline(&mut self.review.text_end);
                                        if ui.button("重新指定正文范围").clicked() {
                                            let parsed = self
                                                .review
                                                .text_start
                                                .parse::<u32>()
                                                .ok()
                                                .zip(self.review.text_end.parse::<u32>().ok());
                                            if let Some((start, end)) = parsed {
                                                let path =
                                                    self.project.root.join(&self.review.text_path);
                                                match collaboration::capture_text_anchor(
                                                    &self.project,
                                                    &path,
                                                    start,
                                                    end,
                                                ) {
                                                    Ok(anchor) => comment.draft.anchor = anchor,
                                                    Err(error) => self.io_error = Some(error),
                                                }
                                            } else {
                                                self.io_error =
                                                    Some("重新锚定行号必须是正整数".into());
                                            }
                                        }
                                    });
                                }
                                let current = comment.version == self.version
                                    && comment.baseline == self.project.content_baseline();
                                if !current {
                                    ui.colored_label(
                                        GOLD,
                                        "工程已变化；旧批注表单不能覆盖当前稿。",
                                    );
                                }
                                if ui
                                    .add_enabled(
                                        current
                                            && !comment.draft.author.trim().is_empty()
                                            && !comment.draft.body.trim().is_empty(),
                                        theme::primary("保存批注"),
                                    )
                                    .clicked()
                                {
                                    comment_saved = self.save_comment_editor(comment);
                                }
                            });
                        }
                        if !comment_saved {
                            self.review.comment_editor = editor;
                        }

                        ui.separator();
                        ui.label(egui::RichText::new("把当前未保存修改存为提案").strong());
                        ui.horizontal_wrapped(|ui| {
                            ui.label("作者");
                            ui.text_edit_singleline(&mut self.review.author);
                            ui.label("提案 ID");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.review.proposal_id)
                                    .hint_text("留空自动生成"),
                            );
                        });
                        ui.label("理由");
                        ui.add(
                            egui::TextEdit::multiline(&mut self.review.reason)
                                .desired_rows(3)
                                .desired_width(f32::INFINITY),
                        );
                        if ui
                            .add_enabled(
                                !self.review.author.trim().is_empty()
                                    && !self.review.reason.trim().is_empty()
                                    && self.project.is_dirty(),
                                theme::primary("保存修改提案"),
                            )
                            .clicked()
                        {
                            self.capture_current_proposal();
                        }

                        if let Some(id) = self.review.selected_proposal.clone() {
                            ui.separator();
                            ui.label(egui::RichText::new("提案三方预览").strong());
                            let proposal = self
                                .snapshot
                                .as_ref()
                                .and_then(|snapshot| snapshot.proposal_index.proposals.get(&id))
                                .map(|proposal| proposal.draft.clone());
                            if let Some(proposal) = proposal {
                                ui.label(format!("{} · {}", proposal.author, proposal.reason));
                                if self
                                    .review
                                    .preview
                                    .as_ref()
                                    .is_none_or(|state| state.proposal_id != id)
                                {
                                    self.refresh_selected_proposal_preview(&id);
                                }
                                let stale = self.review.preview.as_ref().is_some_and(|state| {
                                    state.revision != self.map_revision
                                        || state.baseline != self.project.content_baseline()
                                });
                                if stale {
                                    ui.colored_label(
                                        ERROR,
                                        "内容已变化；当前差异已过期，采纳已禁用",
                                    );
                                }
                                if ui.button("重新比较提案").clicked() {
                                    self.refresh_selected_proposal_preview(&id);
                                }
                                let result = self
                                    .review
                                    .preview
                                    .as_ref()
                                    .map(|state| state.result.clone())
                                    .unwrap_or_else(|| Err("尚未生成审阅预览".into()));
                                match result {
                                    Ok(preview) => {
                                        ui.label(format!(
                                            "内容差异 {} 个文件 · 版式差异 {} 个文件",
                                            preview.content_files(),
                                            preview.presentation_files()
                                        ));
                                        for file in &preview.files {
                                            ui.label(format!(
                                                "{} · {}{}",
                                                file.domain,
                                                file.path,
                                                if file.changed {
                                                    " · 将修改"
                                                } else {
                                                    " · 无变化"
                                                }
                                            ));
                                            if ui.small_button("打开当前原文").clicked() {
                                                self.jump_to_file(&file.path, 1, 1);
                                            }
                                            for difference in &file.differences {
                                                ui.label(format!("变化：{}", difference.path));
                                                if let Some(proposed) = &difference.proposed {
                                                    if ui.small_button("复制提议值以手工解决").clicked() {
                                                        ui.ctx().copy_text(proposed.clone());
                                                    }
                                                }
                                                let sides = [
                                                            ("基底", &difference.base),
                                                            ("当前", &difference.current),
                                                            ("提议", &difference.proposed),
                                                ];
                                                if ui.available_width() >= 780.0 {
                                                    ui.columns(3, |columns| {
                                                        for (column, (heading, value)) in
                                                            columns.iter_mut().zip(sides)
                                                        {
                                                            column.strong(heading);
                                                            column.add(
                                                                egui::Label::new(
                                                                    value.as_deref().unwrap_or("∅"),
                                                                )
                                                                .selectable(true)
                                                                .wrap(),
                                                            );
                                                        }
                                                    });
                                                } else {
                                                    ui.horizontal(|ui| {
                                                        for (index, (heading, _)) in
                                                            sides.iter().enumerate()
                                                        {
                                                            ui.selectable_value(
                                                                &mut self.review.preview_side,
                                                                index,
                                                                *heading,
                                                            );
                                                        }
                                                    });
                                                    let (heading, value) = sides
                                                        [self.review.preview_side.min(sides.len() - 1)];
                                                    ui.strong(heading);
                                                    ui.add(
                                                        egui::Label::new(
                                                            value.as_deref().unwrap_or("∅"),
                                                        )
                                                        .selectable(true)
                                                        .wrap(),
                                                    );
                                                }
                                            }
                                            if file.alignment_uncertain || file.truncated {
                                                ui.colored_label(
                                                    theme::GOLD,
                                                    "对齐不确定或预览截断；请检查三方原文",
                                                );
                                            }
                                            ui.collapsing("三方原文", |ui| {
                                                for (heading, value) in [
                                                    ("基底", &file.raw.base),
                                                    ("当前", &file.raw.current),
                                                    ("提议", &file.raw.proposed),
                                                ] {
                                                    ui.strong(heading);
                                                    ui.add(
                                                        egui::Label::new(
                                                            value.as_deref().unwrap_or("∅"),
                                                        )
                                                        .selectable(true)
                                                        .wrap(),
                                                    );
                                                }
                                            });
                                            for impact in &file.reference_impacts {
                                                ui.label(format!(
                                                    "引用影响 {}:{} · 当前 {} · 提议 {}",
                                                    impact.target.kind,
                                                    impact.target.id,
                                                    impact.current.len(),
                                                    impact.proposed.len()
                                                ));
                                            }
                                            if !file.reference_impact_complete {
                                                ui.colored_label(
                                                    theme::GOLD,
                                                    "引用影响不完整，请打开原文核对",
                                                );
                                            }
                                            for conflict in &file.conflicts {
                                                show_conflict_resolution(
                                                    ui,
                                                    &id,
                                                    file,
                                                    conflict,
                                                    &mut self.review.conflict_resolutions,
                                                );
                                            }
                                        }
                                        let conflicts_resolved = proposal_conflicts_resolved(
                                            &id,
                                            &preview.conflicts,
                                            &self.review.conflict_resolutions,
                                        );
                                        if !preview.conflicts.is_empty() {
                                            ui.label(format!(
                                                "待逐项解决冲突 {} 项",
                                                preview.conflicts.len()
                                            ));
                                        }
                                        let open = proposal.status == ProposalStatus::Open;
                                        if ui
                                            .add_enabled(
                                                open && conflicts_resolved && !stale,
                                                theme::primary("采纳提案"),
                                            )
                                            .clicked()
                                        {
                                            self.apply_selected_proposal(&id);
                                        }
                                        if !conflicts_resolved {
                                            ui.label(theme::muted(
                                                "逐项选择基底、当前或提议值，也可编辑解决方案；提交时 core 会重新验证。",
                                            ));
                                        }
                                    }
                                    Err(error) => {
                                        ui.colored_label(ERROR, error);
                                    }
                                }
                            }
                        }
                    });
            });
    }
}
