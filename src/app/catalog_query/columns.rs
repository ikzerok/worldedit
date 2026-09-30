//! 个人展示列不属于共享查询；仅读现有 core 投影，不引入排序/写入语义。
use super::*;
use worldline_core::{ast::PropertyValue, Analysis};
const KEY: &str = "worldedit.catalog.columns.v1";
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Column {
    EntityType,
    Tags,
    Property(String),
}
impl Column {
    pub fn label(&self) -> &str {
        match self {
            Self::EntityType => "实体分类",
            Self::Tags => "标签",
            Self::Property(key) => key,
        }
    }
}
fn property<'a>(
    analysis: &'a Analysis,
    target: &TargetRef,
    key: &str,
) -> Option<&'a PropertyValue> {
    match target.kind.as_str() {
        "character" => analysis
            .symbols
            .characters
            .get(&target.id)?
            .properties
            .get(key),
        "entity" => analysis
            .catalog
            .entities
            .get(&target.id)?
            .properties
            .get(key),
        "relation" => analysis
            .catalog
            .relations
            .get(&target.id)?
            .properties
            .get(key),
        "tag" => analysis.catalog.tags.get(&target.id)?.properties.get(key),
        "world" => analysis
            .world
            .as_ref()
            .filter(|w| w.id == target.id)?
            .properties
            .get(key),
        _ => None,
    }
}
fn scalar(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Str(s) if s.is_empty() => "\"\"（空文本）".into(),
        PropertyValue::Str(s) => format!("文本：{s}"),
        PropertyValue::Num(n) => format!("数值：{n}"),
        PropertyValue::Bool(v) => format!("布尔：{v}"),
        PropertyValue::Ref(r) => format!("引用：{}:{}", r.kind, r.id),
    }
}
pub(super) fn value(analysis: &Analysis, target: &TargetRef, column: &Column) -> String {
    match column {
        Column::EntityType => analysis
            .catalog
            .entities
            .get(&target.id)
            .filter(|_| target.kind == "entity")
            .map(|e| e.entity_type.clone())
            .unwrap_or_else(|| "—".into()),
        Column::Tags => {
            let tags: BTreeSet<_> = analysis
                .catalog
                .marks
                .iter()
                .filter(|m| m.target == *target)
                .flat_map(|m| m.values.iter())
                .collect();
            if tags.is_empty() {
                "未设置".into()
            } else {
                tags.into_iter()
                    .map(|id| {
                        analysis
                            .catalog
                            .tags
                            .get(id)
                            .map(|tag| format!("{} ({id})", tag.display))
                            .unwrap_or_else(|| id.clone())
                    })
                    .collect::<Vec<_>>()
                    .join("、")
            }
        }
        Column::Property(key) => property(analysis, target, key)
            .map(scalar)
            .unwrap_or_else(|| "未设置".into()),
    }
}
impl WorkbenchState {
    pub(in crate::app) fn restore_columns(&mut self, storage: Option<&dyn eframe::Storage>) {
        let raw = storage.and_then(|s| s.get_string(KEY));
        #[cfg(target_arch = "wasm32")]
        let raw = raw.or_else(|| {
            web_sys::window()?
                .local_storage()
                .ok()??
                .get_item(KEY)
                .ok()?
        });
        let Some(raw) = raw else {
            return;
        };
        if raw.len() > 262144 {
            return;
        }
        if let Ok(mut columns) = serde_json::from_str::<BTreeMap<PathBuf, Vec<Column>>>(&raw) {
            if columns.len() > 64 {
                return;
            }
            for row in columns.values_mut() {
                row.truncate(8);
                let mut seen = Vec::new();
                row.retain(|c| {
                    if seen.contains(c) {
                        false
                    } else {
                        seen.push(c.clone());
                        true
                    }
                });
            }
            self.personal_columns = columns;
        }
    }
    pub(in crate::app) fn save_columns(&self, storage: &mut dyn eframe::Storage) {
        if let Ok(text) = serde_json::to_string(&self.personal_columns) {
            storage.set_string(KEY, text);
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(in crate::app) fn save_browser_columns(&self, ctx: &egui::Context) {
        let Ok(text) = serde_json::to_string(&self.personal_columns) else {
            return;
        };
        let key = egui::Id::new(KEY);
        if ctx.data(|data| data.get_temp::<String>(key)).as_deref() == Some(text.as_str()) {
            return;
        }
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            if storage.set_item(KEY, &text).is_ok() {
                ctx.data_mut(|data| data.insert_temp(key, text));
            }
        }
    }
    pub(super) fn columns_menu(&mut self, app: &WorldeditApp, ui: &mut egui::Ui) {
        let cols = self
            .personal_columns
            .entry(app.personal_workspace_key())
            .or_default();
        ui.menu_button("显示列 · 个人", |ui| {
            for candidate in [Column::EntityType, Column::Tags] {
                let mut active = cols.contains(&candidate);
                if ui.checkbox(&mut active, candidate.label()).changed() {
                    if active {
                        cols.push(candidate);
                    } else {
                        cols.retain(|c| c != &candidate);
                    }
                }
            }
            let mut keys = BTreeSet::new();
            if let Some(snapshot) = &app.snapshot {
                let a = &snapshot.result.analysis;
                for info in a.catalog.entities.values() {
                    keys.extend(info.properties.keys().cloned());
                }
                for info in a.symbols.characters.values() {
                    keys.extend(info.properties.keys().cloned());
                }
                for info in a.catalog.relations.values() {
                    keys.extend(info.properties.keys().cloned());
                }
                for info in a.catalog.tags.values() {
                    keys.extend(info.properties.keys().cloned());
                }
                if let Some(w) = &a.world {
                    keys.extend(w.properties.keys().cloned());
                }
            }
            egui::CollapsingHeader::new("属性列（最多六个）").show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for key in keys {
                            let column = Column::Property(key);
                            let mut active = cols.contains(&column);
                            let count = cols
                                .iter()
                                .filter(|c| matches!(c, Column::Property(_)))
                                .count();
                            if ui
                                .add_enabled(
                                    active || count < 6,
                                    egui::Checkbox::new(&mut active, column.label()),
                                )
                                .changed()
                            {
                                if active {
                                    cols.push(column);
                                } else {
                                    cols.retain(|c| c != &column);
                                }
                            }
                        }
                    });
            });
            ui.separator();
            ui.label("列顺序（不改变记录排序）");
            let mut move_up = None;
            let mut move_down = None;
            let mut remove = None;
            for (index, column) in cols.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(column.label());
                    if ui.add_enabled(index > 0, egui::Button::new("↑")).clicked() {
                        move_up = Some(index);
                    }
                    if ui
                        .add_enabled(index + 1 < cols.len(), egui::Button::new("↓"))
                        .clicked()
                    {
                        move_down = Some(index);
                    }
                    if ui.small_button("移除").clicked() {
                        remove = Some(index);
                    }
                });
            }
            if let Some(i) = move_up {
                cols.swap(i, i - 1);
            }
            if let Some(i) = move_down {
                cols.swap(i, i + 1);
            }
            if let Some(i) = remove {
                cols.remove(i);
            }
            ui.label("仅显示属性，不能按属性排序或批量改稿。");
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_does_not_collapse_missing_false_zero_or_empty() {
        assert_eq!(scalar(&PropertyValue::Bool(false)), "布尔：false");
        assert_eq!(scalar(&PropertyValue::Num(0.0)), "数值：0");
        assert_eq!(scalar(&PropertyValue::Str(String::new())), "\"\"（空文本）");
        assert_eq!(
            scalar(&PropertyValue::Ref(TargetRef::new("entity", "a"))),
            "引用：entity:a"
        );
    }
}
