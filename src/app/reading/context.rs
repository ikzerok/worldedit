//! 通用对象上下文只消费 core 投影；缓存有界、来源动作重验当前缓冲。
use super::WorldeditApp;
use crate::theme;
use std::{collections::VecDeque, sync::Arc};
use worldline_core::{
    RelationQueryDirection, TargetRef, WorldContextKind as Kind, WorldContextLimit as Limit,
    WorldContextOptions, WorldContextRecord, WorldContextResult,
};

const KINDS: [(Kind, &str); 10] = [
    (Kind::RuleCall, "规则调用"),
    (Kind::FragmentCall, "片段调用"),
    (Kind::GlobalRead, "全局读取"),
    (Kind::GlobalWrite, "全局写入"),
    (Kind::FormalRelation, "正式关系"),
    (Kind::LegacyCharacterRelation, "旧式人物关系"),
    (Kind::PropertyReference, "属性引用"),
    (Kind::EventParticipation, "事件参与"),
    (Kind::ExplicitBodyLink, "正文显式链接"),
    (Kind::TextMention, "文字提及（非事实）"),
];
struct Entry {
    version: u64,
    target: TargetRef,
    depth: u8,
    direction: RelationQueryDirection,
    result: Result<Arc<WorldContextResult>, String>,
}
pub(in crate::app) struct ContextCache {
    entries: VecDeque<Entry>,
    depth: u8,
    direction: RelationQueryDirection,
}
impl Default for ContextCache {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            depth: 1,
            direction: RelationQueryDirection::Both,
        }
    }
}

impl WorldeditApp {
    fn current_object_context(
        &mut self,
        target: &TargetRef,
    ) -> Result<Arc<WorldContextResult>, String> {
        let cache = &mut self.reading_context;
        cache.entries.retain(|entry| entry.version == self.version);
        if let Some(entry) = cache.entries.iter().find(|entry| {
            entry.target == *target
                && entry.depth == cache.depth
                && entry.direction == cache.direction
        }) {
            return entry.result.clone();
        }
        let result = self
            .snapshot
            .as_ref()
            .ok_or_else(|| "当前分析快照尚未就绪".to_string())
            .and_then(|snapshot| {
                snapshot
                    .result
                    .query_world_context(
                        target,
                        WorldContextOptions {
                            include_executable: true,
                            depth: cache.depth,
                            direction: cache.direction,
                            ..Default::default()
                        },
                    )
                    .map_err(|error| error.to_string())
            })
            .and_then(|mut result| {
                let conflicts = self.project.conflict_snapshots()?;
                if !conflicts.is_empty() || !self.project.recovery_conflicts().is_empty() {
                    result.mark_source_conflict();
                }
                Ok(Arc::new(result))
            });
        if cache.entries.len() >= 8 {
            cache.entries.pop_front();
        }
        cache.entries.push_back(Entry {
            version: self.version,
            target: target.clone(),
            depth: cache.depth,
            direction: cache.direction,
            result: result.clone(),
        });
        result
    }

