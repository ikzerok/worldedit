use super::{comment_lifecycle::ReviewAction, review_list::anchor_label, WorldeditApp};
use crate::theme;
use worldline_core::collaboration::{self, AnchorStatus, CommentAnchor};

impl WorldeditApp {
    pub(super) fn draw_comment_editor(&mut self, ui: &mut egui::Ui) {
        let Some(mut editor) = self.review.comment_editor.take() else {
            return;
        };
        let mut action = None;
        let mut saved = false;
        let stored = editor
            .original
            .as_ref()
            .and_then(|id| self.snapshot.as_ref()?.comment_index.comments.get(id));
        let status = self
            .snapshot
            .as_ref()
            .map_or(AnchorStatus::Detached, |snapshot| {
                collaboration::comment_anchor_status(
                    &self.project,
                    &snapshot.result,
                    &snapshot.map_index,
                    &editor.draft.anchor,
                )
            });
        let anchor_writable = status == AnchorStatus::Attached
            || stored.is_some_and(|comment| comment.draft.anchor == editor.draft.anchor);
        let read_only = stored.is_some_and(|comment| comment.read_only)
            || !self.project.authoring_diagnostics().is_empty();
        ui.add_space(12.0);
        theme::card().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("批注编辑");
                ui.label(&editor.draft.id);
                if ui.button("关闭批注").clicked() {
                    action = Some(ReviewAction::Close);
                }
            });
            ui.label(format!(
                "{} · {}",
                if editor.draft.resolved {
                    "已解决"
                } else {
                    "未解决"
                },
                if status == AnchorStatus::Attached {
                    "已锚定"
                } else {
                    "失锚"
                }
            ));
            if status == AnchorStatus::Detached {
                ui.colored_label(
                    theme::GOLD(),
                    "原锚点已失效；保留原引用，不自动绑定相邻对象或相似段落。",
                );
            }
            if read_only {
                ui.colored_label(theme::GOLD(), "只读文档或未知能力：可查看，不能改写。");
            }
            ui.add_enabled_ui(!read_only, |ui| {
                ui.label("作者");
                ui.text_edit_singleline(&mut editor.draft.author);
                ui.label("正文");
                ui.add(
                    egui::TextEdit::multiline(&mut editor.draft.body)
                        .desired_rows(5)
                        .desired_width(f32::INFINITY),
                );
                match &mut editor.draft.anchor {
                    CommentAnchor::Object { target } => {
                        ui.collapsing("明确修正对象身份", |ui| {
                            ui.label("完整类型与 ID；不按同名猜测");
                            ui.text_edit_singleline(&mut target.kind);
                            ui.text_edit_singleline(&mut target.id);
                        });
                    }
                    CommentAnchor::MapPlacement {
                        map_id,
                        placement_id,
                    } => {
                        ui.collapsing("明确修正地图标记", |ui| {
                            ui.label("地图 ID 与标记 ID");
                            ui.text_edit_singleline(map_id);
                            ui.text_edit_singleline(placement_id);
                        });
                    }
                    CommentAnchor::TextRange { .. } => {}
                }
                ui.checkbox(&mut editor.draft.resolved, "已解决（锚定状态独立）");
            });
            ui.label(theme::muted(format!(
                "准确来源：{}",
                anchor_label(&editor.draft.anchor)
            )));
            if let CommentAnchor::TextRange { quote, .. } = &editor.draft.anchor {
                ui.strong("锚定原文预览 · 覆盖完整源码行");
                ui.label(theme::muted(
                    "字符选区已扩大为完整行；这是保存到批注的原引用，不自动跟随改稿。",
                ));
                egui::ScrollArea::vertical()
                    .id_salt("review-anchor-quote")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        let mut text = quote.as_str();
                        ui.add(
                            egui::TextEdit::multiline(&mut text)
                                .desired_width(f32::INFINITY)
                                .desired_rows(2),
                        );
                    });
            }
            if ui
                .add_enabled(
                    status == AnchorStatus::Attached,
                    egui::Button::new("定位准确原文 / 对象"),
                )
                .clicked()
            {
                action = Some(ReviewAction::Source(editor.draft.anchor.clone()));
            }
            if status == AnchorStatus::Detached
                && matches!(editor.draft.anchor, CommentAnchor::TextRange { .. })
            {
                ui.collapsing("手动重新锚定（先核对完整行原文）", |ui| {
                    ui.add_enabled_ui(!read_only, |ui| {
                        ui.text_edit_singleline(&mut self.review.text_path);
                        ui.horizontal_wrapped(|ui| {
                            ui.label("起止行");
                            ui.text_edit_singleline(&mut self.review.text_start);
                            ui.text_edit_singleline(&mut self.review.text_end);
                        });
                        if ui.button("预览并选择此正文范围").clicked() {
                            let parsed = self
                                .review
                                .text_start
                                .parse::<u32>()
                                .ok()
                                .zip(self.review.text_end.parse::<u32>().ok());
                            if let Some((start, end)) = parsed {
                                let path = self.project.root.join(&self.review.text_path);
                                match self.review_text_anchor(&path, start, end) {
                                    Ok(anchor) => editor.draft.anchor = anchor,
                                    Err(error) => self.io_error = Some(error),
                                }
                            } else {
                                self.io_error = Some("重新锚定行号必须是正整数".into());
                            }
                        }
                    });
                });
            }
            let current = editor.version == self.version
                && editor.baseline == self.project.content_baseline();
            if !current {
                ui.colored_label(
                    theme::GOLD(),
                    "工程已变化；旧批注表单不能覆盖当前稿。输入保留，请明确处理后重开。",
                );
            }
            if ui
                .add_enabled(
                    !read_only
                        && anchor_writable
                        && current
                        && !editor.draft.author.trim().is_empty()
                        && !editor.draft.body.trim().is_empty(),
                    theme::primary("保存批注"),
                )
                .clicked()
            {
                saved = self.save_comment_editor(&editor);
            }
        });
        if !saved {
            self.review.comment_editor = Some(editor);
        }
        if let Some(action) = action {
            self.request_review_action(action);
        }
    }
}
