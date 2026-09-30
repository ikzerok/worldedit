use super::{target_label, Navigation, PageChange};
use crate::theme::{self, *};
use egui::RichText;
use std::collections::BTreeMap;
use worldline_core::catalog::{Catalog, TargetRef};
use worldline_core::{
    TopicProjectionHistoryEvent, TopicProjectionHistorySource, TopicProjectionResult,
    TopicProjectionTimeStatus,
};

pub(super) fn history_projection(
    ui: &mut egui::Ui,
    catalog: &Catalog,
    result: &TopicProjectionResult,
    target: &TargetRef,
    navigation: &mut Option<Navigation>,
    page_change: &mut Option<PageChange>,
) {
    ui.separator();
    ui.label(RichText::new(format!("历史关联 · {} 条", result.history.items.len())).strong());
    let is_place = target.kind == "entity"
        && catalog
            .entities
            .get(&target.id)
            .is_some_and(|entity| entity.entity_type == "place");
    if target.kind != "character" && !is_place {
        ui.label(theme::muted(
            "人物历史只取显式 with；地点历史只取映射的 event → place 关系。",
        ));
    }
    if result.history.truncated {
        ui.colored_label(
            GOLD(),
            "历史页已截断；使用独立的历史 continuation 继续读取。",
        );
    }
    let events: BTreeMap<_, _> = result
        .history
        .events
        .iter()
        .map(|event| (event.target.clone(), event))
        .collect();
    if result.history.items.is_empty() {
        ui.label(theme::muted(
            "没有符合 core 历史成员规则的事件；不从提及或控制流推导历史。",
        ));
    }
    for item in &result.history.items {
        ui.push_id(
            ("topic-history", &item.event, item.file.as_str(), item.line),
            |ui| {
                ui.group(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .link(format!("阅读 {}", target_label(catalog, &item.event)))
                            .clicked()
                        {
                            *navigation = Some(Navigation::Read(item.event.clone()));
                        }
                        if ui
                            .small_button(format!("定位来源 {}:{}", item.file, item.line))
                            .clicked()
                        {
                            *navigation = Some(Navigation::Source(item.file.clone(), item.line));
                        }
                    });
                    match &item.source {
                        TopicProjectionHistorySource::With => {
                            ui.label(theme::muted("成员来源：正文显式 with"));
                        }
                        TopicProjectionHistorySource::Relation {
                            id,
                            relation_type,
                            role,
                            scope_refs,
                        } => {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(format!("{role} · 类型 {relation_type} · 关系 {id}"));
                                if ui.small_button(format!("阅读关系 {id}")).clicked() {
                                    *navigation =
                                        Some(Navigation::Read(TargetRef::new("relation", id)));
                                }
                            });
                            ui.label(theme::muted(format!(
                                "scope [{}]",
                                scope_refs
                                    .iter()
                                    .map(|scope| format!("{}:{}", scope.kind, scope.id))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )));
                        }
                    }
                    if let Some(event) = events.get(&item.event) {
                        history_time(ui, catalog, event, navigation);
                    }
                });
            },
        );
    }
    if !result.history.temporal_edges.is_empty() {
        ui.label(RichText::new("明确先后约束").strong());
        for edge in &result.history.temporal_edges {
            ui.horizontal_wrapped(|ui| {
                let before = TargetRef::new("event", &edge.before);
                let after = TargetRef::new("event", &edge.after);
                if ui.link(target_label(catalog, &before)).clicked() {
                    *navigation = Some(Navigation::Read(before));
                }
                ui.label("先于");
                if ui.link(target_label(catalog, &after)).clicked() {
                    *navigation = Some(Navigation::Read(after));
                }
            });
        }
    }
    if !result.history.parallel_groups.is_empty() {
        ui.label(RichText::new("同层并列 · 不代表同时发生").strong());
        for group in &result.history.parallel_groups {
            ui.horizontal_wrapped(|ui| {
                for event in group {
                    if ui.link(target_label(catalog, event)).clicked() {
                        *navigation = Some(Navigation::Read(event.clone()));
                    }
                }
            });
        }
    }
    anchors(
        ui,
        catalog,
        "目标锚点",
        &result.history.target_anchors,
        navigation,
    );
    for event in &result.history.events {
        anchors(
            ui,
            catalog,
            &format!("{} 的直接锚点", target_label(catalog, &event.target)),
            &event.anchors,
            navigation,
        );
    }
    if result.history.truncated {
        if let Some(next) = result
            .history
            .next_offset
            .filter(|next| *next > result.history.offset)
        {
            if ui.button("继续读取历史").clicked() {
                *page_change = Some(PageChange::History(next));
            }
        } else {
            ui.label(theme::muted("历史结果已截断，但当前页没有可继续的偏移量。"));
        }
    }
}

fn history_time(
    ui: &mut egui::Ui,
    catalog: &Catalog,
    event: &TopicProjectionHistoryEvent,
    navigation: &mut Option<Navigation>,
) {
    if ui
        .small_button(format!("定位事件 {}:{}", event.file, event.line))
        .clicked()
    {
        *navigation = Some(Navigation::Source(event.file.clone(), event.line));
    }
    match event.time_status {
        TopicProjectionTimeStatus::Unknown => {
            ui.label(theme::muted("时间未知 · core 未补日期或时段"));
        }
        TopicProjectionTimeStatus::PeriodRanked => {
            if let (Some(period), Some(rank)) = (&event.period, event.rank) {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "时段 {} · 拓扑层级 {rank}（不构成全序）",
                        target_label(catalog, period)
                    ));
                    if ui.small_button("阅读时段").clicked() {
                        *navigation = Some(Navigation::Read(period.clone()));
                    }
                });
            }
        }
    }
}

fn anchors(
    ui: &mut egui::Ui,
    catalog: &Catalog,
    heading: &str,
    anchors: &[TargetRef],
    navigation: &mut Option<Navigation>,
) {
    if anchors.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(heading).strong());
        for anchor in anchors {
            if ui.link(target_label(catalog, anchor)).clicked() {
                *navigation = Some(Navigation::Read(anchor.clone()));
            }
        }
    });
}
