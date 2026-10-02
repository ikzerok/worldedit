use super::*;
use std::collections::{BTreeMap, VecDeque};
use worldline_core::project::SnapshotDocument;

fn request(task: WorkTask) -> WorkRequest {
    WorkRequest {
        schema_version: SCHEMA_VERSION,
        job_id: "job-1".into(),
        generation: 1,
        baseline: "baseline".into(),
        entry: "world.wl".into(),
        snapshot_state: None,
        task,
    }
}

#[test]
fn typed_render_metadata_is_rejected_before_allocating_binary_copies() {
    let spec = RasterSpec {
        width: 100,
        height: 50,
        zoom: 1.,
        pan: [0., 0.],
        dpi: 1.,
    };
    let req = request(WorkTask::RenderScene {
        scene: MapScene::new(100., 50.),
        extent: [100., 50.],
        spec: spec.clone(),
    });
    assert!(req.validate().is_ok());
    let output = WorkOutput::RenderedScene { spec: spec.clone() };
    assert!(req.accepts_lengths(&output, &[20_000]).is_ok());
    assert!(req.accepts_lengths(&output, &[MAX_BYTES * 2]).is_err());
    assert!(req.accepts_lengths(&output, &[]).is_err());
    assert!(req
        .accepts_lengths(
            &WorkOutput::MapSvgExport {
                source: "<svg/>".into()
            },
            &[]
        )
        .is_err());
    let mut changed = spec;
    changed.dpi = 2.;
    assert!(req
        .accepts_lengths(&WorkOutput::RenderedScene { spec: changed }, &[20_000])
        .is_err());
    assert!(req.accepts(&output, &[vec![0; 20_000]]).is_ok());
}

#[test]
fn normalized_scene_result_preserves_exact_non_import_intent() {
    let batch = SceneBatch {
        map_id: "atlas".into(),
        expected_revision: Revision::default(),
        expected_documents: BTreeMap::new(),
        operations: vec![
            SceneOp::EnableScene,
            SceneOp::ImportSvg {
                layer_id: "svg".into(),
                title: "图".into(),
                source: "<svg/>".into(),
            },
        ],
    };
    let req = request(WorkTask::ScenePreview {
        revision: Revision::default(),
        batch: batch.clone(),
    });
    let mut normalized = batch;
    normalized.operations[1] = SceneOp::ImportScene {
        layer_id: "svg".into(),
        title: "图".into(),
        scene: MapScene::new(10., 10.),
        width: 10.,
        height: 10.,
    };
    assert!(req
        .accepts(
            &WorkOutput::ScenePreview {
                batch: normalized.clone()
            },
            &[]
        )
        .is_ok());
    normalized.operations[0] = SceneOp::Delete {
        node_ids: vec!["private".into()],
    };
    assert!(req
        .accepts(
            &WorkOutput::ScenePreview {
                batch: normalized.clone()
            },
            &[]
        )
        .is_err());
    normalized.operations[0] = SceneOp::EnableScene;
    normalized.map_id = "different".into();
    assert!(req
        .accepts(&WorkOutput::ScenePreview { batch: normalized }, &[])
        .is_err());
}

#[test]
fn tombstones_roundtrip_as_binary_and_cannot_alias_active_records() {
    let mut req = request(WorkTask::MapSvgExport {
        map_id: "atlas".into(),
        selected: None,
    });
    assert!(req.validate().is_err());
    req.snapshot_state = Some(SnapshotState {
        schema_version: 1,
        documents: vec![SnapshotDocument {
            path: ".world/deleted.json".into(),
            authoring: true,
            deleted: true,
            read_only: true,
            retained_bytes: Some(vec![0, 255, 1]),
        }],
    });
    assert!(req.validate().is_ok());
    let retained = detach_retained(&mut req);
    assert!(!serde_json::to_string(&req)
        .unwrap()
        .contains("retained_bytes"));
    let detached = req.clone();
    assert!(restore_retained(&mut req, retained.clone()).is_ok());
    assert_eq!(
        req.snapshot_state.as_ref().unwrap().documents[0].retained_bytes,
        Some(vec![0, 255, 1])
    );
    assert!(restore_retained(&mut req, retained).is_err());
    let mut missing = detached.clone();
    assert!(restore_retained(&mut missing, vec![]).is_err());
    let mut wrong = detached;
    assert!(restore_retained(&mut wrong, vec![("other.json".into(), vec![])]).is_err());
}

