use super::*;
use worldline_core::catalog_scope::{ScopePlacementKind, ScopeRole};

impl WorkbenchState {
    pub(super) fn render_scope(&mut self, app: &mut WorldeditApp, ui: &mut egui::Ui) {
        let compact = ui.clip_rect().height() < 240.0;
        if !compact {
            ui.heading("查询范围巡检");
        }
        let Some(snapshot) = self.snapshot.clone() else {
            ui.label("尚无完整查询快照");
            return;
        };
        if self.current_snapshot(app).is_none() {
            ui.colored_label(
                crate::theme::GOLD(),
                "范围已过期；旧位置与关系不参与当前画布，请返回查询刷新。",
            );
            return;
        }
        let counts = snapshot.counts();
        ui.label(format!(
            "{} 个命中 · {} 处地图绑定 · {} 个无已知位置",
            counts.matching_objects, counts.matching_placements, counts.unplaced_objects
        ));
        let details = |ui: &mut egui::Ui| {
            ui.label(crate::theme::muted(&snapshot.query().summary));
            ui.label(crate::theme::muted(
                "仅突出当前可见标记；隐藏、锁定与作品内容保持原样。",
            ));
        };
        if compact {
            ui.collapsing("范围详情", details);
        } else {
            details(ui);
        }
        if snapshot.maps_incomplete() || snapshot.query().incomplete {
            ui.colored_label(
                crate::theme::GOLD(),
                "部分来源未完整解析；无已知位置不能视为确定未放置。",
            );
        }
        if counts.matching_objects == 0 {
            ui.strong("此查询没有命中；不代表作品没有资料。");
        }
        if let Some(scope) = &mut self.scope {
            let offset = scope.inspection_offset;
            let total = snapshot.query().total();
            let end = offset.saturating_add(20).min(total);
            let items = snapshot
                .query()
                .matches()
                .get(offset..end)
                .unwrap_or_default();
            ui.horizontal_wrapped(|ui| {
                if offset > 0 && ui.small_button("范围上一页").clicked() {
                    scope.inspection_offset = offset.saturating_sub(20);
                }
                if end < total && ui.small_button("范围下一页").clicked() {
                    scope.inspection_offset = offset + 20;
                }
                ui.label(format!(
                    "显示 {}–{} / {}",
                    if total == 0 { 0 } else { offset + 1 },
                    end,
                    total
                ));
            });
            let mut navigate = None;
            let mut source = None;
            let mut relation = None;
            let mut locate = None;
            let mut render_items = |ui: &mut egui::Ui| {
                for item in items {
                    ui.push_id((&item.target.kind, &item.target.id), |ui| {
                        ui.separator();
                        ui.label(egui::RichText::new(&item.display).strong());
                        ui.label(crate::theme::muted(format!(
                            "命中 · {}:{}",
                            item.target.kind, item.target.id
                        )));
                        ui.horizontal_wrapped(|ui| {
                            if ui.small_button("编辑真实资料").clicked() {
                                navigate = Some(item.target.clone());
                            }
                            if ui.small_button("打开来源").clicked() {
                                source = Some(item.source.clone());
                            }
                            if ui.small_button("正式关系").clicked() {
                                relation = Some(item.target.clone());
                            }
                        });
                        let total = snapshot.placement_count_for(&item.target);
                        let offset = scope
                            .placement_offsets
                            .entry(item.target.clone())
                            .or_default();
                        if total == 0 {
                            ui.label(crate::theme::muted("无已知地图位置"));
                        }
                        if total > 20 {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(format!(
                                    "地图位置 {}–{} / {total}",
                                    *offset + 1,
                                    (*offset + 20).min(total)
                                ));
                                if *offset > 0 && ui.small_button("位置上一批").clicked() {
                                    *offset = offset.saturating_sub(20);
                                }
                                if *offset + 20 < total && ui.small_button("位置下一批").clicked()
                                {
                                    *offset += 20;
                                }
                            });
                        }
                        for placement in snapshot.placements_page(&item.target, *offset, 20) {
                            let kind = if placement.kind == ScopePlacementKind::SceneNode {
                                "图元"
                            } else {
                                "标记"
                            };
                            if ui
                                .add(
                                    egui::Button::new(format!(
                                        "定位 {} / {}",
                                        placement.map_title, placement.placement_id
                                    ))
                                    .wrap(),
                                )
                                .clicked()
                            {
                                locate = Some((
                                    placement.map_id.clone(),
                                    placement.placement_id.clone(),
                                ));
                            }
                            ui.label(crate::theme::muted(format!(
                                "{kind} · 图层 {}{}{}{}",
                                placement.layer_id,
                                if placement.visible {
                                    ""
                                } else {
                                    " · 默认隐藏"
                                },
                                if placement.locked { " · 锁定" } else { "" },
                                if placement.read_only {
                                    " · 只读文档"
                                } else {
                                    ""
                                }
                            )));
                        }
                    });
                }
            };
            // 窄矮材料视图已有外层滚动区，不能再分割出一个只露上下沿的嵌套视口。
            if compact {
                render_items(ui);
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("query-scope-items")
                    .max_height(320.0)
                    .show(ui, render_items);
            }
            if (navigate.is_some() || source.is_some() || relation.is_some() || locate.is_some())
                && self.scope_navigation_ready(app)
            {
                if let Some(scope) = self.scope.as_mut() {
                    scope.last_view = Some(app.author_location(Some(ui.ctx())));
                }
                if let Some(target) = navigate {
                    if let Some(object) = app
                        .snapshot
                        .as_ref()
                        .and_then(|s| s.result.analysis.catalog.object(&target))
                        .cloned()
                    {
                        app.navigate_object(&object);
                    }
                }
                if let Some(source) = source {
                    app.jump_to_file(&source.file, source.line, 1);
                }
                if let Some(target) = relation {
                    self.scope_network(app, target);
                }
                if let Some((map_id, placement_id)) = locate {
                    app.locate_reference_from(
                        &map_id,
                        &placement_id,
                        app.author_location(Some(ui.ctx())),
                    );
                }
            }
        }
        if counts.unresolved_placements > 0 {
            ui.collapsing(
                format!("未解析地图绑定 · {} 处", counts.unresolved_placements),
                |ui| {
                    let offset = &mut self.scope.as_mut().expect("active scope").unresolved_offset;
                    ui.horizontal_wrapped(|ui| {
                        if *offset > 0 && ui.small_button("未解析上一批").clicked() {
                            *offset = offset.saturating_sub(100);
                        }
                        if *offset + 100 < counts.unresolved_placements
                            && ui.small_button("未解析下一批").clicked()
                        {
                            *offset += 100;
                        }
                        ui.label(format!(
                            "{}–{} / {}",
                            *offset + 1,
                            (*offset + 100).min(counts.unresolved_placements),
                            counts.unresolved_placements
                        ));
                    });
                    for placement in snapshot
                        .placements()
                        .iter()
                        .filter(|p| p.role == ScopeRole::Unresolved)
                        .skip(*offset)
                        .take(100)
                    {
                        ui.label(format!(
                            "{} / {} → {}:{}（未解析，不是查询命中）",
                            placement.map_title,
                            placement.placement_id,
                            placement.target.kind,
                            placement.target.id
                        ));
                    }
                },
            );
        }
        if !snapshot.diagnostics().is_empty() {
            ui.collapsing(
                format!("地图来源诊断 · {} 项", snapshot.diagnostics().len()),
                |ui| {
                    for diagnostic in snapshot.diagnostics() {
                        ui.label(format!("{} · {}", diagnostic.code, diagnostic.message));
                        if ui
                            .small_button(format!(
                                "打开 {}:{}",
                                diagnostic.file, diagnostic.span.line
                            ))
                            .clicked()
                        {
                            app.jump_to_file(
                                &diagnostic.file,
                                diagnostic.span.line,
                                diagnostic.span.column,
                            );
                        }
                    }
                },
            );
        }
    }
}
