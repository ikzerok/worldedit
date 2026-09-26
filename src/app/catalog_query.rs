//! 目录组合查询和待办视图；筛选、分页、解释与待办语义仅由 worldline-core 提供。

use super::WorldeditApp;
use egui::{RichText, Ui};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use worldline_core::catalog::{TargetRef, TARGET_KINDS};
use worldline_core::project::Project;
use worldline_core::queries::{
    CatalogQuery, CatalogQueryFilter, CatalogQueryMatch, CatalogQueryOptions, CatalogQueryPage,
    MissingCondition, PropertyCondition, PropertyScalar, QueryError, RelationCondition,
    RelationDirection, SavedQueryDraft, TodoItem, TodoKind, TodoProjection,
    DEFAULT_CATALOG_QUERY_CANDIDATES, MAX_CATALOG_QUERY_CANDIDATES, MAX_CATALOG_QUERY_PAGE_SIZE,
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum View {
    #[default]
    Query,
    Todos,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum ScalarInputKind {
    #[default]
    String,
    Number,
    Boolean,
}

#[derive(Default)]
struct FilterInputs {
    name_value: String,
    tag_value: String,
    property_key: String,
    property_value: String,
    property_kind: ScalarInputKind,
    property_bool: bool,
    relation_type: String,
    relation_related: String,
    relation_direction: RelationDirection,
    scope_file: String,
    missing_property: String,
    missing_relation: String,
}

pub(super) struct WorkbenchState {
    pub(super) open: bool,
    view: View,
    query: CatalogQuery,
    page_size: usize,
    max_candidates: usize,
    page: Option<CatalogQueryPage>,
    saved_query_id: String,
    saved_query_name: String,
    error: Option<String>,
    inputs: FilterInputs,
    local_favorites: BTreeMap<PathBuf, BTreeSet<String>>,
    todo_cache: Option<TodoProjection>,
    #[cfg(not(target_arch = "wasm32"))]
    running: Option<RunningQuery>,
}

impl Default for WorkbenchState {
    fn default() -> Self {
        Self {
            open: false,
            view: View::Query,
            query: CatalogQuery::default(),
            page_size: 50,
            max_candidates: DEFAULT_CATALOG_QUERY_CANDIDATES,
            page: None,
            saved_query_id: String::new(),
            saved_query_name: String::new(),
            error: None,
            inputs: FilterInputs::default(),
            local_favorites: BTreeMap::new(),
            todo_cache: None,
            #[cfg(not(target_arch = "wasm32"))]
            running: None,
        }
    }
}

fn current_options(state: &WorkbenchState) -> CatalogQueryOptions {
    CatalogQueryOptions {
        offset: 0,
        page_size: state.page_size.clamp(1, MAX_CATALOG_QUERY_PAGE_SIZE),
        max_candidates: state.max_candidates.clamp(1, MAX_CATALOG_QUERY_CANDIDATES),
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct RunningQuery {
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    receiver: std::sync::mpsc::Receiver<(String, Result<CatalogQueryPage, String>)>,
    query: CatalogQuery,
    options: CatalogQueryOptions,
    cancel_requested: bool,
}

impl WorkbenchState {
    pub(super) fn reset_for_workspace(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(running) = &self.running {
            running
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let favorites = std::mem::take(&mut self.local_favorites);
        *self = Self::default();
        self.local_favorites = favorites;
    }

    fn render(&mut self, app: &mut WorldeditApp, ctx: &egui::Context) {
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
                self.query = CatalogQuery::default();
            }
        });
        if let Some(error) = &self.error {
            ui.colored_label(crate::theme::ERROR, error);
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
                    let label = format!("{} · {kind}", super::catalog::kind_label(kind));
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

    fn render_results(&self, app: &WorldeditApp, ui: &mut Ui, action: &mut Action) {
        if let Some(page) = &self.page {
            let stale = page.snapshot != app.project.content_baseline();
            if stale {
                ui.colored_label(
                    crate::theme::GOLD,
                    "结果已过期：Project 缓冲发生变化。请重新运行查询。",
                );
                if ui.button("从第一页重新查询").clicked() {
                    *action = Action::Run;
                }
                return;
            }
            ui.separator();
            if page.total == 0 {
                ui.label(RichText::new("没有找到匹配资料").strong());
            } else {
                let first = page.offset + 1;
                let last = page.offset + page.items.len();
                ui.label(
                    RichText::new(format!("{} 个命中 · 显示 {first}–{last}", page.total)).strong(),
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
            egui::ScrollArea::vertical()
                .id_salt("catalog-query-results")
                .max_height(480.0)
                .show(ui, |ui| {
                    for item in &page.items {
                        render_match(ui, app, item, action);
                    }
                });
            ui.horizontal(|ui| {
                if page.offset > 0 && ui.button("上一页").clicked() {
                    let offset = page.offset.saturating_sub(self.page_size);
                    *action = Action::Previous(offset);
                }
                if page.next.is_some() && ui.button("下一页").clicked() {
                    *action = Action::Next;
                }
            });
        } else if self.error.is_none() {
            ui.label(RichText::new("运行查询后显示结果、来源与 core 命中原因。").weak());
        }
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
                    ui.colored_label(crate::theme::GOLD, &diagnostic.message);
                }
                for (id, document) in &saved.queries {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{} · {}", document.draft.name, id));
                        if document.read_only {
                            ui.label(RichText::new("只读").color(crate::theme::GOLD));
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
            ui.label("当前会话的个人收藏，不写入工程清单或共享查询文档。");
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
            ui.colored_label(crate::theme::GOLD, "待办来源已变化，刷新后重新读取。");
            return;
        }
        if !projection.diagnostics.is_empty() {
            egui::CollapsingHeader::new(format!("索引诊断 · {} 项", projection.diagnostics.len()))
                .default_open(true)
                .show(ui, |ui| {
                    for diagnostic in &projection.diagnostics {
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

    fn run_query(&mut self, app: &WorldeditApp, _ctx: &egui::Context) {
        self.page = None;
        self.error = None;
        let options = CatalogQueryOptions {
            offset: 0,
            page_size: self.page_size.clamp(1, MAX_CATALOG_QUERY_PAGE_SIZE),
            max_candidates: self.max_candidates.clamp(1, MAX_CATALOG_QUERY_CANDIDATES),
        };
        let query = self.query.clone();
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.running.is_some() {
                return;
            }
            let project = app.project.clone();
            let snapshot = project.content_baseline();
            let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let cancel_thread = cancel.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            let query_thread = query.clone();
            std::thread::spawn(move || {
                let result = project
                    .query_catalog_cancellable(&query_thread, options, || {
                        cancel_thread.load(std::sync::atomic::Ordering::Relaxed)
                    })
                    .map_err(|error| error.to_string());
                let _ = sender.send((snapshot, result));
            });
            self.running = Some(RunningQuery {
                cancel,
                receiver,
                query,
                options,
                cancel_requested: false,
            });
            _ctx.request_repaint();
        }
        #[cfg(target_arch = "wasm32")]
        {
            let snapshot = app.project.content_baseline();
            match app
                .project
                .query_catalog_cancellable(&query, options, || false)
            {
                Ok(page) => {
                    self.page = Some(page);
                    self.error = None;
                }
                Err(error) => {
                    self.error = Some(error.to_string());
                }
            }
            if snapshot != app.project.content_baseline() {
                self.page = None;
                self.error = Some("Project 在查询期间发生变化；请重新运行查询。".into());
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_query(&mut self, app: &WorldeditApp, ctx: &egui::Context) {
        use std::sync::mpsc::TryRecvError;
        let result = match self.running.as_ref() {
            Some(running) => match running.receiver.try_recv() {
                Ok(result) => Some(Some(result)),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(None),
            },
            None => return,
        };
        match result {
            None => ctx.request_repaint_after(std::time::Duration::from_millis(16)),
            Some(result) => {
                let submitted_query = self
                    .running
                    .as_ref()
                    .map(|running| running.query.clone())
                    .unwrap_or_default();
                let submitted_options = self.running.as_ref().map(|running| running.options);
                let cancel_requested = self
                    .running
                    .as_ref()
                    .is_some_and(|running| running.cancel_requested);
                self.running = None;
                match result {
                    Some((snapshot, Ok(page))) => {
                        if cancel_requested {
                            self.page = None;
                            self.error = Some("查询已取消，未返回部分结果。".into());
                        } else if submitted_query == self.query
                            && submitted_options == Some(current_options(self))
                            && snapshot == app.project.content_baseline()
                        {
                            self.page = Some(page);
                            self.error = None;
                        } else {
                            self.page = None;
                            self.error = Some(
                                "筛选条件或 Project 缓冲已变化；丢弃旧结果，请从第一页重查。"
                                    .into(),
                            );
                        }
                    }
                    Some((_, Err(error))) => {
                        self.page = None;
                        self.error = Some(if cancel_requested || error.contains("查询已取消") {
                            "查询已取消，未返回部分结果。".into()
                        } else if submitted_query != self.query
                            || submitted_options != Some(current_options(self))
                        {
                            "筛选条件或分页参数已变化；旧查询已取消，没有返回部分结果。".into()
                        } else {
                            error
                        });
                    }
                    None => {
                        self.page = None;
                        self.error = Some("查询任务意外结束；请重新运行。".into());
                    }
                }
            }
        }
    }

    fn next_page(&mut self, app: &WorldeditApp) {
        let Some(cursor) = self.page.as_ref().and_then(|page| page.next.as_ref()) else {
            return;
        };
        match app.project.continue_catalog_query(&self.query, cursor) {
            Ok(page) => {
                self.page = Some(page);
                self.error = None;
            }
            Err(QueryError::StaleCursor) => {
                self.page = None;
                self.error = Some("分页游标已过期；请从第一页重新查询。".into());
            }
            Err(error) => {
                self.page = None;
                self.error = Some(error.to_string());
            }
        }
    }

    fn previous_page(&mut self, app: &WorldeditApp, offset: usize) {
        let options = CatalogQueryOptions {
            offset,
            page_size: self.page_size.clamp(1, MAX_CATALOG_QUERY_PAGE_SIZE),
            max_candidates: self.max_candidates.clamp(1, MAX_CATALOG_QUERY_CANDIDATES),
        };
        match app.project.query_catalog(&self.query, options) {
            Ok(page) => {
                self.page = Some(page);
                self.error = None;
            }
            Err(error) => {
                self.page = None;
                self.error = Some(error.to_string());
            }
        }
    }

    fn save_query(&mut self, app: &mut WorldeditApp) {
        let draft = SavedQueryDraft {
            id: self.saved_query_id.clone(),
            name: self.saved_query_name.clone(),
            query: self.query.clone(),
        };
        let baseline = app.project.content_baseline();
        if !app.commit("共享查询定义已保存", move |project| {
            project.save_saved_query(draft, &baseline).map(|_| ())
        }) {
            self.error = app.io_error.clone();
        } else {
            self.error = None;
            self.page = None;
            self.todo_cache = None;
        }
    }

    fn toggle_favorite(&mut self, root: &Path, id: String) {
        let favorites = self.local_favorites.entry(root.to_path_buf()).or_default();
        if !favorites.remove(&id) {
            favorites.insert(id);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn cancel_query(&mut self) {
        if let Some(running) = &mut self.running {
            running
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
            running.cancel_requested = true;
        }
    }
}

impl WorldeditApp {
    pub(super) fn catalog_query_tab(&mut self, ctx: &egui::Context) {
        let mut workbench = std::mem::take(&mut self.catalog_workbench);
        workbench.render(self, ctx);
        self.catalog_workbench = workbench;
    }
}

enum Action {
    None,
    Run,
    Next,
    Previous(usize),
    Save,
    Load(SavedQueryDraft),
    Favorite(String),
    #[cfg(not(target_arch = "wasm32"))]
    Cancel,
    Navigate(TargetRef),
    Jump(String, u32, u32),
}

fn render_filter_row(
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
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut inputs.property_key).hint_text("属性键"),
                    );
                    egui::ComboBox::from_id_salt("query-property-scalar")
                        .selected_text(scalar_kind_label(inputs.property_kind))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut inputs.property_kind,
                                ScalarInputKind::String,
                                "文字",
                            );
                            ui.selectable_value(
                                &mut inputs.property_kind,
                                ScalarInputKind::Number,
                                "数字",
                            );
                            ui.selectable_value(
                                &mut inputs.property_kind,
                                ScalarInputKind::Boolean,
                                "布尔值",
                            );
                        });
                    match inputs.property_kind {
                        ScalarInputKind::String | ScalarInputKind::Number => {
                            ui.add(
                                egui::TextEdit::singleline(&mut inputs.property_value)
                                    .hint_text("精确值"),
                            );
                        }
                        ScalarInputKind::Boolean => {
                            ui.checkbox(&mut inputs.property_bool, "值为真");
                        }
                    }
                    if ui
                        .add_enabled(
                            !inputs.property_key.trim().is_empty()
                                && (inputs.property_kind == ScalarInputKind::Boolean
                                    || !inputs.property_value.is_empty()),
                            egui::Button::new("添加属性值"),
                        )
                        .clicked()
                    {
                        let value = match inputs.property_kind {
                            ScalarInputKind::String => {
                                Some(PropertyScalar::String(inputs.property_value.clone()))
                            }
                            ScalarInputKind::Number => inputs
                                .property_value
                                .parse::<f64>()
                                .ok()
                                .filter(|value| value.is_finite())
                                .map(PropertyScalar::Number),
                            ScalarInputKind::Boolean => {
                                Some(PropertyScalar::Boolean(inputs.property_bool))
                            }
                        };
                        if let Some(equals) = value {
                            let condition = PropertyCondition {
                                key: std::mem::take(&mut inputs.property_key),
                                equals,
                            };
                            if !values.contains(&condition) {
                                values.push(condition);
                            }
                            inputs.property_value.clear();
                        }
                    }
                });
                property_value_row(ui, values);
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

fn property_value_row(ui: &mut Ui, values: &mut Vec<PropertyCondition>) {
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (index, condition) in values.iter().enumerate() {
            let value = match condition.equals {
                PropertyScalar::String(ref value) => value.clone(),
                PropertyScalar::Number(value) => value.to_string(),
                PropertyScalar::Boolean(value) => if value { "真" } else { "假" }.into(),
            };
            if ui
                .small_button(format!("× {} = {value}", condition.key))
                .clicked()
            {
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

fn render_match(ui: &mut Ui, app: &WorldeditApp, item: &CatalogQueryMatch, action: &mut Action) {
    let object = app
        .snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.result.analysis.catalog.object(&item.target));
    let label = object.map_or_else(
        || format!("{} · {}", item.target.kind, item.target.id),
        |object| {
            format!(
                "{} · {} · {}",
                super::catalog::kind_label(&item.target.kind),
                object.display,
                item.target.id
            )
        },
    );
    ui.group(|ui| {
        ui.horizontal_wrapped(|ui| {
            if ui.button(label).clicked() {
                *action = Action::Navigate(item.target.clone());
            }
            if ui.button("定位来源").clicked() {
                *action = Action::Jump(item.source.file.clone(), item.source.line, 1);
            }
            ui.label(format!(
                "{}:{}",
                relative_path(&app.project.root, &item.source.file),
                item.source.line
            ));
        });
        for reason in &item.reasons {
            ui.label(RichText::new(reason).small().weak());
        }
    });
}

fn render_todo(ui: &mut Ui, app: &WorldeditApp, item: &TodoItem, action: &mut Action) {
    ui.group(|ui| {
        ui.label(RichText::new(&item.reason).strong());
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(format!("定位{}来源", todo_kind_label(item.kind)))
                .clicked()
            {
                *action = Action::Jump(
                    item.source.file.clone(),
                    item.source.line,
                    item.column.unwrap_or(1),
                );
            }
            if let Some(target) = &item.related_target {
                if ui.button("查看引用对象").clicked() {
                    *action = Action::Navigate(target.clone());
                }
            }
            ui.label(format!(
                "{}:{}{}",
                relative_path(&app.project.root, &item.source.file),
                item.source.line,
                item.column
                    .map_or(String::new(), |column| format!(" · 列 {column}"))
            ));
        });
    });
}

fn push_empty_filter(query: &mut CatalogQuery, dimension: &str) {
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

fn filter_dimension(filter: &CatalogQueryFilter) -> &'static str {
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

fn relative_path(root: &Path, file: &str) -> String {
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

fn scalar_kind_label(kind: ScalarInputKind) -> &'static str {
    match kind {
        ScalarInputKind::String => "文字",
        ScalarInputKind::Number => "数字",
        ScalarInputKind::Boolean => "布尔值",
    }
}

fn direction_label(direction: RelationDirection) -> &'static str {
    match direction {
        RelationDirection::Outgoing => "出边",
        RelationDirection::Incoming => "入边",
        RelationDirection::Either => "任一方向",
    }
}

fn todo_kind_label(kind: TodoKind) -> &'static str {
    match kind {
        TodoKind::BrokenLink => "断链",
        TodoKind::EntryToCreate => "待建资料",
        TodoKind::DetachedComment => "失锚批注",
        TodoKind::OpenProposal => "待审提案",
    }
}
