//! 只含显示参数、字符锚点与摘要；不持有源码，也不提交作者草稿。
use super::Settings;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(in crate::app) struct SourceView {
    layout: Layout,
    source: String,
    anchor: usize,
    prefer_next_row: bool,
    relative: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Layout {
    width: f32,
    height: f32,
    body_size: f32,
    line_spacing: f32,
    wrap: bool,
}

pub(in crate::app) struct SourceFrame {
    galley: Arc<egui::Galley>,
    origin: egui::Pos2,
    input_offset: egui::Vec2,
    cursor: Option<egui::text::CCursor>,
    layout: Layout,
}

impl SourceFrame {
    pub(in crate::app) fn new(
        output: &egui::text_edit::TextEditOutput,
        viewport: egui::Rect,
        settings: &Settings,
    ) -> Self {
        Self {
            galley: output.galley.clone(),
            origin: output.galley_pos,
            input_offset: viewport.min.to_vec2(),
            cursor: output.state.cursor.char_range().map(|range| range.primary),
            layout: Layout {
                width: viewport.width(),
                height: viewport.height(),
                body_size: settings.body_size,
                line_spacing: settings.line_spacing,
                wrap: settings.source_wrap,
            },
        }
    }

    /// ScrollArea结束后使用其真实视口与限位。布局变化只重定位，不碰TextEdit状态。
    pub(in crate::app) fn finish(
        mut self,
        ctx: &egui::Context,
        scroll: &mut egui::scroll_area::ScrollAreaOutput<()>,
        previous: Option<&SourceView>,
        explicit_navigation: bool,
    ) -> SourceView {
        let source = super::super::writing_workspace::fingerprint(&self.galley.job.text);
        let viewport = scroll.inner_rect;
        // show_viewport的Rect含scroll offset；大偏移相减会产生f32抖动。
        // 重排签名只用屏幕上的真实视口尺寸，不能把动画误判成窗口缩放。
        self.layout.width = viewport.width();
        self.layout.height = viewport.height();
        let mut offset = scroll.state.offset;
        if let Some(previous) = previous.filter(|previous| {
            !explicit_navigation
                && previous.source == source
                && previous.layout != self.layout
                && previous.anchor <= self.galley.job.text.chars().count()
                && previous.relative.iter().all(|value| value.is_finite())
        }) {
            let anchor = egui::text::CCursor {
                index: previous.anchor,
                prefer_next_row: previous.prefer_next_row,
            };
            let local = self.galley.pos_from_cursor(anchor);
            let desired = egui::vec2(
                previous.relative[0].clamp(0.0, viewport.width().max(0.0)),
                previous.relative[1].clamp(0.0, (viewport.height() - local.height()).max(0.0)),
            );
            offset = self.input_offset + self.origin.to_vec2() + local.min.to_vec2()
                - viewport.min.to_vec2()
                - desired;
            let limit = (scroll.content_size - viewport.size()).max(egui::Vec2::ZERO);
            offset = offset.max(egui::Vec2::ZERO).min(limit);
            if self.layout.wrap {
                offset.x = 0.0;
            }
            // 清除上次命中的动画/惯性，避免下一帧继续回到旧像素位置。
            let mut state = egui::scroll_area::State::default();
            state.offset = offset;
            scroll.state = state;
            scroll.state.store(ctx, scroll.id);
            ctx.request_repaint();
        }
        let origin = self.origin + self.input_offset - offset;
        let visible_cursor = self.cursor.filter(|cursor| {
            viewport.contains(
                self.galley
                    .pos_from_cursor(*cursor)
                    .translate(origin.to_vec2())
                    .center(),
            )
        });
        let anchor = visible_cursor
            .unwrap_or_else(|| self.galley.cursor_from_pos(viewport.left_top() - origin));
        let position = self.galley.pos_from_cursor(anchor).min + origin.to_vec2();
        SourceView {
            layout: self.layout,
            source,
            anchor: anchor.index,
            prefer_next_row: anchor.prefer_next_row,
            relative: [position.x - viewport.left(), position.y - viewport.top()],
        }
    }
}
