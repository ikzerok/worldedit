//! 设备个人状态；从不写入 Project、源码或展示文档。
mod source_position;
pub(super) mod source_view;
mod ui;
use super::{Tab, WorldeditApp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use worldline_core::TargetRef;

const KEY: &str = "worldedit.personal.v1";
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Settings {
    pub body_size: f32,
    pub line_spacing: f32,
    pub reading_width: f32,
    pub theme: crate::theme::ThemeMode,
    pub navigation: bool,
    pub diagnostics: bool,
    pub source_wrap: bool,
    pub focus: bool,
    pub dock_references: bool,
    pub references_visible: bool,
    pub navigation_width: f32,
    pub reference_width: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            body_size: 16.0,
            line_spacing: 1.45,
            reading_width: 840.0,
            theme: crate::theme::ThemeMode::Dark,
            navigation: true,
            diagnostics: false,
            source_wrap: false,
            focus: false,
            dock_references: true,
            references_visible: true,
            navigation_width: 212.0,
            reference_width: 330.0,
        }
    }
}
impl Settings {
    fn normalize(&mut self) {
        fn bounded(value: f32, default: f32, min: f32, max: f32) -> f32 {
            if value.is_finite() {
                value.clamp(min, max)
            } else {
                default
            }
        }
        self.body_size = bounded(self.body_size, 16.0, 12.0, 28.0);
        self.line_spacing = bounded(self.line_spacing, 1.45, 1.0, 2.0);
        self.reading_width = bounded(self.reading_width, 840.0, 480.0, 1400.0);
        self.navigation_width = bounded(self.navigation_width, 212.0, 180.0, 320.0);
        self.reference_width = bounded(self.reference_width, 330.0, 260.0, 440.0);
    }
}
#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(super) struct Location {
    pub tab: Option<Tab>,
    pub file: PathBuf,
    pub source_entity: Option<String>,
    pub cursor: Option<usize>,
    pub source_scroll: [f32; 2],
    pub source_baseline: Option<String>,
    pub source_secondary: Option<usize>,
    #[serde(skip)]
    pub source_selection_diagnostic: bool,
    #[serde(skip)]
    pub source_outline: bool,
    #[serde(skip)]
    pub source_problem: Option<std::sync::Arc<super::problems::SourceProblem>>,
    pub source_view: Option<source_view::SourceView>,
    pub target: Option<TargetRef>,
    pub editor: Option<TargetRef>,
    pub event: Option<String>,
    pub manuscript: serde_json::Value,
    #[serde(skip)]
    pub comparison: Option<super::play::comparison::ComparisonLocation>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct WorkspaceView {
    location: Location,
    references: Vec<TargetRef>,
}
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub(super) struct PersonalState {
    schema_version: u32,
    pub settings: Settings,
    last_project: Option<PathBuf>,
    workspaces: BTreeMap<PathBuf, WorkspaceView>,
    #[serde(skip)]
    pub history: Vec<Location>,
    #[serde(skip)]
    pub source_scroll: [f32; 2],
    #[serde(skip)]
    pub restore_source: bool,
    #[serde(skip)]
    pub source_cursor: Option<(PathBuf, usize)>,
    #[serde(skip)]
    pub source_view: Option<(PathBuf, source_view::SourceView)>,
    #[serde(skip)]
    pub preferences_open: bool,
    #[serde(skip)]
    pub catalog_drawer_open: bool,
    #[serde(skip)]
    pub pending_restore: bool,
    #[cfg(target_arch = "wasm32")]
    #[serde(skip)]
    last_browser_saved: String,
}
impl Default for PersonalState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            settings: Settings::default(),
            last_project: None,
            workspaces: BTreeMap::new(),
            history: Vec::new(),
            source_scroll: [0.0; 2],
            restore_source: false,
            source_cursor: None,
            source_view: None,
            preferences_open: false,
            catalog_drawer_open: false,
            pending_restore: false,
            #[cfg(target_arch = "wasm32")]
            last_browser_saved: String::new(),
        }
    }
}
impl PersonalState {
    pub fn restore(storage: Option<&dyn eframe::Storage>) -> Self {
        let text = storage.and_then(|s| s.get_string(KEY));
        #[cfg(target_arch = "wasm32")]
        let text = text.or_else(|| {
            web_sys::window()?
                .local_storage()
                .ok()??
                .get_item(KEY)
                .ok()?
        });
        text.and_then(|s| Self::decode(&s)).unwrap_or_default()
    }
    fn decode(text: &str) -> Option<Self> {
        if text.len() > 1_048_576 {
            return None;
        }
        let mut value: Self = serde_json::from_str(text).ok()?;
        if value.schema_version != 1 || value.workspaces.len() > 64 {
            return None;
        }
        value.settings.normalize();
        for workspace in value.workspaces.values_mut() {
            workspace.references.truncate(2);
        }
        Some(value)
    }
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        if let Ok(text) = serde_json::to_string(self) {
            storage.set_string(KEY, text);
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub fn save_browser(&mut self) {
        if let (Some(storage), Ok(text)) = (
            web_sys::window().and_then(|w| w.local_storage().ok().flatten()),
            serde_json::to_string(self),
        ) {
            if self.last_browser_saved != text && storage.set_item(KEY, &text).is_ok() {
                self.last_browser_saved = text;
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn last_project(&self) -> Option<PathBuf> {
        self.last_project.clone()
    }
}
impl WorldeditApp {
    pub(super) fn personal_workspace_key(&self) -> PathBuf {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.project.root.clone()
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.project
                .root
                .join(format!("session-{}", self.project.checkpoint_session_id()))
        }
    }
    fn open_object_identity(&self) -> Option<TargetRef> {
        if let Some(id) = self
            .entity_editor
            .as_ref()
            .and_then(|f| f.original.as_deref())
        {
            return Some(TargetRef::new("entity", id));
        }
        if let Some(id) = self
            .relation_editor
            .as_ref()
            .and_then(|f| f.original.as_deref())
        {
            return Some(TargetRef::new("relation", id));
        }
        match self.tab {
            Tab::Characters => self
                .character_editor
                .as_ref()
                .and_then(|f| f.original.as_deref())
                .map(|id| TargetRef::new("character", id)),
            Tab::Timeline | Tab::Graph => self
                .event_editor
                .as_ref()
                .and_then(|f| f.original.as_deref())
                .map(|id| TargetRef::new("event", id)),
            _ => None,
        }
    }
    pub(super) fn author_location(&self, ctx: Option<&egui::Context>) -> Location {
        let source = self.source_position_document(&self.active_file);
        let selection = source.and_then(|(id, _)| {
            ctx.and_then(|ctx| egui::TextEdit::load_state(ctx, id))
                .and_then(|state| state.cursor.char_range())
        });
        let cursor = selection.map(|range| range.primary.index).or_else(|| {
            source.and_then(|_| {
                self.personal
                    .source_cursor
                    .as_ref()
                    .filter(|(path, _)| path == &self.active_file)
                    .map(|(_, cursor)| *cursor)
            })
        });
        Location {
            tab: Some(self.tab),
            file: self.active_file.clone(),
            source_entity: self
                .entity_source_navigation
                .as_ref()
                .filter(|(_, path)| *path == self.active_file && self.tab == Tab::Edit)
                .map(|(id, _)| id.clone()),
            cursor,
            source_scroll: self.personal.source_scroll,
            source_view: source.and_then(|_| {
                self.personal
                    .source_view
                    .as_ref()
                    .filter(|(path, _)| path == &self.active_file)
                    .map(|(_, view)| view.clone())
            }),
            source_baseline: source.map(|(_, text)| super::writing_workspace::fingerprint(text)),
            source_problem: self.capture_problem_source(),
            source_secondary: selection.map(|range| range.secondary.index),
            source_outline: false,
            source_selection_diagnostic: source.is_some_and(|(id, _)| {
                ctx.is_some_and(|ctx| super::search::selection_is_diagnostic(ctx, id, selection))
            }),
            target: self
                .open_object_identity()
                .or_else(|| self.catalog_target.clone()),
            editor: self.open_object_identity(),
            event: self.focus_event.clone(),
            manuscript: serde_json::to_value(self.manuscript_session()).unwrap_or_default(),
            comparison: self.comparison_location(ctx),
        }
    }
    pub(super) fn remember_author_position(&mut self) {
        self.remember_author_location(self.author_location(None));
    }
    pub(super) fn remember_author_location(&mut self, position: Location) {
        if self.personal.history.last() != Some(&position) {
            self.personal.history.push(position);
            if self.personal.history.len() > 64 {
                self.personal.history.remove(0);
            }
        }
    }
    pub(super) fn switch_tab(&mut self, tab: Tab) {
        if self.tab != tab {
            self.remember_author_position();
            self.personal.catalog_drawer_open = false;
            self.tab = tab;
        }
    }
    pub(super) fn author_back(&mut self, ctx: &egui::Context) {
        if self.tab == Tab::Edit && (self.ime_composing || self.ime_source_draft.is_some()) {
            self.message =
                Some("输入法组合或未提交稿仍待处理，未恢复旧源码位置；当前输入已保留".into());
            return;
        }
        if self.return_search_source_if_open(ctx) {
            return;
        }
        if let Some(location) = self.personal.history.pop() {
            self.restore_author_location(location, ctx);
        }
    }
    pub(super) fn capture_personal_view(&mut self, ctx: &egui::Context) {
        let location = self.author_location(Some(ctx));
        self.personal.source_cursor = location
            .cursor
            .map(|cursor| (location.file.clone(), cursor));
        if !self.saved_location {
            return;
        }
        let view = WorkspaceView {
            location,
            references: self
                .reading_panels
                .ids()
                .iter()
                .filter_map(|id| self.reading_panels.get(*id).map(|p| p.target.clone()))
                .collect(),
        };
        self.personal.last_project = Some(self.project.entry.clone());
        if self.personal.workspaces.len() >= 64
            && !self
                .personal
                .workspaces
                .contains_key(&self.personal_workspace_key())
        {
            if let Some(key) = self.personal.workspaces.keys().next().cloned() {
                self.personal.workspaces.remove(&key);
            }
        }
        self.personal
            .workspaces
            .insert(self.personal_workspace_key(), view);
    }
    pub(super) fn restore_personal_view(&mut self, ctx: &egui::Context) {
        self.personal.pending_restore = false;
        let Some(view) = self
            .personal
            .workspaces
            .get(&self.personal_workspace_key())
            .cloned()
        else {
            return;
        };
        self.restore_author_location(view.location, ctx);
        self.reading_panels.clear();
        for target in view.references {
            if self
                .snapshot
                .as_ref()
                .is_some_and(|s| s.result.analysis.catalog.object(&target).is_some())
            {
                self.reading_panels.pin(target);
            } else {
                self.message = Some("上次的部分参考对象已不存在，未按同名替换".into());
            }
        }
    }
    pub(in crate::app) fn restore_author_location(
        &mut self,
        mut location: Location,
        ctx: &egui::Context,
    ) {
        let entity_source = self.rebase_entity_source_location(&mut location);
        if location.tab == Some(Tab::Manuscript) {
            if let Ok(mut session) = serde_json::from_value::<super::manuscript::ManuscriptSession>(
                location.manuscript.clone(),
            ) {
                match self.manuscript.validate_session(&self.project, &session) {
                    Err(error) => {
                        self.message = Some(error);
                        return;
                    }
                    Ok(false) => {
                        session.cursor = None;
                        session.restore_offsets = Some(false);
                        self.message = Some("已返回章节与模式；来源或草稿版本已变化，未恢复旧选区和滚动，当前稿完整保留".into());
                    }
                    Ok(true) => {}
                }
                location.manuscript = serde_json::to_value(session).unwrap_or_default();
            }
        }
        super::search::clear_pending_selection(ctx);
        self.jump = None;
        let known = self
            .project
            .documents
            .get(&location.file)
            .is_some_and(|document| !document.is_deleted())
            || self
                .project
                .authoring_document(&location.file)
                .is_ok_and(|d| !d.is_deleted());
        if known {
            let returning_to_current_file = self.active_file == location.file;
            self.active_file = location.file;
            let source_current = location.source_baseline.as_ref().is_some_and(|baseline| {
                self.source_position_document(&self.active_file)
                    .is_some_and(|(_, source)| {
                        super::writing_workspace::fingerprint(source) == *baseline
                            && (!location.source_outline
                                || self
                                    .project
                                    .verify_source_navigation(&self.active_file, source)
                                    .is_ok())
                    })
            });
            if location.tab == Some(Tab::Edit) && source_current {
                self.personal.source_scroll =
                    location
                        .source_scroll
                        .map(|v| if v.is_finite() { v.max(0.0) } else { 0.0 });
                self.personal.restore_source = true;
                self.personal.source_view = location
                    .source_view
                    .clone()
                    .map(|view| (self.active_file.clone(), view));
                if let Some(cursor) = location.cursor {
                    let Some((id, source)) = self.source_position_document(&self.active_file)
                    else {
                        return;
                    };
                    let length = source.chars().count();
                    let cursor = cursor.min(length);
                    self.personal.source_cursor = Some((self.active_file.clone(), cursor));
                    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                    state.cursor.set_char_range(Some(egui::text::CCursorRange {
                        primary: egui::text::CCursor::new(cursor),
                        secondary: egui::text::CCursor::new(
                            location.source_secondary.unwrap_or(cursor).min(length),
                        ),
                        h_pos: None,
                    }));
                    super::search::restore_origin(
                        ctx,
                        id,
                        state.cursor.char_range(),
                        location.source_selection_diagnostic,
                    );
                    egui::TextEdit::store_state(ctx, id, state);
                    ctx.memory_mut(|memory| memory.request_focus(id));
                    super::search::record_navigation_focus(ctx, id);
                }
            } else if location.tab == Some(Tab::Edit) {
                location.source_problem = None;
                let current_selection = returning_to_current_file
                    && super::search::editor_selection(ctx).is_some_and(|selection| {
                        selection.path == self.active_file
                            && selection.target.is_none()
                            && self
                                .source_position_document(&self.active_file)
                                .map(|(_, text)| text)
                                == Some(selection.source.as_str())
                    });
                self.personal.restore_source = !current_selection;
                self.personal.source_view = None;
                if !current_selection {
                    self.personal.source_scroll = [0.0, 0.0];
                    if let Some((id, _)) = self.source_position_document(&self.active_file) {
                        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(
                                egui::text::CCursor::new(0),
                            )));
                        state.store(ctx, id);
                    }
                }
                self.personal.source_cursor = None;
                self.message = Some(
                    if self.source_position_document(&self.active_file).is_none() {
                        "已返回原始文档；没有可定位的有效UTF-8原文，未恢复文本坐标，原字节完整保留"
                            .into()
                    } else {
                        "已返回源文件；来源版本已变化，未恢复旧选区和滚动，当前内容完整保留".into()
                    },
                );
            }
        } else {
            self.message = Some("上次的来源文件已不存在，保留当前入口".into());
            if location.tab == Some(Tab::Edit) {
                return;
            }
        }
        self.restore_comparison_location(ctx, location.comparison);
        self.restore_problem_source(location.source_problem);
        if let Some(tab) = location.tab {
            self.tab = tab;
        }
        self.catalog_target = location.target.filter(|target| {
            self.snapshot
                .as_ref()
                .is_some_and(|s| s.result.analysis.catalog.object(target).is_some())
        });
        self.focus_event = location.event.filter(|id| {
            self.snapshot
                .as_ref()
                .is_some_and(|s| s.result.analysis.symbols.events.contains_key(id))
        });
        if let Some(target) = location.editor {
            if self
                .snapshot
                .as_ref()
                .is_some_and(|s| s.result.analysis.catalog.object(&target).is_some())
            {
                if !self.has_open_authoring_form() {
                    match target.kind.as_str() {
                        "entity" => self.edit_entity(Some(&target.id)),
                        "relation" => self.edit_relation(Some(&target.id), None),
                        "character" if self.tab == Tab::Characters => {
                            self.select_character(&target.id)
                        }
                        "event" if matches!(self.tab, Tab::Timeline | Tab::Graph) => {
                            // 恢复读取 core 草稿，不调用可能迁移旧权限的 select_event。
                            if let Ok((path, draft)) = self.project.event_draft(&target.id) {
                                self.event_editor = Some(super::EventEditor {
                                    baseline: self.project.content_baseline(),
                                    predecessor_query: String::new(),
                                    temporal_cache: None,
                                    path,
                                    original: Some(target.id),
                                    draft,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                self.message = Some("上次编辑的对象已不存在，未按同名替换".into());
            }
        }
        if let Ok(session) = serde_json::from_value(location.manuscript) {
            self.restore_manuscript_session(session);
        }
        if let Some(id) = entity_source {
            self.focus_entity_source(&id);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_wrap_preference_defaults_off_and_survives_device_restore() {
        let old =
            PersonalState::decode(r#"{"schema_version":1,"settings":{"body_size":20}}"#).unwrap();
        assert!(!old.settings.source_wrap);
        let mut state = old;
        state.settings.source_wrap = true;
        let json = serde_json::to_string(&state).unwrap();
        let restored = PersonalState::decode(&json).unwrap();
        assert!(restored.settings.source_wrap);
        assert_eq!(restored.settings.body_size, 20.0);
        assert!(!json.contains("source_view"));
    }

    #[test]
    fn personal_settings_are_bounded_versioned_and_content_free() {
        let state = PersonalState::decode(
            r#"{"schema_version":1,"settings":{"body_size":999,"line_spacing":0}}"#,
        )
        .unwrap();
        assert_eq!(state.settings.body_size, 28.0);
        assert_eq!(state.settings.line_spacing, 1.0);
        assert!(PersonalState::decode(r#"{"schema_version":2}"#).is_none());
        assert!(PersonalState::decode("broken").is_none());
        let json = serde_json::to_string(&state).unwrap();
        assert!(!json.contains("history"));
        assert!(!json.contains("draft"));
    }
}

#[cfg(test)]
mod source_position_tests;

#[cfg(test)]
mod preferences_focus_tests;
