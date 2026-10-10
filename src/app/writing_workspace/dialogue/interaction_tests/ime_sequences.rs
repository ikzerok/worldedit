//! 合成事件验证接收语义；不宣称物理输入法已经验证。
use super::*;
use egui::{
    text::{CCursor, CCursorRange},
    ImeEvent,
};

struct Session {
    project: Project,
    buffer: WritingBuffer,
    target: TargetRef,
    view: ViewState,
    ctx: egui::Context,
}
impl Session {
    fn new(dirty: bool) -> Self {
        let (project, buffer, target) = tests::fixture();
        let mut view = tests::open_form(&project, &buffer, &target);
        if dirty {
            if let DialogueOperation::Update { draft, .. } = &mut view
                .dialogue
                .forms
                .values_mut()
                .next()
                .unwrap()
                .request
                .operation
            {
                draft.parts[0] = DialoguePart::Literal {
                    text: "已改😀尾".into(),
                };
            }
        }
        let mut session = Self {
            project,
            buffer,
            target,
            view,
            ctx: Default::default(),
        };
        session.draw(vec![]);
        let end = session.body().chars().count();
        session.select(session.owner(), end, end);
        session
    }
    fn key(&self) -> Key {
        Key::new(&self.buffer, &self.target)
    }
    fn owner(&self) -> egui::Id {
        egui::Id::new(("dialogue-part", self.key(), 0usize))
    }
    fn body(&self) -> String {
        let DialogueOperation::Update { draft, .. } =
            &self.view.dialogue.forms[&self.key()].request.operation
        else {
            panic!("原 Update 必须保留")
        };
        let DialoguePart::Literal { text } = &draft.parts[0] else {
            panic!("原字面字段必须保留")
        };
        text.clone()
    }
    fn select(&self, id: egui::Id, start: usize, end: usize) {
        let mut state = egui::TextEdit::load_state(&self.ctx, id).unwrap();
        state.cursor.set_char_range(Some(CCursorRange::two(
            CCursor::new(start),
            CCursor::new(end),
        )));
        state.store(&self.ctx, id);
        self.ctx.memory_mut(|memory| memory.request_focus(id));
    }
    fn unavailable(&mut self) {
        let path = self.project.entry.clone();
        self.project
            .set_text(
                &path,
                format!("{}\n// 新来源\n", self.project.document(&path).unwrap()),
            )
            .unwrap();
        self.view.invalidate_projection();
    }
    fn draw(&mut self, events: Vec<Event>) {
        let baseline = self.project.content_baseline();
        let identity = self.buffer.identity();
        let _ = self.ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1700.0, 2000.0),
                )),
                events: events.clone(),
                ..Default::default()
            },
            |ctx| {
                let _composition = self.view.begin_input(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut same_buffer = self.buffer.clone();
                    let mut action = Action::default();
                    super::super::super::editors::draw(
                        ui,
                        &self.project,
                        &mut same_buffer,
                        &self.target,
                        &mut self.view,
                        typography(),
                        &mut action,
                    );
                    assert_eq!(same_buffer.identity(), identity);
                    assert!(action.dialogue_plan.is_none());
                });
                assert_eq!(
                    ctx.input(|input| input.raw.events.clone()),
                    events,
                    "不改持久原输入"
                );
                if events.iter().any(|event| matches!(event, Event::Ime(_))) {
                    assert!(
                        !ctx.input(|input| input
                            .events
                            .iter()
                            .any(|event| matches!(event, Event::Ime(_)))),
                        "原接收者消费后不得向其他字段重放"
                    );
                }
            },
        );
        assert_eq!(self.project.content_baseline(), baseline);
        assert_eq!(self.buffer.identity(), identity);
        assert_eq!(self.view.dialogue.forms.len(), 1);
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.project.root).unwrap();
    }
}
fn ime(event: ImeEvent) -> Event {
    Event::Ime(event)
}
fn commit(text: &str) -> Event {
    ime(ImeEvent::Commit(text.into()))
}
fn preedit(text: &str) -> Event {
    ime(ImeEvent::Preedit(text.into()))
}
fn command(key: egui::Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }
}

