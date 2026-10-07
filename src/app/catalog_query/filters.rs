use super::*;
use egui::Ui;
use std::path::Path;
use worldline_core::project::Project;
use worldline_core::queries::{CatalogQueryFilter, MissingCondition, RelationCondition};
pub(super) fn render_filter_row(
    ui: &mut Ui,
    filter: &mut CatalogQueryFilter,
    inputs: &mut FilterInputs,
    project: &Project,
) -> bool {
    let label = match filter_dimension(filter) {
        "kind" => "对象类型",
        "name" => "名称 / ID / 别名",
        "tag" => "标签",
        "property" => "属性值",
        "relation" => "明确关系",
        "author_scope" => "来源文件",
        _ => "缺少资料",
    };
    let mut remove_filter = false;
    ui.group(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.strong(label);
            let negate = match filter {
                CatalogQueryFilter::Kind { negate, .. }
                | CatalogQueryFilter::Name { negate, .. }
                | CatalogQueryFilter::Tag { negate, .. }
                | CatalogQueryFilter::Property { negate, .. }
                | CatalogQueryFilter::Relation { negate, .. }
                | CatalogQueryFilter::AuthorScope { negate, .. }
                | CatalogQueryFilter::Missing { negate, .. } => negate,
            };
            ui.checkbox(negate, "排除此条件");
            if ui.small_button(format!("删除{label}条件")).clicked() {
                remove_filter = true;
            }
        });
        match filter {
            CatalogQueryFilter::Kind { values, .. } => string_value_row(ui, values),
            CatalogQueryFilter::Name { values, .. } => {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut inputs.name_value)
                            .hint_text("输入名称、ID 或别名"),
                    );
                    if ui
                        .add_enabled(
                            !inputs.name_value.trim().is_empty(),
                            egui::Button::new("添加名称值"),
                        )
                        .clicked()
                    {
                        push_unique(values, std::mem::take(&mut inputs.name_value));
                    }
                });
                string_value_row(ui, values);
            }
            CatalogQueryFilter::Tag {
                values, recursive, ..
            } => {
                ui.checkbox(recursive, "递归解引用标签");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut inputs.tag_value).hint_text("标签 ID"));
                    if ui
                        .add_enabled(
                            !inputs.tag_value.trim().is_empty(),
                            egui::Button::new("添加标签值"),
                        )
                        .clicked()
                    {
                        push_unique(values, std::mem::take(&mut inputs.tag_value));
                    }
                });
                string_value_row(ui, values);
            }
            CatalogQueryFilter::Property { values, .. } => {
                super::property_input::render(ui, values, inputs, &project.root);
            }
            CatalogQueryFilter::Relation { values, .. } => {
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut inputs.relation_type)
                            .hint_text("关系类型 ID（可留空）"),
                    );
                    egui::ComboBox::from_id_salt("query-relation-direction")
                        .selected_text(direction_label(inputs.relation_direction))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut inputs.relation_direction,
                                RelationDirection::Either,
                                "任一方向",
                            );
                            ui.selectable_value(
                                &mut inputs.relation_direction,
                                RelationDirection::Outgoing,
                                "出边",
                            );
                            ui.selectable_value(
                                &mut inputs.relation_direction,
                                RelationDirection::Incoming,
                                "入边",
                            );
                        });
                    ui.add(
                        egui::TextEdit::singleline(&mut inputs.relation_related)
                            .hint_text("目标 kind:id（可留空）"),
                    );
                    if ui.button("添加关系值").clicked() {
                        let related = parse_target(&inputs.relation_related);
                        if inputs.relation_related.trim().is_empty() || related.is_some() {
                            let condition = RelationCondition {
                                relation_type: nonempty(&inputs.relation_type),
                                direction: inputs.relation_direction,
                                related,
                            };
                            if !values.contains(&condition) {
                                values.push(condition);
                            }
                        }
                    }
                });
                relation_value_row(ui, values);
            }
            CatalogQueryFilter::AuthorScope { source_files, .. } => {
                let files = source_files_in(project);
                egui::ComboBox::from_id_salt("query-author-scope-file")
                    .selected_text(if inputs.scope_file.is_empty() {
                        "选择来源 .wl 文件"
                    } else {
                        &inputs.scope_file
                    })
                    .show_ui(ui, |ui| {
                        for file in &files {
                            ui.selectable_value(&mut inputs.scope_file, file.clone(), file);
                        }
                    });
                if ui
                    .add_enabled(
                        !inputs.scope_file.is_empty(),
                        egui::Button::new("添加来源文件"),
                    )
                    .clicked()
                {
                    push_unique(source_files, std::mem::take(&mut inputs.scope_file));
                }
                string_value_row(ui, source_files);
            }
            CatalogQueryFilter::Missing { values, .. } => {
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut inputs.missing_property)
                            .hint_text("缺少属性键"),
                    );
                    if ui
                        .add_enabled(
                            !inputs.missing_property.trim().is_empty(),
                            egui::Button::new("添加缺少属性"),
                        )
                        .clicked()
                    {
                        let condition = MissingCondition::Property {
                            key: std::mem::take(&mut inputs.missing_property),
                        };
                        if !values.contains(&condition) {
                            values.push(condition);
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut inputs.missing_relation)
                            .hint_text("缺少关系类型（空白表示任意）"),
                    );
                    if ui.button("添加缺少关系").clicked() {
                        let condition = MissingCondition::Relation {
                            relation_type: nonempty(&inputs.missing_relation),
                        };
                        if !values.contains(&condition) {
                            values.push(condition);
                        }
                    }
                });
                missing_value_row(ui, values);
            }
        }
    });
    remove_filter
}

