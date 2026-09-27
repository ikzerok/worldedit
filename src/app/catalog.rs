//! 标签指针、素材引用和对象之间的来源导航。
mod display;
mod links;

use super::{Tab, WorldeditApp};
use worldline_core::catalog::CatalogObject;

pub(super) fn kind_label(kind: &str) -> &str {
    match kind {
        "state" => "状态",
        "anchor" => "锚点",
        "event" => "事件",
        "scene" => "场景",
        "character" => "人物",
        "entity" => "实体",
        "relation" => "关系",
        "world" => "世界观",
        "storyline" => "故事线",
        "period" => "时段",
        "variable" => "变量",
        "tag" => "标签",
        "asset" => "素材",
        "file" => "文件",
        _ => kind,
    }
}

impl WorldeditApp {
    pub(super) fn navigate_object(&mut self, object: &CatalogObject) {
        match object.target.kind.as_str() {
            "event" => {
                self.select_event(&object.target.id);
                self.tab = Tab::Timeline;
            }
            "character" => {
                self.select_character(&object.target.id);
                self.tab = Tab::Characters;
            }
            "world" => self.tab = Tab::World,
            "entity" => self.edit_entity(Some(&object.target.id)),
            "relation" => self.edit_relation(Some(&object.target.id), None),
            "tag" | "asset" | "state" | "anchor" => {
                self.catalog_target = Some(object.target.clone());
                self.tag_editor = None;
                self.state_editor = None;
                self.anchor_editor = None;
                self.tab = Tab::Catalog;
            }
            _ => self.jump_to_file(&object.file, object.line, 1),
        }
    }
}
