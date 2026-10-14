use super::{SchemaAuthoringState, SchemaCoverage, Session};
use crate::app::{authoring_forms::FormGuard, WorldeditApp};
use crate::theme;
use worldline_core::TargetRef;

impl WorldeditApp {
    pub(in crate::app) fn open_schema_authoring(&mut self, ctx: &egui::Context, target: TargetRef) {
        if self.auxiliary_ime_active(ctx) {
            self.message = Some("请先完成输入法组合，再切换资料工作表；当前输入已保留".into());
            return;
        }
        if let Err(error) = self.schema_authoring.reserve_target(&target) {
            self.message = Some(error);
            return;
        }
        if self
            .schema_authoring
            .sessions
            .get(&target)
            .is_none_or(|session| !session.dirty())
        {
            match self.current_schema_authoring(&target) {
                Ok(projection) => {
                    self.schema_authoring
                        .sessions
                        .insert(target.clone(), Session::new(projection));
                    self.schema_authoring.error = None;
                }
                Err(error) => self.schema_authoring.error = Some(error),
            }
        }
        self.schema_authoring.active = Some(target);
        self.schema_authoring.return_focus = ctx.memory(|memory| memory.focused());
        self.schema_authoring.open = true;
        self.schema_authoring.focus_pending = true;
        self.schema_authoring.restore_focus = None;
        self.schema_authoring.source_return = false;
    }

    /// 资料入口也提供覆盖键；旧模板与泛型控件只显示说明，不产生第二份编辑事实。
    pub(in crate::app) fn schema_authoring_entry(
        &mut self,
        ui: &mut egui::Ui,
        target: &TargetRef,
        dirty: bool,
    ) -> SchemaCoverage {
        let projection = self.current_schema_authoring_shared(target);
        let keys = projection
            .as_ref()
            .map(|value| value.fields.iter().map(|field| field.key.clone()).collect())
            .unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            if crate::theme::add_enabled(ui, !dirty, egui::Button::new("约束与属性…")).clicked()
            {
                self.open_schema_authoring(ui.ctx(), target.clone());
            }
            match &projection {
                Ok(value) => ui.label(theme::muted(
                    value
                        .binding
                        .as_ref()
                        .map(|binding| format!("schema · {}", binding.schema_id))
                        .unwrap_or_else(|| "可显式绑定已有 schema".into()),
                )),
                Err(_) => ui.label(theme::muted("约束投影暂不可用；打开查看原因")),
            };
        });
        if dirty {
            ui.label(theme::muted(
                "先应用当前资料输入，再打开约束工作表；输入不会合并或丢弃。",
            ));
        }
        SchemaCoverage {
            keys,
            projection: projection.ok(),
        }
    }

    pub(super) fn schema_target_draft_dirty(&self, target: &TargetRef) -> bool {
        let kind = match target.kind.as_str() {
            "world"
                if self
                    .world_editor
                    .as_ref()
                    .is_some_and(|draft| draft.id == target.id) =>
            {
                "世界观"
            }
            "character"
                if self
                    .character_editor
                    .as_ref()
                    .is_some_and(|form| form.original.as_ref() == Some(&target.id)) =>
            {
                "人物资料"
            }
            "entity"
                if self
                    .entity_editor
                    .as_ref()
                    .is_some_and(|form| form.original.as_ref() == Some(&target.id)) =>
            {
                "实体资料"
            }
            "relation"
                if self
                    .relation_editor
                    .as_ref()
                    .is_some_and(|form| form.original.as_ref() == Some(&target.id)) =>
            {
                "语义关系"
            }
            _ => return false,
        };
        self.dirty_draft_names().contains(&kind)
    }

    pub(super) fn refresh_schema_target_editor(&mut self, target: &TargetRef) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        match target.kind.as_str() {
            "world" => {
                if self
                    .world_editor
                    .as_ref()
                    .is_some_and(|draft| draft.id == target.id)
                {
                    self.world_editor = Some(self.current_world_draft());
                }
            }
            "character" => {
                if let (Some(editor), Some(item)) = (
                    &mut self.character_editor,
                    snapshot.result.analysis.symbols.characters.get(&target.id),
                ) {
                    if editor.original.as_ref() == Some(&target.id) {
                        editor.draft.properties = item.properties.clone().into_iter().collect();
                    }
                }
            }
            "entity" => {
                if let (Some(form), Some(item)) = (
                    &mut self.entity_editor,
                    snapshot.result.analysis.catalog.entities.get(&target.id),
                ) {
                    if form.original.as_ref() == Some(&target.id) {
                        form.draft.properties = item.properties.clone().into_iter().collect();
                        form.guard = FormGuard::capture(&self.project, self.version);
                    }
                }
            }
            "relation" => {
                if let (Some(form), Some(item)) = (
                    &mut self.relation_editor,
                    snapshot.result.analysis.catalog.relations.get(&target.id),
                ) {
                    if form.original.as_ref() == Some(&target.id) {
                        form.draft.properties = item.properties.clone().into_iter().collect();
                        form.guard = FormGuard::capture(&self.project, self.version);
                    }
                }
            }
            _ => {}
        }
    }
    pub(in crate::app) fn clear_schema_authoring(&mut self) {
        self.schema_authoring = SchemaAuthoringState::default();
    }
}
