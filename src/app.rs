//! 多文件作者工作台:共享工程快照驱动所有视图。
mod actions;
mod anchors;
mod authoring_forms;
#[cfg(test)]
mod authoring_ui_tests;
#[cfg(target_arch = "wasm32")]
mod browser;
mod catalog;
mod catalog_query;
mod characters;
mod checkpoint_history;
mod choices;
mod chrome;
mod collaboration_ui;
mod commands;
#[cfg(not(target_arch = "wasm32"))]
mod conflicts;
mod deletion;
mod draft_lifecycle;
mod entities;
#[cfg(not(target_arch = "wasm32"))]
mod frame_profile;
mod init;
mod inspector;
mod localization_ui;
mod manuscript;
mod map_creation;
mod maps;
mod markdown_import_ui;
mod network;
mod network_state;
mod object_picker;
mod overview;
#[cfg(not(target_arch = "wasm32"))]
mod package;
mod personal;
mod play;
mod presets;
mod reader_publish;
mod reading;
mod reading_state;
mod refactor_ui;
mod relation_editor;
mod search;
#[cfg(not(target_arch = "wasm32"))]
mod startup;
mod states;
mod tags;
mod template_manager;
mod templates;
mod temporal;
mod topic_views;
mod update;
mod views;
mod wiki;
mod workspace;
mod writing_workspace;

