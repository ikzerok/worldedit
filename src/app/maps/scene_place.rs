use worldline_core::authoring::EntityDraft;
use worldline_core::vector_scene::{SceneEntityPlan, SceneEntityRequest};

pub(super) struct ScenePlaceForm {
    request: SceneEntityRequest,
    job: Option<PlaceJob>,
    plan: Option<SceneEntityPlan>,
    error: Option<String>,
}

struct PlaceJob {
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<Result<SceneEntityPlan, String>>,
    #[cfg(target_arch = "wasm32")]
    worker: crate::worker_host::WorkerJob,
}

impl PlaceJob {
    fn start(
        project: &worldline_core::project::Project,
        request: SceneEntityRequest,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let project = project.clone();
            let ctx = ctx.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("scene-place-preview".into())
                .spawn(move || {
                    let result = worldline_core::vector_scene::preview_entity_binding(
                        &project,
                        request.expected_revision,
                        request,
                    )
                    .map_err(|error| error.to_string());
                    let _ = sender.send(result);
                    ctx.request_repaint();
                })
                .map_err(|error| error.to_string())?;
            Ok(Self { receiver })
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{WorkRequest, WorkTask};
            let (files, state) = super::scene_snapshot::capture(project)?;
            let task = WorkTask::SceneEntityPreview {
                revision: request.expected_revision,
                request: request.clone(),
            };
            let request = WorkRequest {
                schema_version: 1,
                job_id: "scene-place".into(),
                generation: 0,
                baseline: request.expected_baseline,
                entry: project
                    .entry
                    .strip_prefix(&project.root)
                    .map_err(|_| "工程入口越界")?
                    .to_owned(),
                snapshot_state: Some(state),
                task,
            };
            Ok(Self {
                worker: crate::worker_host::WorkerJob::start(request, &files, ctx)?,
            })
        }
    }

    fn poll(
        &mut self,
        _project: &worldline_core::project::Project,
        _request: &SceneEntityRequest,
    ) -> Option<Result<SceneEntityPlan, String>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match self.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("地点预检线程意外结束，输入保留".into()))
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{WorkEvent, WorkOutput};
            match self.worker.take_event()? {
                WorkEvent::Progress { .. } => None,
                WorkEvent::Error(error) => Some(Err(error)),
                WorkEvent::Done { output, binaries } if binaries.is_empty() => {
                    let WorkOutput::SceneEntityPreview { request } = *output else {
                        return Some(Err("地点预检返回类型不匹配".into()));
                    };
                    if serde_json::to_value(&request).ok() != serde_json::to_value(_request).ok() {
                        return Some(Err("地点预检返回请求与原输入不匹配".into()));
                    }
                    Some(
                        worldline_core::vector_scene::preview_entity_binding(
                            _project,
                            request.expected_revision,
                            request,
                        )
                        .map_err(|error| error.to_string()),
                    )
                }
                _ => Some(Err("地点预检返回类型不匹配".into())),
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for PlaceJob {
    fn drop(&mut self) {
        self.worker.cancel();
    }
}

impl super::super::WorldeditApp {
    pub(super) fn open_scene_place(&mut self, node_id: String) {
        let map_id = self.map_canvas.map_id().to_owned();
        let Some(baseline) = self.map_command_baseline(&map_id) else {
            return;
        };
        let id = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                crate::app::authoring_forms::next_id(&snapshot.result.analysis.catalog, "entity")
            })
            .unwrap_or_else(|| "entity_1".into());
        let path = if self.project.documents.contains_key(&self.active_file) {
            self.active_file.clone()
        } else {
            self.project.entry.clone()
        };
        self.map_canvas.scene.place = Some(ScenePlaceForm {
            request: SceneEntityRequest {
                expected_baseline: self.project.content_baseline(),
                expected_revision: baseline.revision,
                expected_documents: baseline.expected_documents,
                map_id,
                node_id,
                path,
                draft: EntityDraft {
                    id,
                    entity_type: "place".into(),
                    ..Default::default()
                },
            },
            job: None,
            plan: None,
            error: None,
        });
    }

    pub(super) fn scene_place_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.map_canvas.scene.place.take() else {
            return;
        };
        if let Some(result) = form
            .job
            .as_mut()
            .and_then(|job| job.poll(&self.project, &form.request))
        {
            form.job = None;
            match result {
                Ok(plan) if form.request.expected_baseline == self.project.content_baseline() => {
                    form.plan = Some(plan);
                    form.error = None;
                }
                Ok(_) => form.error = Some("工程已变化，输入保留；关闭后重新打开以合并".into()),
                Err(error) => form.error = Some(error),
            }
        }
        let mut cancel = false;
        let mut apply = false;
        egui::Window::new("新建地点并绑定矢量对象")
            .id(egui::Id::new("scene-place-binding"))
            .default_width(580.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    apply = ui
                        .add_enabled(
                            form.plan.is_some(),
                            crate::theme::primary("确认创建与绑定（一次撤销）"),
                        )
                        .clicked();
                    cancel = ui.button("取消，保留原工程").clicked();
                });
                ui.label("资料与地图引用由同一核心事务提交；不会自动升级语言。");
                crate::theme::technical_value(ui, "绑定节点", &form.request.node_id);
                let mut changed = false;
                ui.add_enabled_ui(form.job.is_none(), |ui| {
                    ui.label("稳定 ID");
                    changed |= ui
                        .text_edit_singleline(&mut form.request.draft.id)
                        .changed();
                    ui.label("地点名称");
                    changed |= ui
                        .text_edit_singleline(&mut form.request.draft.display)
                        .changed();
                    ui.label("地点说明");
                    changed |= ui
                        .add(
                            egui::TextEdit::multiline(&mut form.request.draft.description)
                                .char_limit(1024 * 1024),
                        )
                        .changed();
                    egui::ComboBox::from_id_salt("scene-place-source")
                        .selected_text(crate::theme::relative_source(
                            &self.project.root,
                            &form.request.path,
                        ))
                        .show_ui(ui, |ui| {
                            for path in self.project.documents.keys() {
                                changed |= ui
                                    .selectable_value(
                                        &mut form.request.path,
                                        path.clone(),
                                        crate::theme::relative_source(&self.project.root, path),
                                    )
                                    .changed();
                            }
                        });
                    if changed {
                        form.plan = None;
                    }
                    if ui.button("预览地点与绑定影响").clicked() {
                        match PlaceJob::start(&self.project, form.request.clone(), ctx) {
                            Ok(job) => form.job = Some(job),
                            Err(error) => form.error = Some(error),
                        }
                    }
                });
                if form.job.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("后台分析资料、源码与地图绑定…");
                    });
                    if ui.button("取消预检，保留输入").clicked() {
                        form.job = None;
                    }
                    ctx.request_repaint_after(std::time::Duration::from_millis(50));
                }
                if let Some(error) = &form.error {
                    ui.colored_label(crate::theme::ERROR(), error);
                }
                if let Some(plan) = &form.plan {
                    ui.label(format!("{} 个文件将一起改变", plan.changed_files.len()));
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .show(ui, |ui| {
                            for change in &plan.source_changes {
                                crate::theme::source_path(ui, &self.project.root, &change.path);
                                egui::CollapsingHeader::new("候选源码（只读）").show(ui, |ui| {
                                    ui.monospace(&change.after);
                                });
                            }
                        });
                }
            });
        if apply {
            if let Some(plan) = &form.plan {
                let before = self.project.clone();
                match worldline_core::vector_scene::apply_entity_binding(
                    &mut self.project,
                    &mut self.map_revision,
                    plan,
                ) {
                    Ok(_) => {
                        self.remember(before);
                        self.recompile();
                        self.message = Some("地点和绑定已一起应用，可一次撤销".into());
                        return;
                    }
                    Err(error) => form.error = Some(error.to_string()),
                }
            }
        }
        if !cancel {
            self.map_canvas.scene.place = Some(form);
        }
    }
}
