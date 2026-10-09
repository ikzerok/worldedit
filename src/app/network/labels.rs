//! Same-frame borrowed labels avoid repeated full Catalog scans while painting.
use std::collections::HashMap;
use worldline_core::{Catalog, TargetRef};

pub(super) struct DisplayLabels<'a> {
    objects: HashMap<&'a TargetRef, &'a str>,
    scope: Option<&'a worldline_core::catalog_scope::CatalogScopeSnapshot>,
}
impl<'a> DisplayLabels<'a> {
    pub(super) fn new(catalog: &'a Catalog) -> Self {
        let mut objects = HashMap::with_capacity(catalog.objects.len());
        for object in &catalog.objects {
            // Catalog::object returns the first occurrence, even in a diagnosed snapshot.
            objects
                .entry(&object.target)
                .or_insert(object.display.as_str());
        }
        Self {
            objects,
            scope: None,
        }
    }

    pub(super) fn from_scope(
        scope: &'a worldline_core::catalog_scope::CatalogScopeSnapshot,
    ) -> Self {
        Self {
            objects: HashMap::new(),
            scope: Some(scope),
        }
    }

    pub(super) fn get<'b>(&'b self, target: &'b TargetRef) -> &'b str {
        if let Some(scope) = self.scope {
            return scope
                .object(target)
                .map_or(target.id.as_str(), |object| object.display.as_str());
        }
        self.objects.get(target).copied().unwrap_or(&target.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldline_core::catalog::CatalogObject;

    fn object(kind: &str, id: &str, display: &str) -> CatalogObject {
        CatalogObject {
            target: TargetRef::new(kind, id),
            display: display.into(),
            file: "world.wl".into(),
            line: 1,
        }
    }

    #[test]
    fn borrowed_labels_match_core_first_identity_and_unknown_fallback() {
        let catalog = Catalog {
            objects: vec![
                object("entity", "same", "第一实体"),
                object("event", "same", "同 ID 事件"),
                object("entity", "same", "重复实体"),
            ],
            ..Default::default()
        };
        let labels = DisplayLabels::new(&catalog);
        for target in [
            TargetRef::new("entity", "same"),
            TargetRef::new("event", "same"),
        ] {
            assert_eq!(
                labels.get(&target),
                catalog.object(&target).unwrap().display
            );
        }
        assert_eq!(labels.get(&TargetRef::new("entity", "失效")), "失效");
    }

    #[test]
    fn new_frame_reads_current_snapshot_labels_without_a_stale_cache() {
        let target = TargetRef::new("entity", "same");
        let mut catalog = Catalog {
            objects: vec![object("entity", "same", "旧名称")],
            ..Default::default()
        };
        assert_eq!(DisplayLabels::new(&catalog).get(&target), "旧名称");
        catalog.objects[0].display = "新名称".into();
        assert_eq!(DisplayLabels::new(&catalog).get(&target), "新名称");
        catalog.objects.clear();
        assert_eq!(DisplayLabels::new(&catalog).get(&target), "same");
    }
    #[test]
    fn scoped_labels_borrow_complete_identity_index_without_copying_all_objects() {
        let root = std::env::temp_dir().join(format!("scope-labels-{}", std::process::id()));
        let mut project = worldline_core::project::Project::new(&root);
        let long = "同名长地点名称".repeat(40);
        project.set_text(&project.entry.clone(), format!("event start\n  -> END\nentity first kind place as \"{long}\"\nentity second kind place as \"{long}\"\n")).unwrap();
        project.create_authoring_document(&project.root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.10","entry":"world.wl","required_features":["content.entities.v1"]}"#.to_vec()).unwrap();
        let snapshot = project
            .catalog_scope_snapshot(&Default::default(), 10_000)
            .unwrap();
        let labels = DisplayLabels::from_scope(&snapshot);
        assert!(labels.objects.is_empty());
        assert_eq!(labels.get(&TargetRef::new("entity", "first")), long);
        assert_eq!(labels.get(&TargetRef::new("entity", "second")), long);
        assert_eq!(labels.get(&TargetRef::new("entity", "missing")), "missing");
        assert_eq!(labels.get(&TargetRef::default()), "");
    }
}
