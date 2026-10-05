//! 当前普通源码的纯文本定位；请求、坐标、上下文与来源保护只消费 core。
mod heading;
#[cfg(test)]
mod tests;
mod view;
use super::{Tab, WorldeditApp};
use std::path::PathBuf;
use worldline_core::source_coordinates::{SourceCoordinates, SourceJumpPreview, SourcePosition};

#[derive(Default)]
pub(super) struct JumpState {
    pub open: bool,
    pub return_focus: Option<egui::Id>,
    path: Option<PathBuf>,
    query: String,
    focus_query: bool,
    notice: Option<String>,
    review: Option<JumpReview>,
    coordinates: Option<CoordinatesCache>,
    projection_serial: u64,
    rendered_serial: u64,
    preview_ignore_momentum: bool,
}
struct CoordinatesCache {
    path: PathBuf,
    source: String,
    index: Result<SourceCoordinates, String>,
}
struct JumpReview {
    version: u64,
    source: String,
    query: String,
    preview: Result<SourceJumpPreview, String>,
}
fn query_id() -> egui::Id {
    egui::Id::new("source-jump-query")
}
impl WorldeditApp {
    fn source_jump_blocked(&self, ctx: &egui::Context) -> bool {
        self.ime_composing
            || self.ime_source_draft.is_some()
            || self.command_palette.ime
            || self.command_palette.ime_frame
            || ctx.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Ime(_)))
            })
    }
    fn source_jump_position(&mut self, ctx: &egui::Context) -> Result<SourcePosition, String> {
        if self.source_jump_blocked(ctx) {
            return Err("输入法组合或未提交稿尚未完成，行列暂不可用".into());
        }
        let source = self.project.document(&self.active_file)?;
        if self
            .source_jump
            .coordinates
            .as_ref()
            .is_none_or(|cache| cache.path != self.active_file || cache.source != source)
        {
            self.source_jump.coordinates = Some(CoordinatesCache {
                path: self.active_file.clone(),
                source: source.to_owned(),
                index: SourceCoordinates::new(source),
            });
        }
        let id = egui::Id::new(("source", &self.active_file));
        let character = egui::TextEdit::load_state(ctx, id)
            .and_then(|state| state.cursor.char_range())
            .map_or(0, |range| range.primary.index);
        self.source_jump
            .coordinates
            .as_ref()
            .unwrap()
            .index
            .as_ref()
            .map_err(Clone::clone)?
            .position_at_character(source, character)
    }
    fn refresh_source_jump(&mut self, force: bool) -> bool {
        let Ok(source) = self.project.document(&self.active_file) else {
            return self.source_jump.review.take().is_some();
        };
        let changed = force
            || self.source_jump.review.as_ref().is_none_or(|review| {
                review.version != self.version
                    || review.source != source
                    || review.query != self.source_jump.query
            });
        if changed {
            let had_review = self.source_jump.review.is_some();
            self.source_jump.preview_ignore_momentum = true;
            self.source_jump.projection_serial = self.source_jump.projection_serial.wrapping_add(1);
            self.source_jump.review = Some(JumpReview {
                version: self.version,
                source: source.to_owned(),
                query: self.source_jump.query.clone(),
                preview: self.project.preview_source_jump(
                    &self.active_file,
                    source,
                    &self.source_jump.query,
                ),
            });
            self.source_jump.notice = had_review.then(|| "预览已更新；请确认目标后再次定位".into());
        }
        changed
    }
    pub(in crate::app) fn open_source_jump(&mut self, ctx: &egui::Context) {
        if self.tab != Tab::Edit || self.project.document(&self.active_file).is_err() {
            self.message = Some("请先打开一份普通源文件，再跳转到行列".into());
            return;
        }
        if self.source_jump_blocked(ctx) {
            self.message =
                Some("输入法组合或未提交稿尚未完成；源码已保留，行列定位暂不可用".into());
            return;
        }
        if self.source_jump.open && self.source_jump.path.as_ref() == Some(&self.active_file) {
            self.source_jump.focus_query = true;
            ctx.memory_mut(|memory| memory.request_focus(query_id()));
            self.sync_edit_layers(ctx);
            return;
        }
        let position = self.source_jump_position(ctx).ok();
        self.source_jump.open = true;
        self.source_jump.path = Some(self.active_file.clone());
        self.source_jump.return_focus = ctx
            .memory(|memory| memory.focused())
            .or(Some(egui::Id::new(("source", &self.active_file))));
        self.source_jump.query = position
            .map(|position| format!("{}:{}", position.line, position.column))
            .unwrap_or_default();
        self.source_jump.focus_query = true;
        self.source_jump.review = None;
        self.source_jump.notice = None;
        self.refresh_source_jump(true);
        ctx.memory_mut(|memory| memory.request_focus(query_id()));
        self.sync_edit_layers(ctx);
    }
    pub(in crate::app) fn close_source_jump(&mut self, ctx: &egui::Context) {
        self.source_jump.open = false;
        if self.tab == Tab::Edit && self.source_jump.path.as_ref() == Some(&self.active_file) {
            if let Some(id) = self.source_jump.return_focus {
                ctx.memory_mut(|memory| memory.request_focus(id));
            }
        }
    }
    pub(in crate::app) fn source_jump_shortcut(&mut self, ctx: &egui::Context) {
        // macOS 也只用 Control+G；不抢占 Command+G 的常见查找习惯。
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::G)) {
            if self.source_jump.open && self.edit_layer_is_top("source-jump") {
                self.close_source_jump(ctx);
            } else if self
                .command_palette
                .focus_stack
                .last()
                .is_none_or(|(kind, _)| matches!(*kind, "search" | "source-outline" | "commands"))
            {
                self.open_source_jump(ctx);
            }
        }
    }
    fn navigate_source_jump(&mut self, ctx: &egui::Context) -> bool {
        if self.source_jump_blocked(ctx)
            || self.tab != Tab::Edit
            || self.source_jump.path.as_ref() != Some(&self.active_file)
        {
            self.source_jump.notice =
                Some("当前稿或输入状态已变化，暂不可定位；请重新打开行列定位".into());
            return false;
        }
        let Some(review) = self.source_jump.review.as_ref() else {
            return false;
        };
        let source = match self.project.document(&self.active_file) {
            Ok(source) => source,
            Err(error) => {
                self.source_jump.notice = Some(error);
                return false;
            }
        };
        let range = if review.version != self.version
            || review.source != source
            || review.query != self.source_jump.query
            || self.source_jump.projection_serial != self.source_jump.rendered_serial
        {
            Err("定位预览已过期，请重新预览并确认；没有使用旧坐标".into())
        } else {
            review
                .preview
                .as_ref()
                .map_err(Clone::clone)
                .and_then(|preview| self.project.resolve_source_jump(preview, source))
        };
        let range = match range {
            Ok(range) => range,
            Err(error) => {
                self.refresh_source_jump(true);
                self.source_jump.notice = Some(error);
                return false;
            }
        };
        let source = source.to_owned();
        let mut origin = self.author_location(Some(ctx));
        // 与结构跳转共用返回时的真实来源/磁盘基线校验，无 AST 依赖。
        origin.source_outline = true;
        self.remember_author_location(origin);
        self.jump = None;
        self.close_source_jump(ctx);
        // 成功定位必须回到唯一源码编辑器；取消则保留所有下层窗口与查询。
        if self.search_open {
            self.close_search(ctx);
        }
        if self.source_outline.open {
            self.close_source_outline(ctx);
        }
        self.command_palette.open = false;
        super::search::request_diagnostic_selection(ctx, self.active_file.clone(), source, range);
        true
    }
}
