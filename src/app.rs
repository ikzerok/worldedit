//! 多文件作者工作台:共享工程快照驱动所有视图。
mod anchors;
#[cfg(target_arch = "wasm32")]
mod browser;
mod catalog;
mod characters;
mod choices;
#[cfg(not(target_arch = "wasm32"))]
mod conflicts;
#[cfg(not(target_arch = "wasm32"))]
mod frame_profile;
mod inspector;
mod map_creation;
mod maps;
mod overview;
#[cfg(not(target_arch = "wasm32"))]
mod package;
mod play;
mod reading;
mod search;
mod states;
mod tags;
mod temporal;
mod views;
mod wiki;
mod workspace;

use crate::{
    fonts::install_cjk_fonts,
    theme::{self, *},
};
use egui::{Pos2, Vec2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use worldline_core::authoring::{CharacterDraft, EventDraft, WorldDraft};
use worldline_core::project::Project;
use worldline_core::{CompileResult, Diagnostic, Severity};
use worldline_runtime::Story;

fn draft_root() -> PathBuf {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::temp_dir().join(format!("worldedit-draft-{}", std::process::id()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        PathBuf::from("/world")
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn frame_profile_input_active(ctx: &egui::Context) -> bool {
    ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Copy
                    | egui::Event::Cut
                    | egui::Event::Paste(_)
                    | egui::Event::Text(_)
                    | egui::Event::Key { .. }
                    | egui::Event::PointerMoved(_)
                    | egui::Event::MouseMoved(_)
                    | egui::Event::PointerButton { .. }
                    | egui::Event::Zoom(_)
                    | egui::Event::Touch { .. }
                    | egui::Event::MouseWheel { .. }
            )
        })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Timeline,
    Graph,
    Map,
    Characters,
    Catalog,
    Wiki,
    World,
    Edit,
    Play,
}
impl Tab {
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "正文概览",
            Self::Timeline => "时间线",
            Self::Graph => "事件关系图",
            Self::Map => "地图画布",
            Self::Characters => "人物",
            Self::Catalog => "资料与状态",
            Self::Wiki => "Wiki 词条",
            Self::World => "世界观",
            Self::Edit => "源文件",
            Self::Play => "试玩",
        }
    }
}
struct Snapshot {
    result: CompileResult,
    wiki: worldline_core::wiki::KeywordIndex,
    map_index: worldline_core::presentation::MapIndex,
}
struct PlayState {
    // 延续运行时借用接口;每次重开产生一个会话快照。
    story: Option<Story<'static>>,
    transcript: String,
    transcript_links: Vec<worldline_core::navigation::RenderedLink>,
    ended: bool,
    error: Option<String>,
    version: u64,
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
    #[cfg(not(target_arch = "wasm32"))]
    last_refresh: std::time::Instant,
    #[cfg(not(target_arch = "wasm32"))]
    disk_stamp: Vec<(PathBuf, u64, Option<std::time::SystemTime>)>,
    active_file: PathBuf,
    saved_location: bool,
    version: u64,
    snapshot: Option<Snapshot>,
    io_error: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    conflict_view: conflicts::ConflictView,
    #[cfg(not(target_arch = "wasm32"))]
    frame_profile: Option<frame_profile::FrameProfiler>,
    stale_form: bool,
    message: Option<String>,
    tab: Tab,
    jump: Option<(u32, u32)>,
    play: Option<PlayState>,
    play_scroll_bottom: bool,
    event_editor: Option<EventEditor>,
    character_editor: Option<CharacterEditor>,
    world_editor: Option<WorldDraft>,
    catalog_target: Option<worldline_core::catalog::TargetRef>,
    catalog_filter: String,
    catalog_query: String,
    catalog_recursive: bool,
    tag_editor: Option<(Option<String>, WorldDraft)>,
    anchor_editor: Option<(Option<String>, worldline_core::anchors::AnchorDraft)>,
    overview_storyline: String,
    overview_query: String,
    overview_cache: Option<(u64, Vec<(PathBuf, EventDraft)>)>,
    reading_target: Option<worldline_core::catalog::TargetRef>,
    reading_history: Vec<worldline_core::catalog::TargetRef>,
    wiki_query: String,
    wiki_target: Option<worldline_core::catalog::TargetRef>,
    wiki_editor: Option<wiki::WikiEditor>,
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
}

