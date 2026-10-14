//! v0.34 正式对白实际 egui 控件链；不是物理输入法或原生桌面验收。
use super::*;
pub(super) fn fixture() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = manuscript_app();
    let source = concat!(
        "character traveler as \"同名\"\n",
        "character second as \"同名\"\n",
        "event arrival as \"抵达\"\n",
        "  say traveler \"原来的正式对白\" direction \"原演出备注\"\n",
        "  scene harbor\n",
        "    港口旁白。\n",
        "    -> END\n",
        "  -> END\n",
        "event departure as \"离港\"\n",
        "  远航。\n",
        "  -> END\n",
        "entity a kind place as \"同名\"\n",
        "entity b kind organization as \"同名\"\n",
    );
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    let path = app.project.root.join(".world/project.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes()).unwrap();
    manifest["language_version"] = "1.11".into();
    app.project
        .set_authoring_document(&path, serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    std::fs::create_dir_all(&app.project.root).unwrap();
    app.project.save().unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    app.tab = Tab::Manuscript;
    for _ in 0..3 {
        frame(&ctx, &mut app, Vec::new(), 13);
    }
    toggle_mode(&ctx, &mut app);
    frame(&ctx, &mut app, Vec::new(), 13);
    (ctx, app)
}
pub(super) fn toggle_mode(ctx: &egui::Context, app: &mut WorldeditApp) {
    fn label_rect(shape: &egui::Shape, label: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| label_rect(shape, label)),
            _ => None,
        }
    }
    let has_label = |output: &egui::FullOutput, label: &str| {
        output.shapes.iter().any(|shape| {
            label_rect(&shape.shape, label).is_some_and(|rect| shape.clip_rect.contains_rect(rect))
        })
    };
    let mut output = frame(ctx, app, vec![], 13);
    if !has_label(&output, "逐句对白") {
        if has_label(&output, "正文工具") {
            click(ctx, app, 13, "正文工具");
            output = frame(ctx, app, vec![], 13);
        }
        assert!(has_label(&output, "对白工具"), "{}", rendered(&output));
        click(ctx, app, 13, "对白工具");
    }
    click(ctx, app, 13, "逐句对白");
}
pub(super) fn rendered(output: &egui::FullOutput) -> String {
    let mut text = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut text);
    }
    text
}
#[test]
fn dialogue_continuous_editor_stages_applies_and_uses_shared_two_level_history() {
    let (ctx, mut app) = fixture();
    let original = app.project.document(&app.active_file).unwrap().to_owned();
    let first = rendered(&frame(&ctx, &mut app, Vec::new(), 13));
    assert!(first.contains("原来的正式对白"), "{first}");
    click(&ctx, &mut app, 13, "编辑此句");
    replace_text_area(&ctx, &mut app, 13, "原来的正式对白", "改过的台词😀\n第二行");
    assert!(app.manuscript.has_dialogue_input());
    assert_eq!(app.project.document(&app.active_file).unwrap(), original);
    click(&ctx, &mut app, 13, "预览语句变更");
    click(&ctx, &mut app, 13, "纳入正文草稿");
    assert!(!app.manuscript.has_dialogue_input());
    let draft = app
        .manuscript
        .writing_buffers()
        .into_iter()
        .find(|buffer| buffer.is_changed())
        .unwrap();
    assert!(draft.source().contains("改过的台词😀\\n第二行"));
    assert_eq!(app.project.document(&app.active_file).unwrap(), original);
    click(&ctx, &mut app, 13, "应用正文草稿");
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("改过的台词"));
    app.edit_undo(false);
    assert_eq!(app.project.document(&app.active_file).unwrap(), original);
    assert!(app
        .manuscript
        .writing_buffers()
        .iter()
        .any(|buffer| buffer.source().contains("改过的台词")));
    app.edit_undo(false);
    assert!(app
        .manuscript
        .writing_buffers()
        .iter()
        .all(|buffer| !buffer.source().contains("改过的台词")));
    app.edit_undo(true);
    app.edit_undo(true);
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("改过的台词"));
    let _ = std::fs::remove_dir_all(app.project.root);
}
#[test]
fn dialogue_unstaged_fields_survive_source_mode_and_chapter_navigation() {
    let (ctx, mut app) = fixture();
    click(&ctx, &mut app, 13, "编辑此句");
    replace_text_area(&ctx, &mut app, 13, "原来的正式对白", "保护中的正式对白");
    click(&ctx, &mut app, 13, "源码");
    assert!(app.manuscript.has_dialogue_input());
    click(&ctx, &mut app, 13, "离港");
    click(&ctx, &mut app, 13, "抵达");
    click(&ctx, &mut app, 13, "写作");
    let text = rendered(&frame(&ctx, &mut app, Vec::new(), 13));
    assert!(text.contains("保护中的正式对白"), "{text}");
    click(&ctx, &mut app, 13, "取消此句输入");
    assert!(app.manuscript.has_dialogue_input());
    click(&ctx, &mut app, 13, "继续保留此句");
    assert!(app.manuscript.has_dialogue_input());
    let _ = std::fs::remove_dir_all(app.project.root);
}
#[test]
fn dialogue_ime_preedit_and_commit_frame_cannot_create_next_statement() {
    let (ctx, mut app) = fixture();
    let original = app.project.document(&app.active_file).unwrap().to_owned();
    click(&ctx, &mut app, 13, "编辑此句");
    frame(&ctx, &mut app, Vec::new(), 13);
    let owner = ctx.memory(|memory| memory.focused());
    assert!(owner.is_some());
    frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Preedit("中文候选".into()))],
        13,
    );
    let next = Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };
    frame(&ctx, &mut app, vec![next.clone()], 13);
    frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Commit("中文提交".into())), next],
        13,
    );
    assert_eq!(app.project.document(&app.active_file).unwrap(), original);
    assert!(app
        .manuscript
        .writing_buffers()
        .iter()
        .all(|buffer| !buffer.is_changed()));
    assert_eq!(ctx.memory(|memory| memory.focused()), owner);
    assert!(app.manuscript.has_dialogue_input());
    let _ = std::fs::remove_dir_all(app.project.root);
}
#[test]
fn dialogue_clean_control_enter_starts_next_typed_draft_without_writing_source() {
    let (ctx, mut app) = fixture();
    let original = app.project.document(&app.active_file).unwrap().to_owned();
    click(&ctx, &mut app, 13, "编辑此句");
    frame(&ctx, &mut app, Vec::new(), 13);
    frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
        13,
    );
    let text = rendered(&frame(&ctx, &mut app, Vec::new(), 13));
    assert!(text.contains("下一句 · 角色可修改"), "{text}");
    assert!(app.manuscript.has_dialogue_input());
    assert_eq!(app.project.document(&app.active_file).unwrap(), original);
    let _ = std::fs::remove_dir_all(app.project.root);
}