use egui::{Pos2, Vec2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use worldline_core::authoring::{CharacterDraft, EventDraft, WorldDraft};
use worldline_core::project::Project;
#[cfg(target_arch = "wasm32")]
use worldline_core::{Analysis, Program};
use worldline_core::{CompileResult, Diagnostic};
#[cfg(target_arch = "wasm32")]
use worldline_runtime::ReplaySession;
use worldline_runtime::{ChoiceExplanation, ReplayCancellation, ReplayResult, ReplayTrace, Story};

pub(super) fn workspace_source_path(project: &Project, path: &Path) -> PathBuf {
    workspace::workspace_source_path(project, path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Tab {
    Overview,
    Timeline,
    Graph,
    Network,
    Review,
    Map,
    Characters,
    Catalog,
    Wiki,
    World,
    Edit,
    Play,
    Localization,
    Manuscript,
    Templates,
    CheckpointHistory,
}
impl Tab {
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "正文概览",
            Self::Timeline => "时间线",
            Self::Graph => "事件关系图",
            Self::Network => "世界关联",
            Self::Review => "协作审阅",
            Self::Map => "地图画布",
            Self::Characters => "人物",
            Self::Catalog => "资料与状态",
            Self::Wiki => "Wiki 词条",
            Self::World => "世界观",
            Self::Edit => "源文件",
            Self::Play => "试玩",
            Self::Localization => "本地化",
            Self::Manuscript => "书稿工作台",
            Self::Templates => "工程模板",
            Self::CheckpointHistory => "检查点历史",
        }
    }
}
struct Snapshot {
    result: CompileResult,
    template_index: worldline_core::project_templates::ProjectTemplateIndex,
    wiki: worldline_core::wiki::KeywordIndex,
    map_index: worldline_core::presentation::MapIndex,
    graph_index: worldline_core::graph_views::GraphViewIndex,
    preset_index: worldline_core::presentation_presets::PresentationPresetIndex,
    comment_index: worldline_core::collaboration::CommentIndex,
    proposal_index: worldline_core::collaboration::ProposalIndex,
}
struct PlayState {
    // 延续运行时借用接口;每次重开产生一个会话快照。
    story: Option<Story<'static>>,
    transcript: String,
    transcript_links: Vec<worldline_core::navigation::RenderedLink>,
    ended: bool,
    error: Option<String>,
    version: u64,
    paused: bool,
}
struct SavedReplayPath {
    name: String,
    trace: ReplayTrace,
}
#[cfg(not(target_arch = "wasm32"))]
struct ReplayJob {
    cancellation: ReplayCancellation,
    receiver: std::sync::mpsc::Receiver<Result<ReplayResult, String>>,
}
#[cfg(target_arch = "wasm32")]
struct ReplayJob {
    cancellation: ReplayCancellation,
    program: Program,
    analysis: Analysis,
    session: ReplaySession,
}
impl Drop for ReplayJob {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum PlayPane {
    Story,
    Debugger,
}
struct ReplayDebugger {
    seed: u64,
    path_name: String,
    saved_paths: Vec<SavedReplayPath>,
    selected_path: Option<usize>,
    import_json: String,
    export_json: String,
    result: Option<ReplayResult>,
    result_path_name: Option<String>,
    result_version: Option<u64>,
    job: Option<ReplayJob>,
    explanations: Option<Vec<ChoiceExplanation>>,
    max_steps: u64,
    time_budget_ms: u64,
    pane: PlayPane,
    notice: Option<String>,
}
impl Default for ReplayDebugger {
    fn default() -> Self {
        Self {
            seed: 1,
            path_name: "路径 1".into(),
            saved_paths: Vec::new(),
            selected_path: None,
            import_json: String::new(),
            export_json: String::new(),
            result: None,
            result_path_name: None,
            result_version: None,
            job: None,
            explanations: None,
            max_steps: 100_000,
            time_budget_ms: 30_000,
            pane: PlayPane::Story,
            notice: None,
        }
    }
}
#[derive(Clone)]
struct EventEditor {
    path: PathBuf,
    original: Option<String>,
    draft: EventDraft,
}
#[derive(Clone)]
struct CharacterEditor {
    path: PathBuf,
    original: Option<String>,
    draft: CharacterDraft,
}
enum Pending {
    Open(PathBuf),
    New,
    Close,
    #[cfg(target_arch = "wasm32")]
    BrowserOpen(crate::web::Files),
}
struct DirectoryDialog {
    #[cfg(not(target_arch = "wasm32"))]
    path: String,
    #[cfg(not(target_arch = "wasm32"))]
    export: bool,
}

pub struct WorldeditApp {
    project: Project,
    personal: personal::PersonalState,
    command_palette: commands::CommandPalette,
    draft_action: Option<Pending>,
    new_draft_baselines: HashMap<&'static str, String>,
    frame_dirty_drafts: Vec<&'static str>,
    event_draft_cache: std::cell::RefCell<Option<(u64, String, EventDraft)>>,
    #[cfg(not(target_arch = "wasm32"))]
    last_refresh: std::time::Instant,
    #[cfg(not(target_arch = "wasm32"))]
    disk_stamp: Vec<(PathBuf, u64, Option<std::time::SystemTime>)>,
    active_file: PathBuf,
    saved_location: bool,
    #[cfg(target_arch = "wasm32")]
    browser_pending_save: bool,
    version: u64,
    snapshot: Option<Snapshot>,
    io_error: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    conflict_view: conflicts::ConflictView,
    markdown_import_wizard: Option<markdown_import_ui::Wizard>,
    #[cfg(not(target_arch = "wasm32"))]
    frame_profile: Option<frame_profile::FrameProfiler>,
    stale_form: bool,
    message: Option<String>,
    tab: Tab,
    jump: Option<(u32, u32)>,
    play: Option<PlayState>,
    replay_debugger: ReplayDebugger,
    play_scroll_bottom: bool,
    event_editor: Option<EventEditor>,
    character_editor: Option<CharacterEditor>,
    world_editor: Option<WorldDraft>,
    catalog_target: Option<worldline_core::catalog::TargetRef>,
    catalog_filter: String,
    catalog_query: String,
    catalog_recursive: bool,
    catalog_workbench: catalog_query::WorkbenchState,
    tag_editor: Option<(Option<String>, WorldDraft)>,
    anchor_editor: Option<(Option<String>, worldline_core::anchors::AnchorDraft)>,
    overview_storyline: String,
    overview_query: String,
    overview_cache: Option<(u64, Vec<(PathBuf, EventDraft)>)>,
    reading_target: Option<worldline_core::catalog::TargetRef>,
    reading_history: Vec<worldline_core::catalog::TargetRef>,
    reading_return: Option<(PathBuf, usize)>,
    ime_composing: bool,
    ime_source_baseline: Option<(PathBuf, String)>,
    ime_source_draft: Option<(PathBuf, String, String)>,
    mention_suppression: Option<(PathBuf, usize, String)>,
    mention_selection: Option<(PathBuf, usize, String, usize)>,
    reading_panels: reading_state::ReadingPanels,
    active_reading_panel: Option<u64>,
    selected_reading_panel: Option<u64>,
    wiki_query: String,
    wiki_target: Option<worldline_core::catalog::TargetRef>,
    wiki_editor: Option<wiki::WikiEditor>,
    entity_editor: Option<authoring_forms::EntityForm>,
    relation_editor: Option<authoring_forms::RelationForm>,
    relation_type_editor: Option<authoring_forms::RelationTypeForm>,
    delete_form: Option<authoring_forms::DeleteForm>,
    rename_form: Option<authoring_forms::RenameForm>,
    preset_editor: Option<presets::PresetEditor>,
    alias_input: String,
    link_query: String,
    state_editor: Option<(Option<String>, worldline_core::states::StateDraft)>,
    search: String,
    search_open: bool,
    search_focus: bool,
    project_query: String,
    focus_event: Option<String>,
    zoom: f32,
    graph_positions: HashMap<String, Pos2>,
    character_positions: HashMap<String, Pos2>,
    temporal_positions: HashMap<String, Pos2>,
    link_from: Option<String>,
    link_label: String,
    character_link: Option<String>,
    dragging: Option<String>,
    history: Vec<Project>,
    redo: Vec<Project>,
    pending: Option<Pending>,
    allow_close: bool,
    directory: Option<DirectoryDialog>,
    new_file: Option<String>,
    new_period: Option<(String, String, Option<String>)>,
    map_canvas: maps::MapCanvas,
    map_selection: Option<String>,
    map_navigation: Option<maps::navigation::MapNavigationController>,
    pending_map_camera: Option<maps::navigation::CameraState>,
    map_creation: map_creation::MapCreationForm,
    map_revision: worldline_core::presentation_commands::Revision,
    map_form: maps::PlacementForm,
    map_search: String,
    map_locate_request: Option<maps::LocateRequest>,
    map_failed_command: Option<maps::PendingMapCommand>,
    network_state: network_state::NetworkState,
    topic_session: u64,
    network_loaded_view: Option<String>,
    network_view_id: String,
    network_view_title: String,
    network_selected: Option<worldline_core::catalog::TargetRef>,
    pending_preset_layers: Option<(String, std::collections::BTreeMap<String, bool>)>,
    review: collaboration_ui::ReviewState,
    manuscript: manuscript::WorkbenchState,
    localization_ui: localization_ui::LocalizationUiState,
    template_manager: template_manager::ManagerState,
    checkpoint_history: checkpoint_history::HistoryState,
    reader_publish: reader_publish::ReaderPublishState,
}
