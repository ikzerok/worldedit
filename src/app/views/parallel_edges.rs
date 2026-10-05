//! 同投影端点只聚合显示；每个 core 边保留独立索引及来源。
use crate::app::{Tab, WorldeditApp};
use crate::theme;
use egui::{Id, Pos2, Rect, Vec2};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use worldline_core::{EdgeKind, GraphEdge, RelationGraph};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EdgeGroup {
    pub from: usize,
    pub to: usize,
    pub edges: Vec<usize>,
}

pub(super) fn projected_groups(graph: &RelationGraph, timeline: bool) -> Vec<EdgeGroup> {
    let root = |id: usize| {
        let node = graph.nodes.get(id)?;
        if timeline && !node.is_event {
            graph
                .ids
                .get(node.name.split('.').next().unwrap_or(""))
                .map(|id| *id as usize)
        } else {
            Some(id)
        }
    };
    let mut groups = BTreeMap::<(usize, usize), Vec<usize>>::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        let (Some(from), Some(to)) = (root(edge.from as usize), root(edge.to as usize)) else {
            continue;
        };
        if timeline && (edge.kind == EdgeKind::Enter || from == to) {
            continue;
        }
        groups.entry((from, to)).or_default().push(index);
    }
    groups
        .into_iter()
        .map(|((from, to), edges)| EdgeGroup { from, to, edges })
        .collect()
}

fn kind_name(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Choice => "选择",
        EdgeKind::Divert => "直达",
        EdgeKind::Drift => "漂流",
        EdgeKind::Enter => "进入",
    }
}

pub(super) fn group_label(group: &EdgeGroup, graph: &RelationGraph) -> String {
    let kinds = [
        EdgeKind::Choice,
        EdgeKind::Divert,
        EdgeKind::Drift,
        EdgeKind::Enter,
    ]
    .into_iter()
    .filter_map(|kind| {
        let count = group
            .edges
            .iter()
            .filter(|index| graph.edges[**index].kind == kind)
            .count();
        (count > 0).then(|| format!("{} {count}", kind_name(kind)))
    })
    .collect::<Vec<_>>()
    .join(" · ");
    if group.edges.len() == 1 {
        let edge = &graph.edges[group.edges[0]];
        edge.label
            .as_ref()
            .map(|label| {
                format!(
                    "{} · {}",
                    kind_name(edge.kind),
                    crate::visual::truncated(label, 12)
                )
            })
            .unwrap_or(kinds)
    } else {
        format!("{} 条连接 · {kinds}", group.edges.len())
    }
}

fn state_id() -> Id {
    Id::new("graph-original-edge-details")
}
fn group_id(timeline: bool, graph: &RelationGraph, group: &EdgeGroup) -> Id {
    Id::new((
        "graph-edge-group",
        timeline,
        &graph.nodes[group.from].name,
        &graph.nodes[group.to].name,
    ))
}

#[derive(Clone)]
struct Details {
    version: u64,
    workspace: PathBuf,
    timeline: bool,
    group: EdgeGroup,
    edges: Vec<GraphEdge>,
    heading: String,
    trigger: Id,
    return_focus: Option<Id>,
    first_focus: bool,
}

/// 包括过期但仍在屏幕上的详情；不能让底层 Tooltip 层盖过前景审阅。
/// 其他视图或工程中保留的返回状态不算当前可见窗口。
pub(super) fn details_visible(app: &WorldeditApp, ctx: &egui::Context) -> bool {
    matches!(app.tab, Tab::Timeline | Tab::Graph)
        && ctx.data(|data| {
            data.get_temp::<Details>(state_id()).is_some_and(|details| {
                details.workspace == app.project.root
                    && details.timeline == (app.tab == Tab::Timeline)
            })
        })
}

