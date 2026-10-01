//! 设备个人状态；从不写入 Project、源码或展示文档。
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
    pub cursor: Option<usize>,
    pub source_scroll: [f32; 2],
    pub target: Option<TargetRef>,
    pub editor: Option<TargetRef>,
    pub event: Option<String>,
    pub manuscript: serde_json::Value,
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
        let cursor = ctx
            .and_then(|ctx| {
                egui::TextEdit::load_state(ctx, egui::Id::new(("source", &self.active_file)))
            })
            .and_then(|state| state.cursor.char_range())
            .map(|range| range.primary.index)
            .or_else(|| {
                self.personal
                    .source_cursor
                    .as_ref()
                    .filter(|(path, _)| path == &self.active_file)
                    .map(|(_, cursor)| *cursor)
            });
        Location {
            tab: Some(self.tab),
            file: self.active_file.clone(),
            cursor,
            source_scroll: self.personal.source_scroll,
            target: self
                .open_object_identity()
                .or_else(|| self.catalog_target.clone()),
            editor: self.open_object_identity(),
            event: self.focus_event.clone(),
            manuscript: serde_json::to_value(self.manuscript_session()).unwrap_or_default(),
        }
    }
    pub(super) fn remember_author_position(&mut self) {
        let position = self.author_location(None);
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
    fn restore_author_location(&mut self, location: Location, ctx: &egui::Context) {
        let known = self.project.documents.contains_key(&location.file)
            || self
                .project
                .authoring_document(&location.file)
                .is_ok_and(|d| !d.is_deleted());
        if known {
            self.active_file = location.file;
            self.personal.source_scroll =
                location
                    .source_scroll
                    .map(|v| if v.is_finite() { v.max(0.0) } else { 0.0 });
            self.personal.restore_source = true;
            if let Some(cursor) = location.cursor {
                let length = self
                    .project
                    .document(&self.active_file)
                    .map(|text| text.chars().count())
                    .unwrap_or(0);
                let cursor = cursor.min(length);
                self.personal.source_cursor = Some((self.active_file.clone(), cursor));
                let id = egui::Id::new(("source", &self.active_file));
                let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(
                        egui::text::CCursor::new(cursor),
                    )));
                egui::TextEdit::store_state(ctx, id, state);
            }
        } else {
            self.message = Some("上次的来源文件已不存在，保留当前入口".into());
        }
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
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
