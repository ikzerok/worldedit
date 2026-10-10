//! Existing user entrypoints, with the same full-App event and paint oracle as migration.
use super::*;
use worldline_core::manuscript::{DialogueEditPlan, DialogueKind, DialogueOperation};
mod appearance;
mod flows;
mod guards;
mod reading;

const OLD: &str = "original plain";
const NEW: &str = "edited ordinary sentence";
const STAGE: &str = "纳入正文草稿";
const LOSS: &str = "我确认丢弃预览列出的演出备注，并重新预览转换";

#[derive(Clone, Copy, Debug)]
enum Case {
    UpdateText,
    UpdateSay,
    InsertSay,
    TextToSay,
    SayToText,
    DeleteText,
    DeleteSay,
    ProseThenUpdate,
}
fn setup(case: Case) -> Harness {
    let statement = match case {
        Case::UpdateText | Case::TextToSay => "  \\original plain\n",
        Case::UpdateSay | Case::SayToText => "  say traveler \"original plain\"\n",
        Case::DeleteText => "  // keep before\n  original plain // keep inline\n  // keep after\n",
        Case::DeleteSay => "  // keep before\n  say traveler \"original plain\" direction \"delete private note\" // keep inline\n  // keep after\n",
        Case::InsertSay | Case::ProseThenUpdate => "",
    };
    let mut h = if matches!(case, Case::UpdateText) {
        Harness::normal_language(statement, "1.9")
    } else {
        Harness::normal(statement)
    };
    if matches!(case, Case::InsertSay) {
        h.begin_fields();
    } else {
        if matches!(case, Case::ProseThenUpdate) {
            h.click("从这里写下第一段……");
            h.frame(vec![Event::Text(OLD.into())]);
            assert!(h.app.manuscript.writing_buffers()[0].source().contains(OLD));
            assert!(!h
                .app
                .project
                .document(&h.app.active_file)
                .unwrap()
                .contains(OLD));
        }
        h.toolbar("逐句对白");
        if matches!(case, Case::DeleteText | Case::DeleteSay) {
            h.click("语句操作");
            h.click("删除此语句…");
        } else if matches!(case, Case::TextToSay | Case::SayToText) {
            h.click("语句操作");
            h.click(if matches!(case, Case::TextToSay) {
                "转为正式台词…"
            } else {
                "转为普通旁白…"
            });
            if matches!(case, Case::TextToSay) {
                h.click("明确选择正式角色");
                h.click("旅人 · character:traveler");
            }
        } else {
            h.click("编辑此句");
            replace(&mut h, OLD, NEW);
        }
    }
    h.click("预览语句变更");
    normal_plan(&h);
    h
}
fn replace(h: &mut Harness, old: &str, new: &str) -> egui::Id {
    h.click(old);
    let owner = h.ctx.memory(|memory| memory.focused()).unwrap();
    assert!(egui::TextEdit::load_state(&h.ctx, owner).is_some());
    h.key(Key::A, Modifiers::COMMAND);
    h.frame(vec![Event::Text(new.into())]);
    assert_eq!(h.literal(), new);
    owner
}
fn normal_plan(h: &Harness) -> DialogueEditPlan {
    let plan = h.current_plan();
    assert!(plan.migration.is_none());
    assert!(!plan.request.enable_language_1_11);
    assert!(matches!(h.app.project.language_version(), "1.9" | "1.11"));
    plan
}
fn disk(h: &Harness) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
    worldline_core::file_access::workspace_files(&h.app.project.root)
        .unwrap()
        .into_iter()
        .map(|p| {
            let bytes = std::fs::read(&p).unwrap();
            (p, bytes)
        })
        .collect()
}
fn apply_body(h: &mut Harness) {
    let out = h.settle();
    if visible_label(&out, "应用正文草稿").is_none() {
        h.click("正文工具");
    }
    h.click("应用正文草稿");
}
fn stage_apply_history(h: &mut Harness, plan: &DialogueEditPlan) {
    let before = h.app.manuscript.writing_buffers().pop().unwrap();
    let mut expected = before.clone();
    h.app
        .project
        .stage_dialogue_edit(&mut expected, plan)
        .unwrap();
    let original = h
        .app
        .project
        .document(&h.app.active_file)
        .unwrap()
        .to_owned();
    let docs = h.state()["docs"].clone();
    let disk_before = disk(h);
    let history = counts(h);
    assert_eq!((history.1, history.3), (0, 0));
    let node = h.app.history_state.current;
    let language = h.app.project.language_version().to_owned();
    h.tab_to(STAGE);
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(
        h.app.manuscript.writing_buffers()[0].source(),
        expected.source()
    );
    assert_eq!(
        h.app.project.document(&h.app.active_file).unwrap(),
        original
    );
    assert_eq!(
        counts(h),
        (history.0, 0, history.2 + 1, 0),
        "stage adds one draft edge, without a Project edge"
    );
    assert_eq!(h.app.history_state.current, node);
    assert!(!h.app.manuscript.has_dialogue_input());
    assert_eq!(h.state()["docs"], docs);
    assert_eq!(disk(h), disk_before);
    apply_body(h);
    assert_eq!(
        h.app.project.document(&h.app.active_file).unwrap(),
        expected.source()
    );
    assert_eq!(
        counts(h),
        (history.0 + 1, 0, history.2 + 1, 0),
        "Apply adds one Project edge and retains its reachable draft edge"
    );
    let applied_node = h.app.history_state.current;
    assert_ne!(applied_node, node);
    assert_eq!(h.app.project.language_version(), language);
    assert_eq!(h.state()["docs"], docs);
    assert_eq!(disk(h), disk_before, "neither action saves disk");
    h.app.edit_undo(false);
    assert_eq!(counts(h), (history.0, 1, history.2 + 1, 0));
    assert_eq!(h.app.history_state.current, node);
    assert_eq!(
        h.app.project.document(&h.app.active_file).unwrap(),
        original
    );
    assert_eq!(
        h.app.manuscript.writing_buffers()[0].source(),
        expected.source()
    );
    h.app.edit_undo(false);
    assert_eq!(counts(h), (history.0, 1, history.2, 1));
    assert_eq!(h.app.history_state.current, node);
    assert_eq!(
        h.app.manuscript.writing_buffers()[0].source(),
        before.source()
    );
    h.app.edit_undo(true);
    assert_eq!(counts(h), (history.0, 1, history.2 + 1, 0));
    assert_eq!(h.app.history_state.current, node);
    assert_eq!(
        h.app.manuscript.writing_buffers()[0].source(),
        expected.source()
    );
    h.app.edit_undo(true);
    assert_eq!(counts(h), (history.0 + 1, 0, history.2 + 1, 0));
    assert_eq!(h.app.history_state.current, applied_node);
    assert_eq!(
        h.app.project.document(&h.app.active_file).unwrap(),
        expected.source()
    );
    assert_eq!(disk(h), disk_before);
}

fn counts(h: &Harness) -> (usize, usize, usize, usize) {
    (
        h.app.history.len(),
        h.app.redo.len(),
        h.app.search_state.undo.len(),
        h.app.search_state.redo.len(),
    )
}
