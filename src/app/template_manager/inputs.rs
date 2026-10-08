use worldline_core::project_templates::{ProjectTemplate, ProjectTemplateField};
use worldline_core::TargetRef;

#[derive(Clone, PartialEq)]
pub(in crate::app) struct MetadataInput {
    pub title: String,
    pub kind: String,
    pub entity_type: String,
    original: (String, String, String),
}
impl MetadataInput {
    pub fn new(template: &ProjectTemplate) -> Self {
        let original = (
            template.title.clone(),
            template.applies_to.kind.clone(),
            template.applies_to_entity_type.clone().unwrap_or_default(),
        );
        Self {
            title: original.0.clone(),
            kind: original.1.clone(),
            entity_type: original.2.clone(),
            original,
        }
    }
    pub fn changed(&self) -> bool {
        (&self.title, &self.kind, &self.entity_type)
            != (&self.original.0, &self.original.1, &self.original.2)
    }
}

#[derive(Clone, PartialEq)]
pub(in crate::app) struct FieldInput {
    pub field: ProjectTemplateField,
    pub choices_text: String,
    pub has_default: bool,
    pub default_text: String,
    pub default_bool: bool,
    pub default_ref: Option<TargetRef>,
    original: ProjectTemplateField,
}
impl FieldInput {
    pub fn new(field: &ProjectTemplateField) -> Self {
        Self {
            field: field.clone(),
            choices_text: field.choices.join("\n"),
            has_default: field.default.is_some(),
            default_text: field
                .default
                .as_ref()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string())
                })
                .unwrap_or_default(),
            default_bool: field
                .default
                .as_ref()
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            default_ref: field.default.as_ref().and_then(|value| {
                Some(TargetRef::new(
                    value.get("kind")?.as_str()?,
                    value.get("id")?.as_str()?,
                ))
            }),
            original: field.clone(),
        }
    }
    pub fn changed(&self) -> bool {
        self != &Self::new(&self.original)
    }
    pub fn reset(&mut self) {
        *self = Self::new(&self.original);
    }
    pub fn change_type(&mut self, kind: &str) {
        self.field.field_type = kind.into();
        self.has_default = false;
        self.default_text.clear();
        self.default_ref = None;
        self.field.choices.clear();
        self.choices_text.clear();
        self.field.target = None;
        self.field.target_entity_type = None;
        if kind == "enum" {
            self.choices_text = "草稿\n完成".into();
        }
        if kind == "object_ref" {
            self.field.target = Some(TargetRef::new("entity", ""));
        }
    }
    pub fn value(&self) -> Result<ProjectTemplateField, String> {
        let mut field = self.field.clone();
        field.choices = if field.field_type == "enum" {
            self.choices_text.split('\n').map(str::to_owned).collect()
        } else {
            Vec::new()
        };
        field.default = if !self.has_default {
            None
        } else {
            Some(match field.field_type.as_str() {
                "text" | "enum" => serde_json::Value::String(self.default_text.clone()),
                "number" => {
                    let number = self
                        .default_text
                        .parse::<f64>()
                        .map_err(|_| "默认数值无效；输入已保留")?;
                    serde_json::Number::from_f64(number)
                        .map(serde_json::Value::Number)
                        .ok_or("默认数值必须有限；输入已保留")?
                }
                "boolean" => serde_json::Value::Bool(self.default_bool),
                "object_ref" => {
                    let target = self
                        .default_ref
                        .as_ref()
                        .ok_or("请选择默认引用对象，或取消默认提示")?;
                    serde_json::json!({"kind": target.kind, "id": target.id})
                }
                _ => return Err("分组不能设置默认提示".into()),
            })
        };
        Ok(field)
    }
}
