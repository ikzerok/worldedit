//! 人物档案、可编辑关系图与反向事件索引。
mod collection;
mod graph;
mod graph_edges;
mod graph_gestures;
mod graph_layout;
mod inspector;
mod results;
mod state;
mod workbench;
use super::{CharacterEditor, Tab, WorldeditApp};
use crate::theme::{self, *};
use egui::RichText;
pub(super) use state::CharacterFocus;
use std::path::PathBuf;
use worldline_core::authoring::CharacterDraft;

impl WorldeditApp {
    pub(super) fn select_character(&mut self, id: &str) {
        if self.prevent_replacing_draft("人物资料") {
            return;
        }
        let info = self
            .snapshot
            .as_ref()
            .and_then(|s| s.result.analysis.symbols.characters.get(id))
            .cloned();
        if let Some(info) = info {
            self.catalog_target = Some(worldline_core::TargetRef::new("character", id));
            self.character_editor = Some(CharacterEditor {
                path: PathBuf::from(info.decl_file),
                original: Some(id.into()),
                draft: CharacterDraft {
                    id: id.into(),
                    display: info.display,
                    properties: info.properties.into_iter().collect(),
                    relations: info
                        .relations
                        .into_iter()
                        .map(|r| (r.target, r.label))
                        .collect(),
                },
            });
        }
    }
    pub(super) fn new_character(&mut self) {
        if self.prevent_replacing_draft("人物资料") {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let characters = &snapshot.result.analysis.symbols.characters;
        let mut index = 1;
        while characters.contains_key(&format!("character_{index}")) {
            index += 1;
        }
        let path = snapshot
            .result
            .analysis
            .symbols
            .character_order
            .first()
            .and_then(|id| characters.get(id))
            .map(|c| PathBuf::from(&c.decl_file))
            .unwrap_or_else(|| self.project.entry.clone());
        self.character_editor = Some(CharacterEditor {
            path,
            original: None,
            draft: CharacterDraft {
                id: format!("character_{index}"),
                display: "新人物".into(),
                ..Default::default()
            },
        });
        self.character_focus.inspector_open = true;
        self.character_focus.focus_zone = Some(3);
        self.reset_new_draft_baseline("人物资料");
    }
}
