use super::*;
use crate::theme;
use worldline_core::catalog_import::{
    CatalogBlankPolicy as Blank, CatalogImportField as Field, CatalogImportType as Type,
};

impl ImportState {
    pub(super) fn mapping_ui(&mut self, app: &WorldeditApp, ui: &mut egui::Ui) {
        ui.heading("1 · 明确每列的用途");
        ui.label("kind + id 唯一匹配；不会按显示名合并。只新增或修订人物与实体，不删除资料。");
        ui.label("UTF-8 逗号 CSV：最多 2 MiB、500 行、64 列。number 为有限十进制数，bool 只认 true / false；ref 单元格仅填写 ID。");
        if self.table.is_none() {
            ui.add_space(16.0);
            ui.label(
                "尚无已读取的 CSV 表格。选择文件后，在此逐列映射；文件选择取消不影响现有输入。",
            );
            return;
        }
        let headers = self.table.as_ref().unwrap().headers.clone();
        let count = self.table.as_ref().unwrap().rows.len();
        ui.label(format!(
            "已读取 {count} 行 · {} 列；每列均须选择字段或“明确忽略”。",
            headers.len()
        ));
        ui.separator();
        ui.heading("新对象写入位置");
        ui.label(
            "请选择已有活动源码文件；更新对象仍留在原声明文件，不移动。未选择时整批不可应用。",
        );
        let old_destination = self.destination.clone();
        let mut destination_selected = false;
        let destination = egui::ComboBox::from_id_salt("catalog-import-destination")
            .width(ui.available_width().min(440.0))
            .selected_text(if self.destination.as_os_str().is_empty() {
                "选择新对象的源码文件…".into()
            } else {
                self.destination.display().to_string()
            })
            .show_ui(ui, |ui| {
                for path in app.project.sources().keys() {
                    let relative = path
                        .strip_prefix(&app.project.root)
                        .unwrap_or(path)
                        .to_path_buf();
                    let label = relative.display().to_string();
                    let Ok(portable) = portable_destination(&relative) else {
                        ui.add_enabled(
                            false,
                            egui::Button::new(format!("{label}（路径不可传输）")).wrap(),
                        );
                        continue;
                    };
                    let option = ui.selectable_value(&mut self.destination, portable, label);
                    scroll_popup_focus(&option);
                    if option.clicked() {
                        destination_selected = true;
                        ui.close();
                    }
                }
            });
        if destination_selected {
            destination.response.request_focus();
        }
        scroll_new_focus(&destination.response);
        if self.destination != old_destination {
            self.invalidate();
        }
        ui.separator();
        let before = self.columns.clone();
        for (column, header) in headers.iter().enumerate() {
            ui.push_id(("catalog-column", column), |ui| {
                egui::Frame::new()
                    .fill(theme::canvas_background())
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("第 {} 列 · {header}", column + 1))
                                    .strong(),
                            )
                            .wrap(),
                        );
                        let mapping = &mut self.columns[column];
                        let previous = field_choice(mapping.as_ref().map(|m| &m.field));
                        let mut choice = previous;
                        let mut field_selected = false;
                        let field = egui::ComboBox::from_id_salt("field")
                            .selected_text(field_label(choice))
                            .width(ui.available_width().min(270.0))
                            .height(380.0)
                            .show_ui(ui, |ui| {
                                for value in 0..=12 {
                                    let option =
                                        ui.selectable_value(&mut choice, value, field_label(value));
                                    scroll_popup_focus(&option);
                                    if option.clicked() {
                                        field_selected = true;
                                        ui.close();
                                    }
                                }
                            });
                        if field_selected {
                            field.response.request_focus();
                        }
                        scroll_new_focus(&field.response);
                        if choice != previous {
                            let key = mapping
                                .as_ref()
                                .and_then(|m| match &m.field {
                                    Field::Property { key, .. } => Some(key.clone()),
                                    _ => None,
                                })
                                .unwrap_or_default();
                            *mapping =
                                choice_field(choice, key).map(|field| CatalogColumnMapping {
                                    column,
                                    field,
                                    blank: Blank::Error,
                                });
                        }
                        if let Some(mapping) = mapping {
                            if let Field::Property { key, .. } = &mut mapping.field {
                                let input = ui.add(
                                    egui::TextEdit::singleline(key)
                                        .hint_text("属性键（明确填写，例如 age）")
                                        .desired_width(ui.available_width()),
                                );
                                scroll_new_focus(&input);
                            }
                            if mapping.field != Field::Ignore {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label("空单元格");
                                    let mut blank_selected = false;
                                    let blank = egui::ComboBox::from_id_salt("blank")
                                        .selected_text(blank_label(mapping.blank))
                                        .show_ui(ui, |ui| {
                                            for blank in
                                                [Blank::Error, Blank::Keep, Blank::EmptyText]
                                            {
                                                let option = ui.selectable_value(
                                                    &mut mapping.blank,
                                                    blank,
                                                    blank_label(blank),
                                                );
                                                scroll_popup_focus(&option);
                                                if option.clicked() {
                                                    blank_selected = true;
                                                    ui.close();
                                                }
                                            }
                                        });
                                    if blank_selected {
                                        blank.response.request_focus();
                                    }
                                    scroll_new_focus(&blank.response);
                                });
                                ui.label(theme::muted(
                                    "保留 = 跳过空值；空文本只适用于文本字段。空格不是空单元格。",
                                ));
                            } else {
                                ui.label("此列不写入 .wl 资料字段；预览会明确列出忽略列。");
                            }
                        }
                        if let Some(sample) = self
                            .table
                            .as_ref()
                            .and_then(|t| t.rows.first())
                            .and_then(|r| r.cells.get(column))
                        {
                            let sample =
                                ui.collapsing("查看第一个数据行的原始单元格", |ui| {
                                    ui.add(egui::Label::new(format!("{sample:?}")).wrap());
                                });
                            scroll_new_focus(&sample.header_response);
                        }
                    });
            });
            ui.add_space(8.0);
        }
        if self.columns != before {
            self.invalidate();
        }
    }
}