#[test]
fn dialogue_ime_sequences_are_single_delivery_in_normal_and_unavailable_forms() {
    let sequences = [
        (vec![vec![commit("甲")]], "甲"),
        (vec![vec![ime(ImeEvent::Enabled)], vec![commit("甲")]], "甲"),
        (vec![vec![ime(ImeEvent::Enabled), commit("甲")]], "甲"),
        (
            vec![vec![
                ime(ImeEvent::Disabled),
                commit("甲"),
                ime(ImeEvent::Disabled),
            ]],
            "甲",
        ),
        (vec![vec![preedit(""), commit("甲")]], "甲"),
        (vec![vec![preedit("候选")], vec![commit("甲")]], "甲"),
        (
            vec![vec![preedit("候选")], vec![preedit(""), commit("甲")]],
            "甲",
        ),
        (
            vec![vec![preedit("候选")], vec![preedit("")], vec![commit("甲")]],
            "甲",
        ),
        (
            vec![
                vec![preedit("候选")],
                vec![
                    ime(ImeEvent::Disabled),
                    commit("甲"),
                    ime(ImeEvent::Disabled),
                ],
            ],
            "甲",
        ),
        (vec![vec![commit("甲"), commit("乙")]], "甲乙"),
        (vec![vec![commit("甲")], vec![commit("乙")]], "甲乙"),
    ];
    for unavailable in [false, true] {
        for dirty in [false, true] {
            for (index, (frames, suffix)) in sequences.iter().enumerate() {
                let mut session = Session::new(dirty);
                let original = session.body();
                for (frame_index, events) in frames.iter().enumerate() {
                    let enabled_only = matches!(events.as_slice(), [Event::Ime(ImeEvent::Enabled)]);
                    let mut events = events.clone();
                    if frame_index + 1 == frames.len() {
                        if unavailable {
                            session.unavailable();
                        }
                        events.push(command(egui::Key::Enter));
                    }
                    session.draw(events);
                    if enabled_only {
                        assert!(!session.view.ime_active, "Enabled不是组合输入");
                    }
                }
                assert_eq!(
                    session.body(),
                    format!("{original}{suffix}"),
                    "sequence={index}, unavailable={unavailable}, dirty={dirty}"
                );
                assert_eq!(
                    session.ctx.memory(|memory| memory.focused()),
                    Some(session.owner())
                );
                assert!(session.view.dialogue.forms[&session.key()].plan.is_none());
                // 无新事件的下一帧不得重放任何一个 Commit。
                session.draw(vec![]);
                assert_eq!(session.body(), format!("{original}{suffix}"));
            }
        }
    }
}

#[test]
fn dialogue_ime_selection_preserves_unicode_and_existing_preedit_rejection() {
    for unavailable in [false, true] {
        let mut session = Session::new(true);
        session.select(session.owner(), 1, 3); // “改😀”：字符位置，不是 UTF-8 字节。
        if unavailable {
            session.unavailable();
        }
        session.draw(vec![commit("替换")]);
        assert_eq!(session.body(), "已替换尾");
    }
    for disabled_before_commit in [false, true] {
        let mut session = Session::new(false);
        session.draw(vec![preedit("候选")]);
        let old = session.body();
        session.select(session.owner(), 0, 0);
        session.draw(vec![commit("\n"), commit("\r")]);
        assert_eq!(session.body(), old, "egui忽略的换行Commit不重置旧range凭证");
        let mut events = vec![];
        if disabled_before_commit {
            events.push(ime(ImeEvent::Disabled));
        }
        events.push(commit("不能落在新光标"));
        session.draw(events);
        assert_eq!(session.body(), old, "已有preedit继续由egui旧range拒绝");
    }
}

#[test]
fn dialogue_ime_cancellation_empty_commit_and_form_lifetime_do_not_poison_new_input() {
    for clear in [false, true] {
        let mut session = Session::new(false);
        session.draw(vec![preedit("候选")]);
        let mut cancellation = vec![];
        if clear {
            cancellation.push(preedit(""));
        }
        cancellation.push(ime(ImeEvent::Disabled));
        session.draw(cancellation);
        let cancelled = session.body();
        session.select(session.owner(), 0, 0);
        session.draw(vec![commit("新")]);
        assert_eq!(session.body(), format!("新{cancelled}"));
    }
    for had_preedit in [false, true] {
        let mut session = Session::new(false);
        if had_preedit {
            session.draw(vec![preedit("候选")]);
        }
        let before = session.body();
        session.draw(vec![commit(""), command(egui::Key::Enter)]);
        assert_eq!(session.body(), before);
        session.draw(vec![]);
        assert!(!session.view.ime_active);
    }
    let mut session = Session::new(false);
    session.draw(vec![preedit("旧候选")]);
    session.view.dialogue.forms.clear();
    let projection = session
        .project
        .project_dialogue_buffer(&session.buffer, &session.target)
        .unwrap();
    let statement = &projection.statements[0];
    session.view.dialogue.begin_context(
        &session.ctx,
        &session.buffer,
        &projection,
        DialogueOperation::Update {
            statement_id: statement.id.clone(),
            draft: statement.draft.clone(),
        },
        "重开".into(),
    );
    session.draw(vec![]);
    session.select(session.owner(), 0, 0);
    session.draw(vec![commit("新句")]);
    assert_eq!(session.body(), "新句原话");
}

