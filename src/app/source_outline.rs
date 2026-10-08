//! 普通源码的单文件结构；所有声明、层级与字节范围只读 core 正式投影。
mod heading;
#[cfg(test)]
mod tests;
mod view;
use super::{Tab, WorldeditApp};
use std::path::PathBuf;
use worldline_core::source_outline::{SourceOutline, SourceOutlineEntry, SourceOutlineStatus};

#[derive(Default)]
pub(super) struct OutlineState {
    pub open: bool,
    pub return_focus: Option<egui::Id>,
    path: Option<PathBuf>,
    query: String,
    last_query: String,
    selected: usize,
    focus_query: bool,
    scroll_selected: bool,
    notice: Option<String>,
    projection_serial: u64,
    rendered_serial: u64,
    cache: Option<CachedOutline>,
}
struct CachedOutline {
    version: u64,
    source: String,
    outline: SourceOutline,
}
fn query_id() -> egui::Id {
    egui::Id::new("source-outline-query")
}
fn metadata(entry: &SourceOutlineEntry) -> String {
    let kind = match entry.kind.as_str() {
        "scene" => "场景",
        "fragment" => "片段",
        "let" => "变量",
        "const" => "常量",
        "rule" => "规则",
        "schema" => "资料模式",
        other => super::catalog::kind_label(other),
    };
    let detail = entry
        .entity_type
        .as_ref()
        .map(|kind| format!(" / {kind}"))
        .unwrap_or_default();
    format!("{kind}{detail} · {} · 第 {} 行", entry.id, entry.line)
}
fn filtered(outline: &SourceOutline, query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    outline
        .entries
        .iter()
        .filter(|entry| {
            [&entry.display, &entry.id, &entry.kind, &metadata(entry)]
                .iter()
                .any(|part| part.to_lowercase().contains(&query))
        })
        .map(|entry| entry.occurrence)
        .collect()
}

impl WorldeditApp {
    fn source_outline_blocked(&self, ctx: &egui::Context) -> bool {
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
    /// 文件、真实正文与工程代次相同就复用。查询/光标移动不触发解析。
    fn refresh_source_outline(&mut self, force: bool) -> bool {
        let Ok(source) = self.project.document(&self.active_file) else {
            return self.source_outline.cache.take().is_some();
        };
        let changed = force
            || self.source_outline.cache.as_ref().is_none_or(|cache| {
                cache.version != self.version
                    || cache.outline.path != self.active_file
                    || cache.source != source
            });
        if changed {
            self.source_outline.projection_serial =
                self.source_outline.projection_serial.wrapping_add(1);
            self.source_outline.notice = None;
            self.source_outline.cache = Some(CachedOutline {
                version: self.version,
                source: source.to_owned(),
                outline: self.project.source_outline(&self.active_file, source),
            });
            self.source_outline.selected = 0;
            self.source_outline.scroll_selected = true;
        }
        changed
    }
    fn source_outline_current(&self, ctx: &egui::Context) -> Option<usize> {
        if self.source_outline_blocked(ctx) {
            return None;
        }
        let cache = self.source_outline.cache.as_ref()?;
        if cache.version != self.version
            || cache.outline.path != self.active_file
            || self.project.document(&self.active_file).ok() != Some(cache.source.as_str())
        {
            return None;
        }
        let range = egui::TextEdit::load_state(ctx, egui::Id::new(("source", &self.active_file)))?
            .cursor
            .char_range()?;
        // 非空选区的末端是半开边界；用选区内紧邻活动端的字符确认归属。
        let cursor = range.primary.index - usize::from(range.primary.index > range.secondary.index);
        let byte = cache
            .source
            .char_indices()
            .nth(cursor)
            .map_or(cache.source.len(), |(at, _)| at);
        cache
            .outline
            .current_item(byte)
            .map(|entry| entry.occurrence)
    }
    pub(in crate::app) fn open_source_outline(&mut self, ctx: &egui::Context) {
        if self.tab != Tab::Edit || self.project.document(&self.active_file).is_err() {
            self.message = Some("请先打开一份普通源文件，再查看本文件结构".into());
            return;
        }
        if self.source_outline_blocked(ctx) {
            self.message =
                Some("输入法组合或未提交稿尚未完成；源码已保留，结构定位暂不可用".into());
            return;
        }
        self.refresh_source_outline(true);
        self.source_outline.open = true;
        self.source_outline.path = Some(self.active_file.clone());
        self.source_outline.return_focus = Some(egui::Id::new(("source", &self.active_file)));
        self.source_outline.query.clear();
        self.source_outline.last_query.clear();
        self.source_outline.selected = self.source_outline_current(ctx).unwrap_or(0);
        self.source_outline.focus_query = true;
        self.source_outline.scroll_selected = true;
        self.source_outline.notice = None;
        ctx.memory_mut(|memory| memory.request_focus(query_id()));
        self.sync_edit_layers(ctx);
    }
    pub(in crate::app) fn close_source_outline(&mut self, ctx: &egui::Context) {
        self.source_outline.open = false;
        if self.tab == Tab::Edit && self.source_outline.path.as_ref() == Some(&self.active_file) {
            if let Some(id) = self.source_outline.return_focus {
                ctx.memory_mut(|memory| memory.request_focus(id));
            }
        }
    }
    pub(in crate::app) fn source_outline_shortcut(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|input| {
            input.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::O,
            )
        }) {
            if self.source_outline.open && self.edit_layer_is_top("source-outline") {
                self.close_source_outline(ctx);
            } else if self.command_palette.focus_stack.is_empty() {
                self.open_source_outline(ctx);
            }
        }
    }
    fn navigate_source_outline(&mut self, ctx: &egui::Context, occurrence: usize) -> bool {
        if self.source_outline_blocked(ctx)
            || self.tab != Tab::Edit
            || self.source_outline.path.as_ref() != Some(&self.active_file)
        {
            self.source_outline.notice =
                Some("当前稿或输入状态已变化，暂不可定位；请完成输入后重新打开结构".into());
            return false;
        }
        let Some(cache) = self.source_outline.cache.as_ref() else {
            return false;
        };
        let source = match self.project.document(&self.active_file) {
            Ok(source) => source,
            Err(error) => {
                self.source_outline.notice = Some(error);
                return false;
            }
        };
        let range = if cache.version != self.version || cache.source != source {
            Err("源码已变化，请刷新结构后重新选择；没有使用旧坐标".into())
        } else {
            self.project
                .source_outline_range(&cache.outline, source, occurrence)
        };
        let range = match range {
            Ok(range) => range,
            Err(error) => {
                self.refresh_source_outline(true);
                self.source_outline.notice = Some(error);
                return false;
            }
        };
        let source = source.to_owned();
        let mut origin = self.author_location(Some(ctx));
        origin.source_outline = true;
        self.remember_author_location(origin);
        self.jump = None;
        self.close_source_outline(ctx);
        super::search::request_diagnostic_selection(ctx, self.active_file.clone(), source, range);
        true
    }
}
