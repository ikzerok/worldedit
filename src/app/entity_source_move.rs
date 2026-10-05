//! 单实体移源只消费 core 的声明身份、活动源码及完整计划。
mod ui;
use super::{Tab, WorldeditApp};
use std::path::PathBuf;
use worldline_core::source_lifecycle::{
    SourceLifecycleFailureKind, SourceLifecyclePlan, SourceLifecycleRequest,
};
use worldline_core::TargetRef;

pub(super) struct EntitySourceMoveForm {
    pub root: PathBuf,
    pub id: String,
    display: String,
    pub source: PathBuf,
    pub destination: PathBuf,
    plan: Option<SourceLifecyclePlan>,
    error: Option<String>,
    failure_kind: Option<SourceLifecycleFailureKind>,
    focus_on_open: bool,
    reveal_preview: bool,
    target_focus_ids: Vec<egui::Id>,
    preview_focus: Option<egui::Id>,
    apply_focus: Option<egui::Id>,
    cancel_focus: Option<egui::Id>,
}
impl EntitySourceMoveForm {
    fn request(&self) -> SourceLifecycleRequest {
        SourceLifecycleRequest::MoveEntity {
            id: self.id.clone(),
            to: self
                .destination
                .strip_prefix(&self.root)
                .unwrap_or(&self.destination)
                .to_path_buf(),
        }
    }
    pub(super) fn changed(&self) -> bool {
        self.destination != self.source
    }
    fn select(&mut self, destination: PathBuf) {
        if self.destination != destination {
            self.destination = destination;
            self.plan = None;
            self.reveal_preview = false;
            self.error = None;
            self.failure_kind = None;
        }
    }
    fn current(&self, app: &WorldeditApp) -> bool {
        self.root == app.project.root
            && self.plan.as_ref().is_some_and(|plan| {
                plan.content_baseline == app.project.content_baseline()
                    && plan.request == self.request()
            })
    }
}
impl WorldeditApp {
    fn entity_source_move_blocker(&self) -> Option<String> {
        let inputs: Vec<_> = self
            .unapplied_export_inputs()
            .into_iter()
            .filter(|input| input.kind != "实体移源")
            .map(|input| format!("{} · {}", input.kind, input.source))
            .collect();
        (!inputs.is_empty()).then(|| {
            format!(
                "请先处理未应用输入，再移动实体声明；输入全部保留：{}",
                inputs.join("；")
            )
        })
    }
    fn entity_move_targets(&self) -> Vec<PathBuf> {
        self.project
            .documents
            .iter()
            .filter(|(path, document)| {
                !document.is_deleted()
                    && path.extension().is_some_and(|ext| ext == "wl")
                    && self
                        .project
                        .source_selection()
                        .is_none_or(|selection| selection.is_active(path))
            })
            .map(|(path, _)| path.clone())
            .collect()
    }
    pub(super) fn begin_current_entity_source_move(&mut self) {
        let id = self
            .entity_editor
            .as_ref()
            .and_then(|form| form.original.clone())
            .or_else(|| {
                self.reading_target
                    .as_ref()
                    .filter(|target| target.kind == "entity")
                    .map(|target| target.id.clone())
            })
            .or_else(|| {
                self.catalog_target
                    .as_ref()
                    .filter(|target| target.kind == "entity")
                    .map(|target| target.id.clone())
            });
        if let Some(id) = id {
            self.begin_entity_source_move(&id);
        } else {
            self.message = Some("请先选中一个 entity 实体资料，再使用“移到其他源码…”。".into());
        }
    }
    pub(super) fn begin_entity_source_move(&mut self, id: &str) {
        if self.prevent_replacing_draft("实体移源") {
            return;
        }
        if let Some(message) = self.entity_source_move_blocker() {
            self.message = Some(message);
            return;
        }
        let target = TargetRef::new("entity", id);
        let Some(object) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.result.analysis.catalog.object(&target))
        else {
            self.message =
                Some("请选择已有的 entity 实体资料；人物、事件和片段不在此动作范围。".into());
            return;
        };
        self.entity_source_move_form = Some(EntitySourceMoveForm {
            root: self.project.root.clone(),
            id: id.into(),
            display: object.display.clone(),
            source: PathBuf::from(&object.file),
            destination: PathBuf::from(&object.file),
            plan: None,
            error: None,
            failure_kind: None,
            focus_on_open: true,
            reveal_preview: false,
            target_focus_ids: Vec::new(),
            preview_focus: None,
            apply_focus: None,
            cancel_focus: None,
        });
    }
    fn preview_entity_source_move(&self, form: &mut EntitySourceMoveForm) {
        form.plan = None;
        form.reveal_preview = false;
        form.failure_kind = None;
        form.error = if form.root != self.project.root {
            Some("工作区已切换，原选择保留；请取消后重新打开。".into())
        } else {
            self.entity_source_move_blocker()
        };
        if form.error.is_some() {
            return;
        }
        match self
            .project
            .preview_source_lifecycle_classified(&form.request())
        {
            Ok(plan) => {
                form.plan = Some(plan);
                form.reveal_preview = true;
            }
            Err(error) => {
                form.failure_kind = Some(error.kind);
                form.error = Some(error.message);
            }
        }
    }
    fn apply_entity_source_move(&mut self, form: &mut EntitySourceMoveForm) -> bool {
        form.failure_kind = None;
        if !form.current(self) {
            form.error = Some("工程、目标或计划已变化，未应用；选择保留，请重新预览。".into());
            form.failure_kind = Some(SourceLifecycleFailureKind::SourceChanged);
            return false;
        }
        if let Some(error) = self.entity_source_move_blocker() {
            form.error = Some(error);
            return false;
        }
        let Some(plan) = &form.plan else {
            return false;
        };
        let before = self.project.clone();
        match self.project.apply_source_lifecycle_plan_classified(plan) {
            Ok(applied) => {
                if applied.changes.is_empty() {
                    self.message =
                        Some("实体仍在同一源码；没有修改，也没有新增撤销或保存动作。".into());
                    return true;
                }
                self.remember(before);
                self.remember_author_position();
                // 没有未应用输入才可来到这里。只关闭该实体的干净资料表单。
                if self
                    .entity_editor
                    .as_ref()
                    .is_some_and(|editor| editor.original.as_deref() == Some(&form.id))
                {
                    self.entity_editor = None;
                }
                self.manuscript.rebase_clean(&self.project);
                self.recompile();
                self.focus_entity_source(&form.id);
                self.io_error = None;
                self.message = Some("实体声明已移到目标源码、尚未保存；只改两份源码，可一次撤销/重做。保存全部后写入磁盘。".into());
                true
            }
            Err(error) => {
                form.failure_kind = Some(error.kind);
                form.error = Some(error.message);
                false
            }
        }
    }
    // 每次从当前 core 目录解析稳定身份，不保存推测行号，也不改写其他对象的历史。
    pub(super) fn focus_entity_source(&mut self, id: &str) {
        let target = TargetRef::new("entity", id);
        if let Some(object) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.result.analysis.catalog.object(&target))
        {
            self.active_file = PathBuf::from(&object.file);
            self.jump = Some((object.line, 1));
            self.tab = Tab::Edit;
            self.catalog_target = Some(target);
            self.entity_source_navigation = Some((id.into(), self.active_file.clone()));
            self.personal.source_view = None;
            self.personal.source_cursor = None;
        } else {
            self.entity_source_navigation = None;
        }
    }
    pub(super) fn rebase_entity_source_location(
        &mut self,
        location: &mut super::personal::Location,
    ) -> Option<String> {
        self.entity_source_navigation = None;
        let id = location
            .source_entity
            .as_ref()
            .filter(|_| location.tab == Some(Tab::Edit))?;
        let object = self
            .snapshot
            .as_ref()?
            .result
            .analysis
            .catalog
            .object(&TargetRef::new("entity", id))?;
        let file = PathBuf::from(&object.file);
        let current = location.file == file
            && location.source_baseline.as_ref().is_some_and(|baseline| {
                self.project
                    .document(&file)
                    .is_ok_and(|source| super::writing_workspace::fingerprint(source) == *baseline)
            });
        self.entity_source_navigation = Some((id.clone(), file.clone()));
        if current {
            return None;
        }
        location.file = file;
        location.cursor = None;
        location.source_baseline = None;
        location.source_view = None;
        location.source_problem = None;
        Some(id.clone())
    }
    pub(super) fn jump_to_entity_source(&mut self, id: &str) {
        self.remember_author_position();
        self.focus_entity_source(id);
    }
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
