use super::{CommentEditor, WorldeditApp};
use crate::app::Tab;
use worldline_core::collaboration::{CommentAnchor, CommentDraft, CommentReviewFilter};

#[derive(Clone)]
pub(in crate::app) enum ReviewAction {
    Edit(String),
    New(CommentAnchor),
    Filter(CommentReviewFilter),
    Close,
    Unresolved,
    Navigate(Tab),
    Source(CommentAnchor),
}
impl WorldeditApp {
    pub(in crate::app) fn review_ime_active(&self) -> bool {
        self.ime_composing || self.command_palette.ime || self.review.composition_frame
    }
    pub(in crate::app) fn comment_draft_dirty(&self) -> bool {
        self.review
            .comment_editor
            .as_ref()
            .is_some_and(|editor| editor.draft != editor.initial || self.review_ime_active())
    }
    pub(in crate::app) fn request_review_action(&mut self, action: ReviewAction) {
        if self.comment_draft_dirty() {
            self.review.pending_comment_action = Some(action);
            return;
        }
        self.perform_review_action(action);
    }
    fn perform_review_action(&mut self, action: ReviewAction) {
        match action {
            ReviewAction::Edit(id) => {
                let Some(comment) = self
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.comment_index.comments.get(&id))
                else {
                    self.io_error = Some("批注已不存在；原输入保留，请刷新后检查。".into());
                    return;
                };
                self.review.comment_editor = Some(CommentEditor {
                    original: Some(id.clone()),
                    draft: comment.draft.clone(),
                    initial: comment.draft.clone(),
                    baseline: self.project.content_baseline(),
                    version: self.version,
                });
                self.io_error = None;
                self.review.selected_comment = Some(id);
                self.review.scroll_selection = true;
                self.tab = Tab::Review;
            }
            ReviewAction::New(anchor) => {
                let draft = CommentDraft {
                    id: self.next_comment_id(),
                    author: self.review.author.clone(),
                    body: String::new(),
                    anchor,
                    resolved: false,
                };
                self.review.comment_editor = Some(CommentEditor {
                    original: None,
                    initial: draft.clone(),
                    draft,
                    baseline: self.project.content_baseline(),
                    version: self.version,
                });
                self.io_error = None;
                self.reset_new_draft_baseline("审阅批注");
                self.tab = Tab::Review;
            }
            ReviewAction::Filter(filter) => {
                self.review.filter = filter;
                self.review.scroll_selection = true;
            }
            ReviewAction::Unresolved => {
                self.review.filter = CommentReviewFilter::default();
                self.tab = Tab::Review;
                self.review.scroll_selection = true;
            }
            ReviewAction::Close => self.review.comment_editor = None,
            ReviewAction::Navigate(tab) => self.tab = tab,
            ReviewAction::Source(anchor) => self.navigate_comment_anchor(&anchor),
        }
        self.review.last_tab = Some(self.tab);
    }
    pub(in crate::app) fn review_navigation_guard(&mut self, ctx: &egui::Context) {
        // 失焦产生的纯 Disabled 不是组合/提交；不能让鼠标入口必须点两次。
        self.review.composition_frame = ctx.input(|input| {
            input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Ime(
                        egui::ImeEvent::Enabled
                            | egui::ImeEvent::Preedit(_)
                            | egui::ImeEvent::Commit(_)
                    )
                )
            })
        });

        if self.review.last_tab == Some(Tab::Review)
            && self.tab != Tab::Review
            && self.comment_draft_dirty()
        {
            let destination = self.tab;
            self.tab = Tab::Review;
            if self.review.pending_comment_action.is_none() {
                self.review.pending_comment_action = Some(ReviewAction::Navigate(destination));
            }
        }
        if let Some((path, source, range)) = self.review.pending_source_selection.take() {
            crate::app::search::request_selection(ctx, path, source, range);
        }
        self.review.last_tab = Some(self.tab);
    }
    pub(in crate::app) fn review_draft_dialog(&mut self, ctx: &egui::Context) {
        if self.review.pending_comment_action.is_none() {
            return;
        }
        let mut discard = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::new("review-draft-guard")).show(ctx, |ui| {
            ui.heading("批注仍有未应用输入");
            ui.label("取消可继续编辑原批注；明确放弃只清除此批注输入，不影响正文或其他草稿。");
            ui.horizontal_wrapped(|ui| {
                if ui.button("取消，保留批注输入").clicked() {
                    cancel = true;
                }
                if ui
                    .add_enabled(
                        !self.review_ime_active(),
                        egui::Button::new("明确放弃此批注输入并继续"),
                    )
                    .clicked()
                {
                    discard = true;
                }
            });
        });
        if cancel {
            self.review.pending_comment_action = None;
        }
        if discard {
            let action = self.review.pending_comment_action.take().unwrap();
            // 丢弃仅恢复本表单打开时的原稿；不动工程、其他草稿和撤销层。
            if let Some(editor) = self.review.comment_editor.as_mut() {
                editor.draft = editor.initial.clone();
            }
            self.perform_review_action(action);
        }
    }
    pub(in crate::app) fn open_unresolved_comments(&mut self) {
        self.request_review_action(ReviewAction::Unresolved);
    }
}
