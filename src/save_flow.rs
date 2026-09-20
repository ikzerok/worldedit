//! 浏览器保存的边界编排；宿主 I/O 通过 trait 注入，便于在 native 上验证失败顺序。

pub trait SaveHost {
    fn request_download(&mut self, name: &str, bytes: &[u8], mime: &str) -> Result<(), String>;
    fn persist(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn record_export_revision(&mut self, revision: u64);
    fn record_local_snapshot_revision(&mut self, revision: u64);
}

pub fn save_project_package<H, F>(
    host: &mut H,
    revision: u64,
    bytes: &[u8],
    on_success: F,
) -> Result<(), String>
where
    H: SaveHost,
    F: FnOnce(),
{
    host.request_download("worldedit-project.zip", bytes, "application/zip")?;
    host.record_export_revision(revision);
    host.persist(bytes)?;
    host.record_local_snapshot_revision(revision);
    on_success();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{save_project_package, SaveHost};

    #[derive(Default)]
    struct FakeHost {
        events: Vec<&'static str>,
        fail_download: bool,
        fail_persist: bool,
        export_revision: Option<u64>,
        local_revision: Option<u64>,
    }

    impl SaveHost for FakeHost {
        fn request_download(
            &mut self,
            _name: &str,
            _bytes: &[u8],
            _mime: &str,
        ) -> Result<(), String> {
            self.events.push("download");
            if self.fail_download {
                Err("download failed".into())
            } else {
                Ok(())
            }
        }

        fn persist(&mut self, _bytes: &[u8]) -> Result<(), String> {
            self.events.push("persist");
            if self.fail_persist {
                Err("persist failed".into())
            } else {
                Ok(())
            }
        }

        fn record_export_revision(&mut self, revision: u64) {
            self.events.push("export_revision");
            self.export_revision = Some(revision);
        }

        fn record_local_snapshot_revision(&mut self, revision: u64) {
            self.events.push("local_revision");
            self.local_revision = Some(revision);
        }
    }

    #[test]
    fn download_failure_skips_persist_and_success_baseline() {
        let (mut project, root) = dirty_project("download");
        let sources = project.sources();
        let authoring = authoring_bytes(&project);
        let mut host = FakeHost {
            fail_download: true,
            ..Default::default()
        };
        let mut baseline = false;

        let result = save_project_package(&mut host, 7, b"zip", || {
            project.mark_saved();
            baseline = true;
        });

        assert_eq!(result, Err("download failed".into()));
        assert_eq!(host.events, ["download"]);
        assert_eq!(host.export_revision, None);
        assert_eq!(host.local_revision, None);
        assert!(project.is_dirty());
        assert!(!baseline);
        assert_eq!(project.sources(), sources);
        assert_eq!(authoring_bytes(&project), authoring);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn persist_failure_keeps_previous_local_revision_but_records_export() {
        let (mut project, root) = dirty_project("persist");
        let sources = project.sources();
        let authoring = authoring_bytes(&project);
        let mut host = FakeHost {
            fail_persist: true,
            export_revision: Some(2),
            local_revision: Some(2),
            ..Default::default()
        };
        let mut baseline = false;

        let result = save_project_package(&mut host, 7, b"zip", || {
            project.mark_saved();
            baseline = true;
        });

        assert_eq!(result, Err("persist failed".into()));
        assert_eq!(host.events, ["download", "export_revision", "persist"]);
        assert_eq!(host.export_revision, Some(7));
        assert_eq!(host.local_revision, Some(2));
        assert!(project.is_dirty());
        assert!(!baseline);
        assert_eq!(project.sources(), sources);
        assert_eq!(authoring_bytes(&project), authoring);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn success_records_both_revisions_before_advancing_baseline() {
        let (mut project, root) = dirty_project("success");
        let sources = project.sources();
        let authoring = authoring_bytes(&project);
        let mut host = FakeHost::default();
        let current_revision = 7;
        let mut baseline_revision = None;

        let result = save_project_package(&mut host, current_revision, b"zip", || {
            project.mark_saved();
            baseline_revision = Some(current_revision);
        });

        assert_eq!(result, Ok(()));
        assert_eq!(
            host.events,
            ["download", "export_revision", "persist", "local_revision",]
        );
        assert_eq!(host.export_revision, Some(7));
        assert_eq!(host.local_revision, Some(7));
        assert_eq!(baseline_revision, Some(current_revision));
        assert!(!project.is_dirty());
        assert_eq!(project.sources(), sources);
        assert_eq!(authoring_bytes(&project), authoring);
        let _ = std::fs::remove_dir_all(root);
    }

    fn dirty_project(name: &str) -> (worldline_core::project::Project, std::path::PathBuf) {
        let root =
            std::env::temp_dir().join(format!("worldedit-save-flow-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut project = worldline_core::project::Project::new(&root);
        let entry = project.entry.clone();
        project.set_text(&entry, "draft source".into()).unwrap();
        let manifest = root.join(".world/project.json");
        let map = root.join(".world/maps/draft.json");
        project
            .create_authoring_document(
                &manifest,
                br#"{"schema_version":1,"maps":{"draft":".world/maps/draft.json"}}"#.to_vec(),
            )
            .unwrap();
        project
            .create_authoring_document(&map, br#"{"unknown":true}"#.to_vec())
            .unwrap();
        assert!(project.is_dirty());
        (project, root)
    }

    fn authoring_bytes(
        project: &worldline_core::project::Project,
    ) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
        project
            .authoring_documents
            .iter()
            .map(|(path, document)| (path.clone(), document.bytes().to_vec()))
            .collect()
    }
}