impl WorldeditApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        install_cjk_fonts(&cc.egui_ctx);
        theme::install(&cc.egui_ctx);
        let project = Project::new(&draft_root());
        let mut app = Self {
            active_file: project.entry.clone(),
            project,
            #[cfg(not(target_arch = "wasm32"))]
            last_refresh: std::time::Instant::now(),
            #[cfg(not(target_arch = "wasm32"))]
            disk_stamp: Vec::new(),
            saved_location: false,
            version: 0,
            snapshot: None,
            io_error: None,
            #[cfg(not(target_arch = "wasm32"))]
            conflict_view: conflicts::ConflictView::default(),
            #[cfg(not(target_arch = "wasm32"))]
            frame_profile: frame_profile::FrameProfiler::from_env(),
            stale_form: false,
            message: None,
            tab: Tab::Timeline,
            jump: None,
            play: None,
            play_scroll_bottom: false,
            event_editor: None,
            character_editor: None,
            world_editor: None,
            catalog_target: None,
            catalog_filter: "tag".into(),
            catalog_query: String::new(),
            catalog_recursive: true,
            tag_editor: None,
            state_editor: None,
            anchor_editor: None,
            overview_storyline: String::new(),
            overview_query: String::new(),
            overview_cache: None,
            reading_target: None,
            reading_history: Vec::new(),
            wiki_query: String::new(),
            wiki_target: None,
            wiki_editor: None,
            alias_input: String::new(),
            link_query: String::new(),
            search: String::new(),
            search_open: false,
            search_focus: false,
            project_query: String::new(),
            focus_event: None,
            zoom: 1.0,
            graph_positions: HashMap::new(),
            character_positions: HashMap::new(),
            temporal_positions: HashMap::new(),
            link_from: None,
            dragging: None,
            link_label: "继续".into(),
            character_link: None,
            history: Vec::new(),
            redo: Vec::new(),
            pending: None,
            allow_close: false,
            directory: None,
            new_file: None,
            new_period: None,
            map_canvas: maps::MapCanvas::new(maps::MapRenderSnapshot::empty(Vec2::new(
                2048.0, 1536.0,
            ))),
            map_selection: None,
            map_navigation: None,
            pending_map_camera: None,
            map_creation: map_creation::MapCreationForm::default(),
            map_revision: worldline_core::presentation_commands::Revision::default(),
            map_form: maps::PlacementForm::default(),
            map_search: String::new(),
            map_locate_request: None,
            map_failed_command: None,
        };
        if let Some(path) = initial_file {
            app.load_project(path);
        }
        app.recompile();
        app
    }

    fn recompile(&mut self) {
        self.version += 1;
        self.map_revision.content_generation = self.map_revision.content_generation.wrapping_add(1);
        self.map_canvas.invalidate_rasters();
        let result = self.project.compile();
        let wiki = worldline_core::wiki::KeywordIndex::new(&result);
        let map_index =
            worldline_core::presentation_commands::map_index_with_content(&self.project, &result);
        self.snapshot = Some(Snapshot {
            result,
            wiki,
            map_index,
        });
    }

    fn refresh_presentation_after_map_command(&mut self) {
        self.version += 1;
        self.map_canvas.invalidate_rasters();
        let Some(content) = self.snapshot.as_ref().map(|snapshot| &snapshot.result) else {
            return;
        };
        let map_index =
            worldline_core::presentation_commands::map_index_with_content(&self.project, content);
        if let Some(snapshot) = self.snapshot.as_mut() {
            snapshot.map_index = map_index;
        }
    }
    fn diagnostics(&self) -> &[Diagnostic] {
        self.snapshot
            .as_ref()
            .map(|s| s.result.diagnostics.as_slice())
            .unwrap_or(&[])
    }
    fn remember(&mut self, before: Project) {
        self.allow_close = false;
        if self.history.len() == 40 {
            self.history.remove(0);
        }
        self.history.push(before);
        self.redo.clear();
    }
    fn commit(
        &mut self,
        label: &str,
        operation: impl FnOnce(&mut Project) -> Result<(), String>,
    ) -> bool {
        if self.stale_form {
            self.io_error = Some("表单打开后源码已被外部修改。请复制需要保留的输入，关闭表单并重新打开后合并，避免覆盖新内容。".into());
            return false;
        }
        let before = self.project.clone();
        match self.project.edit(operation) {
            Ok(()) => {
                self.remember(before);
                self.recompile();
                self.io_error = None;
                self.message = Some(label.into());
                true
            }
            Err(e) => {
                self.io_error = Some(e);
                false
            }
        }
    }
    fn load_project(&mut self, path: PathBuf) {
        match Project::open(&path) {
            Ok(project) => {
                self.active_file = project.entry.clone();
                self.project = project;
                self.saved_location = true;
                self.reset_views();
                self.recompile();
                self.message = Some("已载入整个工程".into());
            }
            Err(e) => self.io_error = Some(e),
        }
    }
    fn reset_views(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.conflict_view = conflicts::ConflictView::default();
        }
        self.stale_form = false;
        self.reading_target = None;
        self.reading_history.clear();
        self.wiki_query.clear();
        self.wiki_target = None;
        self.wiki_editor = None;
        self.alias_input.clear();
        self.link_query.clear();
        self.play = None;
        self.event_editor = None;
        self.character_editor = None;
        self.world_editor = None;
        self.catalog_target = None;
        self.tag_editor = None;
        self.state_editor = None;
        self.anchor_editor = None;
        self.io_error = None;
        self.history.clear();
        self.redo.clear();
        self.graph_positions.clear();
        self.character_positions.clear();
        self.temporal_positions.clear();
        self.character_link = None;
        self.new_period = None;
        self.link_from = None;
        self.dragging = None;
        self.search.clear();
        self.focus_event = None;
        self.map_selection = None;
        self.map_navigation = None;
        self.pending_map_camera = None;
        self.map_creation = map_creation::MapCreationForm::default();
        self.map_revision = worldline_core::presentation_commands::Revision::default();
        self.map_form = maps::PlacementForm::default();
        self.map_search.clear();
        self.map_locate_request = None;
        self.map_failed_command = None;
        self.map_canvas.clear();
    }
    fn request_action(&mut self, action: Pending, ctx: &egui::Context) {
        if self.project.is_dirty() {
            self.pending = Some(action);
        } else {
            self.perform_action(action, ctx);
        }
    }
    fn perform_action(&mut self, action: Pending, _ctx: &egui::Context) {
        match action {
            Pending::Open(path) => self.load_project(path),
            Pending::New => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let Some(root) = rfd::FileDialog::new()
                        .set_title("选择新工作区目录（须为空）")
                        .pick_folder()
                    else {
                        return;
                    };
                    if std::fs::read_dir(&root).map_or(true, |mut entries| entries.next().is_some())
                    {
                        self.io_error = Some("新建工作区请选择空目录".into());
                        return;
                    }
                    let mut project = Project::new(&root);
                    if let Err(e) = project.save() {
                        self.io_error = Some(e);
                        return;
                    }
                    self.project = project;
                }
                #[cfg(target_arch = "wasm32")]
                {
                    self.project = Project::new(&draft_root());
                }
                #[cfg(target_arch = "wasm32")]
                crate::web::mount(Default::default());
                self.active_file = self.project.entry.clone();
                self.saved_location = cfg!(not(target_arch = "wasm32"));
                self.reset_views();
                self.recompile();
                self.tab = Tab::Timeline;
            }
            Pending::Close => {
                #[cfg(target_arch = "wasm32")]
                {
                    self.allow_close = true;
                    self.message = Some("可以关闭此浏览器标签页".into());
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.allow_close = true;
                    _ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            #[cfg(target_arch = "wasm32")]
            Pending::BrowserOpen(files) => self.browser_open(files),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn open_dialog(&mut self, ctx: &egui::Context, _folder: bool) {
        let dialog = rfd::FileDialog::new().set_directory(&self.project.root);
        let path = dialog.pick_folder();
        if let Some(path) = path {
            self.request_action(Pending::Open(path), ctx);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn save(&mut self) -> bool {
        if !self.saved_location {
            self.directory_dialog(false);
            return false;
        }
        match self.project.save() {
            Ok(()) => {
                self.recompile();
                self.message = Some("全部文件已保存".into());
                self.io_error = None;
                true
            }
            Err(e) => {
                self.io_error = Some(e);
                false
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn directory_dialog(&mut self, export: bool) {
        let id = self
            .snapshot
            .as_ref()
            .and_then(|s| s.result.analysis.world.as_ref())
            .map(|w| w.id.as_str())
            .unwrap_or("world-project");
        let parent = if self.saved_location {
            self.project.root.parent().unwrap_or(Path::new("."))
        } else {
            Path::new(".")
        };
        self.directory = Some(DirectoryDialog {
            export,
            path: parent
                .join(format!("{id}{}", if export { "-export" } else { "" }))
                .to_string_lossy()
                .into_owned(),
        });
    }
    fn jump_to_file(&mut self, file: &str, line: u32, column: u32) {
        let path = PathBuf::from(file);
        let known_source = self.project.documents.contains_key(&path)
            || self
                .project
                .authoring_document(&path)
                .is_ok_and(|document| !document.is_deleted());
        if known_source {
            self.active_file = path;
            self.tab = Tab::Edit;
            self.jump = Some((line, column));
        }
    }
    fn default_event_file(&self) -> PathBuf {
        self.snapshot
            .as_ref()
            .and_then(|s| {
                s.result
                    .analysis
                    .graph
                    .nodes
                    .iter()
                    .find(|n| n.is_event && Path::new(&n.file) == self.active_file)
                    .or_else(|| s.result.analysis.graph.nodes.iter().find(|n| n.is_event))
            })
            .map(|n| PathBuf::from(&n.file))
            .unwrap_or_else(|| self.active_file.clone())
    }
    fn new_event(&mut self, lane: Option<&str>) {
        let mut index = 1;
        while self.snapshot.as_ref().is_some_and(|s| {
            s.result
                .analysis
                .symbols
                .events
                .contains_key(&format!("event_{index}"))
        }) {
            index += 1;
        }
        let storyline = lane
            .map(str::to_string)
            .or_else(|| {
                self.snapshot
                    .as_ref()
                    .and_then(|s| s.result.analysis.symbols.storyline_order.first().cloned())
            })
            .unwrap_or_else(|| "main".into());
        self.event_editor = Some(EventEditor {
            path: self.default_event_file(),
            original: None,
            draft: EventDraft {
                id: format!("event_{index}"),
                summary: "新的事件".into(),
                storyline,
                period: self.snapshot.as_ref().and_then(|s| {
                    s.result
                        .analysis
                        .timeline
                        .periods
                        .first()
                        .map(|p| p.id.clone())
                }),
                body: "故事从这里继续。\n-> END".into(),
                ..Default::default()
            },
        });
    }
    fn select_event(&mut self, id: &str) {
        let before = self.project.clone();
        match self.project.migrate_permissions() {
            Ok(count) if count > 0 => {
                self.remember(before);
                self.recompile();
                self.message = Some("旧权限已转换为叙事身份状态，可撤销；保存后写入文件".into());
            }
            Err(error) => {
                self.io_error = Some(error);
                return;
            }
            _ => {}
        }
        match self.project.event_draft(id) {
            Ok((path, draft)) => {
                self.focus_event = Some(id.into());
                self.event_editor = Some(EventEditor {
                    path,
                    original: Some(id.into()),
                    draft,
                })
            }
            Err(e) => self.io_error = Some(e),
        }
    }
    fn undo(&mut self, forward: bool) {
        let previous = if forward {
            self.redo.pop()
        } else {
            self.history.pop()
        };
        if let Some(previous) = previous {
            let current = self.project.clone();
            let source_before = self.project.sources();
            if !self.project.restore(previous.clone()) {
                if forward {
                    self.redo.push(previous);
                } else {
                    self.history.push(previous);
                }
                self.io_error = Some("撤销快照已因外部刷新失效，未改变当前工程".into());
                return;
            }
            if forward {
                self.history.push(current);
            } else {
                self.redo.push(current);
            }
            if !self.project.documents.contains_key(&self.active_file) {
                self.active_file = self.project.entry.clone();
            }
            self.event_editor = None;
            self.character_editor = None;
            self.world_editor = None;
            self.map_failed_command = None;
            self.map_canvas.reset_local_preview();
            if source_before == self.project.sources() {
                self.map_revision = self.map_revision.next_presentation();
                self.refresh_presentation_after_map_command();
            } else {
                self.recompile();
            }
            self.io_error = None;
        }
    }
}

impl eframe::App for WorldeditApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        let input_active = self
            .frame_profile
            .as_ref()
            .filter(|profile| profile.is_active())
            .map(|_| frame_profile_input_active(ctx));
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(profile) = self.frame_profile.as_mut() {
            profile.begin_frame(_frame.info().cpu_usage);
        }
        if self.event_editor.is_none()
            && self.character_editor.is_none()
            && self.world_editor.is_none()
            && self.tag_editor.is_none()
            && self.state_editor.is_none()
            && self.anchor_editor.is_none()
            && self.wiki_editor.is_none()
        {
            self.stale_form = false;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
            if self.saved_location
                && self.last_refresh.elapsed() >= std::time::Duration::from_secs(1)
            {
                self.last_refresh = std::time::Instant::now();
                let scan = worldline_core::file_access::workspace_files(&self.project.root)
                    .and_then(|paths| {
                        paths
                            .into_iter()
                            .map(|p| {
                                let m = std::fs::metadata(&p)?;
                                Ok((p, m.len(), m.modified().ok()))
                            })
                            .collect::<std::io::Result<Vec<_>>>()
                    });
                match scan {
                    Ok(stamp) if stamp != self.disk_stamp => match self.project.refresh() {
                        Ok(conflicts) => {
                            self.disk_stamp = stamp;
                            self.history.clear();
                            self.redo.clear();
                            if !self.project.documents.contains_key(&self.active_file) {
                                self.active_file = self.project.entry.clone();
                            }
                            self.recompile();
                            if !conflicts.is_empty() {
                                self.io_error = Some(format!(
                                    "外部修改与未保存内容冲突，已保留缓冲：{}",
                                    conflicts
                                        .iter()
                                        .map(|p| p.display().to_string())
                                        .collect::<Vec<_>>()
                                        .join("、")
                                ));
                            }
                        }
                        Err(e) => self.io_error = Some(e),
                    },
                    Err(e) => self.io_error = Some(e.to_string()),
                    _ => {}
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        self.browser_events(ctx);
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.project.is_dirty()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.open_dialog(ctx, false);
        }
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::F,
            )
        }) {
            self.search_open = true;
            self.search_focus = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.link_from = None;
            self.character_link = None;
        }
        self.top_bar(ctx);
        self.status_bar(ctx);
        self.sidebar(ctx);
        match self.tab {
            Tab::Timeline | Tab::Graph => {
                self.event_inspector(ctx);
                self.canvas_tab(ctx);
            }
            Tab::Map => self.map_tab(ctx),
            Tab::Overview => self.overview_tab(ctx),
            Tab::Edit => self.source_tab(ctx),
            Tab::Characters => self.characters_tab(ctx),
            Tab::Catalog => self.catalog_tab(ctx),
            Tab::Wiki => self.wiki_tab(ctx),
            Tab::World => self.world_tab(ctx),
            Tab::Play => self.play_tab(ctx),
        }
        self.dialogs(ctx);
        self.project_search(ctx);
        self.reading_window(ctx);
        self.wiki_editor_window(ctx);
        #[cfg(not(target_arch = "wasm32"))]
        self.conflict_view.show(ctx);
        #[cfg(not(target_arch = "wasm32"))]
        crate::chrome::resize_edges(ctx);
        #[cfg(target_arch = "wasm32")]
        crate::web::set_dirty(self.project.is_dirty() && !self.allow_close);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(input_active) = input_active {
            if let Some(profile) = self.frame_profile.as_mut() {
                profile.finish_frame(self.tab == Tab::Map, input_active, ctx.pixels_per_point());
            }
        }
    }
}

impl WorldeditApp {
    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top")
            .frame(crate::chrome::title_frame(ctx))
            .show(ctx, |ui| {
                crate::chrome::title_drag(ui);
                ui.horizontal(|ui| {
                    let close = ui.horizontal(crate::chrome::controls).inner;
                    if close {
                        self.request_action(Pending::Close, ctx);
                    }
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new("worldedit")
                            .strong()
                            .size(18.0)
                            .color(TEXT),
                    );
                    ui.add_space(12.0);
                    crate::chrome::subtitle(ui, "世界创作工作台");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(theme::primary("导出工程  ↗")).clicked() {
                            self.directory_dialog(true);
                        }
                        if ui.button("保存全部").clicked() {
                            self.save();
                        }
                        if ui
                            .button("搜索")
                            .on_hover_text("搜索所有文件 · Ctrl+Shift+F")
                            .clicked()
                        {
                            self.search_open = true;
                            self.search_focus = true;
                        }
                        if ui.button("▶ 试玩").clicked() {
                            self.tab = Tab::Play;
                        }
                        ui.menu_button("工程", |ui| {
                            if ui.button("新建世界").clicked() {
                                self.request_action(Pending::New, ctx);
                                ui.close();
                            }
                            if ui.button("打开文件夹…").clicked() {
                                self.open_dialog(ctx, true);
                                ui.close();
                            }
                            if ui.button("选择工作区…  Ctrl+O").clicked() {
                                self.open_dialog(ctx, false);
                                ui.close();
                            }
                            if ui.button("另存工程…").clicked() {
                                self.directory_dialog(false);
                                ui.close();
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            if ui.button("导出 ZIP 工程包…").clicked() {
                                ui.close();
                                self.export_package();
                            }
                            if ui.button("从磁盘重新载入").clicked() {
                                self.request_action(Pending::Open(self.project.entry.clone()), ctx);
                                ui.close();
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            if ui.button("查看冲突差异").clicked() {
                                self.conflict_view =
                                    conflicts::ConflictView::capture(&self.project);
                                ui.close();
                            }
                        });
                    });
                });
            });
        if let Some(error) = self.io_error.clone() {
            egui::TopBottomPanel::top("error")
                .frame(theme::panel().fill(egui::Color32::from_rgb(65, 36, 44)))
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(ERROR, error);
                        #[cfg(not(target_arch = "wasm32"))]
                        if ui.small_button("查看冲突差异").clicked() {
                            self.conflict_view = conflicts::ConflictView::capture(&self.project);
                        }
                        if ui.small_button("关闭").clicked() {
                            self.io_error = None;
                        }
                    });
                });
        }
    }
    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .corner_radius(egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: 12,
                        se: 12,
                    })
                    .inner_margin(egui::Margin::symmetric(18, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let errors = self
                        .diagnostics()
                        .iter()
                        .filter(|d| d.severity == Severity::Error)
                        .count();
                    let warnings = self
                        .diagnostics()
                        .iter()
                        .filter(|d| d.severity == Severity::Warning)
                        .count();
                    ui.colored_label(
                        if errors > 0 { ERROR } else { ACCENT },
                        if errors > 0 {
                            format!("● {errors} 个错误")
                        } else {
                            "● 编译通过".into()
                        },
                    );
                    if warnings > 0 {
                        ui.label(theme::muted(format!("{warnings} 个提醒")));
                    }
                    ui.separator();
                    ui.label(theme::muted(if self.project.is_dirty() {
                        "有未保存修改"
                    } else {
                        "全部文件已保存"
                    }));
                    if let Some(message) = &self.message {
                        ui.label(theme::muted(message));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(theme::muted(format!(
                            "WORLDLINE {}  ·  UTF-8",
                            self.project.language_version()
                        )));
                        if ui
                            .add_enabled(!self.redo.is_empty(), egui::Button::new("重做").small())
                            .clicked()
                        {
                            self.undo(true);
                        }
                        if ui
                            .add_enabled(
                                !self.history.is_empty(),
                                egui::Button::new("撤销").small(),
                            )
                            .clicked()
                        {
                            self.undo(false);
                        }
                    });
                });
            });
    }
    fn page_heading(&self, ui: &mut egui::Ui, title: &str, subtitle: &str) {
        ui.heading(title);
        ui.label(theme::muted(subtitle));
        ui.add_space(12.0);
    }
}