#[test]
fn dialogue_ime_keeps_text_edit_undo_and_rejects_pointer_handoff() {
    let mut session = Session::new(false);
    let before = session.body();
    let position = egui::pos2(1600.0, 1000.0);
    let mut events = vec![commit("一次提交")];
    events.extend(press(position, true));
    session.draw(events);
    session.draw(press(position, false));
    assert_eq!(
        session.ctx.memory(|memory| memory.focused()),
        Some(session.owner())
    );
    assert_eq!(session.body(), format!("{before}一次提交"));
    session.draw(vec![command(egui::Key::Z)]);
    assert_eq!(session.body(), before);
}

#[test]
fn dialogue_ime_only_changes_the_focused_typed_part_or_private_direction() {
    for unavailable in [false, true] {
        for field in 0..6 {
            let mut session = Session::new(false);
            let key = session.key();
            let form = session.view.dialogue.forms.get_mut(&key).unwrap();
            let DialogueOperation::Update { draft, .. } = &mut form.request.operation else {
                unreachable!()
            };
            draft.parts.push(DialoguePart::Expression {
                source: "score + 1".into(),
            });
            draft.parts.push(DialoguePart::Link {
                target: TargetRef::new("character", "b"),
                label: "同名".into(),
            });
            session.draw(vec![]);
            let (id, end) = match field {
                0 => (session.owner(), 2),
                1 => (egui::Id::new(("dialogue-part", &key, 1usize)), 9),
                2 => (
                    egui::Id::new(("dialogue-part", &key, 2usize)).with("kind"),
                    9,
                ),
                3 => (egui::Id::new(("dialogue-part", &key, 2usize)).with("id"), 1),
                4 => (egui::Id::new(("dialogue-part", &key, 2usize)), 2),
                _ => (egui::Id::new(("dialogue-direction", &key)), 4),
            };
            session.select(id, end, end);
            let mut expected = session.view.dialogue.forms[&key].request.clone();
            let DialogueOperation::Update { draft, .. } = &mut expected.operation else {
                unreachable!()
            };
            match field {
                0 => {
                    if let DialoguePart::Literal { text } = &mut draft.parts[0] {
                        text.push_str("输入");
                    }
                }
                1 => {
                    if let DialoguePart::Expression { source } = &mut draft.parts[1] {
                        source.push_str("输入");
                    }
                }
                2..=4 => {
                    if let DialoguePart::Link { target, label } = &mut draft.parts[2] {
                        match field {
                            2 => target.kind.push_str("输入"),
                            3 => target.id.push_str("输入"),
                            _ => label.push_str("输入"),
                        }
                    }
                }
                _ => draft.direction.as_mut().unwrap().push_str("输入"),
            }
            if unavailable {
                session.unavailable();
            }
            session.draw(vec![commit("输入"), command(egui::Key::Enter)]);
            assert_eq!(
                session.view.dialogue.forms[&key].request, expected,
                "field={field}, unavailable={unavailable}"
            );
            assert_eq!(session.ctx.memory(|memory| memory.focused()), Some(id));
        }
    }
}

#[test]
fn dialogue_ime_does_not_intercept_another_window_receiver() {
    for hide_form in [false, true] {
        let mut session = Session::new(false);
        session.draw(vec![preedit("原候选")]);
        let before = session.view.dialogue.forms[&session.key()].request.clone();
        let original = session.body();
        let external_id = egui::Id::new("external-ime-window");
        let mut external = String::new();
        for events in [
            vec![],
            vec![],
            vec![
                ime(ImeEvent::Enabled),
                commit("外部接收"),
                ime(ImeEvent::Disabled),
            ],
        ] {
            let _ = session.ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1700.0, 2000.0),
                    )),
                    events: events.clone(),
                    ..Default::default()
                },
                |ctx| {
                    let _composition = session.view.begin_input(ctx);
                    if !hide_form {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let mut buffer = session.buffer.clone();
                            super::super::super::editors::draw(
                                ui,
                                &session.project,
                                &mut buffer,
                                &session.target,
                                &mut session.view,
                                typography(),
                                &mut Action::default(),
                            );
                        });
                    }
                    assert_eq!(
                        ctx.input(|input| input.events.clone()),
                        events,
                        "外部receiver的事件不可被formal消费"
                    );
                    egui::Window::new("其他窗口").show(ctx, |ui| {
                        ui.add(egui::TextEdit::singleline(&mut external).id(external_id))
                            .request_focus();
                    });
                },
            );
        }
        assert_eq!(external, "外部接收");
        assert_eq!(session.view.dialogue.forms[&session.key()].request, before);
        assert_eq!(
            session.ctx.memory(|memory| memory.focused()),
            Some(external_id)
        );
        session.select(session.owner(), 0, 0);
        session.draw(vec![]); // 隐藏路径先重新登记字段，再接收新会话。
        session.draw(vec![commit("新会话")]);
        assert_eq!(session.body(), format!("新会话{original}"));
    }
}
