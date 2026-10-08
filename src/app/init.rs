use super::*;
use crate::{fonts::install_cjk_fonts, theme};

pub(super) fn draft_root() -> PathBuf {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::temp_dir().join(format!("worldedit-draft-{}", std::process::id()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        PathBuf::from("/world")
    }
}
impl WorldeditApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        install_cjk_fonts(&cc.egui_ctx);
        let personal = personal::PersonalState::restore(cc.storage);
        let _theme = theme::configure_appearance(&cc.egui_ctx, personal.appearance());
        let project = Project::new(&draft_root());
        let mut app = Self {
            problems: problems::ProblemsState::default(),
            active_file: project.entry.clone(),
            project,
            personal,
            command_palette: commands::CommandPalette::default(),
            source_outline: source_outline::OutlineState::default(),
            source_jump: source_jump::JumpState::default(),
            draft_action: None,
            new_draft_baselines: HashMap::new(),
            frame_dirty_drafts: Vec::new(),
            event_draft_cache: std::cell::RefCell::new(None),
            #[cfg(not(target_arch = "wasm32"))]
            last_refresh: std::time::Instant::now(),
            #[cfg(not(target_arch = "wasm32"))]
            disk_stamp: Vec::new(),
            saved_location: false,
            #[cfg(target_arch = "wasm32")]
            browser_pending_save: false,
            version: 0,
            snapshot: None,
            io_error: None,
            #[cfg(not(target_arch = "wasm32"))]
            conflict_view: conflicts::ConflictView::default(),
            markdown_import_wizard: None,
            capability_ui: None,
            #[cfg(not(target_arch = "wasm32"))]
            frame_profile: frame_profile::FrameProfiler::from_env(),
            stale_form: false,
            message: None,
            tab: Tab::Manuscript,
            jump: None,
            play: None,
            play_confirmation: None,
            replay_debugger: ReplayDebugger::default(),
            comparison: play::comparison::ComparisonState::default(),
            playthrough_report: play::report::PlaythroughReportState::default(),
            play_scroll_bottom: false,
            play_keyboard: play::keyboard::PlayKeyboard::default(),
            event_editor: None,
            character_editor: None,
            world_editor: None,
            catalog_target: None,
            catalog_filter: "tag".into(),
            catalog_query: String::new(),
            catalog_recursive: true,
            catalog_workbench: catalog_query::WorkbenchState::default(),
            catalog_import: catalog_import::ImportState::default(),
            tag_editor: None,
            state_editor: None,
            anchor_editor: None,
            overview_storyline: String::new(),
            overview_query: String::new(),
            overview_cache: None,
            reading_target: None,
            reading_context: reading::ContextCache::default(),
            reading_history: Vec::new(),
            reading_return: None,
            ime_composing: false,
            ime_source_baseline: None,
            ime_source_draft: None,
            mention_suppression: None,
            mention_selection: None,
            reading_panels: reading_state::ReadingPanels::default(),
            active_reading_panel: None,
            selected_reading_panel: None,
            wiki_query: String::new(),
            wiki_target: None,
            wiki_editor: None,
            entity_editor: None,
            relation_editor: None,
            relation_type_editor: None,
            delete_form: None,
            rename_form: None,
            source_move_form: None,
            entity_source_move_form: None,
            entity_source_navigation: None,
            preset_editor: None,
            alias_input: String::new(),
            link_query: String::new(),
            search: String::new(),
            search_open: false,
            search_focus: false,
            project_query: String::new(),
            search_state: Default::default(),
            focus_event: None,
            zoom: 1.0,
            graph_positions: HashMap::new(),
            character_positions: HashMap::new(),
            character_focus: characters::CharacterFocus::default(),
            temporal_issues: temporal::issues::TemporalIssues::default(),
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
            export_confirmation: None,
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
            network_state: network_state::NetworkState::default(),
            topic_session: 0,
            network_loaded_view: None,
            network_view_id: String::new(),
            network_view_title: String::new(),
            network_selected: None,
            pending_preset_layers: None,
            review: collaboration_ui::ReviewState::default(),
            manuscript: manuscript::WorkbenchState::default(),
            localization_ui: localization_ui::LocalizationUiState::default(),
            template_manager: template_manager::ManagerState::default(),
            schema_ui: schema_ui::SchemaUiState::default(),
            checkpoint_history: checkpoint_history::HistoryState::default(),
            reader_publish: reader_publish::ReaderPublishState::default(),
        };
        app.catalog_workbench.restore_favorites(cc.storage);
        app.catalog_workbench.restore_columns(cc.storage);
        if let Some(path) = initial_file {
            app.load_project(path);
        }
        app.recompile();
        app.restore_personal_view(&cc.egui_ctx);
        app
    }

    pub(super) fn recompile(&mut self) {
        self.manuscript.invalidate_query_cache();
        self.version += 1;
        self.map_revision.content_generation = self.map_revision.content_generation.wrapping_add(1);
        self.map_canvas.invalidate_rasters();
        let result = self.project.compile();
        let template_index = self.project.template_index_with_content(&result);
        let wiki = worldline_core::wiki::KeywordIndex::new(&result);
        let map_index =
            worldline_core::presentation_commands::map_index_with_content(&self.project, &result);
        let graph_index =
            worldline_core::graph_views::build_graph_view_index(&self.project, &result);
        let preset_index = worldline_core::presentation_presets::build_preset_index(
            &self.project,
            &result,
            &map_index,
            &graph_index,
        );
        let comment_index =
            worldline_core::collaboration::build_comment_index(&self.project, &result, &map_index);
        let proposal_index = worldline_core::collaboration::build_proposal_index(&self.project);
        self.snapshot = Some(Snapshot {
            graph_index,
            template_index,
            preset_index,
            comment_index,
            proposal_index,
            result,
            wiki,
            map_index,
        });
        self.refresh_map_binding_guards();
        if self.reader_publish.open {
            self.reader_publish
                .refresh_choices(&self.project, self.snapshot.as_ref());
        }
    }

    pub(super) fn refresh_presentation_after_map_command(&mut self) {
        self.version += 1;
        self.map_canvas.invalidate_rasters();
        let Some(content) = self.snapshot.as_ref().map(|snapshot| &snapshot.result) else {
            return;
        };
        let map_index =
            worldline_core::presentation_commands::map_index_with_content(&self.project, content);
        let graph_index =
            worldline_core::graph_views::build_graph_view_index(&self.project, content);
        let preset_index = worldline_core::presentation_presets::build_preset_index(
            &self.project,
            content,
            &map_index,
            &graph_index,
        );
        let comment_index =
            worldline_core::collaboration::build_comment_index(&self.project, content, &map_index);
        let proposal_index = worldline_core::collaboration::build_proposal_index(&self.project);
        if let Some(snapshot) = self.snapshot.as_mut() {
            snapshot.graph_index = graph_index;
            snapshot.preset_index = preset_index;
            snapshot.comment_index = comment_index;
            snapshot.proposal_index = proposal_index;
            snapshot.map_index = map_index;
        }
        self.refresh_map_binding_guards();
        self.reader_publish.refresh_profiles(&self.project);
    }
    pub(super) fn diagnostics(&self) -> &[Diagnostic] {
        self.snapshot
            .as_ref()
            .map(|s| s.result.diagnostics.as_slice())
            .unwrap_or(&[])
    }
    pub(super) fn remember(&mut self, before: Project) {
        self.allow_close = false;
        if self.history.len() == 40 {
            self.history.remove(0);
        }
        self.history.push(before);
        self.redo.clear();
    }
    pub(super) fn commit(
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
    pub(super) fn load_project(&mut self, path: PathBuf) {
        match Project::open(&path) {
            Ok(project) => {
                self.active_file = project.entry.clone();
                self.project = project;
                self.saved_location = true;
                self.reset_views();
                self.recompile();
                self.personal.pending_restore = true;
                self.message = Some("已载入整个工程".into());
            }
            Err(e) => self.io_error = Some(e),
        }
    }
    pub(super) fn reset_views(&mut self) {
        self.problems.reset_for_workspace();
        self.personal.history.clear();
        self.personal.source_scroll = [0.0; 2];
        self.command_palette = commands::CommandPalette::default();
        self.source_outline = source_outline::OutlineState::default();
        self.source_jump = source_jump::JumpState::default();
        self.export_confirmation = None;
        self.directory = None;
        self.draft_action = None;
        self.new_draft_baselines.clear();
        self.frame_dirty_drafts.clear();
        self.event_draft_cache.replace(None);
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.conflict_view = conflicts::ConflictView::default();
        }
        self.markdown_import_wizard = None;
        self.capability_ui = None;
        self.play_confirmation = None;
        self.stale_form = false;
        self.reading_target = None;
        self.reading_context = reading::ContextCache::default();
        self.reading_history.clear();
        self.ime_composing = false;
        self.ime_source_baseline = None;
        self.ime_source_draft = None;
        self.mention_suppression = None;
        self.mention_selection = None;
        self.reading_panels.clear();
        self.active_reading_panel = None;
        self.selected_reading_panel = None;
        self.wiki_query.clear();
        self.wiki_target = None;
        self.wiki_editor = None;
        self.alias_input.clear();
        self.link_query.clear();
        self.play = None;
        self.play_keyboard.new_session();
        self.comparison = play::comparison::ComparisonState::default();
        self.playthrough_report = play::report::PlaythroughReportState::default();
        self.replay_debugger.explanations = None;
        self.replay_debugger.inspection = Default::default();
        self.event_editor = None;
        self.character_editor = None;
        self.world_editor = None;
        self.entity_editor = None;
        self.relation_editor = None;
        self.relation_type_editor = None;
        self.delete_form = None;
        self.rename_form = None;
        self.source_move_form = None;
        self.entity_source_move_form = None;
        self.entity_source_navigation = None;
        self.preset_editor = None;
        self.catalog_target = None;
        self.catalog_workbench.reset_for_workspace();
        self.catalog_import.discard();
        self.tag_editor = None;
        self.state_editor = None;
        self.anchor_editor = None;
        self.io_error = None;
        self.history.clear();
        self.redo.clear();
        self.graph_positions.clear();
        self.character_positions.clear();
        self.character_focus = characters::CharacterFocus::default();
        self.temporal_issues = temporal::issues::TemporalIssues::default();
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
        self.network_state = network_state::NetworkState::default();
        self.topic_session = self.topic_session.wrapping_add(1);
        self.network_loaded_view = None;
        self.network_view_id.clear();
        self.network_view_title.clear();
        self.network_selected = None;
        self.pending_preset_layers = None;
        self.review = collaboration_ui::ReviewState::default();
        self.manuscript = manuscript::WorkbenchState::default();
        self.schema_ui = schema_ui::SchemaUiState::default();
        self.checkpoint_history = checkpoint_history::HistoryState::default();
        self.localization_ui = localization_ui::LocalizationUiState::default();
        self.reader_publish = reader_publish::ReaderPublishState::default();
    }
    pub(super) fn has_open_authoring_form(&self) -> bool {
        !self.dirty_draft_names().is_empty()
    }
}
