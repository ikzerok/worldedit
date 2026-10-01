use super::WorldeditApp;
use crate::app::{search, Tab};
use worldline_core::collaboration::{self, CommentAnchor};

impl WorldeditApp {
    pub(in crate::app) fn comment_current_selection(&mut self, ctx: &egui::Context) {
        if self.review_ime_active() || self.ime_source_draft.is_some() {
            self.io_error = Some("请先完成输入法组合，再选择批注范围；当前输入保留。".into());
            return;
        }
        let Some(selection) = search::editor_selection(ctx) else {
            self.io_error = Some("请先在当前正文中选择一段文字，再添加批注。".into());
            return;
        };
        let current = if self.tab == Tab::Manuscript {
            self.manuscript.comment_selection_is_current_mode()
                && self
                    .manuscript
                    .active_writing_target()
                    .is_some_and(|(target, path)| {
                        path == selection.path && selection.target.as_ref() == Some(&target)
                    })
        } else {
            self.tab == Tab::Edit
                && selection.path == self.active_file
                && selection.target.is_none()
        };
        if !current {
            self.io_error = Some("选区不是当前文稿/编辑模式的可见来源；切换模式后请重新选择，没有猜测隐藏选区或其他文件。".into());
            return;
        }
        if self.review_file_has_unapplied(&selection.path) {
            self.io_error = Some(
                "此来源仍有未应用正文；请先明确应用此文件，再重新选择，未建立旧版锚点。".into(),
            );
            return;
        }
        match collaboration::capture_text_selection(
            &self.project,
            &selection.path,
            &selection.source,
            selection.range,
        ) {
            Ok(anchor) => self.new_comment_for_anchor(anchor),
            Err(error) => self.io_error = Some(error),
        }
    }

    fn review_file_has_unapplied(&self, path: &std::path::Path) -> bool {
        self.manuscript
            .writing_buffers()
            .iter()
            .any(|buffer| buffer.path() == path && buffer.is_changed())
    }
    pub(super) fn review_text_anchor(
        &self,
        path: &std::path::Path,
        start: u32,
        end: u32,
    ) -> Result<CommentAnchor, String> {
        if self.ime_composing
            || self.ime_source_draft.is_some()
            || self.review_file_has_unapplied(path)
        {
            return Err("此文件存在未应用正文或输入法草稿；请先明确应用/完成输入，再重新指定范围，未锚定旧版。".into());
        }
        collaboration::capture_text_anchor(&self.project, path, start, end)
    }

    pub(super) fn navigate_comment_anchor(&mut self, anchor: &CommentAnchor) {
        match anchor {
            CommentAnchor::Object { target } => {
                let object = self
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.result.analysis.catalog.object(target))
                    .cloned();
                if let Some(object) = object {
                    self.navigate_object(&object);
                } else {
                    self.io_error = Some("原对象已失效；批注保留，未导航到同名对象。".into());
                }
            }
            CommentAnchor::TextRange {
                path,
                start_line,
                end_line,
                quote,
                ..
            } => {
                let absolute = self.project.root.join(path);
                let source = match self.project.document(&absolute) {
                    Ok(source) => source.to_owned(),
                    Err(error) => {
                        self.io_error = Some(error);
                        return;
                    }
                };
                let current = collaboration::capture_text_anchor(
                    &self.project,
                    &absolute,
                    *start_line,
                    *end_line,
                );
                if current.as_ref() != Ok(anchor) {
                    self.io_error = Some("原文锚点已失效；保留原引用，不自动定位相似段落。".into());
                    return;
                }
                let mut start = 0;
                for _ in 1..*start_line {
                    start += source[start..].find('\n').map_or(0, |at| at + 1);
                }
                // quote 按行规范化换行；选择范围按当前源原始字节计算。
                let end = source
                    .split_inclusive('\n')
                    .take(*end_line as usize)
                    .map(str::len)
                    .sum::<usize>();
                let end = end.min(source.len());
                let _ = quote;
                self.jump_to_file(&absolute.to_string_lossy(), *start_line, 1);
                // 精确范围恢复拥有光标；普通行号跳转不能在同帧末覆盖为单光标。
                self.jump = None;
                self.review.pending_source_selection = Some((absolute, source, start..end));
            }
            CommentAnchor::MapPlacement {
                map_id,
                placement_id,
            } => {
                let exists = self
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.map_index.maps.get(map_id))
                    .is_some_and(|map| map.placements.contains_key(placement_id));
                if exists {
                    self.locate_reference(map_id, placement_id);
                    self.message = Some(format!("批注来源：地图 {map_id} · 标记 {placement_id}"));
                } else {
                    self.io_error = Some("原地图标记已失效；未选择其他标记。".into());
                }
            }
        }
    }
}
