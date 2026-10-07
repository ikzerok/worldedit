use super::*;
use worldline_core::catalog::Catalog;
use worldline_core::vector_scene::{SceneNode, SceneOp};

#[derive(Clone, Default)]
pub(super) enum BindingGuard {
    #[default]
    AuthorDraft,
    Search {
        query: String,
        version: u64,
    },
    Unconfirmed {
        target: TargetRef,
        temporary: bool,
    },
}

impl BindingGuard {
    pub(super) fn refresh(
        &mut self,
        target: &mut Option<TargetRef>,
        query: &str,
        version: u64,
        catalog: Option<&Catalog>,
    ) {
        // 作者原有身份的解析状态随目录更新；临时选择失效则必须显式确认。
        if matches!(
            self,
            Self::Unconfirmed {
                temporary: false,
                ..
            }
        ) {
            *self = Self::AuthorDraft;
        }
        let stale_search = matches!(self, Self::Search { query: selected, version: old }
            if selected != query || *old != version || catalog.is_none());
        if stale_search {
            if let Some(previous) = target.take() {
                *self = Self::Unconfirmed {
                    target: previous,
                    temporary: true,
                };
            }
        } else if matches!(self, Self::AuthorDraft) {
            if let Some(previous) = target
                .as_ref()
                .filter(|target| catalog.is_none_or(|catalog| catalog.object(target).is_none()))
            {
                // 已载入的作者绑定保留完整身份；缺失时仍须明确处理。
                *self = Self::Unconfirmed {
                    target: previous.clone(),
                    temporary: false,
                };
            }
        }
    }

    pub(super) fn pending(&self) -> Option<&TargetRef> {
        match self {
            Self::Unconfirmed { target, .. } => Some(target),
            _ => None,
        }
    }

    pub(super) fn blocks_search_binding(&self) -> bool {
        matches!(
            self,
            Self::Unconfirmed {
                temporary: true,
                ..
            }
        )
    }

    pub(super) fn notice(&self, ui: &mut egui::Ui) {
        if let Some(target) = self.pending() {
            let message = if self.blocks_search_binding() {
                format!("待确认引用 {}:{}：查询或来源已变化，请重新选择对象、明确不使用对象引用，或取消表单。", target.kind, target.id)
            } else {
                format!(
                    "原有引用 {}:{} 当前未解析；完整身份保留，不会自动解绑或替换。",
                    target.kind, target.id
                )
            };
            ui.colored_label(crate::theme::GOLD(), message);
        }
    }
}

impl super::super::WorldeditApp {
    pub(in crate::app) fn refresh_map_binding_guards(&mut self) {
        let catalog = self
            .snapshot
            .as_ref()
            .map(|snapshot| &snapshot.result.analysis.catalog);
        self.map_form.binding.refresh(
            &mut self.map_form.target,
            &self.map_form.target_query,
            self.version,
            catalog,
        );
        let scene = &mut self.map_canvas.scene;
        if let Some(node) = scene.inspector.as_mut() {
            if scene.binding_node != node.id {
                scene.binding = BindingGuard::default();
                scene.binding_node = node.id.clone();
            }
            scene.binding.refresh(
                &mut node.target_ref,
                &scene.binding_query,
                self.version,
                catalog,
            );
            if matches!(
                scene.binding,
                BindingGuard::Unconfirmed {
                    temporary: true,
                    ..
                }
            ) {
                scene.inspector_dirty = true;
            }
        }
    }

    pub(super) fn marker_binding_ready(&mut self) -> bool {
        self.refresh_map_binding_guards();
        if self.map_form.binding.pending().is_some() {
            self.io_error = Some("对象引用待确认；请重新选择、不使用对象引用或取消表单。".into());
            false
        } else {
            true
        }
    }

    pub(super) fn select_marker_binding(&mut self, target: TargetRef) {
        self.map_form.target = Some(target);
        self.map_form.target_query.clear();
        self.map_form.binding = BindingGuard::Search {
            query: String::new(),
            version: self.version,
        };
    }

    pub(super) fn omit_marker_binding(&mut self) {
        self.map_form.target = None;
        self.map_form.target_query.clear();
        self.map_form.binding = BindingGuard::default();
    }

    pub(super) fn refresh_scene_binding(&mut self, node: &mut SceneNode) {
        let scene = &mut self.map_canvas.scene;
        if scene.binding_node != node.id {
            scene.binding = BindingGuard::default();
            scene.binding_node = node.id.clone();
        }
        scene.binding.refresh(
            &mut node.target_ref,
            &scene.binding_query,
            self.version,
            self.snapshot
                .as_ref()
                .map(|snapshot| &snapshot.result.analysis.catalog),
        );
    }

    pub(super) fn discard_scene_binding_submission(&mut self, id: &str) {
        let owns = |operation: &SceneOp| {
            matches!(operation,
            SceneOp::Insert { node, .. } | SceneOp::Update { node } if node.id == id)
        };
        let scene = &mut self.map_canvas.scene;
        scene.operations.retain(|operation| !owns(operation));
        scene.retry_operations.retain(|operation| !owns(operation));
        if scene
            .job
            .as_ref()
            .is_some_and(|job| job.batch.operations.iter().any(owns))
        {
            scene.job = None;
        }
        if scene
            .review_plan
            .as_ref()
            .is_some_and(|plan| plan.affected_nodes.iter().any(|node| node == id))
        {
            scene.review_plan = None;
        }
    }
}
