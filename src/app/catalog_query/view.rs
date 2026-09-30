use super::filters::{filter_dimension, push_empty_filter, render_filter_row};
use super::results::{render_todo, todo_kind_label};
use super::*;
use egui::{RichText, Ui};
use worldline_core::catalog::TARGET_KINDS;
use worldline_core::project::Project;
use worldline_core::queries::{CatalogQueryFilter, TodoKind};
impl WorkbenchState {
    pub(super) fn render(&mut self, app: &mut WorldeditApp, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        self.poll_query(app, ctx);
        if self.view == View::Todos
            && self
                .todo_cache
                .as_ref()
                .is_none_or(|todo| todo.snapshot != app.project.content_baseline())
        {
            self.todo_cache = Some(app.project.todo_projection());
        }

        let mut action = Action::None;
        let previous_query = self.query.clone();
        let previous_options = current_options(self);
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("catalog-query-workbench")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("资料库组合查询与待办");
                        if ui.button("返回资料索引").clicked() {
                            self.open = false;
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut self.view, View::Query, "资料筛选");
                        ui.selectable_value(&mut self.view, View::Todos, "统一待办");
                        ui.label(
                            egui::RichText::new("只读浏览；内容处理须另行编辑")
                                .small()
                                .weak(),
                        );
                    });
                    ui.separator();
                    match self.view {
                        View::Query => self.query_view(app, ui, &mut action),
                        View::Todos => self.todo_view(app, ui, &mut action),
                    }
                });
        });

        if self.query != previous_query || current_options(self) != previous_options {
            self.page = None;
            self.error = None;
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(running) = &mut self.running {
                running
                    .cancel
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                running.cancel_requested = true;
            }
        }
        match action {
            Action::None => {}
            Action::Run => self.run_query(app, ctx),
            Action::Sort(sort) => self.apply_sort(app, ctx, sort),
            Action::Next => self.next_page(app),
            Action::Previous(offset) => self.previous_page(app, offset),
            Action::Save => self.save_query(app),
            Action::Load(draft) => {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(running) = &mut self.running {
                    running
                        .cancel
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    running.cancel_requested = true;
                }
                self.query = draft.query;
                self.saved_query_id = draft.id;
                self.saved_query_name = draft.name;
                self.page = None;
                self.error = None;
            }
            Action::Favorite(id) => self.toggle_favorite(&app.project.root, id),
            #[cfg(not(target_arch = "wasm32"))]
            Action::Cancel => self.cancel_query(),
            Action::Navigate(target) => {
                if let Some(object) = app
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.result.analysis.catalog.object(&target))
                    .cloned()
                {
                    app.navigate_object(&object);
                } else {
                    app.open_reading(target);
                }
            }
            Action::Jump(file, line, column) => app.jump_to_file(&file, line, column),
        }
    }

    fn query_view(&mut self, app: &WorldeditApp, ui: &mut Ui, action: &mut Action) {
        ui.heading("筛选条件");
        self.render_conditions(&app.project, ui);
        ui.label(
            RichText::new(format!("核心摘要：{}", self.query.summary()))
                .small()
                .weak(),
        );
        ui.horizontal_wrapped(|ui| {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(running) = &self.running {
                if ui.button("取消查询").clicked() {
                    *action = Action::Cancel;
                }
                if running.cancel_requested {
                    ui.label("正在取消；等核心检查点返回，不会显示部分结果。");
                } else {
                    ui.spinner();
                    ui.label("正在查询当前缓冲…");
                }
            } else if ui.button("运行查询").clicked() {
                *action = Action::Run;
            }
            #[cfg(target_arch = "wasm32")]
            {
                if ui.button("运行查询").clicked() {
                    *action = Action::Run;
                }
                ui.label("浏览器查询同步执行；查询结果与诊断仍来自 core。");
            }
            ui.label("每页");
            ui.add(
                egui::DragValue::new(&mut self.page_size)
                    .range(1..=MAX_CATALOG_QUERY_PAGE_SIZE)
                    .speed(1),
            );
            ui.label("候选上限");
            ui.add(
                egui::DragValue::new(&mut self.max_candidates)
                    .range(1..=MAX_CATALOG_QUERY_CANDIDATES)
                    .speed(100),
            );
            if ui.button("清空条件").clicked() {
                self.query.filters.clear();
            }
        });
        if let Some(error) = &self.error {
            ui.colored_label(crate::theme::ERROR(), error);
        }
        self.render_results(app, ui, action);
        self.render_saved_queries(app, ui, action);
    }

    fn render_conditions(&mut self, project: &Project, ui: &mut Ui) {
        self.add_condition_menu(ui);
        ui.add_space(4.0);
        if self.query.filters.is_empty() {
            ui.label("未添加条件时，核心查询返回全部资料。");
        }
        let mut remove_filters = Vec::new();
        let query = &mut self.query;
        let inputs = &mut self.inputs;
        egui::ScrollArea::vertical()
            .id_salt("catalog-query-filters")
            .max_height(370.0)
            .show(ui, |ui| {
                for (index, filter) in query.filters.iter_mut().enumerate() {
                    if render_filter_row(ui, filter, inputs, project) {
                        remove_filters.push(index);
                    }
                }
            });
        for index in remove_filters.into_iter().rev() {
            self.query.filters.remove(index);
        }
    }

    fn add_condition_menu(&mut self, ui: &mut Ui) {
        ui.menu_button("＋ 添加条件", |ui| {
            ui.menu_button("对象类型", |ui| {
                for kind in TARGET_KINDS {
                    let label = format!("{} · {kind}", super::super::catalog::kind_label(kind));
                    if ui.button(label).clicked() {
                        match self
                            .query
                            .filters
                            .iter_mut()
                            .find_map(|filter| match filter {
                                CatalogQueryFilter::Kind { values, .. } => Some(values),
                                _ => None,
                            }) {
                            Some(values) if !values.iter().any(|old| old == kind) => {
                                values.push((*kind).to_string());
                            }
                            Some(_) => {}
                            None => self.query.filters.push(CatalogQueryFilter::Kind {
                                values: vec![(*kind).to_string()],
                                negate: false,
                            }),
                        }
                        ui.close();
                    }
                }
            });
            for (label, dimension) in [
                ("名称 / ID / 别名", "name"),
                ("标签", "tag"),
                ("属性值", "property"),
                ("明确关系", "relation"),
                ("来源文件", "author_scope"),
                ("缺少资料", "missing"),
            ] {
                let exists = self
                    .query
                    .filters
                    .iter()
                    .any(|filter| filter_dimension(filter) == dimension);
                if ui.add_enabled(!exists, egui::Button::new(label)).clicked() {
                    push_empty_filter(&mut self.query, dimension);
                    ui.close();
                }
            }
        });
    }

    fn render_saved_queries(&mut self, app: &WorldeditApp, ui: &mut Ui, action: &mut Action) {
        ui.separator();
        ui.collapsing("保存为共享查询", |ui| {
            ui.label("共享定义进入当前 Project 展示文档；本地收藏与待办状态不写入作品。");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.saved_query_id)
                        .hint_text("稳定 ID")
                        .desired_width(160.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.saved_query_name)
                        .hint_text("查询名称")
                        .desired_width(220.0),
                );
                if ui.button("保存共享定义").clicked() {
                    *action = Action::Save;
                }
            });
        });

        let saved = app.project.saved_query_index();
        ui.collapsing(
            format!("共享查询定义 · {} 项", saved.queries.len()),
            |ui| {
                for diagnostic in &saved.diagnostics {
                    ui.colored_label(crate::theme::GOLD(), &diagnostic.message);
                }
                for (id, document) in &saved.queries {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{} · {}", document.draft.name, id));
                        if document.read_only {
                            ui.label(RichText::new("只读").color(crate::theme::GOLD()));
                        }
                        if ui.button("载入").clicked() {
                            *action = Action::Load(document.draft.clone());
                        }
                        let favorite = self
                            .local_favorites
                            .entry(app.project.root.clone())
                            .or_default()
                            .contains(id);
                        if ui
                            .button(if favorite {
                                "★ 本地收藏"
                            } else {
                                "☆ 收藏到本机"
                            })
                            .clicked()
                        {
                            *action = Action::Favorite(id.clone());
                        }
                    });
                    ui.label(RichText::new(document.draft.query.summary()).small().weak());
                }
                if saved.queries.is_empty() && saved.diagnostics.is_empty() {
                    ui.label("尚无共享查询定义。");
                }
            },
        );
        let favorites = self
            .local_favorites
            .entry(app.project.root.clone())
            .or_default();
        ui.collapsing(format!("本地收藏 · {} 项", favorites.len()), |ui| {
            ui.label("保存在当前用户或浏览器存储中的个人收藏，不写入工程清单或共享查询文档。");
            for id in favorites.iter() {
                if let Some(document) = saved.queries.get(id) {
                    if ui
                        .button(format!("{} · {id}", document.draft.name))
                        .clicked()
                    {
                        *action = Action::Load(document.draft.clone());
                    }
                } else {
                    ui.label(format!("{id} · 对应共享定义已不可用"));
                }
            }
        });
    }

    fn todo_view(&self, app: &WorldeditApp, ui: &mut Ui, action: &mut Action) {
        ui.heading("当前缓冲中的待办");
        ui.label("读取 core 投影；定位、筛选和查看不修改内容。修复必须显式编辑对应来源。");
        let Some(projection) = &self.todo_cache else {
            ui.label("正在读取待办…");
            return;
        };
        if projection.snapshot != app.project.content_baseline() {
            ui.colored_label(crate::theme::GOLD(), "待办来源已变化，刷新后重新读取。");
            return;
        }
        if !projection.diagnostics.is_empty() {
            egui::CollapsingHeader::new(format!("索引诊断 · {} 项", projection.diagnostics.len()))
                .default_open(true)
                .show(ui, |ui| {
                    for diagnostic in &projection.diagnostics {
                        ui.colored_label(
                            if diagnostic.severity == worldline_core::Severity::Error {
                                crate::theme::ERROR()
                            } else {
                                crate::theme::GOLD()
                            },
                            format!(
                                "{}:{} · {}",
                                diagnostic.file, diagnostic.span.line, diagnostic.message
                            ),
                        );
                    }
                });
        }
        if projection.items.is_empty() {
            ui.label(RichText::new("没有待处理事项。").strong());
            return;
        }
        ui.label(format!("{} 项 · 按类别和来源分组", projection.items.len()));
        egui::ScrollArea::vertical()
            .id_salt("catalog-todos")
            .show(ui, |ui| {
                for kind in [
                    TodoKind::BrokenLink,
                    TodoKind::EntryToCreate,
                    TodoKind::DetachedComment,
                    TodoKind::OpenProposal,
                ] {
                    let items: Vec<_> = projection
                        .items
                        .iter()
                        .filter(|item| item.kind == kind)
                        .collect();
                    if items.is_empty() {
                        continue;
                    }
                    egui::CollapsingHeader::new(format!(
                        "{} · {} 项",
                        todo_kind_label(kind),
                        items.len()
                    ))
                    .default_open(true)
                    .show(ui, |ui| {
                        for item in items {
                            render_todo(ui, app, item, action);
                        }
                    });
                }
            });
    }
}
