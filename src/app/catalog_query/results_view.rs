use super::filters::relative_path;
use super::*;
use egui::{RichText, Ui};
use worldline_core::queries::CatalogQueryMatch;

impl WorkbenchState {
    pub(super) fn render_results(&self, app: &WorldeditApp, ui: &mut Ui, action: &mut Action) {
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.strong("查询结果");
            ui.menu_button(format!("排序：{}", sort_label(self.query.sort)), |ui| {
                for sort in sort_options() {
                    if ui.selectable_label(self.query.sort == sort, sort_label(sort)).clicked() {
                        *action = Action::Sort(sort);
                        ui.close();
                    }
                }
            }).response.on_hover_text("只调整浏览顺序；显式保存共享定义后才持久化。名称按 ASCII 不区分大小写的 Unicode 顺序，非拼音、非数字自然排序；缺值始终置后。");
        });
        let Some(page) = &self.page else {
            if self.error.is_none() {
                ui.label(RichText::new("运行查询后显示结果、来源与 core 命中原因。").weak());
            }
            return;
        };
        if page.snapshot != app.project.content_baseline() {
            ui.colored_label(
                crate::theme::GOLD,
                "结果已过期：Project 缓冲发生变化。请重新运行查询。",
            );
            if ui.button("从第一页重新查询").clicked() {
                *action = Action::Run;
            }
            return;
        }
        if page.total == 0 {
            ui.strong("没有找到匹配资料");
        } else {
            ui.label(
                RichText::new(format!(
                    "{} 个命中 · 显示 {}–{}",
                    page.total,
                    page.offset + 1,
                    page.offset + page.items.len()
                ))
                .strong(),
            );
        }
        if !page.diagnostics.is_empty() {
            egui::CollapsingHeader::new(format!("索引诊断 · {} 项", page.diagnostics.len()))
                .default_open(true)
                .show(ui, |ui| {
                    for diagnostic in &page.diagnostics {
                        ui.colored_label(
                            if diagnostic.severity == worldline_core::Severity::Error {
                                crate::theme::ERROR
                            } else {
                                crate::theme::GOLD
                            },
                            format!(
                                "{}:{} · {}",
                                diagnostic.file, diagnostic.span.line, diagnostic.message
                            ),
                        );
                    }
                });
        }
        let compact = ui.available_width() < 700.0;
        if !compact {
            table_header(ui, self.query.sort, action);
            ui.separator();
        }
        result_scroll_area().show(ui, |ui| {
            for (index, item) in page.items.iter().enumerate() {
                // Widget identity follows the complete object, never its position after sorting.
                ui.push_id((&item.target.kind, &item.target.id), |ui| {
                    let fill = if index % 2 == 0 {
                        ui.visuals().faint_bg_color
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    egui::Frame::NONE
                        .fill(fill)
                        .inner_margin(egui::Margin::symmetric(6, 5))
                        .show(ui, |ui| {
                            render_row(ui, app, item, compact, action);
                        });
                });
            }
        });
        ui.horizontal_wrapped(|ui| {
            if page.offset > 0 && ui.button("上一页").clicked() {
                *action = Action::Previous(page.offset.saturating_sub(self.page_size));
            }
            if page.next.is_some() && ui.button("下一页").clicked() {
                *action = Action::Next;
            }
        });
    }
}

fn sort_options() -> [Option<CatalogQuerySort>; 5] {
    [
        None,
        Some(CatalogQuerySort {
            field: CatalogSortField::Name,
            direction: CatalogSortDirection::Ascending,
        }),
        Some(CatalogQuerySort {
            field: CatalogSortField::Name,
            direction: CatalogSortDirection::Descending,
        }),
        Some(CatalogQuerySort {
            field: CatalogSortField::Kind,
            direction: CatalogSortDirection::Ascending,
        }),
        Some(CatalogQuerySort {
            field: CatalogSortField::Kind,
            direction: CatalogSortDirection::Descending,
        }),
    ]
}

fn sort_label(sort: Option<CatalogQuerySort>) -> &'static str {
    match sort {
        None => "默认顺序",
        Some(CatalogQuerySort {
            field: CatalogSortField::Name,
            direction: CatalogSortDirection::Ascending,
        }) => "名称升序",
        Some(CatalogQuerySort {
            field: CatalogSortField::Name,
            direction: CatalogSortDirection::Descending,
        }) => "名称降序",
        Some(CatalogQuerySort {
            field: CatalogSortField::Kind,
            direction: CatalogSortDirection::Ascending,
        }) => "对象类型升序",
        Some(CatalogQuerySort {
            field: CatalogSortField::Kind,
            direction: CatalogSortDirection::Descending,
        }) => "对象类型降序",
    }
}

fn column_widths(ui: &Ui) -> [f32; 3] {
    let width = (ui.available_width() - ui.spacing().item_spacing.x * 2.0).max(0.0);
    [width * 0.44, width * 0.2, width * 0.36]
}

