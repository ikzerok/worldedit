//! 协作审阅 UI：只编排 core 的批注/提案事务，不实现第二套合并逻辑。
use super::{Tab, WorldeditApp};
use crate::theme::{self, *};
use worldline_core::collaboration::{
    self, AnchorStatus, ApplyProposalCommand, CommentAnchor, CommentCommand, CommentDraft,
    ProposalCommand, ProposalStatus,
};

#[derive(Clone)]
pub(super) struct CommentEditor {
    pub original: Option<String>,
    pub draft: CommentDraft,
    pub baseline: String,
    pub version: u64,
}

pub(super) struct ReviewState {
    pub author: String,
    pub reason: String,
    pub proposal_id: String,
    pub selected_proposal: Option<String>,
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
            comment_editor: None,
            text_path: "world.wl".into(),
            text_start: "1".into(),
            text_end: "1".into(),
        }
    }
}

impl WorldeditApp {
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
        let before = self.project.clone();
        let command = ApplyProposalCommand {
            expected_revision: self.map_revision,
            proposal_id: id.into(),
        };
        match collaboration::apply_proposal(&mut self.project, &mut self.map_revision, command) {
            Ok(_) => {
                self.remember(before);
                self.recompile();
                self.io_error = None;
                self.message = Some("提案已采纳；内容与版式差异已按三方规则提交".into());
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
                                .and_then(|snapshot| snapshot.proposal_index.proposals.get(&id));
                            if let Some(proposal) = proposal {
                                ui.label(format!(
                                    "{} · {}",
                                    proposal.draft.author, proposal.draft.reason
                                ));
                                match collaboration::preview_proposal(
                                    &self.project,
                                    &proposal.draft,
                                ) {
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
                                        }
                                        for conflict in &preview.conflicts {
                                            ui.colored_label(
                                                ERROR,
                                                format!(
                                                    "{}{} · {}",
                                                    conflict.path,
                                                    conflict.location,
                                                    conflict.message
                                                ),
                                            );
                                        }
                                        let open = proposal.draft.status == ProposalStatus::Open;
                                        if ui
                                            .add_enabled(
                                                open && preview.can_apply(),
                                                theme::primary("采纳提案"),
                                            )
                                            .clicked()
                                        {
                                            self.apply_selected_proposal(&id);
                                        }
                                        if !preview.can_apply() {
                                            ui.label(theme::muted(
                                                "存在冲突时不会部分写入；请先人工合并或更新提案。",
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
