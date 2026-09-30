//! 专题查询仅调用 core 投影；映射、范围和分页都保存在当前 egui 会话。
mod history;
mod relations;

use self::history::history_projection;
use self::relations::relation_projection;
use super::{catalog::kind_label, WorldeditApp};
use crate::theme::{self, *};
use egui::RichText;
use std::collections::BTreeMap;
use std::sync::Arc;
use worldline_core::catalog::{Catalog, TargetRef};
use worldline_core::{RelationQueryDirection, TopicProjectionOptions, TopicProjectionResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TopicViewKind {
    Network,
    Family,
    Organization,
    History,
}

#[derive(Clone, PartialEq)]
struct TopicFilterKey {
    revision: u64,
    target: TargetRef,
    role_mapping: BTreeMap<String, String>,
    depth: u8,
    direction: RelationQueryDirection,
    scope_refs: Vec<TargetRef>,
    include_unscoped: bool,
    include_period_children: bool,
}

#[derive(Clone, PartialEq)]
struct TopicQueryKey {
    filters: TopicFilterKey,
    relation_offset: usize,
    history_offset: usize,
}

#[derive(Clone)]
struct TopicViewState {
    mode: TopicViewKind,
    role_labels: BTreeMap<TopicViewKind, BTreeMap<String, String>>,
    scope_refs: Vec<TargetRef>,
    scope_input: String,
    scope_error: Option<String>,
    depth: u8,
    direction: RelationQueryDirection,
    include_unscoped: bool,
    include_period_children: bool,
    relation_offset: usize,
    history_offset: usize,
    filter_key: Option<TopicFilterKey>,
    query_key: Option<TopicQueryKey>,
    result: Option<Result<Arc<TopicProjectionResult>, String>>,
}

impl Default for TopicViewState {
    fn default() -> Self {
        Self {
            mode: TopicViewKind::Network,
            role_labels: BTreeMap::new(),
            scope_refs: Vec::new(),
            scope_input: String::new(),
            scope_error: None,
            depth: 1,
            direction: RelationQueryDirection::Both,
            include_unscoped: false,
            include_period_children: false,
            relation_offset: 0,
            history_offset: 0,
            filter_key: None,
            query_key: None,
            result: None,
        }
    }
}

enum Navigation {
    Read(TargetRef),
    Source(String, u32),
}

enum PageChange {
    Relations(usize),
    History(usize),
}

fn state_id(root: &std::path::Path, session: u64) -> egui::Id {
    egui::Id::new(("worldedit-topic-view-state", root, session))
}
fn mode_id(root: &std::path::Path, session: u64) -> egui::Id {
    egui::Id::new(("worldedit-topic-view-mode", root, session))
}

fn target_label(catalog: &Catalog, target: &TargetRef) -> String {
    let display = catalog
        .object(target)
        .map(|object| object.display.as_str())
        .unwrap_or(&target.id);
    format!("{} · {} · {}", kind_label(&target.kind), display, target.id)
}

fn target_input(value: &str) -> Option<TargetRef> {
    let (kind, id) = value.trim().split_once(':')?;
    let (kind, id) = (kind.trim(), id.trim());
    (!kind.is_empty() && !id.is_empty()).then(|| TargetRef::new(kind, id))
}

fn role_mapping(state: &TopicViewState) -> BTreeMap<String, String> {
    state
        .role_labels
        .get(&state.mode)
        .into_iter()
        .flat_map(|labels| labels.iter())
        .filter_map(|(id, label)| {
            let label = label.trim();
            (!label.is_empty()).then(|| (id.clone(), label.to_owned()))
        })
        .collect()
}

impl WorldeditApp {
    pub(super) fn topic_view_active(&self, ctx: &egui::Context) -> bool {
        ctx.data_mut(|data| {
            data.get_temp::<TopicViewKind>(mode_id(&self.project.root, self.topic_session))
                .is_some_and(|mode| mode != TopicViewKind::Network)
        })
    }

    /// Draws the topic selector and, when selected, its read-only projection.
    /// Returns false for the unchanged general network canvas.
    pub(super) fn topic_views(&mut self, ui: &mut egui::Ui) -> bool {
        let mut state = ui
            .ctx()
            .data_mut(|data| {
                data.get_temp::<TopicViewState>(state_id(&self.project.root, self.topic_session))
            })
            .unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("专题视图").strong());
            ui.selectable_value(&mut state.mode, TopicViewKind::Network, "通用网络");
            ui.selectable_value(&mut state.mode, TopicViewKind::Family, "家族关系");
            ui.selectable_value(&mut state.mode, TopicViewKind::Organization, "组织归属");
            ui.selectable_value(&mut state.mode, TopicViewKind::History, "人物 / 地点历史");
        });
        ui.ctx().data_mut(|data| {
            data.insert_temp(mode_id(&self.project.root, self.topic_session), state.mode)
        });
        if state.mode == TopicViewKind::Network {
            ui.ctx().data_mut(|data| {
                data.insert_temp(state_id(&self.project.root, self.topic_session), state)
            });
            return false;
        }

        let Some(snapshot) = self.snapshot.as_ref() else {
            ui.label(theme::muted("等待 core 分析快照。"));
            ui.ctx().data_mut(|data| {
                data.insert_temp(state_id(&self.project.root, self.topic_session), state)
            });
            return true;
        };
        let Some(target) = self.network_state.focus.clone() else {
            ui.label(theme::muted("先从资料页选择“查看关联”，再打开专题视图。"));
            ui.ctx().data_mut(|data| {
                data.insert_temp(state_id(&self.project.root, self.topic_session), state)
            });
            return true;
        };
        let analysis = &snapshot.result.analysis;
        let catalog = &analysis.catalog;
        for labels in state.role_labels.values_mut() {
            labels.retain(|id, _| catalog.relation_types.contains_key(id));
        }

        let mut navigation = None;
        let mut page_change = None;
        egui::ScrollArea::vertical()
            .id_salt((&self.project.root, self.topic_session, "topic-view-content"))
            .show(ui, |ui| {
                ui.separator();
                ui.label(RichText::new(topic_title(state.mode)).strong().size(18.0));
                ui.label(theme::muted(format!(
                    "中心对象：{}",
                    target_label(catalog, &target)
                )));
                ui.label(theme::muted(
            "只展示 core 返回的显式关系；角色需按稳定类型 ID 手动映射，不猜测亲属、成员或因果。",
        ));
                ui.horizontal_wrapped(|ui| {
                    ui.label("关系范围");
                    egui::ComboBox::from_id_salt("topic-view-depth")
                        .selected_text(format!("{} 层", state.depth))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut state.depth, 1, "1 层");
                            ui.selectable_value(&mut state.depth, 2, "2 层");
                        });
                    egui::ComboBox::from_id_salt("topic-view-direction")
                        .selected_text(direction_label(state.direction))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut state.direction,
                                RelationQueryDirection::Both,
                                "双向读取",
                            );
                            ui.selectable_value(
                                &mut state.direction,
                                RelationQueryDirection::Outgoing,
                                "只看出向",
                            );
                            ui.selectable_value(
                                &mut state.direction,
                                RelationQueryDirection::Incoming,
                                "只看入向",
                            );
                        });
                });
                scope_controls(ui, &mut state);

                ui.label(RichText::new("关系类型 → 专题角色（仅本次视图）").strong());
                ui.label(theme::muted(
            "空白标签不纳入查询。此映射不写入源码或共享布局；生亲 / 养亲等含义由作者明确填写。",
        ));
                for relation_type in catalog.relation_types.values() {
                    let id = &relation_type.id;
                    let role = state
                        .role_labels
                        .entry(state.mode)
                        .or_default()
                        .entry(id.clone())
                        .or_default();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "{} · {} · {}:{}",
                            relation_type.display, id, relation_type.file, relation_type.line
                        ));
                        ui.add(
                            egui::TextEdit::singleline(role)
                                .hint_text(format!("角色标签 · {id}"))
                                .desired_width(170.0),
                        );
                        if ui.small_button("定位类型来源").clicked() {
                            navigation = Some(Navigation::Source(
                                relation_type.file.clone(),
                                relation_type.line,
                            ));
                        }
                    });
                }

                let filter_key = TopicFilterKey {
                    revision: self.version,
                    target: target.clone(),
                    role_mapping: role_mapping(&state),
                    depth: state.depth,
                    direction: state.direction,
                    scope_refs: state.scope_refs.clone(),
                    include_unscoped: state.include_unscoped,
                    include_period_children: state.include_period_children,
                };
                if state.filter_key.as_ref() != Some(&filter_key) {
                    state.relation_offset = 0;
                    state.history_offset = 0;
                    state.filter_key = Some(filter_key.clone());
                    state.query_key = None;
                }
                let query_key = TopicQueryKey {
                    filters: filter_key,
                    relation_offset: state.relation_offset,
                    history_offset: state.history_offset,
                };
                if state.query_key.as_ref() != Some(&query_key) {
                    let options = TopicProjectionOptions {
                        role_mapping: query_key.filters.role_mapping.clone(),
                        offset: state.relation_offset,
                        history_offset: state.history_offset,
                        depth: state.depth,
                        direction: state.direction,
                        scope_refs: state.scope_refs.clone(),
                        include_unscoped: state.include_unscoped,
                        include_period_children: state.include_period_children,
                        ..TopicProjectionOptions::default()
                    };
                    state.result = Some(
                        analysis
                            .query_topic_projection(&target, options)
                            .map(Arc::new)
                            .map_err(|error| error.to_string()),
                    );
                    state.query_key = Some(query_key.clone());
                }

                match state.result.as_ref() {
                    Some(Ok(result)) => {
                        if !query_key.filters.role_mapping.is_empty() {
                            relation_projection(
                                ui,
                                catalog,
                                result,
                                state.mode,
                                &mut navigation,
                                &mut page_change,
                            );
                        } else {
                            ui.label(theme::muted("尚未为任何关系类型填写角色标签。"));
                        }
                        if state.mode == TopicViewKind::History {
                            history_projection(
                                ui,
                                catalog,
                                result,
                                &target,
                                &mut navigation,
                                &mut page_change,
                            );
                        }
                    }
                    Some(Err(error)) => {
                        ui.colored_label(ERROR(), format!("专题查询失败：{error}"));
                    }
                    None => {}
                }
                // Tab/Shift+Tab 能聚焦屏外链接；让专题区域跟随自己的新焦点滚动，
                // 否则窄屏列表虽然收到键盘输入，用户却看不见当前操作目标。
                if let Some(response) = ui
                    .ctx()
                    .memory(|memory| memory.focused())
                    .and_then(|id| ui.ctx().read_response(id))
                {
                    if response.gained_focus()
                        && response.layer_id == ui.layer_id()
                        && ui.min_rect().contains_rect(response.rect)
                    {
                        response.scroll_to_me(None);
                    }
                }
            });
        match page_change {
            Some(PageChange::Relations(offset)) => state.relation_offset = offset,
            Some(PageChange::History(offset)) => state.history_offset = offset,
            None => {}
        }
        ui.ctx().data_mut(|data| {
            data.insert_temp(state_id(&self.project.root, self.topic_session), state)
        });
        match navigation {
            Some(Navigation::Read(target)) => self.open_reading(target),
            Some(Navigation::Source(file, line)) => self.jump_to_file(&file, line, 1),
            None => {}
        }
        true
    }
}