fn field_label(index: usize) -> &'static str {
    [
        "尚未映射",
        "对象种类 · kind",
        "稳定 ID · id",
        "显示名 · display",
        "实体类型 · entity_type",
        "实体描述 · description",
        "属性 · 文本 text",
        "属性 · 数值 number",
        "属性 · 布尔 bool",
        "属性 · 实体引用 ref entity",
        "属性 · 人物引用 ref character",
        "属性 · 关系引用 ref relation",
        "明确忽略此列",
    ][index]
}
fn field_choice(field: Option<&Field>) -> usize {
    match field {
        None => 0,
        Some(Field::Kind) => 1,
        Some(Field::Id) => 2,
        Some(Field::Display) => 3,
        Some(Field::EntityType) => 4,
        Some(Field::Description) => 5,
        Some(Field::Property { value_type, .. }) => match value_type {
            Type::Text => 6,
            Type::Number => 7,
            Type::Bool => 8,
            Type::Ref { target_kind } => match target_kind.as_str() {
                "character" => 10,
                "relation" => 11,
                _ => 9,
            },
        },
        Some(Field::Ignore) => 12,
    }
}
fn choice_field(choice: usize, key: String) -> Option<Field> {
    Some(match choice {
        1 => Field::Kind,
        2 => Field::Id,
        3 => Field::Display,
        4 => Field::EntityType,
        5 => Field::Description,
        12 => Field::Ignore,
        6..=11 => Field::Property {
            key,
            value_type: match choice {
                6 => Type::Text,
                7 => Type::Number,
                8 => Type::Bool,
                _ => Type::Ref {
                    target_kind: match choice {
                        10 => "character",
                        11 => "relation",
                        _ => "entity",
                    }
                    .into(),
                },
            },
        },
        _ => return None,
    })
}
fn blank_label(blank: Blank) -> &'static str {
    match blank {
        Blank::Error => "报错（默认）",
        Blank::Keep => "保留现值 / 跳过",
        Blank::EmptyText => "设置为空文本",
    }
}

// Only a new keyboard focus moves the mapping viewport. A held focus never
// snaps the author's manual scroll, and an open menu retains its anchor.
fn scroll_new_focus(response: &egui::Response) {
    if response.gained_focus() && !egui::Popup::is_any_open(&response.ctx) {
        response.scroll_to_me(Some(egui::Align::Center));
    }
}
fn scroll_popup_focus(response: &egui::Response) {
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
}

// A typed native path must cross the portable core DTO boundary losslessly.
// Never replace a literal Unix backslash with a separator to another file.
pub(super) fn portable_destination(path: &std::path::Path) -> Result<PathBuf, String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(part) = component else {
            return Err("导入目标必须是相对来源路径".into());
        };
        let part = part.to_str().ok_or("导入目标路径必须为 UTF-8")?;
        if part.contains('\\') {
            return Err("导入目标文件名不能含字面反斜杠".into());
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return Err("尚未选择导入目标".into());
    }
    Ok(PathBuf::from(parts.join("/")))
}
