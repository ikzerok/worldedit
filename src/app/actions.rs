use super::*;
use std::path::Path;

impl WorldeditApp {
    pub(super) fn request_action(&mut self, action: Pending, ctx: &egui::Context) {
        if self.template_manager.composition_blocks_actions(ctx) {
            self.message = Some("模板定义仍在输入法组合中；请先完成输入，原字段已保留。".into());
            return;
        }
        if self.has_open_authoring_form() {
            self.draft_action = Some(action);
            self.message = Some(
                "仍有未提交的创作输入，请先应用、完成或恢复后，再切换或关闭工程。输入已保留。"
                    .into(),
            );
            return;
        }
        if self.project.is_dirty() {
            self.pending = Some(action);
        } else {
            self.perform_action(action, ctx);
        }
    }
    pub(super) fn perform_action(&mut self, action: Pending, _ctx: &egui::Context) {
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
                    self.project = Project::new(&super::init::draft_root());
                }
                #[cfg(target_arch = "wasm32")]
                crate::web::mount(Default::default());
                self.active_file = self.project.entry.clone();
                self.saved_location = cfg!(not(target_arch = "wasm32"));
                self.reset_views();
                self.recompile();
                self.tab = Tab::Manuscript;
            }
            Pending::Close => {
                self.map_canvas.discard_local_work();
                if !self.prepare_reader_app_close(_ctx) {
                    _ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    return;
                }
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
    pub(super) fn default_event_file(&self) -> PathBuf {
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
    pub(super) fn new_event(&mut self, lane: Option<&str>) {
        if self.prevent_replacing_draft("事件正文与分支") {
            return;
        }
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
            baseline: self.project.content_baseline(),
            predecessor_query: String::new(),
            temporal_cache: None,
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
        self.reset_new_draft_baseline("事件正文与分支");
    }
    pub(super) fn select_event(&mut self, id: &str) {
        if self.prevent_replacing_draft("事件正文与分支") {
            return;
        }
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
                    baseline: self.project.content_baseline(),
                    predecessor_query: String::new(),
                    temporal_cache: None,
                    path,
                    original: Some(id.into()),
                    draft,
                })
            }
            Err(e) => self.io_error = Some(e),
        }
    }
    pub(super) fn undo(&mut self, forward: bool) {
        if (if forward { &self.redo } else { &self.history }).is_empty() {
            return;
        }
        {
            let entity_navigation = self
                .entity_source_navigation
                .as_ref()
                .filter(|(_, path)| *path == self.active_file && self.tab == Tab::Edit)
                .map(|(id, _)| id.clone());
            let source_before = self.project.sources();
            let options_before = self.project.compile_options();
            match self.restore_history_step(forward) {
                Ok(true) => {}
                Ok(false) => return,
                Err(error) => {
                    self.io_error = Some(error);
                    return;
                }
            }
            self.localization_ui.clear_operation_status();
            if !self.project.documents.contains_key(&self.active_file) {
                self.active_file = self.project.entry.clone();
            }
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
            self.map_failed_command = None;
            self.map_canvas.reset_for_history();
            if source_before == self.project.sources()
                && options_before == self.project.compile_options()
            {
                self.map_revision = self.map_revision.next_presentation();
                self.refresh_presentation_after_map_command();
            } else {
                self.recompile();
            }
            if let Some(id) = entity_navigation {
                self.focus_entity_source(&id);
            }
            self.io_error = None;
        }
    }
}
