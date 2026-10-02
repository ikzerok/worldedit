//! 只读枚举未应用输入；身份与来源来自表单/core，不解析第二套源码。
use super::{UnappliedInput, WorldeditApp};
use std::path::Path;
use worldline_core::TargetRef;

impl WorldeditApp {
    fn export_source_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.project.root)
            .unwrap_or(path)
            .display()
            .to_string()
    }

    fn export_target_source(&self, kind: &str, id: &str) -> String {
        let path = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .result
                    .analysis
                    .catalog
                    .object(&TargetRef::new(kind, id))
            })
            .map(|object| self.export_source_path(Path::new(&object.file)))
            .unwrap_or_else(|| "待应用的对象".into());
        format!("{kind}:{id} · {path}")
    }

    pub(in crate::app) fn unapplied_export_inputs(&self) -> Vec<UnappliedInput> {
        let mut result = Vec::new();
        let mut add = |kind, source| result.push(UnappliedInput { kind, source });
        for kind in self.dirty_draft_names() {
            let source = match kind {
                "正在输入的源码 / 输入法" => self.export_source_path(
                    self.ime_source_draft
                        .as_ref()
                        .map(|(path, _, _)| path)
                        .or_else(|| self.ime_source_baseline.as_ref().map(|(path, _)| path))
                        .unwrap_or(&self.active_file),
                ),
                "事件正文与分支" => self
                    .event_editor
                    .as_ref()
                    .map(|f| {
                        format!(
                            "event:{} · {}",
                            f.draft.id,
                            self.export_source_path(&f.path)
                        )
                    })
                    .unwrap_or_default(),
                "人物资料" => self
                    .character_editor
                    .as_ref()
                    .map(|f| {
                        format!(
                            "character:{} · {}",
                            f.draft.id,
                            self.export_source_path(&f.path)
                        )
                    })
                    .unwrap_or_default(),
                "世界观" => self
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.result.analysis.world.as_ref())
                    .map(|world| self.export_source_path(Path::new(&world.file)))
                    .unwrap_or_else(|| self.export_source_path(&self.project.entry)),
                "实体资料" => self
                    .entity_editor
                    .as_ref()
                    .map(|f| {
                        format!(
                            "entity:{} · {}",
                            f.draft.id,
                            self.export_source_path(&f.path)
                        )
                    })
                    .unwrap_or_default(),
                "语义关系" => self
                    .relation_editor
                    .as_ref()
                    .map(|f| {
                        self.export_target_source(
                            "relation",
                            f.original.as_deref().unwrap_or(&f.draft.id),
                        )
                    })
                    .unwrap_or_default(),
                "关系类型" => self
                    .relation_type_editor
                    .as_ref()
                    .map(|f| {
                        let id = f.original.as_deref().unwrap_or(&f.draft.id);
                        let path = self
                            .snapshot
                            .as_ref()
                            .and_then(|s| s.result.analysis.catalog.relation_types.get(id))
                            .map(|r| self.export_source_path(Path::new(&r.file)))
                            .unwrap_or_else(|| "待应用的类型".into());
                        format!("relation_type:{id} · {path}")
                    })
                    .unwrap_or_default(),
                "标签" => self
                    .tag_editor
                    .as_ref()
                    .map(|(id, draft)| {
                        self.export_target_source("tag", id.as_deref().unwrap_or(&draft.id))
                    })
                    .unwrap_or_default(),
                "状态" => self
                    .state_editor
                    .as_ref()
                    .map(|(id, draft)| {
                        self.export_target_source("state", id.as_deref().unwrap_or(&draft.id))
                    })
                    .unwrap_or_default(),
                "锚点" => self
                    .anchor_editor
                    .as_ref()
                    .map(|(id, draft)| {
                        self.export_target_source("anchor", id.as_deref().unwrap_or(&draft.id))
                    })
                    .unwrap_or_default(),
                "Wiki词条" => self
                    .wiki_editor
                    .as_ref()
                    .map(|f| {
                        let target = f.draft_target();
                        self.export_target_source(&target.kind, &target.id)
                    })
                    .unwrap_or_default(),
                "对象重命名" => self
                    .rename_form
                    .as_ref()
                    .map(|f| self.export_target_source(&f.target.kind, &f.target.id))
                    .unwrap_or_default(),
                "展示预设" => self
                    .preset_editor
                    .as_ref()
                    .map(|f| {
                        self.snapshot
                            .as_ref()
                            .and_then(|s| {
                                s.preset_index
                                    .presets
                                    .get(f.original.as_deref().unwrap_or(&f.draft.id))
                            })
                            .map(|preset| {
                                format!(
                                    "preset:{} · {}",
                                    f.draft.id,
                                    self.export_source_path(&preset.path)
                                )
                            })
                            .unwrap_or_else(|| format!("新建 preset:{}", f.draft.id))
                    })
                    .unwrap_or_default(),
                "源码路径" => self
                    .source_move_form
                    .as_ref()
                    .map(|f| format!("{} → {}", self.export_source_path(&f.source), f.destination))
                    .unwrap_or_default(),
                "文件名称" => self.new_file.clone().unwrap_or_default(),
                "时段资料" => self
                    .new_period
                    .as_ref()
                    .map(|(id, _, _)| self.export_target_source("period", id))
                    .unwrap_or_default(),
                "审阅批注" => self
                    .review
                    .comment_editor
                    .as_ref()
                    .map(|f| {
                        self.snapshot
                            .as_ref()
                            .and_then(|s| {
                                s.comment_index
                                    .comments
                                    .get(f.original.as_deref().unwrap_or(&f.draft.id))
                            })
                            .map(|comment| {
                                format!(
                                    "comment:{} · {}",
                                    f.draft.id,
                                    self.export_source_path(&comment.path)
                                )
                            })
                            .unwrap_or_else(|| format!("新建 comment:{}", f.draft.id))
                    })
                    .unwrap_or_default(),
                "新建地图" => format!("map:{}", self.map_creation.id),
                "地图草稿" => format!(
                    "map:{}",
                    self.map_selection.as_deref().unwrap_or("未选地图")
                ),
                "持续资料约束草稿" => self
                    .schema_ui
                    .draft_path()
                    .map(|p| self.export_source_path(p))
                    .unwrap_or_else(|| "schema 工作台".into()),
                "书稿 / 正文草稿" => {
                    for source in self.manuscript.unapplied_sources() {
                        add(kind, source);
                    }
                    continue;
                }
                "本地化草稿" => format!(
                    "本地化工作台 · {} → {} / 交换 JSON",
                    self.localization_ui.source_locale, self.localization_ui.target_locale
                ),
                _ => kind.into(),
            };
            add(kind, source);
        }
        if let Some(source) = self
            .markdown_import_wizard
            .as_ref()
            .and_then(|wizard| wizard.unapplied_source())
        {
            add("Markdown 导入草稿", source);
        }
        if let Some(source) = self.template_manager.unapplied_source(
            self.snapshot
                .as_ref()
                .map(|snapshot| &snapshot.template_index),
        ) {
            add("工程模板草稿", source);
        }
        if !self.review.reason.trim().is_empty()
            || !self.review.proposal_id.trim().is_empty()
            || !self.review.conflict_resolutions.is_empty()
        {
            add(
                "审阅提案",
                format!(
                    "协作审阅 · {}",
                    self.review
                        .selected_proposal
                        .as_deref()
                        .unwrap_or(&self.review.proposal_id)
                ),
            );
        }
        if !self.network_view_id.trim().is_empty() || !self.network_view_title.trim().is_empty() {
            let draft = self.network_state.draft(
                self.network_view_id.clone(),
                self.network_view_title.clone(),
            );
            let saved = self.snapshot.as_ref().and_then(|s| {
                s.graph_index.views.get(
                    self.network_loaded_view
                        .as_deref()
                        .unwrap_or(&self.network_view_id),
                )
            });
            if saved.is_none_or(|view| Some(&view.draft) != draft.as_ref()) {
                add(
                    "共享网络布局",
                    format!("graph_view:{}", self.network_view_id),
                );
            }
        }
        if let Some(source) = self.catalog_workbench.unapplied_saved_query(&self.project) {
            add("共享查询定义", source);
        }
        for source in self.unapplied_search_sources() {
            add("正文替换预览", source);
        }
        result.sort_by(|a, b| (a.kind, &a.source).cmp(&(b.kind, &b.source)));
        result.dedup();
        result
    }
}
