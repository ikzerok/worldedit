use super::super::WorldeditApp;
use super::{
    change_marker, proposal_conflicts_resolved, show_conflict_resolution, update_resolution_context,
};
use crate::theme::{self, *};
use worldline_core::collaboration::{self, AnchorStatus, CommentAnchor, ProposalStatus};

impl WorldeditApp {
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

    pub(in crate::app) fn review_tab(&mut self, ctx: &egui::Context) {
        self.capture_new_draft_baselines();
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
            .frame(theme::panel().fill(BG()))
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
                                        GOLD(),
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
                                        GOLD(),
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
                            let close_preview = ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("提案三方预览").strong());
                                ui.small_button("关闭预览").clicked()
                            }).inner;
                            if close_preview {
                                // 仅收起；保留原比较基线与解决草稿，重开不能绕过过期门。
                                self.review.selected_proposal = None;
                                return;
                            }
                            let proposal = self
                                .snapshot
                                .as_ref()
                                .and_then(|snapshot| snapshot.proposal_index.proposals.get(&id))
                                .map(|proposal| proposal.draft.clone());
                            if let Some(proposal) = proposal {
                                update_resolution_context(
                                    &mut self.review,
                                    &id,
                                    proposal.status == ProposalStatus::Open,
                                );
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
                                        ERROR(),
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
                                            "当前合并结果拟写入：内容 {} 个文件 · 版式 {} 个文件",
                                            preview.content_files(),
                                            preview.presentation_files()
                                        ));
                                        let conflict_files = preview.files.iter()
                                            .filter(|file| !file.conflicts.is_empty()).count();
                                        if conflict_files > 0 {
                                            ui.colored_label(theme::GOLD(), format!(
                                                "{conflict_files} 个文件有冲突；解决草稿尚未计入上面的数量，冲突不是无变化。"
                                            ));
                                        }
                                        for file in &preview.files {
                                            let status = if !file.conflicts.is_empty() {
                                                if stale {
                                                    "有冲突 · 比较已过期，暂不可应用"
                                                } else if proposal_conflicts_resolved(&id, &file.conflicts, &self.review.conflict_resolutions) {
                                                    "冲突解决草稿已齐 · 仍待明确采纳"
                                                } else {
                                                    "待解决冲突 · 暂不可应用"
                                                }
                                            } else if file.changed {
                                                "当前合并结果将修改"
                                            } else {
                                                "当前合并结果无改动"
                                            };
                                            ui.label(format!("{} · {} · {status}", file.domain, file.path));
                                            if ui.small_button("打开当前原文").clicked() {
                                                let path = super::super::workspace_source_path(
                                                    &self.project,
                                                    std::path::Path::new(&file.path),
                                                );
                                                self.jump_to_file(&path.to_string_lossy(), 1, 1);
                                            }
                                            for difference in &file.differences {
                                                ui.label(format!(
                                                    "当前{} · 提议{} · 变化：{}",
                                                    change_marker(
                                                        difference.base.as_deref(),
                                                        difference.current.as_deref()
                                                    ),
                                                    change_marker(
                                                        difference.base.as_deref(),
                                                        difference.proposed.as_deref()
                                                    ),
                                                    difference.path
                                                ));
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
                                                            read_only_review_text(
                                                                column,
                                                                egui::Id::new(("review-difference", &id, &file.path, &difference.path, heading)),
                                                                value.as_deref().unwrap_or("∅"),
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
                                                    read_only_review_text(
                                                        ui,
                                                        egui::Id::new(("review-difference", &id, &file.path, &difference.path, heading)),
                                                        value.as_deref().unwrap_or("∅"),
                                                    );
                                                }
                                            }
                                            if file.alignment_uncertain || file.truncated {
                                                ui.colored_label(
                                                    theme::GOLD(),
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
                                                    read_only_review_text(
                                                        ui,
                                                        egui::Id::new(("review-raw", &id, &file.path, heading)),
                                                        value.as_deref().unwrap_or("∅"),
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
                                                    theme::GOLD(),
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
                                            if stale {
                                                ui.label("待重新比较：解决草稿保留，暂不可应用");
                                            } else if conflicts_resolved {
                                                ui.label("解决草稿已齐 · 仍待明确采纳；提交时 core 会重新验证");
                                            } else {
                                                ui.label(format!("待逐项解决冲突 {} 项", preview.conflicts.len()));
                                            }
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
                                        ui.colored_label(ERROR(), error);
                                    }
                                }
                            }
                        }
                        if let Some(response) = ui
                            .ctx()
                            .memory(|memory| memory.focused())
                            .and_then(|id| ui.ctx().read_response(id))
                        {
                            if response.gained_focus()
                                && response.layer_id == ui.layer_id()
                                && ui.min_rect().contains_rect(response.rect)
                            {
                                response.scroll_to_me(None);
                            }
                        }
                    });
            });
    }
}

fn read_only_review_text(ui: &mut egui::Ui, id: egui::Id, mut text: &str) {
    // &str 实现不可变 TextBuffer：可用键盘选择与复制，但输入、剪切不会改写原文。
    ui.add(
        egui::TextEdit::multiline(&mut text)
            .id(id)
            .font(egui::TextStyle::Body)
            .desired_width(f32::INFINITY)
            .desired_rows(1)
            .frame(false),
    );
}
