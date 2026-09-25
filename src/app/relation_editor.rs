//! 显式关系和关系类型编辑；选择器只读取 core 目录。
use super::authoring_forms::{RelationForm, RelationTypeForm};
use super::catalog::kind_label;
use super::inspector::{field, properties};
use super::WorldeditApp;
use crate::theme;
use worldline_core::catalog::{Catalog, TargetRef, TARGET_KINDS};
use worldline_core::RelationDirection;

pub(super) fn object_label(catalog: &Catalog, target: &TargetRef) -> String {
    match catalog.object(target) {
        Some(object) => format!(
            "{} · {}:{}",
            object.display,
            kind_label(&target.kind),
            target.id
        ),
        None if target.id.is_empty() => "请选择对象".into(),
        None => format!("缺失 · {}:{}", target.kind, target.id),
    }
}
pub(super) fn object_picker(
    ui: &mut egui::Ui,
    catalog: &Catalog,
    label: &str,
    target: &mut TargetRef,
    query: &mut String,
    constraint: Option<&str>,
) -> bool {
    let mut changed = false;
    ui.push_id(label, |ui| {
        ui.label(label);
        ui.menu_button(object_label(catalog, target), |ui| {
            ui.set_min_width(320.0);
            ui.add(egui::TextEdit::singleline(query).hint_text("搜索名称、别名或 ID"));
            let matches: Vec<_> = catalog
                .search_objects(query)
                .into_iter()
                .filter(|object| constraint.is_none_or(|kind| kind == object.target.kind))
                .collect();
            ui.label(theme::muted(format!(
                "{} 个匹配对象；同名对象以类型与 ID 区分",
                matches.len()
            )));
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for object in matches.iter().take(100) {
                        if ui.button(object_label(catalog, &object.target)).clicked() {
                            *target = object.target.clone();
                            changed = true;
                            ui.close();
                        }
                    }
                });
            if matches.len() > 100 {
                ui.label("仅显示前 100 项，请继续输入以缩小范围。");
            }
        });
        if constraint.is_some_and(|kind| !target.id.is_empty() && kind != target.kind) {
            ui.colored_label(
                theme::ERROR,
                "当前端点不符合关系类型约束，请明确选择新的对象。",
            );
        }
    });
    changed
}
fn kind_constraint(ui: &mut egui::Ui, label: &str, kind: &mut Option<String>) {
    egui::ComboBox::from_id_salt(label)
        .selected_text(format!(
            "{label}：{}",
            kind.as_deref().map(kind_label).unwrap_or("不限制")
        ))
        .show_ui(ui, |ui| {
            ui.selectable_value(kind, None, "不限制");
            for value in TARGET_KINDS {
                ui.selectable_value(kind, Some((*value).into()), kind_label(value));
            }
        });
}
fn optional_text(ui: &mut egui::Ui, label: &str, value: &mut Option<String>, multiline: bool) {
    ui.label(label);
    let mut text = value.clone().unwrap_or_default();
    let changed = if multiline {
        ui.add(
            egui::TextEdit::multiline(&mut text)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        )
        .changed()
    } else {
        ui.text_edit_singleline(&mut text).changed()
    };
    if changed {
        *value = (!text.is_empty()).then_some(text);
    }
}