fn topic_title(mode: TopicViewKind) -> &'static str {
    match mode {
        TopicViewKind::Network => "局部关系网络",
        TopicViewKind::Family => "家族关系",
        TopicViewKind::Organization => "组织归属",
        TopicViewKind::History => "人物 / 地点历史",
    }
}

fn direction_label(direction: RelationQueryDirection) -> &'static str {
    match direction {
        RelationQueryDirection::Both => "双向读取",
        RelationQueryDirection::Outgoing => "只看出向",
        RelationQueryDirection::Incoming => "只看入向",
    }
}

fn scope_controls(ui: &mut egui::Ui, state: &mut TopicViewState) {
    ui.horizontal_wrapped(|ui| {
        ui.label("范围");
        ui.add(
            egui::TextEdit::singleline(&mut state.scope_input)
                .hint_text("kind:id，例如 period:era")
                .desired_width(210.0),
        );
        if ui.button("加入范围").clicked() {
            match target_input(&state.scope_input) {
                Some(scope) => {
                    state.scope_refs.push(scope);
                    state.scope_refs.sort();
                    state.scope_refs.dedup();
                    state.scope_input.clear();
                    state.scope_error = None;
                }
                None => state.scope_error = Some("范围格式应为 kind:id".into()),
            }
        }
        if ui.small_button("清空范围").clicked() {
            state.scope_refs.clear();
            state.scope_error = None;
        }
    });
    for scope in &state.scope_refs {
        ui.label(theme::muted(format!("范围：{}:{}", scope.kind, scope.id)));
    }
    if !state.scope_refs.is_empty() {
        ui.checkbox(&mut state.include_unscoped, "同时包含未标范围的关系");
        if state.scope_refs.iter().any(|scope| scope.kind == "period") {
            ui.checkbox(&mut state.include_period_children, "包含子时段");
        }
    }
    if let Some(error) = &state.scope_error {
        ui.colored_label(ERROR(), error);
    }
}
