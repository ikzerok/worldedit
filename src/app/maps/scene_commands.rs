use super::scene_batch_job::{PreparedScene, SceneBatchJob};
use worldline_core::vector_scene::{SceneBatch, ScenePlan};

impl super::super::WorldeditApp {
    pub(super) fn submit_scene_operations(&mut self, ctx: &egui::Context) {
        if self.map_canvas.scene.job.is_some() || self.map_canvas.scene.operations.is_empty() {
            return;
        }
        if !self.map_canvas.is_edit_mode() {
            self.map_canvas.scene.error = Some("请先进入编辑展示，待提交输入已保留".into());
            return;
        }
        let map_id = self.map_canvas.map_id().to_owned();
        let Some(baseline) = self
            .map_canvas
            .scene
            .intent_baseline
            .clone()
            .or_else(|| self.map_command_baseline(&map_id))
        else {
            self.map_canvas.scene.error = Some("无法读取当前地图基线，输入已保留".into());
            return;
        };
        let batch = SceneBatch {
            map_id,
            expected_revision: baseline.revision,
            expected_documents: baseline.expected_documents,
            operations: self.map_canvas.scene.operations.clone(),
        };
        let generation = self.map_canvas.scene.generation;
        let review = self.map_canvas.scene.review_requested;
        match SceneBatchJob::start(
            &self.project,
            self.map_revision,
            batch,
            generation,
            review,
            ctx,
        ) {
            Ok(job) => {
                self.map_canvas.scene.job = Some(job);
                self.map_canvas.scene.operations.clear();
                self.map_canvas.scene.intent_baseline = None;
                self.map_canvas.scene.review_requested = false;
                self.map_canvas.scene.error = None;
            }
            Err(error) => {
                self.map_canvas.scene.error = Some(error);
                self.map_canvas.scene.retry_operations =
                    std::mem::take(&mut self.map_canvas.scene.operations);
                self.map_canvas.scene.retry_review = review;
                self.map_canvas.scene.intent_baseline = None;
            }
        }
    }

    pub(super) fn poll_scene_operations(&mut self) {
        let Some(result) = self
            .map_canvas
            .scene
            .job
            .as_mut()
            .and_then(SceneBatchJob::poll)
        else {
            return;
        };
        let job = self.map_canvas.scene.job.take().expect("polled scene job");
        let review = job.review;
        let valid = job.baseline == self.project.content_baseline()
            && job.generation == self.map_canvas.scene.generation
            && job.batch.map_id == self.map_canvas.map_id();
        let original = job.batch.operations.clone();
        let result = if valid {
            result
        } else {
            Err("工程或地图已改变，旧预览不可提交；输入已保留".into())
        };
        #[cfg(not(target_arch = "wasm32"))]
        let plan = result.map(|PreparedScene::Plan(plan)| plan);
        #[cfg(target_arch = "wasm32")]
        let plan = result.and_then(|PreparedScene::Batch(batch)| {
            worldline_core::vector_scene::preview_batch(&self.project, self.map_revision, batch)
                .map_err(|error| error.to_string())
        });
        match plan {
            Ok(plan) if review || !plan.diagnostics.is_empty() => {
                self.map_canvas.scene.review_plan = Some(plan);
                self.map_canvas.scene.retry_operations = original;
                self.map_canvas.scene.retry_review = true;
            }
            Ok(plan) => {
                if let Err(error) = self.apply_scene_plan(&plan) {
                    self.map_canvas.scene.error = Some(error);
                    self.map_canvas.scene.retry_operations = original;
                    self.map_canvas.scene.retry_review = false;
                }
            }
            Err(error) => {
                self.map_canvas.scene.error = Some(error);
                self.map_canvas.scene.retry_operations = original;
                self.map_canvas.scene.retry_review = review;
            }
        }
    }

    pub(super) fn apply_scene_plan(&mut self, plan: &ScenePlan) -> Result<(), String> {
        let before = self.project.clone();
        let old_ids: std::collections::BTreeSet<_> = self
            .map_canvas
            .scene
            .source
            .as_ref()
            .map(|scene| scene.nodes.keys().cloned().collect())
            .unwrap_or_default();
        worldline_core::vector_scene::apply_batch(&mut self.project, &mut self.map_revision, plan)
            .map_err(|error| error.to_string())?;
        self.remember(before);
        self.map_canvas.accept_local_preview();
        self.map_canvas.scene.inspector_dirty = false;
        self.map_canvas.scene.retry_operations.clear();
        self.map_canvas.scene.error = None;
        self.refresh_presentation_after_map_command();
        if let Some(scene) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.map_index.maps.get(&plan.map_id))
            .and_then(|map| map.scene.as_ref())
        {
            let created: std::collections::BTreeSet<_> = scene
                .nodes
                .keys()
                .filter(|id| !old_ids.contains(*id))
                .cloned()
                .collect();
            if !created.is_empty() {
                self.map_canvas.scene.selection = created
                    .iter()
                    .filter(|id| {
                        scene.nodes.get(*id).is_some_and(|node| {
                            node.parent_id
                                .as_ref()
                                .is_none_or(|parent| !created.contains(parent))
                        })
                    })
                    .cloned()
                    .collect();
            }
        }
        self.message = Some("矢量修改已应用；保存工程可写入磁盘".into());
        Ok(())
    }

    pub(super) fn scene_job_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(job) = self.map_canvas.scene.job.as_ref() {
            let progress = job.status();
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!(
                    "{} · {} / {}",
                    progress.stage, progress.completed, progress.total
                ));
                if ui.button("取消矢量任务").clicked() {
                    if let Some(job) = self.map_canvas.scene.job.take() {
                        self.map_canvas.scene.retry_operations = job.batch.operations.clone();
                        self.map_canvas.scene.retry_review = job.review;
                    }
                }
            });
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        if let Some(error) = &self.map_canvas.scene.error {
            ui.colored_label(crate::theme::ERROR(), error);
        }
        if !self.map_canvas.scene.retry_operations.is_empty()
            && self.map_canvas.scene.review_plan.is_none()
        {
            ui.horizontal(|ui| {
                if ui.button("按当前版本重新预检").clicked() {
                    self.map_canvas.scene.intent_baseline = None;
                    self.map_canvas.scene.operations =
                        self.map_canvas.scene.retry_operations.clone();
                    self.map_canvas.scene.review_requested = self.map_canvas.scene.retry_review;
                    self.map_canvas.scene.retry_operations.clear();
                }
                if ui.button("放弃这次未提交修改").clicked() {
                    self.map_canvas.scene.retry_operations.clear();
                    self.map_canvas.scene.error = None;
                }
            });
        }
    }
}
