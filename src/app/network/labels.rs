//! Same-frame borrowed labels avoid repeated full Catalog scans while painting.
use std::collections::HashMap;
use worldline_core::{Catalog, TargetRef};

pub(super) struct DisplayLabels<'a> {
    objects: HashMap<&'a TargetRef, &'a str>,
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
        Self { objects }
    }

    pub(super) fn get<'b>(&'b self, target: &'b TargetRef) -> &'b str {
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
}