pub(super) fn draw_groups(
    app: &mut WorldeditApp,
    ui: &mut egui::Ui,
    graph: &RelationGraph,
    rects: &HashMap<usize, Rect>,
    canvas: Rect,
    zoom: f32,
    timeline: bool,
) {
    let painter = ui.painter().clone();
    for group in projected_groups(graph, timeline) {
        let (Some(from), Some(to)) = (rects.get(&group.from), rects.get(&group.to)) else {
            continue;
        };
        let first = &graph.edges[group.edges[0]];
        let mixed = group
            .edges
            .iter()
            .any(|index| graph.edges[*index].kind != first.kind);
        let color = if mixed {
            theme::TEXT()
        } else {
            match first.kind {
                EdgeKind::Drift => theme::BLUE(),
                EdgeKind::Choice => theme::ACCENT(),
                EdgeKind::Divert => theme::GOLD(),
                EdgeKind::Enter => theme::MUTED(),
            }
        };
        let start = from.right_center();
        let end = to.left_center();
        let bend = (end.x - start.x)
            .abs()
            .mul_add(0.45, 40.0 * zoom)
            .min(180.0 * zoom);
        let c0 = start + Vec2::new(bend, 0.0);
        let c1 = end - Vec2::new(bend, 0.0);
        painter.add(egui::Shape::line(
            crate::visual::bezier_points(start, c0, c1, end, 30),
            egui::Stroke::new(
                if group.edges.len() > 1 {
                    2.3_f32
                } else {
                    1.5_f32
                },
                color.gamma_multiply(0.65),
            ),
        ));
        crate::visual::draw_arrow(&painter, c1, end, color);
        let middle = crate::visual::bezier_points(start, c0, c1, end, 2)[1];
        // 放在卡片上沿之外；可操作尺寸不随缩放缩小。
        let center = Pos2::new(
            middle.x,
            (middle.y - from.height().min(to.height()) * 0.5 - 15.0).max(canvas.top() + 14.0),
        );
        painter.line_segment(
            [middle, center + Vec2::new(0.0, 12.0)],
            egui::Stroke::new(1.0_f32, color.gamma_multiply(0.35)),
        );
        let id = group_id(timeline, graph, &group);
        let label = group_label(&group, graph);
        let width = (label.chars().count() as f32 * 9.0 + 18.0).clamp(76.0, 360.0);
        let rect = Rect::from_center_size(center, Vec2::new(width, 26.0));
        let response = ui
            .push_id(id, |ui| {
                ui.put(
                    rect,
                    egui::Button::new(egui::RichText::new(label).size(12.0).color(color))
                        .truncate(),
                )
            })
            .inner;
        if response.has_focus() {
            ui.scroll_to_rect(rect.expand(8.0), None);
        }
        if response.clicked() {
            ui.ctx().data_mut(|data| {
                data.insert_temp(
                    state_id(),
                    Details {
                        version: app.version,
                        workspace: app.project.root.clone(),
                        timeline,
                        edges: group
                            .edges
                            .iter()
                            .map(|index| graph.edges[*index].clone())
                            .collect(),
                        heading: format!(
                            "{} → {}",
                            graph.nodes[group.from].name, graph.nodes[group.to].name
                        ),
                        group,
                        trigger: response.id,
                        return_focus: None,
                        first_focus: true,
                    },
                )
            });
        }
    }
}

fn detail_context(ui: &mut egui::Ui, edge: &GraphEdge) {
    for (index, context) in edge.contexts.iter().enumerate() {
        if edge.contexts.len() > 1 {
            ui.label(theme::muted(format!("可能分支 {}", index + 1)));
        }
        for choice in &context.choices {
            ui.add(egui::Label::new(format!("选择上下文：{choice}")).wrap());
        }
        for condition in &context.conditions {
            ui.add(egui::Label::new(format!("条件：{condition}")).wrap());
        }
    }
    if let Some(requirement) = &edge.target_requirement {
        ui.add(egui::Label::new(format!("目标准入：{requirement}")).wrap());
    }
    if edge.target_requirement.is_none()
        && edge
            .contexts
            .iter()
            .all(|context| context.choices.is_empty() && context.conditions.is_empty())
    {
        ui.label(theme::muted("没有显式条件上下文"));
    }
}