    pub(in crate::app) fn reading_object_context(&mut self, ui: &mut egui::Ui, target: &TargetRef) {
        egui::CollapsingHeader::new("使用处与相关上下文")
            .default_open(matches!(target.kind.as_str(), "rule" | "fragment" | "variable"))
            .show(ui, |ui| {
                ui.label(theme::muted("静态调用和读写描述源码使用处；实际执行与写入请查看试玩证据。未应用草稿不纳入此快照。"));
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.reading_context.depth, 1, "一跳");
                    ui.selectable_value(&mut self.reading_context.depth, 2, "两跳");
                    ui.separator();
                    ui.selectable_value(&mut self.reading_context.direction, RelationQueryDirection::Both, "全部方向");
                    ui.selectable_value(&mut self.reading_context.direction, RelationQueryDirection::Incoming, "谁使用它");
                    ui.selectable_value(&mut self.reading_context.direction, RelationQueryDirection::Outgoing, "它使用谁");
                    if ui.button("刷新上下文").clicked() { self.reading_context.entries.clear(); }
                });
                let result = match self.current_object_context(target) {
                    Ok(result) => result,
                    Err(error) => { ui.colored_label(theme::WARNING(), format!("上下文不可用：{error}，不能据此判断没有使用处")); return; }
                };
                ui.label(format!("返回 {} / {} 条 · {}", result.returned,
                    result.total.map_or_else(|| "总量未知".into(), |total| total.to_string()),
                    if result.complete { "所选范围完整" } else { "范围不完整" }));
                for reason in &result.reasons { ui.colored_label(theme::WARNING(), limit_label(*reason)); }
                if result.records.is_empty() && result.complete { ui.label("所选范围没有使用处或关系"); }
                for (kind, label) in KINDS {
                    let rows: Vec<_> = result.records.iter().filter(|row| row.kind == kind).collect();
                    if rows.is_empty() { continue; }
                    egui::CollapsingHeader::new(format!("{label} · {} 处", rows.len())).default_open(true).show(ui, |ui| {
                        for row in rows {
                            ui.push_id(&row.id, |ui| self.object_context_row(ui, &result, row));
                        }
                    });
                }
            });
    }

    fn object_context_row(
        &mut self,
        ui: &mut egui::Ui,
        result: &WorldContextResult,
        row: &WorldContextRecord,
    ) {
        ui.horizontal_wrapped(|ui| {
            for (index, target) in [&row.from_ref, &row.to_ref].into_iter().enumerate() {
                if index == 1 {
                    ui.label("→");
                }
                let node = result.nodes.iter().find(|node| node.target == *target);
                let display = node.map_or(target.id.as_str(), |node| node.display.as_str());
                if crate::theme::add_enabled(
                    ui,
                    node.is_some_and(|node| node.exists),
                    egui::Button::new(display),
                )
                .on_hover_text(format!("{}:{}", target.kind, target.id))
                .clicked()
                {
                    self.open_reading(target.clone());
                }
            }
        });
        ui.label(&row.role);
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::muted(format!(
                "{}:{}:{}",
                theme::relative_source(&self.project.root, std::path::Path::new(&row.source.file)),
                row.source.line,
                row.source.column.unwrap_or(1)
            )));
            if ui.small_button("定位使用处").clicked() {
                self.jump_object_context_source(result, row);
            }
        });
        ui.separator();
    }

    fn jump_object_context_source(
        &mut self,
        result: &WorldContextResult,
        row: &WorldContextRecord,
    ) -> bool {
        if self.ime_composing
            || self.ime_source_draft.is_some()
            || !self.dirty_draft_names().is_empty()
        {
            self.message =
                Some("有未应用草稿或正在输入的内容；请先处理草稿，再定位当前稿使用处".into());
            return false;
        }
        if let Err(error) = self.project.verify_review_navigation() {
            self.message = Some(format!("使用处来源需要刷新，未离开当前位置：{error}"));
            return false;
        }
        let valid = self.snapshot.as_ref().is_some_and(|snapshot| {
            !snapshot.result.has_errors()
                && snapshot.result.world_context_snapshot() == result.snapshot
                && snapshot.result.sources.iter().all(|(path, text)| {
                    self.project
                        .document(path)
                        .is_ok_and(|current| current == text)
                })
                && snapshot
                    .result
                    .sources
                    .get(std::path::Path::new(&row.source.file))
                    .is_some_and(|source| {
                        worldline_core::source_coordinates::SourceCoordinates::new(source)
                            .is_ok_and(|coordinates| {
                                coordinates
                                    .locate(
                                        source,
                                        &format!(
                                            "{}:{}",
                                            row.source.line,
                                            row.source.column.unwrap_or(1)
                                        ),
                                    )
                                    .is_ok()
                            })
                    })
        });
        if !valid {
            self.message = Some("稿件或来源已变化，请刷新上下文；未使用旧位置".into());
            return false;
        }
        self.jump_to_file(
            &row.source.file,
            row.source.line,
            row.source.column.unwrap_or(1),
        );
        self.close_transient_reading();
        true
    }
}

fn limit_label(reason: Limit) -> &'static str {
    match reason {
        Limit::InvalidSource => "稿件有错误，已知使用处不能代表完整结果",
        Limit::SourceConflict => "当前缓冲与磁盘存在冲突，请先核对两份内容",
        Limit::CandidateBudget => "相关候选预算耗尽，总量未知；请收窄方向或深度",
        Limit::NodeLimit => "节点显示达到上限，可打开相邻对象继续查看",
        Limit::RecordLimit => "记录显示达到上限，可打开相邻对象继续查看",
        Limit::ExecutableIndexBudget => "静态使用处索引预算耗尽，总量未知",
        Limit::SourceUnavailable => "部分来源无法确认，已省略不可靠的位置",
    }
}

#[cfg(test)]
mod tests;