impl WorldeditApp {
    pub(super) fn edit_relation(&mut self, id: Option<&str>, from: Option<TargetRef>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        match RelationForm::open(
            &self.project,
            &snapshot.result.analysis.catalog,
            self.version,
            id,
            from,
        ) {
            Ok(form) => self.relation_editor = Some(form),
            Err(error) => self.io_error = Some(error),
        }
    }
    pub(super) fn edit_relation_type(&mut self, id: Option<&str>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        match RelationTypeForm::open(
            &self.project,
            &snapshot.result.analysis.catalog,
            self.version,
            id,
        ) {
            Ok(form) => self.relation_type_editor = Some(form),
            Err(error) => self.io_error = Some(error),
        }
    }
    pub(super) fn relation_editor_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.relation_editor.take() else {
            return;
        };
        let Some(snapshot) = &self.snapshot else {
            self.relation_editor = Some(form);
            return;
        };
        let catalog = snapshot.result.analysis.catalog.clone();
        let mut open = true;
        let mut applied = false;
        egui::Window::new(if form.original.is_some() {
            "编辑独立关系"
        } else {
            "新建独立关系"
        })
        .id(egui::Id::new("relation-editor"))
        .open(&mut open)
        .default_width(650.0)
        .default_height(760.0)
        .resizable(true)
        .vscroll(true)
        .show(ctx, |ui| {
            egui::ComboBox::from_id_salt("relation-type")
                .selected_text(
                    catalog
                        .relation_types
                        .get(&form.draft.relation_type)
                        .map(|kind| format!("{} · {}", kind.display, kind.id))
                        .unwrap_or_else(|| "请选择关系类型".into()),
                )
                .show_ui(ui, |ui| {
                    for kind in catalog.relation_types.values() {
                        ui.selectable_value(
                            &mut form.draft.relation_type,
                            kind.id.clone(),
                            format!("{} · {}", kind.display, kind.id),
                        );
                    }
                });
            if catalog.relation_types.is_empty() {
                ui.label(theme::muted(
                    "尚无关系类型。请先关闭此表单，在资料与状态页创建关系类型，再选择两端对象。",
                ));
            }
            let kind = catalog.relation_types.get(&form.draft.relation_type);
            if let Some(kind) = kind {
                ui.label(match kind.direction {
                    RelationDirection::Directed => "有向关系：起点 → 终点",
                    RelationDirection::Undirected => "无向关系：起点 — 终点",
                });
                if let Some(inverse) = &kind.inverse_display {
                    ui.label(theme::muted(format!(
                        "反向读取：{inverse}；不生成第二条关系"
                    )));
                }
            }
            object_picker(
                ui,
                &catalog,
                "起点",
                &mut form.draft.from,
                &mut form.from_search,
                kind.and_then(|kind| kind.from_kind.as_deref()),
            );
            object_picker(
                ui,
                &catalog,
                "终点",
                &mut form.draft.to,
                &mut form.to_search,
                kind.and_then(|kind| kind.to_kind.as_deref()),
            );
            ui.label("关系说明");
            ui.add(
                egui::TextEdit::multiline(&mut form.draft.description)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY),
            );
            optional_text(ui, "来源记录（可选）", &mut form.draft.source_note, true);
            egui::CollapsingHeader::new("作者明确指定的范围").show(ui, |ui| {
                let mut remove = None;
                for (i, scope) in form.draft.scope_refs.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(object_label(&catalog, scope));
                        if ui.small_button("移除此范围").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    form.draft.scope_refs.remove(i);
                }
                let query_id = ui.id().with("scope-search");
                let mut query = ui
                    .data_mut(|data| data.get_temp::<String>(query_id))
                    .unwrap_or_default();
                let mut selected = TargetRef::default();
                if object_picker(ui, &catalog, "添加范围", &mut selected, &mut query, None)
                    && !form.draft.scope_refs.contains(&selected)
                {
                    form.draft.scope_refs.push(selected);
                }
                ui.data_mut(|data| data.insert_temp(query_id, query));
                ui.label(theme::muted(
                    "范围是作者资料筛选，不表示关系会自动产生、终止或演化。",
                ));
            });
            egui::CollapsingHeader::new("自定义属性")
                .show(ui, |ui| properties(ui, &mut form.draft.properties));
            ui.label(theme::muted(
                "关系 ID · 同一对端点可有多条关系，不能按名称合并",
            ));
            ui.add_enabled(
                form.original.is_none(),
                egui::TextEdit::singleline(&mut form.draft.id),
            );
            let current = form.guard.is_current(&self.project, self.version);
            if !current {
                ui.colored_label(
                    theme::GOLD,
                    "工程已变化。请保留输入，关闭并重新打开表单后合并。",
                );
            }
            let ready = current
                && self.project.language_version() == "1.10"
                && kind.is_some()
                && catalog.object(&form.draft.from).is_some()
                && catalog.object(&form.draft.to).is_some();
            if ui
                .add_enabled(ready, theme::primary("应用独立关系"))
                .clicked()
            {
                let before = self.project.clone();
                let result = form.apply(&mut self.project, self.version);
                applied = self.finish_content_command(before, result, "独立关系已更新");
                if applied {
                    self.catalog_filter = "relation".into();
                    self.catalog_query.clear();
                    self.catalog_target = Some(TargetRef::new("relation", &form.draft.id));
                    self.open_reading(TargetRef::new("relation", &form.draft.id));
                }
            }
            if let Some(id) = &form.original {
                if ui.button("删除这条关系…").clicked() {
                    self.plan_content_deletion(TargetRef::new("relation", id));
                }
                ui.label(theme::muted(
                    "只想不显示这条边时，请使用网络中的“仅隐藏显示”，不要删除关系。",
                ));
            }
        });
        if open && !applied {
            self.relation_editor = Some(form);
        }
    }
    pub(super) fn relation_type_editor_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.relation_type_editor.take() else {
            return;
        };
        let mut open = true;
        let mut applied = false;
        egui::Window::new("关系类型资料").id(egui::Id::new("relation-type-editor")).open(&mut open)
            .default_width(530.0).resizable(true).vscroll(true).show(ctx, |ui| {
                field(ui, "显示名称", &mut form.draft.display);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut form.draft.direction, RelationDirection::Directed, "有向 →");
                    ui.selectable_value(&mut form.draft.direction, RelationDirection::Undirected, "无向 —");
                });
                optional_text(ui, "反向读取名称（可选，不生成反向副本）", &mut form.draft.inverse_display, false);
                kind_constraint(ui, "起点类型", &mut form.draft.from_kind);
                kind_constraint(ui, "终点类型", &mut form.draft.to_kind);
                ui.label(theme::muted("稳定 ID"));
                ui.add_enabled(form.original.is_none(), egui::TextEdit::singleline(&mut form.draft.id));
                let current = form.guard.is_current(&self.project, self.version);
                if !current { ui.colored_label(theme::GOLD, "工程已变化，请保留输入并重新打开后合并。"); }
                if self.project.language_version() != "1.10" { ui.colored_label(theme::GOLD, "关系类型需要显式启用语言 1.10 与 content.relations.v1，不会自动升级旧项目。"); }
                if ui.add_enabled(current && self.project.language_version() == "1.10" && !form.draft.display.trim().is_empty(), theme::primary("应用关系类型")).clicked() {
                    let before = self.project.clone();
                    let result = form.apply(&mut self.project, self.version);
                    applied = self.finish_content_command(before, result, "关系类型已更新；现在可以新建关系");
                }
            });
        if open && !applied {
            self.relation_type_editor = Some(form);
        }
    }
}
