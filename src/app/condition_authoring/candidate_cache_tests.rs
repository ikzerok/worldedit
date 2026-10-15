use super::*;

fn project() -> Project {
    let root = std::env::temp_dir().join(format!(
        "condition-candidate-cache-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut project = Project::new(&root);
    let entry = project.entry.clone();
    project.documents.retain(|path, _| path == &entry);
    project
        .set_text(&entry, "tag calm\nevent start\n  -> END\n".into())
        .unwrap();
    project.save().unwrap();
    project
}

fn context<'a>(project: &'a Project, compiled: &'a CompileResult) -> Context<'a> {
    Context {
        project,
        compiled,
        version: 7,
        baseline: "unused-form-baseline",
        current_baseline: project.content_baseline(),
        draft_signature: String::new(),
        composing: false,
    }
}

#[test]
fn condition_ui_candidate_cache_does_not_reuse_same_bytes_from_another_root_or_compile() {
    let mut first = project();
    let mut second = project();
    assert_ne!(first.root, second.root);
    assert_eq!(first.content_baseline(), second.content_baseline());
    let first_compile = first.compile();
    let second_compile = second.compile();
    let mut picker = Picker::default();
    let a = picker
        .page(
            &context(&first, &first_compile),
            ConditionCandidateKind::Tag,
        )
        .unwrap();
    let b = picker
        .page(
            &context(&second, &second_compile),
            ConditionCandidateKind::Tag,
        )
        .unwrap();
    assert_eq!(a.total, b.total);
    assert_eq!(a.items[0].target, b.items[0].target);
    assert!(std::path::Path::new(&a.items[0].file).starts_with(&first.root));
    assert!(std::path::Path::new(&b.items[0].file).starts_with(&second.root));
    assert_ne!(a.items[0].file, b.items[0].file);

    // 同 root/版本/字节，但传入另一个工程的旧 compile：不能借已缓存页面绕过 core guard。
    let error = picker
        .page(
            &context(&second, &first_compile),
            ConditionCandidateKind::Tag,
        )
        .unwrap_err();
    assert!(error.contains("快照已过期"), "{error}");
    assert!(picker.cache.is_none());
    let restored = picker
        .page(
            &context(&second, &second_compile),
            ConditionCandidateKind::Tag,
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::to_value(b).unwrap()
    );
    std::fs::remove_dir_all(&first.root).unwrap();
    std::fs::remove_dir_all(&second.root).unwrap();
}

#[test]
fn condition_ui_candidate_cache_rechecks_changed_author_bytes_and_retains_core_error() {
    let mut project = project();
    let compiled = project.compile();
    let mut picker = Picker::default();
    picker
        .page(&context(&project, &compiled), ConditionCandidateKind::Tag)
        .unwrap();
    let entry = project.entry.clone();
    project
        .set_text(&entry, "tag alert\nevent start\n  -> END\n".into())
        .unwrap();
    let error = picker
        .page(&context(&project, &compiled), ConditionCandidateKind::Tag)
        .unwrap_err();
    assert!(error.contains("快照已过期"), "{error}");
    assert!(picker.cache.is_none());
    std::fs::remove_dir_all(&project.root).unwrap();
}
