//! 焦点显示状态；结果和所有语义均来自 core。
use crate::app::network_state::GraphCamera;
use std::collections::BTreeMap;
use worldline_core::world_context::{WorldContextKind, WorldContextResult};

pub(super) const KINDS: [(WorldContextKind, &str, &str); 6] = [
    (WorldContextKind::FormalRelation, "正式关系", "实线"),
    (
        WorldContextKind::LegacyCharacterRelation,
        "旧式人物关系",
        "实线·旧",
    ),
    (WorldContextKind::PropertyReference, "属性引用", "虚线·引"),
    (WorldContextKind::EventParticipation, "事件参与", "虚线·参"),
    (WorldContextKind::ExplicitBodyLink, "显式链接", "虚线·链"),
    (WorldContextKind::TextMention, "文字提及", "点线·非事实"),
];

pub(in crate::app) struct CharacterFocus {
    pub result: Option<WorldContextResult>,
    pub error: Option<String>,
    pub source_status: Option<String>,
    pub source_error: Option<String>,
    pub cache: Option<(u64, String, bool)>,
    pub camera: GraphCamera,
    pub canvas: Option<egui::Rect>,
    pub positions: BTreeMap<String, [f64; 2]>,
    pub fit: bool,
    pub auto_fit: bool,
    pub layout_manual: bool,
    pub layout_size: Option<egui::Vec2>,
    pub layout_members: Vec<String>,
    pub filters: [bool; 6],
    pub mentions: bool,
    pub full: bool,
    pub full_page: usize,
    pub inspector_open: bool,
    pub index_open: bool,
    pub focus_zone: Option<usize>,
    pub zone: usize,
    pub query: String,
    pub show_results: bool,
}
impl Default for CharacterFocus {
    fn default() -> Self {
        Self {
            result: None,
            error: None,
            source_status: None,
            source_error: None,
            cache: None,
            camera: GraphCamera::default(),
            canvas: None,
            positions: BTreeMap::new(),
            fit: true,
            auto_fit: true,
            layout_manual: false,
            layout_size: None,
            layout_members: Vec::new(),
            filters: [true; 6],
            mentions: false,
            full: false,
            full_page: 0,
            inspector_open: false,
            index_open: false,
            focus_zone: None,
            zone: 0,
            query: String::new(),
            show_results: true,
        }
    }
}
impl CharacterFocus {
    pub fn visible(&self, kind: &WorldContextKind) -> bool {
        KINDS
            .iter()
            .position(|(k, _, _)| k == kind)
            .is_some_and(|i| self.filters[i])
    }
}
pub(super) fn key(target: &worldline_core::TargetRef) -> String {
    format!("{}:{}", target.kind, target.id)
}
pub(super) fn kind_label(kind: &WorldContextKind) -> &'static str {
    KINDS
        .iter()
        .find(|(k, _, _)| k == kind)
        .map(|(_, label, _)| *label)
        .unwrap_or("未知分类")
}
/// 辅助pane仅在主内容仍能保留480px时停靠。
pub(super) fn dock_index(width: f32) -> bool {
    width >= 740.0
}
pub(super) fn dock_inspector(width: f32) -> bool {
    width >= 1040.0
}

pub(super) fn precision_label(
    value: worldline_core::world_context::WorldContextPrecision,
) -> &'static str {
    match value {
        worldline_core::world_context::WorldContextPrecision::Line => "行级来源",
        worldline_core::world_context::WorldContextPrecision::Column => "精确到列",
    }
}
pub(super) fn provenance_label(
    value: &worldline_core::world_context::WorldContextProvenance,
) -> String {
    use worldline_core::world_context::WorldContextProvenance::*;
    match value {
        FormalRelation {
            relation_id,
            relation_type,
            ..
        } => format!("正式关系 {relation_id} · 类型 {relation_type}"),
        LegacyCharacterRelation { occurrence } => {
            format!("旧式人物关系 · 第 {occurrence} 个出现位置")
        }
        PropertyReference { property } => format!("属性强引用 · {property}"),
        EventParticipation { event } => format!("事件参与 · {event}"),
        ExplicitBodyLink { label } => format!("正文显式链接 · {label}"),
        Executable {
            context,
            occurrence,
        } => format!("静态使用处 · {} · 第 {occurrence} 处", context.label()),
        TextMention { preview } => format!("可选文字提及，非正式事实 · {preview}"),
    }
}
