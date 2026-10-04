//! 表单存在与真实改动分开；退出决策不偷偷提交或清除作者输入。
mod new_forms;
use super::*;
use worldline_core::authoring::{EventDraft, WorldDraft};
fn same_world(a: &WorldDraft, b: &WorldDraft) -> bool {
    a.id == b.id
        && a.display == b.display
        && a.description == b.description
        && a.properties == b.properties
}
fn same_event(a: &EventDraft, b: &EventDraft) -> bool {
    a.id == b.id
        && a.summary == b.summary
        && a.storyline == b.storyline
        && a.characters == b.characters
        && a.order == b.order
        && a.period == b.period
        && a.predecessors == b.predecessors
        && a.perm == b.perm
        && a.after == b.after
        && a.effects == b.effects
        && a.body == b.body
}
impl WorldeditApp {
    fn event_draft_dirty(&self, form: &EventEditor) -> bool {
        let Some(id) = form.original.as_ref() else {
            return true;
        };
        if let Some((version, cached_id, draft)) = self.event_draft_cache.borrow().as_ref() {
            if *version == self.version && cached_id == id {
                return !same_event(&form.draft, draft);
            }
        }
        let Ok((_, draft)) = self.project.event_draft(id) else {
            return true;
        };
        let changed = !same_event(&form.draft, &draft);
        self.event_draft_cache
            .replace(Some((self.version, id.clone(), draft)));
        changed
    }
    pub(super) fn current_world_draft(&self) -> WorldDraft {
        self.snapshot
            .as_ref()
            .and_then(|s| s.result.analysis.world.as_ref())
            .map(|w| WorldDraft {
                id: w.id.clone(),
                display: w.display.clone(),
                description: w.description.clone(),
                properties: w.properties.clone().into_iter().collect(),
            })
            .unwrap_or_else(|| WorldDraft {
                id: "my_world".into(),
                display: "新的世界".into(),
                ..Default::default()
            })
    }
    pub(super) fn world_draft_dirty(&self) -> bool {
        self.world_editor
            .as_ref()
            .is_some_and(|d| !same_world(d, &self.current_world_draft()))
    }
    pub(super) fn dirty_draft_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        let catalog = self.snapshot.as_ref().map(|s| &s.result.analysis.catalog);
        if self.ime_composing || self.ime_source_draft.is_some() {
            names.push("正在输入的源码 / 输入法");
        }
        if self
            .event_editor
            .as_ref()
            .is_some_and(|form| self.event_draft_dirty(form))
        {
            names.push("事件正文与分支");
        }
        if self.character_editor.as_ref().is_some_and(|f| {
            f.original
                .as_deref()
                .and_then(|id| {
                    self.snapshot
                        .as_ref()?
                        .result
                        .analysis
                        .symbols
                        .characters
                        .get(id)
                })
                .is_none_or(|d| {
                    f.original.as_deref() != Some(f.draft.id.as_str())
                        || f.draft.display != d.display
                        || f.draft.properties
                            != d.properties.clone().into_iter().collect::<Vec<_>>()
                        || f.draft.relations
                            != d.relations
                                .iter()
                                .map(|r| (r.target.clone(), r.label.clone()))
                                .collect::<Vec<_>>()
                })
        }) {
            names.push("人物资料");
        }
        if self.world_draft_dirty() {
            names.push("世界观");
        }
        if self.entity_editor.as_ref().is_some_and(|f| {
            f.original
                .as_deref()
                .and_then(|id| catalog?.entities.get(id))
                .is_none_or(|d| {
                    f.draft.id != d.id
                        || f.draft.entity_type != d.entity_type
                        || f.draft.display != d.display
                        || f.draft.description != d.description
                        || f.draft.properties
                            != d.properties.clone().into_iter().collect::<Vec<_>>()
                })
        }) {
            names.push("实体资料");
        }
        if self.relation_editor.as_ref().is_some_and(|f| {
            f.original
                .as_deref()
                .and_then(|id| catalog?.relations.get(id))
                .is_none_or(|d| {
                    f.draft.id != d.id
                        || f.draft.relation_type != d.relation_type
                        || f.draft.from != d.from_ref
                        || f.draft.to != d.to_ref
                        || f.draft.description != d.description
                        || f.draft.source_note != d.source_note
                        || f.draft.scope_refs != d.scope_refs
                        || f.draft.properties
                            != d.properties.clone().into_iter().collect::<Vec<_>>()
                })
        }) {
            names.push("语义关系");
        }
        if self.relation_type_editor.as_ref().is_some_and(|f| {
            f.original
                .as_deref()
                .and_then(|id| catalog?.relation_types.get(id))
                .is_none_or(|d| {
                    f.draft.id != d.id
                        || f.draft.display != d.display
                        || f.draft.direction != d.direction
                        || f.draft.inverse_display != d.inverse_display
                        || f.draft.from_kind != d.from_kind
                        || f.draft.to_kind != d.to_kind
                })
        }) {
            names.push("关系类型");
        }
        if self.tag_editor.as_ref().is_some_and(|(id, f)| {
            id.as_deref()
                .and_then(|id| catalog?.tags.get(id))
                .is_none_or(|d| {
                    f.id != d.id
                        || f.display != d.display
                        || f.description != d.description
                        || f.properties != d.properties.clone().into_iter().collect::<Vec<_>>()
                })
        }) {
            names.push("标签");
        }
        if self.state_editor.as_ref().is_some_and(|(id, f)| {
            id.as_deref()
                .and_then(|id| catalog?.states.get(id))
                .is_none_or(|d| {
                    f.id != d.id
                        || f.display != d.display
                        || f.target != d.target
                        || f.tags != d.tags
                })
        }) {
            names.push("状态");
        }
        if self.anchor_editor.as_ref().is_some_and(|(id, f)| {
            id.as_deref()
                .and_then(|id| catalog?.anchors.get(id))
                .is_none_or(|d| {
                    f.id != d.id
                        || f.display != d.display
                        || f.description != d.description
                        || f.targets != d.links.iter().map(|l| l.target.clone()).collect::<Vec<_>>()
                })
        }) {
            names.push("锚点");
        }
        if self
            .wiki_editor
            .as_ref()
            .is_some_and(|f| f.is_dirty(catalog))
        {
            names.push("Wiki词条");
        }
        if self.preset_editor.as_ref().is_some_and(|f| {
            f.original
                .as_deref()
                .and_then(|id| self.snapshot.as_ref()?.preset_index.presets.get(id))
                .is_none_or(|d| f.draft != d.draft)
        }) {
            names.push("展示预设");
        }
        if self
            .rename_form
            .as_ref()
            .is_some_and(|f| f.new_id != f.target.id)
        {
            names.push("对象重命名");
        }
        if self
            .source_move_form
            .as_ref()
            .is_some_and(|form| form.changed())
        {
            names.push("源码路径");
        }
        if self.new_file.is_some() {
            names.push("文件名称");
        }
        if self.new_period.is_some() {
            names.push("时段资料");
        }
        if self.comment_draft_dirty() {
            names.push("审阅批注");
        }
        if self.map_creation.open {
            names.push("新建地图");
        }
        if self.map_form.has_uncommitted_work()
            || self.map_canvas.has_uncommitted_work()
            || self.map_failed_command.is_some()
        {
            names.push("地图草稿");
        }
        if self.schema_ui.has_unsubmitted_work() {
            names.push("持续资料约束草稿");
        }
        if self.manuscript.has_unsubmitted_work() {
            names.push("书稿 / 正文草稿");
        }
        if self.localization_ui.has_unsubmitted_work() {
            names.push("本地化草稿");
        }
        if self.catalog_import.has_unsubmitted_work() {
            names.push("世界资料导入");
        }
        names.retain(|kind| !self.pristine_new_draft(kind));
        names
    }
    pub(super) fn prevent_replacing_draft(&mut self, kind: &'static str) -> bool {
        if self.dirty_draft_names().contains(&kind) || self.frame_dirty_drafts.contains(&kind) {
            self.message = Some(format!(
                "{kind}还有未应用输入，请先应用或恢复；原输入已保留"
            ));
            true
        } else {
            false
        }
    }
    pub(super) fn prevent_catalog_switch(&mut self) -> bool {
        for kind in ["标签", "状态", "锚点"] {
            if self.prevent_replacing_draft(kind) {
                return true;
            }
        }
        false
    }
    pub(super) fn discard_authoring_drafts(&mut self) {
        self.new_draft_baselines.clear();
        self.frame_dirty_drafts.clear();
        self.ime_composing = false;
        self.ime_source_baseline = None;
        self.ime_source_draft = None;
        self.event_editor = None;
        self.character_editor = None;
        self.world_editor = None;
        self.entity_editor = None;
        self.relation_editor = None;
        self.relation_type_editor = None;
        self.tag_editor = None;
        self.state_editor = None;
        self.anchor_editor = None;
        self.wiki_editor = None;
        self.preset_editor = None;
        self.rename_form = None;
        self.source_move_form = None;
        self.delete_form = None;
        self.new_file = None;
        self.new_period = None;
        self.review.comment_editor = None;
        self.review.pending_comment_action = None;
        self.map_creation = map_creation::MapCreationForm::default();
        self.map_form = maps::PlacementForm::default();
        self.map_canvas.discard_local_work();
        self.map_failed_command = None;
        self.manuscript = manuscript::WorkbenchState::default();
        self.schema_ui = schema_ui::SchemaUiState::default();
        self.localization_ui = localization_ui::LocalizationUiState::default();
        self.catalog_import.discard();
        self.stale_form = false;
    }
    pub(super) fn draft_exit_dialog(&mut self, ctx: &egui::Context) {
        self.review_draft_dialog(ctx);
        if self.draft_action.is_none() {
            return;
        }
        let names = self.dirty_draft_names();
        let mut discard = false;
        let mut cancel = false;
        let mut return_to = None;
        egui::Window::new("还有未应用的创作输入")
            .id(egui::Id::new("draft-exit"))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(
                    "这些输入尚未进入工程文件。回到对应页面应用，再保存；或明确丢弃这些草稿。",
                );
                for name in &names {
                    ui.label(format!("• {name}"));
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button("回到草稿").clicked() {
                        return_to = names.first().copied();
                        cancel = true;
                    }
                    if ui
                        .add_enabled(
                            !self.ime_composing,
                            egui::Button::new("丢弃未应用输入并继续"),
                        )
                        .clicked()
                    {
                        discard = true;
                    }
                    if ui.button("取消离开").clicked() {
                        cancel = true;
                    }
                });
                if self.ime_composing {
                    ui.label("请先完成或取消输入法组合，再决定是否丢弃。");
                }
            });
        if let Some(name) = return_to {
            self.tab = match name {
                "世界观" => Tab::World,
                "人物资料" => Tab::Characters,
                "事件正文与分支" => Tab::Timeline,
                "书稿 / 正文草稿" => Tab::Manuscript,
                "本地化草稿" => Tab::Localization,
                "世界资料导入" => Tab::CatalogImport,
                "审阅批注" => Tab::Review,
                "地图草稿" | "新建地图" => Tab::Map,
                "时段资料" => Tab::Timeline,
                "文件名称" | "源码路径" => Tab::Edit,
                "标签" | "状态" | "锚点" => Tab::Catalog,
                "正在输入的源码 / 输入法" => Tab::Edit,
                _ => self.tab,
            };
        }
        if discard {
            let action = self.draft_action.take();
            self.discard_authoring_drafts();
            if let Some(action) = action {
                self.request_action(action, ctx);
            }
        } else if cancel {
            self.draft_action = None;
        }
    }
}
