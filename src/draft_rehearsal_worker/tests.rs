//! 真正引擎/线程生命周期回归；浏览器同源加载另由发行宿主验证。
use super::{engine::Engine, protocol::*};
use worldline_core::{draft_rehearsal::DraftRehearsalRequest, project::Project};
use worldline_runtime::{ReplayBudget, ReplayCancellation, StateInspectionQuery};

fn fixture() -> (Project, Prepare) {
    let root = std::env::temp_dir().join(format!(
        "editor-rehearsal-worker-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut project = Project::new(&root);
    project
        .set_text(
            &project.entry.clone(),
            "let n = 0\nevent start\n  旧稿\n  -> END\n".into(),
        )
        .unwrap();
    let mut buffer = project.open_source_writing_buffer(&project.entry).unwrap();
    buffer.replace_source(
        "let n = 0\nevent start\n  新稿\n  choice \"继续\"\n    set n = 4\n    -> END\n".into(),
    );
    let input =
        DraftRehearsalRequest::from_writing_buffers(&project, &[buffer], vec![], false).unwrap();
    let prepare = Prepare {
        schema_version: VERSION,
        session_id: "test".into(),
        request_id: "0".into(),
        input,
        entry: "world.wl".into(),
        snapshot_state: project.snapshot_state().unwrap(),
    };
    (project, prepare)
}
fn command(id: u64, action: Action) -> Command {
    Command {
        schema_version: VERSION,
        session_id: "test".into(),
        request_id: id.to_string(),
        action,
    }
}

#[test]
fn prepare_confirm_choose_inspect_and_source_keep_the_same_runtime() {
    let (project, prepare) = fixture();
    let baseline = project.content_baseline();
    let mut engine = Engine::prepare(&project, &prepare).unwrap();
    let prepared = engine.prepared();
    assert!(prepared.view.unwrap().seed.is_none());
    let token = ReplayCancellation::new();
    let first = engine.execute(
        command(
            1,
            Action::Start {
                seed: 3,
                budget: ReplayBudget::new(100, 250),
            },
        ),
        &token,
    );
    assert!(first.error.is_none());
    let view = first.view.unwrap();
    assert_eq!(view.outputs[0].content, "新稿");
    let run = view.inspection.as_ref().unwrap().stamp.run_id;
    let source = view.inspection.unwrap().items[0].source.clone().unwrap();
    let response = engine.execute(command(2, Action::DeclarationSource { source }), &token);
    assert_eq!(
        response.source.unwrap().path,
        std::path::PathBuf::from("world.wl")
    );
    let chosen = engine.execute(
        command(
            3,
            Action::Choose {
                id: view.choices[0].id.clone(),
                budget: ReplayBudget::new(100, 250),
            },
        ),
        &token,
    );
    assert!(chosen.error.is_none());
    assert!(chosen.view.unwrap().ended);
    let inspected = engine.execute(
        command(
            4,
            Action::Inspect {
                query: StateInspectionQuery::default(),
            },
        ),
        &token,
    );
    let page = inspected.view.unwrap().inspection.unwrap();
    assert_eq!(page.stamp.run_id, run);
    assert_eq!(page.items[0].current.display, "4");
    assert_eq!(project.content_baseline(), baseline);
}

#[test]
fn protocol_rejects_stale_foreign_duplicate_and_unknown_fields() {
    let (project, prepare) = fixture();
    let mut engine = Engine::prepare(&project, &prepare).unwrap();
    let token = ReplayCancellation::new();
    let start = command(
        1,
        Action::Start {
            seed: 1,
            budget: ReplayBudget::new(100, 250),
        },
    );
    let mut foreign = start.clone();
    foreign.session_id = "wrong".into();
    assert!(engine.execute(foreign, &token).error.is_some());
    assert!(engine.execute(start.clone(), &token).error.is_none());
    assert!(engine.execute(start, &token).error.is_some());
    assert!(decode::<Command>(r#"{"schema_version":1,"session_id":"test","request_id":"2","action":{"action":"continue","budget":{"max_steps":100,"time_budget_ms":250}},"extra":true}"#,4096).is_err());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_single_compile_permit_cancel_wait_and_reopen_release_resources() {
    let (project, prepare) = fixture();
    let context = egui::Context::default();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !super::SessionWorker::available() {
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let mut first = loop {
        match super::SessionWorker::start(&project, &prepare.input, "first".into(), &context) {
            Ok(worker) => break worker,
            Err(error) if error.contains("上一份隔离编译") => {
                assert!(std::time::Instant::now() < until);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(error) => panic!("{error}"),
        }
    };
    assert!(first.busy());
    assert!(first
        .submit(Action::Continue {
            budget: ReplayBudget::new(1, 1)
        })
        .is_err());
    drop(first);
    while !super::SessionWorker::available() {
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let mut reopened = loop {
        match super::SessionWorker::start(&project, &prepare.input, "reopened".into(), &context) {
            Ok(worker) => break worker,
            Err(error) if error.contains("上一份隔离编译") => {
                assert!(std::time::Instant::now() < until);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(error) => panic!("{error}"),
        }
    };
    loop {
        if let Some(response) = reopened.poll() {
            let response = response.unwrap();
            assert_eq!(response.session_id, "reopened");
            assert_eq!(response.request_id, "0");
            break;
        }
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

fn assert_initialization_failure_keeps_no_live_story(source: &str, expected: &str) {
    let (project, mut prepare) = fixture();
    prepare.input.drafts[0].source = source.into();
    let original_sources = project.sources();
    let baseline = project.content_baseline();
    let mut engine = Engine::prepare(&project, &prepare).unwrap();
    assert!(engine.prepared().view.unwrap().error.is_none());
    let token = ReplayCancellation::new();
    let response = engine.execute(
        command(
            1,
            Action::Start {
                seed: 17,
                budget: ReplayBudget::new(100, 250),
            },
        ),
        &token,
    );
    let error = response.error.as_ref().expect("必须报告真实初始化错误");
    assert!(error.contains(expected), "{error}");
    let view = response.view.unwrap();
    assert_eq!(view.error.as_ref(), Some(error), "宿主必须看到停止错误态");
    assert!(view.inspection.is_none());
    assert!(view.outputs.is_empty());
    assert!(view.choices.is_empty());
    assert!(view.conditions.is_empty());
    assert!(view.node.is_none());
    assert_eq!(view.turns, 0);
    let retry = engine.execute(
        command(
            2,
            Action::Continue {
                budget: ReplayBudget::new(100, 250),
            },
        ),
        &token,
    );
    assert!(retry.error.is_some(), "Continue不能偷偷创建或替换Story");
    let retry_view = retry.view.unwrap();
    assert_eq!(retry_view.error.as_ref(), Some(error));
    assert!(retry_view.outputs.is_empty());
    assert!(retry_view.inspection.is_none());
    assert_eq!(retry_view.turns, 0);
    assert_eq!(project.content_baseline(), baseline);
    assert_eq!(project.sources(), original_sources);
}

#[test]
fn empty_program_start_reports_a_stopped_error_without_fallback_story() {
    assert_initialization_failure_keeps_no_live_story("", "没有可运行入口");
}

#[test]
fn failed_global_initializer_reports_a_stopped_error_without_fake_state() {
    assert_initialization_failure_keeps_no_live_story(
        "let divisor = 0\nlet bad = 1 / divisor\nevent start\n  尚未执行\n  -> END\n",
        "初始化变量 `bad` 失败",
    );
}

#[test]
fn actual_runtime_failure_clears_old_choice_outcome_but_rejected_requests_do_not() {
    let (project, mut prepare) = fixture();
    prepare.input.drafts[0].source = concat!(
        "let divisor = 0\nevent start\n  choice \"除零分支\"\n",
        "    当前计算 {1 / divisor}\n    -> END\n",
    )
    .into();
    let mut engine = Engine::prepare(&project, &prepare).unwrap();
    let token = ReplayCancellation::new();
    let budget = ReplayBudget::new(100, 250);
    let first = engine.execute(command(1, Action::Start { seed: 17, budget }), &token);
    assert!(first.error.is_none());
    let first_view = first.view.unwrap();
    assert_eq!(
        first_view.outcome,
        Some(worldline_runtime::ContinuationOutcome::Choice)
    );
    let stamp = first_view.inspection.unwrap().stamp;
    let actual_id = first_view.choices[0].id.clone();
    let mut foreign = command(
        2,
        Action::Choose {
            id: actual_id.clone(),
            budget,
        },
    );
    foreign.session_id = "foreign-session".into();
    let rejected = engine.execute(foreign, &token);
    assert!(rejected.error.is_some());
    assert_eq!(
        rejected.view.unwrap().outcome,
        Some(worldline_runtime::ContinuationOutcome::Choice)
    );
    let rejected = engine.execute(
        command(
            2,
            Action::Choose {
                id: "not-a-real-choice".into(),
                budget,
            },
        ),
        &token,
    );
    assert!(rejected.error.is_some());
    let rejected_view = rejected.view.unwrap();
    assert_eq!(
        rejected_view.outcome,
        Some(worldline_runtime::ContinuationOutcome::Choice)
    );
    assert_eq!(rejected_view.inspection.unwrap().stamp, stamp);
    assert!(rejected_view.error.is_none());
    let failed = engine.execute(
        command(
            3,
            Action::Choose {
                id: actual_id,
                budget,
            },
        ),
        &token,
    );
    assert!(failed.error.is_some());
    let view = failed.view.unwrap();
    assert!(view.error.as_ref().unwrap().contains("除以零"));
    assert!(
        view.outcome.is_none(),
        "真实运行已失败，不能继承旧的等待选择状态"
    );
    assert_eq!(
        view.inspection.unwrap().status,
        worldline_runtime::InspectionStatus::Failed
    );
}