#[test]
fn progress_is_constant_space_and_terminal_does_not_wait_behind_history() {
    let mut queue = VecDeque::new();
    for index in 0..10_000 {
        assert!(!push_event(
            &mut queue,
            WorkEvent::Progress {
                stage: "compile".into(),
                completed: index,
                total: 10_000
            }
        ));
        assert_eq!(queue.len(), 1);
    }
    match queue.pop_front().unwrap() {
        WorkEvent::Progress {
            stage,
            completed,
            total,
        } => {
            assert_eq!(stage, "compile");
            assert_eq!(completed, 9999);
            assert_eq!(total, 10_000);
        }
        _ => panic!("expected progress"),
    }
    assert!(push_event(
        &mut queue,
        WorkEvent::Done {
            output: Box::new(WorkOutput::MapSvgExport {
                source: "<svg/>".into()
            }),
            binaries: vec![]
        }
    ));
    assert!(!push_event(
        &mut queue,
        WorkEvent::Progress {
            stage: "late".into(),
            completed: 0,
            total: 1
        }
    ));
    match queue.pop_front().unwrap() {
        WorkEvent::Done { output, binaries } => {
            let WorkOutput::MapSvgExport { source } = *output else {
                panic!("wrong output")
            };
            assert_eq!(source, "<svg/>");
            assert!(binaries.is_empty());
        }
        _ => panic!("late progress replaced terminal"),
    }
    assert!(push_event(
        &mut queue,
        WorkEvent::Error("disconnected".into())
    ));
    match queue.pop_front().unwrap() {
        WorkEvent::Error(message) => assert_eq!(message, "disconnected"),
        _ => panic!("expected error"),
    }
}

#[test]
fn job_schema_snapshot_type_and_javascript_generation_are_explicit() {
    let mut req = request(WorkTask::SvgPreview {
        source: "<svg/>".into(),
    });
    assert!(req.validate().is_ok());
    req.schema_version = 99;
    assert!(req.validate().is_err());
    req.schema_version = SCHEMA_VERSION;
    req.generation = u64::MAX;
    assert!(req.validate().is_err());
}

#[test]
fn scene_error_text_keeps_xml_position_and_field_context() {
    let mut error =
        worldline_core::vector_scene::SceneError::new("SCENE_SVG_PROFILE", "不支持的字段")
            .at("curve", "href");
    error.line = Some(17);
    error.column = Some(9);
    error.operation_index = Some(2);
    let message = scene_error(&error);
    for part in [
        "SCENE_SVG_PROFILE",
        "行17",
        "列9",
        "curve",
        "href",
        "操作#2",
    ] {
        assert!(message.contains(part));
    }
}

#[test]
fn reader_profile_plan_keeps_every_original_route_field_and_known_scope() {
    use worldline_core::reader_export::ReaderProfileRoute;
    let selection = ReaderExportSelection {
        schema_version: 3,
        required_features: vec!["reader.world_site.v1".into()],
        site_title: "公开站点".into(),
        objects: vec![],
        fields: vec![],
        maps: vec![],
        manuscripts: vec![],
        attachments: vec![],
    };
    let original = ReaderPublicationProfile {
        schema_version: 1,
        required_features: vec!["reader.profiles.v1".into(), "future.required.v99".into()],
        id: "public".into(),
        title: "公开配置".into(),
        selection: selection.clone(),
        routes: vec![ReaderProfileRoute {
            target: Some(worldline_core::catalog::TargetRef::new("entity", "a")),
            manuscript_id: None,
            chapter_id: None,
            output_path: "objects/o0001.html".into(),
        }],
    };
    let req = request(WorkTask::ReaderProfileSavePlan {
        selection,
        profile: Some(original.clone()),
        id: original.id.clone(),
        title: original.title.clone(),
    });
    let mut plan = ReaderProfileSavePlan {
        profile: original.clone(),
        content_baseline: "baseline".into(),
        document_path: ".world/reader-profiles/public.json".into(),
        document_before_hash: None,
        plan_digest: "digest".into(),
    };
    plan.profile.routes.push(ReaderProfileRoute {
        target: Some(worldline_core::catalog::TargetRef::new("entity", "b")),
        manuscript_id: None,
        chapter_id: None,
        output_path: "objects/o0002.html".into(),
    });
    assert!(req
        .accepts_lengths(
            &WorkOutput::ReaderProfileSavePlan { plan: plan.clone() },
            &[]
        )
        .is_ok());
    assert!(req
        .accepts_lengths(
            &WorkOutput::ReaderProfileSavePlan { plan: plan.clone() },
            &[0]
        )
        .is_err());
    for changed in [
        ReaderProfileRoute {
            output_path: "objects/o0003.html".into(),
            ..original.routes[0].clone()
        },
        ReaderProfileRoute {
            target: Some(worldline_core::catalog::TargetRef::new("entity", "other")),
            ..original.routes[0].clone()
        },
        ReaderProfileRoute {
            manuscript_id: Some("unexpected".into()),
            ..original.routes[0].clone()
        },
        ReaderProfileRoute {
            chapter_id: Some("unexpected".into()),
            ..original.routes[0].clone()
        },
    ] {
        let mut changed_plan = plan.clone();
        changed_plan.profile.routes[0] = changed;
        assert!(req
            .accepts_lengths(
                &WorkOutput::ReaderProfileSavePlan { plan: changed_plan },
                &[]
            )
            .is_err());
    }
    plan.profile.required_features.pop();
    assert!(req
        .accepts_lengths(&WorkOutput::ReaderProfileSavePlan { plan }, &[])
        .is_err());
}
