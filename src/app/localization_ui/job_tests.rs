use super::workbench_tests::{fixture, stage};
use super::*;
use crate::localization_job::{Job, Output, Task};

#[test]
fn localization_jobs_latest_query_wins_and_cancel_keeps_last_stable_page() {
    let (project, mut state) = fixture(85);
    let ctx = egui::Context::default();
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    assert_eq!(state.workbench.page.as_ref().unwrap().total, 85);
    state.workbench.search = "源文 8".into();
    state.workbench.invalidate();
    catalog::refresh(&project, &mut state, 1);
    jobs::pump(&ctx, &project, &mut state, 1);
    state.workbench.search = "源文 84".into();
    state.workbench.invalidate();
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    assert_eq!(state.workbench.page.as_ref().unwrap().total, 1);
    assert_eq!(
        state.workbench.page.as_ref().unwrap().entries[0]
            .id
            .as_deref(),
        Some("line84")
    );
    state.workbench.search = String::new();
    state.workbench.invalidate();
    catalog::refresh(&project, &mut state, 1);
    jobs::pump(&ctx, &project, &mut state, 1);
    state.jobs.cancel();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !Job::available() {
        assert!(std::time::Instant::now() < end);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    jobs::pump(&ctx, &project, &mut state, 1);
    assert_eq!(state.workbench.page.as_ref().unwrap().total, 1);
    assert!(!state.jobs.pending());
    assert!(!project.root.exists());
}

#[test]
fn localization_jobs_workspace_version_switch_and_changed_input_reject_old_results() {
    let (project, mut state) = fixture(85);
    let ctx = egui::Context::default();
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    stage(&mut state, "未提交译文");
    plans::preview_edits(&project, &mut state);
    jobs::pump(&ctx, &project, &mut state, 1);
    state.cancel_preview();
    jobs::settle(&project, &mut state, 1);
    assert!(state.workbench.preview.is_none());
    assert_eq!(state.workbench.drafts.len(), 1);
    state.workbench.invalidate();
    catalog::refresh(&project, &mut state, 1);
    jobs::pump(&ctx, &project, &mut state, 1);
    let (other, _) = fixture(2);
    jobs::pump(&ctx, &other, &mut state, 2);
    catalog::refresh(&other, &mut state, 2);
    jobs::settle(&other, &mut state, 2);
    assert_eq!(state.workbench.page.as_ref().unwrap().all_total, 2);
    assert_eq!(state.workbench.drafts.len(), 1);
    assert!(state.workbench.preview.is_none());
}

#[test]
fn localization_task_rejects_wrong_kind_baseline_and_locale_without_reparsing_rules() {
    let (project, _) = fixture(2);
    let task = Task::Catalog {
        query: worldline_core::localization::LocalizationCatalogQuery {
            target_locale: Some("zh-Hant".into()),
            ..Default::default()
        },
    };
    let output = task.run(&project).unwrap();
    assert!(task.accepts(&output, &project.content_baseline()).is_ok());
    assert!(task.accepts(&output, "old").is_err());
    let Output::Catalog { mut page } = output else {
        panic!("catalog output")
    };
    page.target_locale = Some("different".into());
    assert!(task
        .accepts(&Output::Catalog { page }, &project.content_baseline())
        .is_err());
}

#[test]
fn localization_oversized_paste_is_retained_but_never_parsed_or_queued() {
    let (project, mut state) = fixture(1);
    state.exchange_json = " ".repeat(MAX_LOCALIZATION_JSON_BYTES + 1);
    exchange::preview_import(&project, &mut state);
    assert!(!state.jobs.pending());
    assert!(state.import_plan.is_none());
    assert_eq!(state.exchange_json.len(), MAX_LOCALIZATION_JSON_BYTES + 1);
    assert!(state.status.as_ref().is_some_and(Result::is_err));
    assert!(!project.root.exists());
}

#[test]
fn localization_runtime_return_is_exact_and_blocks_unapplied_source_revision() {
    let (project, mut state) = fixture(85);
    state.open_translation("zh-Hant", Some("line84"));
    state.expect_runtime_revision("different-draft-revision".into());
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    assert_eq!(
        state.workbench.selected_entry().unwrap().id.as_deref(),
        Some("line84")
    );
    assert!(state.workbench.runtime_revision_mismatch);
    assert!(state.workbench.drafts.is_empty());
    assert!(!project.root.exists());
}