fn string_value_row(ui: &mut Ui, values: &mut Vec<String>) {
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (index, value) in values.iter().enumerate() {
            if ui.small_button(format!("× {value}")).clicked() {
                remove = Some(index);
            }
        }
    });
    if let Some(index) = remove {
        values.remove(index);
    }
}

fn relation_value_row(ui: &mut Ui, values: &mut Vec<RelationCondition>) {
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (index, condition) in values.iter().enumerate() {
            let label = format!(
                "× {} · {} · {}",
                condition.relation_type.as_deref().unwrap_or("任意类型"),
                direction_label(condition.direction),
                condition
                    .related
                    .as_ref()
                    .map_or("任意目标".into(), |target| format!(
                        "{}:{}",
                        target.kind, target.id
                    ))
            );
            if ui.small_button(label).clicked() {
                remove = Some(index);
            }
        }
    });
    if let Some(index) = remove {
        values.remove(index);
    }
}

fn missing_value_row(ui: &mut Ui, values: &mut Vec<MissingCondition>) {
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (index, condition) in values.iter().enumerate() {
            let label = match condition {
                MissingCondition::Property { key } => format!("× 缺少属性 {key}"),
                MissingCondition::Relation { relation_type } => format!(
                    "× 缺少关系 {}",
                    relation_type.as_deref().unwrap_or("任意类型")
                ),
            };
            if ui.small_button(label).clicked() {
                remove = Some(index);
            }
        }
    });
    if let Some(index) = remove {
        values.remove(index);
    }
}
pub(super) fn push_empty_filter(query: &mut CatalogQuery, dimension: &str) {
    if query
        .filters
        .iter()
        .any(|filter| filter_dimension(filter) == dimension)
    {
        return;
    }
    let filter = match dimension {
        "name" => CatalogQueryFilter::Name {
            values: Vec::new(),
            negate: false,
        },
        "tag" => CatalogQueryFilter::Tag {
            values: Vec::new(),
            recursive: false,
            negate: false,
        },
        "property" => CatalogQueryFilter::Property {
            values: Vec::new(),
            negate: false,
        },
        "relation" => CatalogQueryFilter::Relation {
            values: Vec::new(),
            negate: false,
        },
        "author_scope" => CatalogQueryFilter::AuthorScope {
            source_files: Vec::new(),
            negate: false,
        },
        "missing" => CatalogQueryFilter::Missing {
            values: Vec::new(),
            negate: false,
        },
        _ => return,
    };
    query.filters.push(filter);
}

pub(super) fn filter_dimension(filter: &CatalogQueryFilter) -> &'static str {
    match filter {
        CatalogQueryFilter::Kind { .. } => "kind",
        CatalogQueryFilter::Name { .. } => "name",
        CatalogQueryFilter::Tag { .. } => "tag",
        CatalogQueryFilter::Property { .. } => "property",
        CatalogQueryFilter::Relation { .. } => "relation",
        CatalogQueryFilter::AuthorScope { .. } => "author_scope",
        CatalogQueryFilter::Missing { .. } => "missing",
    }
}
pub(super) fn relative_path(root: &Path, file: &str) -> String {
    Path::new(file)
        .strip_prefix(root)
        .unwrap_or_else(|_| Path::new(file))
        .to_string_lossy()
        .replace('\\', "/")
}

fn source_files_in(project: &Project) -> Vec<String> {
    project
        .sources()
        .keys()
        .map(|path| relative_path(&project.root, &path.to_string_lossy()))
        .filter(|path| path.ends_with(".wl"))
        .collect()
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|old| old == &value) {
        values.push(value);
    }
}

fn nonempty(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
}

fn parse_target(value: &str) -> Option<TargetRef> {
    let (kind, id) = value.trim().split_once(':')?;
    (!kind.trim().is_empty() && !id.trim().is_empty())
        .then(|| TargetRef::new(kind.trim(), id.trim()))
}
fn direction_label(direction: RelationDirection) -> &'static str {
    match direction {
        RelationDirection::Outgoing => "出边",
        RelationDirection::Incoming => "入边",
        RelationDirection::Either => "任一方向",
    }
}