fn cell(ui: &mut Ui, width: f32, contents: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, 0.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(width);
            contents(ui);
        },
    );
}

fn table_header(ui: &mut Ui, sort: Option<CatalogQuerySort>, action: &mut Action) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(6, 0))
        .show(ui, |ui| {
            let widths = column_widths(ui);
            ui.horizontal_top(|ui| {
                for (field, label, width) in [
                    (CatalogSortField::Name, "名称", widths[0]),
                    (CatalogSortField::Kind, "对象类型", widths[1]),
                ] {
                    cell(ui, width, |ui| {
                        let active = sort.filter(|sort| sort.field == field);
                        let suffix = active.map_or("", |sort| {
                            if sort.direction == CatalogSortDirection::Ascending {
                                " ↑"
                            } else {
                                " ↓"
                            }
                        });
                        if ui
                            .button(RichText::new(format!("{label}{suffix}")).strong())
                            .on_hover_text(if field == CatalogSortField::Name {
                                "按名称排序；Unicode 顺序，非拼音、非数字自然排序"
                            } else {
                                "按稳定对象类型标识排序；不是实体自定义分类"
                            })
                            .clicked()
                        {
                            let direction = if active.is_some_and(|sort| {
                                sort.direction == CatalogSortDirection::Ascending
                            }) {
                                CatalogSortDirection::Descending
                            } else {
                                CatalogSortDirection::Ascending
                            };
                            *action = Action::Sort(Some(CatalogQuerySort { field, direction }));
                        }
                    });
                }
                cell(ui, widths[2], |ui| {
                    ui.strong("来源");
                });
            });
        });
}

fn render_row(
    ui: &mut Ui,
    app: &WorldeditApp,
    item: &CatalogQueryMatch,
    compact: bool,
    action: &mut Action,
) {
    ui.spacing_mut().item_spacing.y = 4.0;
    ui.spacing_mut().interact_size.y = 22.0;
    ui.spacing_mut().button_padding = egui::vec2(6.0, 2.0);
    let name = if item.display.trim().is_empty() {
        "未命名"
    } else {
        &item.display
    };
    let kind = super::super::catalog::kind_label(&item.target.kind);
    if compact {
        if ui
            .add(
                egui::Button::new(RichText::new(name).strong())
                    .frame(false)
                    .wrap(),
            )
            .clicked()
        {
            *action = Action::Navigate(item.target.clone());
        }
        ui.label(
            RichText::new(format!("{kind} · {}", item.target.id))
                .small()
                .weak(),
        );
        source_cell(ui, app, item, action);
        reasons_cell(ui, item);
    } else {
        let widths = column_widths(ui);
        ui.horizontal_top(|ui| {
            cell(ui, widths[0], |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new(name).strong())
                            .frame(false)
                            .wrap(),
                    )
                    .clicked()
                {
                    *action = Action::Navigate(item.target.clone());
                }
                ui.add(egui::Label::new(RichText::new(&item.target.id).small().weak()).truncate())
                    .on_hover_text(&item.target.id);
            });
            cell(ui, widths[1], |ui| {
                ui.label(kind).on_hover_text(&item.target.kind);
            });
            cell(ui, widths[2], |ui| {
                source_cell(ui, app, item, action);
                reasons_cell(ui, item);
            });
        });
    }
}

fn reasons_cell(ui: &mut Ui, item: &CatalogQueryMatch) {
    egui::CollapsingHeader::new(RichText::new("命中原因").small().weak())
        .id_salt("reasons")
        .show(ui, |ui| {
            for reason in &item.reasons {
                ui.label(RichText::new(reason).small().weak());
            }
        });
}

fn source_cell(ui: &mut Ui, app: &WorldeditApp, item: &CatalogQueryMatch, action: &mut Action) {
    let source = format!(
        "{}:{}",
        relative_path(&app.project.root, &item.source.file),
        item.source.line
    );
    if ui
        .add(
            egui::Button::new(RichText::new(&source).color(crate::theme::ACCENT))
                .frame(false)
                .wrap(),
        )
        .on_hover_text("定位来源")
        .clicked()
    {
        *action = Action::Jump(item.source.file.clone(), item.source.line, 1);
    }
}

// The surrounding page scrolls too. Reserve a usable result viewport instead of
// allowing nested scrolling to collapse it to the default 64 px in a short window.
fn result_scroll_area() -> egui::ScrollArea {
    egui::ScrollArea::vertical()
        .id_salt("catalog-query-results")
        .min_scrolled_height(240.0)
        .max_height(480.0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn catalog_results_keep_a_usable_viewport_when_parent_has_little_height() {
        let ctx = egui::Context::default();
        let mut height = 0.0;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 160.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let output = super::result_scroll_area().show(ui, |ui| {
                        ui.allocate_space(egui::vec2(300.0, 1200.0));
                    });
                    height = output.inner_rect.height();
                    assert!(output.content_size.y > height);
                });
            },
        );
        assert!(
            height >= 240.0,
            "nested result viewport collapsed to {height}"
        );
    }
}
