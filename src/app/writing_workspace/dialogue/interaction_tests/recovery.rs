//! 来源失效时的恢复字段与输入法所有权。
use super::*;

#[test]
fn dialogue_unavailable_recovery_is_current_key_read_only_and_uses_shared_cancel() {
    let (mut project, _, target) = tests::fixture();
    let path = project.entry.clone();
    let source = format!(
        "{}\nevent other\n  say b \"其他正文\"\n  -> END\n",
        project.document(&path).unwrap()
    );
    project.set_text(&path, source).unwrap();
    project.save().unwrap();
    let buffer = project.open_writing_buffer(&target).unwrap();
    let other = TargetRef::new("event", "other");
    let ctx = egui::Context::default();
    let mut view = tests::open_form(&project, &buffer, &target);
    let key = Key::new(&buffer, &target);
    let other_projection = project.project_dialogue_buffer(&buffer, &other).unwrap();
    let statement = &other_projection.statements[0];
    view.dialogue.begin_context(
        &ctx,
        &buffer,
        &other_projection,
        DialogueOperation::Update {
            statement_id: statement.id.clone(),
            draft: statement.draft.clone(),
        },
        "其他目标".into(),
    );
    for (key, form) in &mut view.dialogue.forms {
        if let DialogueOperation::Update { draft, .. } = &mut form.request.operation {
            draft.parts = vec![DialoguePart::Literal {
                text: if key.target == target {
                    "当前待救援文字"
                } else {
                    "其他目标字段不可冒领"
                }
                .into(),
            }];
            draft.direction = Some("私密方向完整保留".into());
        }
    }
    let requests = view.dialogue_runtime_drafts(&project.root);
    project
        .set_text(
            &path,
            format!("{}\n// 较新工程内容\n", project.document(&path).unwrap()),
        )
        .unwrap();
    let baseline = project.content_baseline();
    let identity = buffer.identity();
    let output = frame(
        &ctx,
        &project,
        &buffer,
        &target,
        &mut view,
        vec![Event::Text("不应写入只读字段".into())],
    );
    for expected in [
        "当前待救援文字",
        "私密方向完整保留",
        "恢复用原输入，含作者私密备注；非台本交付",
    ] {
        assert!(
            output
                .shapes
                .iter()
                .any(|shape| contains(&shape.shape, expected)),
            "{expected}"
        );
    }
    assert!(!output
        .shapes
        .iter()
        .any(|shape| contains(&shape.shape, "其他目标字段不可冒领")));
    assert!(!output
        .shapes
        .iter()
        .any(|shape| point(&shape.shape, "纳入正文草稿").is_some()));
    assert_eq!(view.dialogue_runtime_drafts(&project.root), requests);
    let click = |label: &str, view: &mut ViewState| {
        let output = frame(&ctx, &project, &buffer, &target, view, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| point(&shape.shape, label))
            .unwrap();
        for down in [true, false] {
            frame(&ctx, &project, &buffer, &target, view, press(pos, down));
        }
    };
    click("取消此句输入", &mut view);
    assert!(view.dialogue.forms[&key].discard_confirm);
    assert_eq!(view.dialogue_runtime_drafts(&project.root), requests);
    click("继续保留此句", &mut view);
    assert_eq!(view.dialogue_runtime_drafts(&project.root), requests);
    click("取消此句输入", &mut view);
    click("确认取消此句输入", &mut view);
    assert!(!view.dialogue.forms.contains_key(&key));
    assert_eq!(view.dialogue.forms.len(), 1);
    assert_eq!(project.content_baseline(), baseline);
    assert_eq!(buffer.identity(), identity);
    std::fs::remove_dir_all(project.root).unwrap();
}

#[test]
fn dialogue_unavailable_recovery_keeps_original_ime_receiver_without_source_writes() {
    for (preedit, commit) in [
        (true, Some("组合提交保留")),
        (false, Some("直接提交保留")),
        (true, Some("")),
        (false, Some("")),
        (true, None),
    ] {
        let (mut project, buffer, target) = tests::fixture();
        let mut view = tests::open_form(&project, &buffer, &target);
        let ctx = egui::Context::default();
        frame(&ctx, &project, &buffer, &target, &mut view, vec![]);
        let owner = ctx.memory(|memory| memory.focused());
        if preedit {
            frame(
                &ctx,
                &project,
                &buffer,
                &target,
                &mut view,
                vec![Event::Ime(egui::ImeEvent::Preedit("候选".into()))],
            );
        }
        let path = project.entry.clone();
        project
            .set_text(
                &path,
                format!("{}\n// 来源变化\n", project.document(&path).unwrap()),
            )
            .unwrap();
        // 与宿主 recompile 的 query invalidation 相同；组件测试不绕过该边界。
        view.invalidate_projection();
        let baseline = project.content_baseline();
        let identity = buffer.identity();
        let mut events = match commit {
            Some(value) => vec![Event::Ime(egui::ImeEvent::Commit(value.into()))],
            None => vec![
                Event::Ime(egui::ImeEvent::Preedit(String::new())),
                Event::Ime(egui::ImeEvent::Disabled),
            ],
        };
        events.push(Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        });
        frame(&ctx, &project, &buffer, &target, &mut view, events);
        assert_eq!(ctx.memory(|memory| memory.focused()), owner);
        let form = &view.dialogue.forms[&Key::new(&buffer, &target)];
        assert!(matches!(
            form.request.operation,
            DialogueOperation::Update { .. }
        ));
        if let Some(value) = commit.filter(|value| !value.is_empty()) {
            assert!(
                view.dialogue_runtime_drafts(&project.root)
                    .values()
                    .any(|text| text.contains(value)),
                "preedit={preedit}, commit={value}"
            );
        }
        assert_eq!(project.content_baseline(), baseline);
        assert_eq!(buffer.identity(), identity);
        assert_eq!(view.dialogue.forms.len(), 1);
        std::fs::remove_dir_all(project.root).unwrap();
    }
}
