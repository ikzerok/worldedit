//! Real worker engine and native lifecycle coverage; browser delivery is verified separately.
use super::{engine::Engine, protocol::*};
use worldline_core::{
    draft_rehearsal::DraftRehearsalRequest,
    localization::{
        LocalizationPart, LocalizationPresentationPolicy, LocalizationPresentationRequest,
        LocalizationSelection, LocalizationStatus,
    },
    project::Project,
};
use worldline_runtime::{ReplayBudget, ReplayCancellation};

const SOURCE: &str = "event start\n  Hello {rnd(1, 1000)} / {rnd(1, 1000)} #wl-localization:body\n  choice \"Continue\" #wl-localization:go\n    -> END\n";
fn fixture(changed: bool, fallback: bool) -> (Project, Prepare) {
    let root = std::env::temp_dir().join(format!(
        "worker-locale-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut project = Project::new(&root);
    project
        .set_text(&project.entry.clone(), SOURCE.into())
        .unwrap();
    project
        .create_authoring_document(
            &root.join(".world/project.json"),
            br#"{
        "schema_version":1,"language_version":"1.10","required_features":["content.localization.v1"]
    }"#
            .to_vec(),
        )
        .unwrap();
    let selection = LocalizationSelection {
        schema_version: 1,
        source_locale: "en".into(),
        target_locale: "zh-Hant".into(),
        string_ids: vec!["body".into(), "go".into()],
    };
    let mut exchange = project
        .preview_localization_export(&selection)
        .unwrap()
        .exchange;
    for entry in &mut exchange.entries {
        entry.translation_parts = Some(if entry.id == "body" {
            vec![
                LocalizationPart::Text {
                    text: "译 ".into()
                },
                LocalizationPart::Placeholder { token: "p1".into() },
                LocalizationPart::Text { text: " / ".into() },
                LocalizationPart::Placeholder { token: "p0".into() },
            ]
        } else {
            vec![LocalizationPart::Text {
                text: "继续 👋".into(),
            }]
        });
    }
    let plan = project
        .preview_localization_import_candidate(&selection, &exchange)
        .unwrap();
    assert!(plan.can_apply, "{:?}", plan.diagnostics);
    project
        .apply_localization_import_candidate(&selection, &exchange, &plan.plan_digest)
        .unwrap();
    let mut buffer = project.open_source_writing_buffer(&project.entry).unwrap();
    buffer.replace_source(if changed {
        SOURCE.replace("Hello", "Changed")
    } else {
        format!("{SOURCE}// 本次保留的正文草稿注释\n")
    });
    let input =
        DraftRehearsalRequest::from_writing_buffers(&project, &[buffer], vec![], false).unwrap();
    let prepare = Prepare {
        schema_version: VERSION,
        session_id: "locale-test".into(),
        request_id: "0".into(),
        input,
        entry: "world.wl".into(),
        snapshot_state: project.snapshot_state().unwrap(),
        presentation: Some(LocalizationPresentationRequest {
            schema_version: 1,
            target_locale: "zh-Hant".into(),
            policy: if fallback {
                LocalizationPresentationPolicy::SourceFallback
            } else {
                LocalizationPresentationPolicy::Strict
            },
        }),
    };
    (project, prepare)
}
fn command(request: u64, action: Action) -> Command {
    Command {
        schema_version: VERSION,
        session_id: "locale-test".into(),
        request_id: request.to_string(),
        action,
    }
}
fn start() -> Action {
    Action::Start {
        seed: 99,
        budget: ReplayBudget::new(100_000, 100),
    }
}

#[test]
fn locale_worker_response_roundtrip_keeps_same_eval_source_and_choice_identity() {
    let (project, prepare) = fixture(false, false);
    let baseline = project.content_baseline();
    let mut engine = Engine::prepare(&project, &prepare).unwrap();
    let response = engine.execute(command(1, start()), &ReplayCancellation::new());
    assert!(response.error.is_none(), "{:?}", response.error);
    let response: Response = decode(&encode(&response).unwrap(), MAX_RESPONSE).unwrap();
    let view = response.view.unwrap();
    assert_eq!(view.presentation.unwrap().request.target_locale, "zh-Hant");
    assert_eq!(view.outputs.len(), 1);
    let output = &view.outputs[0];
    let source = output.localization.as_ref().unwrap();
    assert!(source.source_links.is_empty());
    assert!(output.links.is_empty());
    assert_eq!(source.status, LocalizationStatus::Translated);
    let values: Vec<_> = source
        .source_content
        .trim_start_matches("Hello ")
        .split(" / ")
        .collect();
    assert_eq!(output.content, format!("译 {} / {}", values[1], values[0]));
    assert_eq!(view.choices[0].label, "继续 👋");
    let duplicate = engine.execute(command(1, start()), &ReplayCancellation::new());
    assert!(duplicate.error.is_some());
    assert_eq!(project.content_baseline(), baseline);
    assert!(!project.root.exists());
}

#[test]
fn locale_worker_strict_changed_draft_fails_before_execution_and_fallback_can_return_exact_source()
{
    let (project, mut prepare) = fixture(true, false);
    let baseline = project.content_baseline();
    assert!(Engine::prepare(&project, &prepare).is_err());
    prepare.presentation.as_mut().unwrap().policy = LocalizationPresentationPolicy::SourceFallback;
    let mut engine = Engine::prepare(&project, &prepare).unwrap();
    let response = engine.execute(command(1, start()), &ReplayCancellation::new());
    let view = response.view.unwrap();
    let metadata = view.outputs[0].localization.as_ref().unwrap();
    assert_eq!(metadata.status, LocalizationStatus::StaleSource);
    assert!(view.outputs[0].content.starts_with("Changed "));
    let located = engine.execute(
        command(
            2,
            Action::LocalizationSource {
                source: metadata.source.clone(),
            },
        ),
        &ReplayCancellation::new(),
    );
    assert!(located.error.is_none(), "{:?}", located.error);
    let hit = located.source.unwrap();
    assert!(hit.draft);
    assert_eq!(hit.line, 2);
    assert!(hit.preview.starts_with("Changed "));
    assert_eq!(
        prepare.input.drafts[0].source.get(hit.range.clone()),
        Some(hit.preview.as_str())
    );
    let mut forged = metadata.source.clone();
    forged.line = 99;
    assert!(engine
        .execute(
            command(3, Action::LocalizationSource { source: forged }),
            &ReplayCancellation::new()
        )
        .error
        .is_some());
    assert_eq!(project.content_baseline(), baseline);
    assert!(!project.root.exists());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn locale_native_worker_cancel_reopen_drops_old_locale_and_repeated_start_cannot_advance() {
    let (project, prepare) = fixture(false, false);
    let ctx = egui::Context::default();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let create = |id: &str, presentation: Option<LocalizationPresentationRequest>| loop {
        match super::SessionWorker::start_with_presentation(
            &project,
            &prepare.input,
            id.into(),
            &ctx,
            presentation.clone(),
        ) {
            Ok(worker) => break worker,
            Err(error) if error.contains("上一份隔离编译仍在退出") => {
                assert!(std::time::Instant::now() < end);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(error) => panic!("{error}"),
        }
    };
    let old = create("old-zh", prepare.presentation.clone());
    drop(old);
    let mut current = create("new-source", None);
    let prepared = loop {
        if let Some(result) = current.poll() {
            break result.unwrap();
        }
        assert!(std::time::Instant::now() < end);
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert!(prepared.view.unwrap().presentation.is_none());
    current.submit(start()).unwrap();
    assert!(current.submit(start()).is_err());
    let response = loop {
        if let Some(result) = current.poll() {
            break result.unwrap();
        }
        assert!(std::time::Instant::now() < end);
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    let view = response.view.unwrap();
    assert!(view.presentation.is_none());
    assert!(view.outputs[0].content.starts_with("Hello "));
    assert!(view.outputs[0].localization.is_none());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn locale_rehearsal_budget_preflight_rejects_host_and_engine_before_worker() {
    use worldline_core::localization::MAX_LOCALIZATION_TRACKED_FILES;
    let (mut project, mut prepare) = fixture(false, false);
    let mut document = project.documents[&project.entry].clone();
    document.text = "// private tracked-budget fixture\n".into();
    while project.documents.len() + project.authoring_documents.len()
        <= MAX_LOCALIZATION_TRACKED_FILES
    {
        let index = project.documents.len();
        project.documents.insert(
            project.root.join(format!("tracked_{index}.wl")),
            document.clone(),
        );
    }
    assert_eq!(
        project.documents.len() + project.authoring_documents.len(),
        4097
    );
    let mut buffer = project.open_source_writing_buffer(&project.entry).unwrap();
    buffer.replace_source(format!("{SOURCE}// real unchanged-semantics draft\n"));
    prepare.input =
        DraftRehearsalRequest::from_writing_buffers(&project, &[buffer], vec![], false).unwrap();
    prepare.snapshot_state = project.snapshot_state().unwrap();
    let baseline = project.content_baseline();
    let expected = project.check_localization_budget().unwrap_err();
    assert_eq!(expected.code, "BUDGET_EXCEEDED");
    let ctx = egui::Context::default();
    let rejected = super::SessionWorker::start_with_presentation(
        &project,
        &prepare.input,
        "over-budget".into(),
        &ctx,
        prepare.presentation.clone(),
    );
    assert_eq!(
        rejected
            .err()
            .expect("locale host must not create a worker"),
        expected.message
    );
    let mut invalid = prepare.clone();
    invalid.input.drafts.clear();
    assert_eq!(
        Engine::prepare(&project, &invalid)
            .err()
            .expect("preflight precedes draft compilation"),
        expected.message
    );
    prepare.presentation = None;
    let original_error = project
        .compile_draft_rehearsal(&prepare.input)
        .err()
        .expect("existing source organization budget remains authoritative");
    assert_ne!(original_error, expected.message);
    assert_eq!(
        Engine::prepare(&project, &prepare)
            .err()
            .expect("source-only keeps its existing guard"),
        original_error
    );
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut source_worker = loop {
        match super::SessionWorker::start(&project, &prepare.input, "source-unchanged".into(), &ctx)
        {
            Ok(worker) => break worker,
            Err(error) if error.contains("上一份隔离编译仍在退出") => {
                assert!(std::time::Instant::now() < end);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(error) => panic!("source-only host changed behavior: {error}"),
        }
    };
    let response = loop {
        if let Some(response) = source_worker.poll() {
            break response;
        }
        assert!(std::time::Instant::now() < end);
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(
        response
            .err()
            .expect("source-only worker reports the original core error"),
        original_error
    );
    drop(source_worker);
    assert_eq!(project.content_baseline(), baseline);
    assert!(!project.root.exists());
    let (small, mut small_prepare) = fixture(false, false);
    small_prepare.presentation = None;
    assert!(Engine::prepare(&small, &small_prepare)
        .unwrap()
        .prepared()
        .view
        .unwrap()
        .presentation
        .is_none());
    let mut worker = loop {
        match super::SessionWorker::start(&small, &small_prepare.input, "source-valid".into(), &ctx)
        {
            Ok(worker) => break worker,
            Err(error) if error.contains("上一份隔离编译仍在退出") => {
                assert!(std::time::Instant::now() < end);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(error) => panic!("valid source-only host failed: {error}"),
        }
    };
    let prepared = loop {
        if let Some(response) = worker.poll() {
            break response.unwrap();
        }
        assert!(std::time::Instant::now() < end);
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert!(prepared.view.unwrap().presentation.is_none());
    assert!(!small.root.exists());
}
