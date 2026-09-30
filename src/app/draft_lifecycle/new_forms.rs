//! 新建表单的默认值不是编辑；初始签名只在内存中，不保存正文草稿。
use crate::app::WorldeditApp;
use serde_json::json;
const KINDS: &[&str] = &[
    "事件正文与分支",
    "人物资料",
    "实体资料",
    "语义关系",
    "关系类型",
    "标签",
    "状态",
    "锚点",
    "Wiki词条",
    "展示预设",
    "文件名称",
    "时段资料",
    "审阅批注",
    "新建地图",
];
impl WorldeditApp {
    pub(in crate::app) fn capture_new_draft_baselines(&mut self) {
        for &kind in KINDS {
            if let Some(value) = self.new_draft_signature(kind) {
                self.new_draft_baselines.entry(kind).or_insert(value);
            }
        }
    }
    pub(in crate::app) fn reset_new_draft_baseline(&mut self, kind: &'static str) {
        self.new_draft_baselines.remove(kind);
        self.capture_new_draft_baselines();
    }
    pub(super) fn pristine_new_draft(&self, kind: &str) -> bool {
        self.new_draft_baselines
            .get(kind)
            .is_some_and(|baseline| self.new_draft_signature(kind).as_ref() == Some(baseline))
    }
    fn new_draft_signature(&self, kind: &str) -> Option<String> {
        let value = match kind {
            "事件正文与分支" => {
                let f = self
                    .event_editor
                    .as_ref()
                    .filter(|f| f.original.is_none())?;
                let d = &f.draft;
                json!([
                    d.id,
                    d.summary,
                    d.storyline,
                    d.characters,
                    d.order,
                    d.period,
                    d.predecessors,
                    d.perm,
                    d.after,
                    format!("{:?}", d.effects),
                    d.body
                ])
            }
            "人物资料" => {
                let f = self
                    .character_editor
                    .as_ref()
                    .filter(|f| f.original.is_none())?;
                let d = &f.draft;
                json!([d.id, d.display, d.properties, d.relations])
            }
            "实体资料" => serde_json::to_value(
                &self
                    .entity_editor
                    .as_ref()
                    .filter(|f| f.original.is_none())?
                    .draft,
            )
            .ok()?,
            "语义关系" => serde_json::to_value(
                &self
                    .relation_editor
                    .as_ref()
                    .filter(|f| f.original.is_none())?
                    .draft,
            )
            .ok()?,
            "关系类型" => {
                let d = &self
                    .relation_type_editor
                    .as_ref()
                    .filter(|f| f.original.is_none())?
                    .draft;
                json!([
                    d.id,
                    d.display,
                    d.inverse_display,
                    format!("{:?}", d.direction),
                    d.from_kind,
                    d.to_kind
                ])
            }
            "标签" => {
                let (_, d) = self.tag_editor.as_ref().filter(|(id, _)| id.is_none())?;
                json!([d.id, d.display, d.description, d.properties])
            }
            "状态" => {
                let (_, d) = self.state_editor.as_ref().filter(|(id, _)| id.is_none())?;
                json!([d.id, d.display, d.target, d.tags])
            }
            "锚点" => {
                let (_, d) = self.anchor_editor.as_ref().filter(|(id, _)| id.is_none())?;
                json!([d.id, d.display, d.description, d.targets])
            }
            "Wiki词条" => self.wiki_editor.as_ref()?.new_draft_value()?,
            "展示预设" => serde_json::to_value(
                &self
                    .preset_editor
                    .as_ref()
                    .filter(|f| f.original.is_none())?
                    .draft,
            )
            .ok()?,
            "文件名称" => json!(self.new_file.as_ref()?),
            "时段资料" => json!(self.new_period.as_ref()?),
            "审阅批注" => serde_json::to_value(
                &self
                    .review
                    .comment_editor
                    .as_ref()
                    .filter(|f| f.original.is_none())?
                    .draft,
            )
            .ok()?,
            "新建地图" => {
                let f = &self.map_creation;
                if !f.open {
                    return None;
                }
                json!([f.id, f.title, f.width, f.height])
            }
            _ => return None,
        };
        Some(value.to_string())
    }
}
