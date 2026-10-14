//! H08's pure-value preview/author-state witness; no focus or rendering hooks.
use super::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;

impl WorldeditApp {
    pub(in crate::app) fn h08_world_link_preview_state(&self) -> Value {
        let state = self.manuscript.world_links.as_ref().expect("actual W form");
        let plan = state.plan.as_ref().expect("actual Preview result");
        let request = state.request().expect("actual W request");
        let target = TargetRef::new("character", "traveler");
        assert!(state.open && plan.can_apply && state.error.is_none());
        assert_eq!(state.chosen.as_ref(), Some(&target));
        assert_eq!(plan.target, target);
        assert_eq!(state.selection.expected_text, "港口");
        assert_eq!(state.source, TargetRef::new("scene", "arrival.harbor"));
        assert_eq!(plan.source_path, state.selection.path);
        assert_eq!(
            serde_json::to_value(plan.request()).unwrap(),
            serde_json::to_value(&request).unwrap()
        );
        let buffers: BTreeMap<_, _> = self
            .manuscript
            .writing_buffers()
            .into_iter()
            .map(|b| {
                (
                    b.path().to_owned(),
                    (
                        b.identity().to_owned(),
                        b.source().to_owned(),
                        b.baseline().to_owned(),
                        b.generation(),
                        b.is_changed(),
                    ),
                )
            })
            .collect();
        let project = project_bytes(&self.project);
        let disk: BTreeMap<_, _> = worldline_core::file_access::workspace_files(&self.project.root)
            .unwrap()
            .into_iter()
            .map(|path| {
                let metadata = std::fs::metadata(&path).unwrap();
                #[cfg(unix)]
                let mode = {
                    use std::os::unix::fs::PermissionsExt;
                    metadata.permissions().mode() & 0o777
                };
                #[cfg(not(unix))]
                let mode = 0;
                let bytes = std::fs::read(&path).unwrap();
                (path, (bytes, mode, metadata.permissions().readonly()))
            })
            .collect();
        let project_edges: Vec<_> = [&self.history, &self.redo]
            .into_iter()
            .map(|stack| {
                stack
                    .iter()
                    .map(|edge| project_bytes(&edge.snapshot))
                    .collect::<Vec<_>>()
            })
            .collect();
        let draft_edges: Vec<_> = [&self.search_state.undo, &self.search_state.redo]
            .into_iter()
            .map(|stack| {
                stack
                    .iter()
                    .map(|edge| {
                        (
                            format!("{:?}", edge.node),
                            std::sync::Arc::as_ptr(&edge.guard) as usize,
                            project_bytes(&edge.guard),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        json!({"chosen":state.chosen,"request":request,"target":plan.target,
            "plan_digest":plan.plan_digest,"selection":state.selection,"open":state.open,
            "can_apply":plan.can_apply,"error":state.error,"buffers":buffers,
            "fields":self.manuscript.writing_view.retained_runtime_drafts(&self.project.root),
            "project":project,"disk":disk,"project_history":project_edges,"draft_history":draft_edges,
            "current_node":format!("{:?}",self.history_state.current),
            "history_counts":[self.history.len(),self.redo.len(),self.search_state.undo.len(),self.search_state.redo.len()]})
    }
}
fn project_bytes(project: &Project) -> Value {
    let sources: BTreeMap<_, _> = project.sources().into_iter().collect();
    let documents: BTreeMap<_, _> = project
        .authoring_documents
        .iter()
        .map(|(path, doc)| (path, (doc.bytes(), doc.is_deleted())))
        .collect();
    json!({"sources":sources,"documents":documents,"baseline":project.content_baseline(),"dirty":project.is_dirty()})
}
