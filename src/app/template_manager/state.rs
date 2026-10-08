use super::*;
use worldline_core::ast::PropertyValue;

#[derive(Clone, Copy, Default, PartialEq)]
pub(in crate::app) enum Mode {
    Design,
    Trial,
    #[default]
    Json,
}
#[derive(Clone)]
pub(in crate::app) enum Action {
    Select(String),
    New,
    Copy(String),
    Reload(String),
}
#[derive(Default)]
pub(in crate::app) struct ManagerState {
    pub selected_id: Option<String>,
    pub editor: String,
    pub baseline_editor: String,
    pub is_new: bool,
    pub existing_id: Option<String>,
    pub reserved_field_ids: Vec<String>,
    pub reserved_keys: Vec<String>,
    pub preview: Option<ProjectTemplatePreview>,
    pub impact_page: usize,
    pub error: Option<String>,
    pub mode: Mode,
    pub catalog_open: bool,
    pub pending: Option<Action>,
    pub remove_field: Option<String>,
    pub selected_field: Option<String>,
    pub metadata: Option<MetadataInput>,
    pub properties: Option<FieldInput>,
    pub trial_values: Vec<(String, PropertyValue)>,
    pub projection: Option<ProjectTemplateDraftProjection>,
    pub projection_version: Option<u64>,
    pub draft_history: Vec<String>,
    pub move_parent: Option<String>,
    pub ime_composing: bool,
    pub(super) composition: super::input::Composition,
}
impl ManagerState {
    pub(in crate::app) fn has_unsubmitted_work(&self) -> bool {
        self.is_new
            || self.ime_composing
            || self.editor != self.baseline_editor
            || self.has_property_input()
            || self.preview.as_ref().is_some_and(|preview| {
                matches!(preview.mutation, ProjectTemplateMutation::Delete { .. })
            })
    }
    pub(in crate::app) fn composition_busy(&self) -> bool {
        self.ime_composing
    }
    pub(in crate::app) fn composition_blocks_actions(&self, ctx: &egui::Context) -> bool {
        super::input::blocks_actions(self, ctx)
    }
    pub(in crate::app) fn filter_raw_input(
        &mut self,
        ctx: &egui::Context,
        raw: &mut egui::RawInput,
        available: bool,
    ) {
        super::input::filter_raw_input(self, ctx, raw, available);
    }
    pub(in crate::app) fn discard(&mut self) {
        *self = Self::default();
    }
    pub(in crate::app) fn unapplied_source(
        &self,
        _index: Option<&ProjectTemplateIndex>,
    ) -> Option<String> {
        self.has_unsubmitted_work().then(|| {
            self.selected_id
                .as_ref()
                .map(|id| format!("模板管理器 · {id}"))
                .unwrap_or_else(|| "模板管理器 · 待导入".into())
        })
    }
    pub fn has_property_input(&self) -> bool {
        self.metadata.as_ref().is_some_and(MetadataInput::changed)
            || self.properties.as_ref().is_some_and(FieldInput::changed)
    }
    pub fn draft(&self) -> ProjectTemplateDraft {
        ProjectTemplateDraft {
            source_bytes: self.editor.as_bytes().to_vec(),
            existing_id: self.existing_id.clone(),
            reserved_field_ids: self.reserved_field_ids.clone(),
            reserved_keys: self.reserved_keys.clone(),
        }
    }
    pub fn invalidate(&mut self) {
        self.preview = None;
        self.impact_page = 0;
        self.projection = None;
        self.projection_version = None;
        self.error = None;
    }
    pub fn request(&mut self, action: Action) -> Option<Action> {
        if self.ime_composing {
            self.error = Some("请先完成或取消输入法组合；当前草稿已保留".into());
            return None;
        }
        if let Action::Select(id) = &action {
            if self.selected_id.as_ref() == Some(id) {
                return None;
            }
        }
        if self.has_unsubmitted_work() {
            self.pending = Some(action);
            None
        } else {
            Some(action)
        }
    }
    pub fn install(
        &mut self,
        projection: ProjectTemplateDraftProjection,
        is_new: bool,
    ) -> Result<(), String> {
        let text = String::from_utf8(projection.draft.source_bytes.clone())
            .map_err(|_| "此模板包含无效 UTF-8；原字节已保留在工程，不能转换为可编辑文本")?;
        let continuing =
            self.existing_id.is_some() && self.existing_id == projection.draft.existing_id;
        let mut ids = if continuing {
            self.reserved_field_ids.clone()
        } else {
            Vec::new()
        };
        let mut keys = if continuing {
            self.reserved_keys.clone()
        } else {
            Vec::new()
        };
        ids.extend(projection.draft.reserved_field_ids.iter().cloned());
        ids.sort();
        ids.dedup();
        keys.extend(projection.draft.reserved_keys.iter().cloned());
        keys.sort();
        keys.dedup();
        self.existing_id = projection.draft.existing_id.clone();
        self.reserved_field_ids = ids;
        self.reserved_keys = keys;
        self.editor = text.clone();
        self.baseline_editor = text;
        self.is_new = is_new;
        self.metadata = projection.template.as_ref().map(MetadataInput::new);
        self.properties = None;
        self.selected_field = None;
        self.move_parent = None;
        self.catalog_open = false;
        self.projection = Some(projection);
        self.projection_version = None;
        self.preview = None;
        self.impact_page = 0;
        self.error = None;
        self.pending = None;
        self.remove_field = None;
        self.trial_values.clear();
        self.draft_history.clear();
        Ok(())
    }
    pub fn accept_edit(&mut self, projection: ProjectTemplateDraftProjection) {
        let Ok(text) = String::from_utf8(projection.draft.source_bytes.clone()) else {
            return;
        };
        if self.editor != text {
            self.draft_history.push(self.editor.clone());
        }
        self.editor = text;
        self.reserved_field_ids = projection.draft.reserved_field_ids.clone();
        self.reserved_keys = projection.draft.reserved_keys.clone();
        self.preview = None;
        self.impact_page = 0;
        self.error = None;
        self.metadata = projection.template.as_ref().map(MetadataInput::new);
        self.properties = self.selected_field.as_deref().and_then(|id| {
            find_field(&projection.template.as_ref()?.fields, id).map(FieldInput::new)
        });
        self.projection = Some(projection);
    }
    pub fn select_field(&mut self, template: &ProjectTemplate, id: &str) {
        if self.composition_busy() {
            return;
        }
        if self.selected_field.as_deref() == Some(id) {
            return;
        }
        if self.properties.as_ref().is_some_and(FieldInput::changed) {
            self.error = Some("字段输入尚未更新，请先更新字段草稿或还原字段输入".into());
            return;
        }
        self.selected_field = Some(id.into());
        self.properties = find_field(&template.fields, id).map(FieldInput::new);
        self.move_parent = parent_of(&template.fields, id, None);
    }
}
pub(super) fn find_field<'a>(
    fields: &'a [ProjectTemplateField],
    id: &str,
) -> Option<&'a ProjectTemplateField> {
    fields.iter().find_map(|field| {
        if field.id == id {
            Some(field)
        } else {
            find_field(&field.fields, id)
        }
    })
}
pub(super) fn parent_of(
    fields: &[ProjectTemplateField],
    id: &str,
    parent: Option<&str>,
) -> Option<String> {
    for field in fields {
        if field.id == id {
            return parent.map(str::to_owned);
        }
        if find_field(&field.fields, id).is_some() {
            return parent_of(&field.fields, id, Some(&field.id));
        }
    }
    None
}
