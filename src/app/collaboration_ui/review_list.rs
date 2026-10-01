use super::{comment_lifecycle::ReviewAction, WorldeditApp};
use crate::theme;
use worldline_core::collaboration::{
    AnchorStatus, CommentAnchor, CommentAnchorFilter, CommentResolutionFilter, ProposalStatus,
};

pub(super) fn anchor_label(anchor: &CommentAnchor) -> String {
    match anchor {
        CommentAnchor::Object { target } => format!("{}:{}", target.kind, target.id),
        CommentAnchor::MapPlacement {
            map_id,
            placement_id,
        } => format!("地图 {map_id} / {placement_id}"),
        CommentAnchor::TextRange {
            path,
            start_line,
            end_line,
            ..
        } => format!("{path}:{start_line}–{end_line}"),
    }
}
impl WorldeditApp {
    pub(super) fn review_sidebar(
        &mut self,
        ctx: &egui::Context,
        proposals: &[(String, String, String, ProposalStatus)],
    ) {
        egui::SidePanel::right("collaboration-index")
            .default_width(320.0)
            .width_range(260.0..=440.0)
            .frame(theme::panel())
            .show(ctx, |ui| {
                ui.heading("修订清单");
                let mut desired = self.review.filter.clone();
                ui.add(
                    egui::TextEdit::singleline(&mut desired.text)
                        .hint_text("搜索正文 / 作者 / 批注 ID")
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut desired.resolution,
                        CommentResolutionFilter::Open,
                        "未解决",
                    );
                    ui.selectable_value(
                        &mut desired.resolution,
                        CommentResolutionFilter::All,
                        "全部",
                    );
                    ui.selectable_value(
                        &mut desired.resolution,
                        CommentResolutionFilter::Resolved,
                        "已解决",
                    );
                });
                egui::ComboBox::from_id_salt("review-anchor-filter")
                    .selected_text(match desired.anchor {
                        CommentAnchorFilter::All => "全部锚定状态",
                        CommentAnchorFilter::Attached => "已锚定",
                        CommentAnchorFilter::Detached => "失锚",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut desired.anchor,
                            CommentAnchorFilter::All,
                            "全部锚定状态",
                        );
                        ui.selectable_value(
                            &mut desired.anchor,
                            CommentAnchorFilter::Attached,
                            "已锚定",
                        );
                        ui.selectable_value(
                            &mut desired.anchor,
                            CommentAnchorFilter::Detached,
                            "失锚",
                        );
                    });
                if desired != self.review.filter {
                    self.request_review_action(ReviewAction::Filter(desired));
                }
                let projection = self
                    .snapshot
                    .as_ref()
                    .map(|s| s.comment_index.review_projection(&self.review.filter))
                    .unwrap_or_else(|| {
                        worldline_core::collaboration::CommentIndex::default()
                            .review_projection(&self.review.filter)
                    });
                ui.label(format!(
                    "命中 {} / 全部 {} · 未解决 {}",
                    projection.matched, projection.total, projection.unresolved
                ));
                if !projection.diagnostics.is_empty() {
                    egui::CollapsingHeader::new(format!(
                        "索引诊断 · {} 项（不是无批注）",
                        projection.diagnostics.len()
                    ))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("review-index-errors")
                            .max_height(100.0)
                            .show(ui, |ui| {
                                for diagnostic in &projection.diagnostics {
                                    ui.colored_label(
                                        theme::GOLD(),
                                        format!("{} · {}", diagnostic.code, diagnostic.message),
                                    );
                                }
                            });
                    });
                }
                ui.label(theme::muted("焦点在列表时 ↑↓ 选择，Enter 打开"));
                let list_id = egui::Id::new("review-comment-list");
                let mut moved = false;
                let mut activate = false;
                if ui.memory(|m| m.has_focus(list_id))
                    && !self.review_ime_active()
                    && !self.command_palette.ime_frame
                {
                    let delta = ui.input_mut(|i| {
                        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                            1
                        } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                            -1
                        } else {
                            0
                        }
                    });
                    if delta != 0 && !projection.items.is_empty() {
                        let current = self.review.selected_comment.as_ref().and_then(|id| {
                            projection
                                .items
                                .iter()
                                .position(|item| &item.draft.id == id)
                        });
                        let index = current.map_or(0, |at| {
                            (at as isize + delta).clamp(0, projection.items.len() as isize - 1)
                                as usize
                        });
                        self.review.selected_comment =
                            Some(projection.items[index].draft.id.clone());
                        moved = true;
                    }
                    activate =
                        ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                }
                if ui.button("进入批注列表（键盘）").clicked() {
                    ui.memory_mut(|m| m.request_focus(list_id));
                    if self.review.selected_comment.is_none() {
                        self.review.selected_comment =
                            projection.items.first().map(|item| item.draft.id.clone());
                    }
                    moved = true;
                }
                let scroll = self.review.scroll_selection || moved;
                self.review.scroll_selection = false;
                // 键盘列表焦点位于固定工具区，不能随滚动离屏后被 egui 丢弃。
                let focus = ui.interact(
                    egui::Rect::from_min_size(
                        ui.cursor().min,
                        egui::vec2(ui.available_width(), 1.0),
                    ),
                    list_id,
                    egui::Sense::focusable_noninteractive(),
                );
                if focus.has_focus() {
                    ui.memory_mut(|memory| {
                        memory.set_focus_lock_filter(
                            list_id,
                            egui::EventFilter {
                                vertical_arrows: true,
                                ..Default::default()
                            },
                        )
                    });
                }
                if focus.gained_focus() {
                    self.review.scroll_selection = true;
                }
                egui::ScrollArea::vertical()
                    .id_salt("review-comments-scroll")
                    .animated(false)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if projection.items.is_empty() {
                            ui.label("没有匹配的批注；可更改筛选。索引诊断单独列出。");
                        }
                        for item in &projection.items {
                            let selected =
                                self.review.selected_comment.as_deref() == Some(&item.draft.id);
                            let resolution = if item.draft.resolved {
                                "已解决"
                            } else {
                                "未解决"
                            };
                            let attachment = if item.anchor_status == AnchorStatus::Attached {
                                "已锚定"
                            } else {
                                "失锚"
                            };
                            let excerpt: String = item
                                .draft
                                .body
                                .replace('\n', " ")
                                .chars()
                                .take(72)
                                .collect();
                            let lines = [
                                format!("{resolution} · {attachment} · {}", item.draft.id),
                                anchor_label(&item.draft.anchor),
                                excerpt,
                            ];
                            let row = ui.add_sized(
                                [ui.available_width(), 68.0],
                                egui::Button::selectable(selected, ""),
                            );
                            row.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::Button,
                                    ui.is_enabled(),
                                    selected,
                                    lines.join(" · "),
                                )
                            });
                            // 每行独立截断，避免 multiline Button 的单行 Truncate 吞掉来源和摘要。
                            for (line, text) in lines.into_iter().enumerate() {
                                let color = if line == 1 {
                                    theme::MUTED()
                                } else {
                                    theme::TEXT()
                                };
                                let mut job = egui::text::LayoutJob::simple_singleline(
                                    text,
                                    egui::FontId::proportional(13.0),
                                    color,
                                );
                                job.wrap.max_width = (row.rect.width() - 16.0).max(1.0);
                                job.wrap.max_rows = 1;
                                job.wrap.break_anywhere = true;
                                let galley = ui.fonts(|fonts| fonts.layout_job(job));
                                ui.painter().galley(
                                    row.rect.min + egui::vec2(8.0, 6.0 + line as f32 * 19.0),
                                    galley,
                                    color,
                                );
                            }
                            if selected && ui.memory(|m| m.has_focus(list_id)) {
                                ui.painter().rect_stroke(
                                    row.rect.shrink(1.0),
                                    3.0,
                                    egui::Stroke::new(1.5_f32, theme::ACCENT()),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            if selected && scroll {
                                row.scroll_to_me(Some(egui::Align::Center));
                            }
                            if row.clicked()
                                || (selected && activate && ui.clip_rect().contains_rect(row.rect))
                            {
                                self.edit_comment(&item.draft.id);
                            } else if selected && activate {
                                self.review.scroll_selection = true;
                                self.message = Some(
                                    "选中批注不在视野内；已请求定位，请看到条目后再按 Enter。"
                                        .into(),
                                );
                            }
                            row.on_hover_text(format!(
                                "作者：{}\n{}\n批注文档：{}{}",
                                item.draft.author,
                                item.draft.body,
                                item.document,
                                if item.read_only { "\n只读文档" } else { "" }
                            ));
                        }
                        ui.separator();
                        ui.strong(format!("提案 · {}", proposals.len()));
                        for (id, author, reason, status) in proposals {
                            let marker = if *status == ProposalStatus::Open {
                                "待审阅"
                            } else {
                                "已采纳"
                            };
                            if ui
                                .selectable_label(
                                    self.review.selected_proposal.as_deref() == Some(id),
                                    format!("{marker} · {author} · {reason}"),
                                )
                                .clicked()
                            {
                                self.review.selected_proposal = Some(id.clone());
                            }
                        }
                    });
            });
    }
}
