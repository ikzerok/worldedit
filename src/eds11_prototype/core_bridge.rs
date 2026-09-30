//! 隔离虚构工程的真实 core 接线。只改变内存缓冲，绝不调用 Project 保存/导出。
use std::collections::BTreeMap;
use worldline_core::{
    collaboration::{self, ProposalDraft, ProposalFileChange, ProposalPreview, ProposalStatus},
    presentation::MapGeometry,
    presentation_commands::{self as commands, Command, CommandEnvelope, Revision, UndoRecord},
    project::Project,
};

pub(super) struct MemoryDemo {
    pub project: Project,
    pub revision: Revision,
    pub undo: Option<UndoRecord>,
    pub undo_from: Option<f32>,
    pub applied_commands: u64,
    pub runtime: Option<RuntimeEvidence>,
    pub review: Option<ProposalPreview>,
}

pub(super) struct RuntimeEvidence {
    pub output: String,
    pub ended: bool,
    pub start_visits: u32,
}

fn source(sentence: &str) -> String {
    format!("entity a kind place as \"资料 A\"\nentity b kind place as \"资料 B\"\nevent start\n  {sentence}\n  -> END\n")
}

impl MemoryDemo {
    pub fn new(index: usize) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        #[cfg(not(target_arch = "wasm32"))]
        let root = std::env::temp_dir().join(format!(
            "worldedit-eds11-memory-{}-{}-{serial}-{index}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        #[cfg(target_arch = "wasm32")]
        let root = std::path::PathBuf::from(format!("/eds11-memory/{serial}/{index}"));
        let mut project = Project::new(&root);
        let entry = project.entry.clone();
        project.documents.retain(|path, _| path == &entry);
        project
            .set_text(&entry, source("雾港的晨钟再次响了。"))
            .unwrap();
        let root = project.root.clone();
        project.create_authoring_document(&root.join(".world/project.json"), br#"{
            "schema_version":1,"language_version":"1.10","required_features":["content.entities.v1"],
            "maps":{"harbor":".world/maps/harbor.json"},"graph_views":{}
        }"#.to_vec()).unwrap();
        project.create_authoring_document(&root.join(".world/maps/harbor.json"), br#"{
            "schema_version":1,"id":"harbor","title":"Demo",
            "canvas":{"width":1000,"height":800,"unit":"normalized"},
            "layer_order":["places"],"layers":{"places":{"title":"Places","visible_default":true,"locked":false}},
            "placements":{"p":{"layer_id":"places","target_ref":{"kind":"entity","id":"b"},
                "geometry":{"kind":"point","position":[0.3,0.5]},"annotation":"P","role":"entrance","future":{"keep":true}}}
        }"#.to_vec()).unwrap();
        Self {
            project,
            revision: Revision::default(),
            undo: None,
            undo_from: None,
            applied_commands: 0,
            runtime: None,
            review: None,
        }
    }

    fn map_path(&self) -> std::path::PathBuf {
        self.project.root.join(".world/maps/harbor.json")
    }

    /// 模式开关构造 core 拒绝样例；失败候选不替换当前内存工程。
    fn candidate(&self, read_only: bool, locked: bool) -> Result<Project, String> {
        let mut candidate = self.project.clone();
        if read_only || locked {
            let path = self.map_path();
            let mut map: serde_json::Value =
                serde_json::from_slice(candidate.authoring_document(&path)?.bytes())
                    .map_err(|error| error.to_string())?;
            if read_only {
                map["schema_version"] = 2.into();
            }
            if locked {
                map["layers"]["places"]["locked"] = true.into();
            }
            candidate.set_authoring_document(&path, serde_json::to_vec_pretty(&map).unwrap())?;
        }
        Ok(candidate)
    }

    fn expected_revision(&self, stale: bool) -> Revision {
        if stale {
            Revision {
                content_generation: self.revision.content_generation.wrapping_add(1),
                ..self.revision
            }
        } else {
            self.revision
        }
    }

    pub fn position(&self) -> f32 {
        let index = self.project.map_index();
        match &index.map("harbor").unwrap().placements["p"].geometry {
            MapGeometry::Point { position } => (position[0] * 100.0) as f32,
            _ => unreachable!("built-in demo placement is a point"),
        }
    }

    pub fn apply_position(
        &mut self,
        position: f32,
        read_only: bool,
        stale: bool,
        locked: bool,
    ) -> Result<(), String> {
        let mut candidate = self.candidate(read_only, locked)?;
        let path = self.map_path();
        let mut revision = self.revision;
        let envelope = CommandEnvelope {
            expected_revision: self.expected_revision(stale),
            expected_documents: BTreeMap::from([(
                path.clone(),
                commands::document_hash(candidate.authoring_document(&path)?.bytes()),
            )]),
            command: Command::UpdatePlacement {
                map_id: "harbor".into(),
                placement_id: "p".into(),
                geometry: Some(MapGeometry::Point {
                    position: [f64::from(position) / 100.0, 0.5],
                }),
                target_ref: None,
                annotation: None,
                role: None,
                label_override: None,
                layer_id: None,
            },
        };
        let result = commands::apply(&mut candidate, &mut revision, envelope)
            .map_err(|error| error.to_string())?;
        self.undo_from = Some(self.position());
        self.project = candidate;
        self.revision = revision;
        self.undo = Some(result.undo_record);
        self.applied_commands += 1;
        Ok(())
    }

    pub fn undo_position(
        &mut self,
        read_only: bool,
        stale: bool,
        locked: bool,
    ) -> Result<(), String> {
        let Some(record) = &self.undo else {
            return Ok(());
        };
        let mut candidate = self.candidate(read_only, locked)?;
        let mut revision = self.revision;
        commands::undo(
            &mut candidate,
            &mut revision,
            self.expected_revision(stale),
            record,
        )
        .map_err(|error| error.to_string())?;
        self.project = candidate;
        self.revision = revision;
        self.undo = None;
        self.undo_from = None;
        self.applied_commands += 1;
        Ok(())
    }

    pub fn run_event(&mut self) -> Result<(), String> {
        let compiled = self.project.compile();
        if compiled.has_errors() {
            return Err("core 编译有错误，未启动 runtime".into());
        }
        let mut story = worldline_runtime::Story::new(&compiled.program, &compiled.analysis)
            .map_err(|error| error.to_string())?;
        let output = story.continue_story().map_err(|error| error.to_string())?;
        self.runtime = Some(RuntimeEvidence {
            ended: output
                .iter()
                .any(|output| matches!(output, worldline_runtime::Output::Ended)),
            output: serde_json::to_string_pretty(&output).unwrap(),
            start_visits: story.visits().get("start").copied().unwrap_or_default(),
        });
        Ok(())
    }

    pub fn review_sides(&self) -> [String; 3] {
        [
            source("雾港的晨钟响了。"),
            self.project
                .document(&self.project.entry)
                .unwrap()
                .to_string(),
            source("雾港钟声消失了。"),
        ]
    }

    pub fn compare(&mut self) -> Result<(), String> {
        let sides = self.review_sides();
        let proposal = ProposalDraft {
            id: "prototype-review".into(),
            author: "AI demo".into(),
            reason: "isolated three-way preview".into(),
            status: ProposalStatus::Open,
            changes: vec![ProposalFileChange {
                path: "world.wl".into(),
                domain: "content".into(),
                base: Some(sides[0].clone()),
                proposed: Some(sides[2].clone()),
            }],
        };
        self.review = Some(collaboration::preview_proposal(&self.project, &proposal)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_map_command_undo_preserves_source_identity_and_never_creates_a_workspace() {
        let mut demo = MemoryDemo::new(0);
        let root = demo.project.root.clone();
        let original = demo
            .project
            .authoring_document(&demo.map_path())
            .unwrap()
            .bytes()
            .to_vec();
        let source = demo.project.sources();
        let fingerprint = demo.project.compile().analysis.fingerprint;
        assert!(!root.exists());
        demo.apply_position(72.0, false, false, false).unwrap();
        assert_eq!(demo.position(), 72.0);
        assert_eq!(demo.revision.presentation_generation, 1);
        assert_eq!(demo.applied_commands, 1);
        assert_eq!(demo.project.sources(), source);
        assert_eq!(demo.project.compile().analysis.fingerprint, fingerprint);
        let map: serde_json::Value = serde_json::from_slice(
            demo.project
                .authoring_document(&demo.map_path())
                .unwrap()
                .bytes(),
        )
        .unwrap();
        assert_eq!(map["placements"]["p"]["target_ref"]["id"], "b");
        assert_eq!(map["placements"]["p"]["future"]["keep"], true);
        demo.undo_position(false, false, false).unwrap();
        assert_eq!(
            demo.project
                .authoring_document(&demo.map_path())
                .unwrap()
                .bytes(),
            original
        );
        assert_eq!(demo.revision.presentation_generation, 2);
        assert!(!root.exists());
    }

    #[test]
    fn core_rejects_read_only_stale_and_locked_candidates_atomically() {
        for (read_only, stale, locked) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            let mut demo = MemoryDemo::new(0);
            let baseline = demo.project.content_baseline();
            assert!(demo.apply_position(90.0, read_only, stale, locked).is_err());
            assert_eq!(demo.project.content_baseline(), baseline);
            assert_eq!(demo.position(), 30.0);
            assert_eq!(demo.revision, Revision::default());
            assert!(demo.undo.is_none());
            assert_eq!(demo.applied_commands, 0);
            assert!(!demo.project.root.exists());
        }
    }

    #[test]
    fn runtime_and_three_way_preview_are_real_but_do_not_write_project_buffers() {
        let mut demo = MemoryDemo::new(0);
        let baseline = demo.project.content_baseline();
        demo.run_event().unwrap();
        let run = demo.runtime.as_ref().unwrap();
        assert!(run.ended && run.start_visits == 1);
        assert!(run.output.contains("雾港的晨钟再次响了。"));
        demo.compare().unwrap();
        let preview = demo.review.as_ref().unwrap();
        assert_eq!(preview.files.len(), 1);
        assert_eq!(preview.conflicts.len(), 1);
        assert!(!preview.can_apply());
        assert_eq!(demo.project.content_baseline(), baseline);
        assert_eq!(demo.applied_commands, 0);
        assert!(!demo.project.root.exists());
    }
}