impl WorldeditApp {
    pub(super) fn graph_edge_details(&mut self, ctx: &egui::Context) {
        if self.command_palette.open {
            return;
        }
        let Some(mut details) = ctx.data(|data| data.get_temp::<Details>(state_id())) else {
            return;
        };
        if details.workspace != self.project.root {
            ctx.data_mut(|data| data.remove::<Details>(state_id()));
            return;
        }
        if details.timeline != (self.tab == Tab::Timeline) {
            return;
        }
        let current = details.version == self.version && details.workspace == self.project.root;
        let mut open = true;
        let mut source = None;
        let mut close_requested = false;
        let escape = !self.ime_composing
            && !self.command_palette.ime
            && !self.command_palette.ime_frame
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let width = (ctx.screen_rect().width() - 48.0).clamp(240.0, 620.0);
        let height = (ctx.screen_rect().height() - 100.0).max(180.0);
        egui::Window::new("连接详情 · 原始分支")
            .id(Id::new("graph-edge-detail-window"))
            .open(&mut open)
            .order(egui::Order::Foreground)
            .default_width(width)
            .max_width(width)
            .max_height(height)
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(&details.heading).strong()).wrap());
                ui.label(format!("{} 条独立连接 · 仅聚合显示", details.edges.len()));
                ui.label(theme::muted("静态显式上下文，不推演运行必然性"));
                if !current {
                    ui.colored_label(
                        theme::GOLD(),
                        "来源版本已变化，请关闭后重新打开当前连接；旧来源定位已停用",
                    );
                }
                let close = ui.button("关闭连接详情（Esc）");
                if details.first_focus {
                    close.request_focus();
                    details.first_focus = false;
                }
                if close.clicked() {
                    close_requested = true;
                }
                egui::ScrollArea::vertical()
                    .id_salt(("original-edges", details.trigger))
                    .max_height((height - 120.0).max(120.0))
                    .show(ui, |ui| {
                        ui.set_max_width(width - 32.0);
                        for (index, edge) in details.edges.iter().enumerate() {
                            ui.push_id(index, |ui| {
                                ui.separator();
                                ui.label(theme::muted(format!(
                                    "{} · {}",
                                    index + 1,
                                    kind_name(edge.kind)
                                )));
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(
                                            edge.label.as_deref().unwrap_or("无选择标签"),
                                        )
                                        .strong(),
                                    )
                                    .wrap(),
                                );
                                detail_context(ui, edge);
                                let relative = theme::relative_source(
                                    &self.project.root,
                                    std::path::Path::new(&edge.file),
                                );
                                ui.add(
                                    egui::Label::new(theme::muted(format!(
                                        "{relative} · 第 {} 行",
                                        edge.line
                                    )))
                                    .wrap(),
                                );
                                let response = ui.add_enabled(
                                    current,
                                    egui::Button::new(format!("定位第 {} 条来源", index + 1)),
                                );
                                if details.return_focus == Some(response.id) {
                                    response.request_focus();
                                    details.return_focus = None;
                                }
                                if response.has_focus() {
                                    response.scroll_to_me(Some(egui::Align::Center));
                                }
                                if response.clicked() {
                                    source = Some((index, response.id));
                                }
                            });
                        }
                    });
            });
        if escape || !open || close_requested {
            ctx.data_mut(|data| data.remove::<Details>(state_id()));
            ctx.memory_mut(|memory| memory.request_focus(details.trigger));
            return;
        }
        if let Some((index, focus)) = source {
            if self.navigate_graph_edge(&details, index) {
                details.return_focus = Some(focus);
            }
        }
        ctx.data_mut(|data| data.insert_temp(state_id(), details));
    }

    fn navigate_graph_edge(&mut self, details: &Details, index: usize) -> bool {
        let result = (|| {
            if details.workspace != self.project.root || details.version != self.version {
                return Err("连接来源版本已变化，请重新打开当前连接".to_owned());
            }
            self.project.verify_review_navigation()?;
            let snapshot = self.snapshot.as_ref().ok_or("当前图不可用")?;
            if snapshot.result.has_errors() {
                return Err("当前源码有错误，不能定位旧连接来源".into());
            }
            let edge_index = *details.group.edges.get(index).ok_or("连接已不存在")?;
            let edge = snapshot
                .result
                .analysis
                .graph
                .edges
                .get(edge_index)
                .ok_or("连接已不存在")?;
            let saved = details.edges.get(index).ok_or("连接已不存在")?;
            if serde_json::to_value(edge).ok() != serde_json::to_value(saved).ok() {
                return Err("连接身份已变化，未按标签或端点猜测来源".into());
            }
            let path = PathBuf::from(&edge.file);
            let source = snapshot
                .result
                .sources
                .get(&path)
                .ok_or("连接来源文件不可用")?;
            self.project.verify_source_navigation(&path, source)?;
            Ok((edge.file.clone(), edge.line))
        })();
        match result {
            Ok((file, line)) => {
                self.jump_to_file(&file, line, 1);
                true
            }
            Err(error) => {
                self.message = Some(error);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests;