#[test]
fn dialogue_same_frame_text_or_paste_and_control_enter_retains_current_input() {
    for edit in [
        Event::Text("同帧新输入".into()),
        Event::Paste("同帧粘贴输入".into()),
    ] {
        let (ctx, mut app) = fixture();
        let original = app.project.document(&app.active_file).unwrap().to_owned();
        click(&ctx, &mut app, 13, "编辑此句");
        frame(&ctx, &mut app, Vec::new(), 13);
        frame(
            &ctx,
            &mut app,
            vec![
                edit,
                Event::Key {
                    key: egui::Key::Enter,
                    physical_key: Some(egui::Key::Enter),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
            ],
            13,
        );
        let text = rendered(&frame(&ctx, &mut app, Vec::new(), 13));
        assert!(
            text.contains("同帧"),
            "batched input must remain visible: {text}"
        );
        assert!(
            !text.contains("下一句 · 角色可修改"),
            "a new form must not replace same-frame input: {text}"
        );
        assert!(
            text.contains("纳入并写下一句"),
            "batched input must be previewed before continuation: {text}"
        );
        assert!(app.manuscript.has_dialogue_input());
        assert_eq!(app.project.document(&app.active_file).unwrap(), original);
        assert!(app
            .manuscript
            .writing_buffers()
            .iter()
            .all(|buffer| !buffer.is_changed()));
        let _ = std::fs::remove_dir_all(app.project.root);
    }
}

#[test]
fn dialogue_new_character_refusal_preserves_existing_editor_and_navigation() {
    let (ctx, mut app) = fixture();
    app.new_character();
    app.character_editor.as_mut().unwrap().draft.display = "保留的人物资料".into();
    let old_id = app.character_editor.as_ref().unwrap().draft.id.clone();
    let history = app.personal.history.clone();
    click(&ctx, &mut app, 13, "编辑此句");
    click(&ctx, &mut app, 13, "新建人物…");
    assert_eq!(app.tab, Tab::Manuscript);
    assert_eq!(app.character_editor.as_ref().unwrap().draft.id, old_id);
    assert_eq!(
        app.character_editor.as_ref().unwrap().draft.display,
        "保留的人物资料"
    );
    assert!(app.personal.history == history);
    assert!(app.message.as_deref().unwrap().contains("还有未应用输入"));
    let _ = std::fs::remove_dir_all(app.project.root);
}
