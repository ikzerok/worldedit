use super::*;
pub(super) fn fixture() -> (Project, WritingBuffer, TargetRef) {
    let root = std::env::temp_dir().join(format!(
        "dialogue-ui-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut project = Project::new(&root);
    let path = project.entry.clone();
    project.set_text(&path, "character a as \"同名\"\ncharacter b as \"同名\"\nevent start\n  say a \"原话\" direction \"私密备注\"\n  -> END\n".into()).unwrap();
    project.create_authoring_document(&project.root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.11","required_features":[],"maps":{},"graph_views":{}}"#.to_vec()).unwrap();
    std::fs::create_dir_all(&project.root).unwrap();
    project.save().unwrap();
    let target = TargetRef::new("event", "start");
    let buffer = project.open_writing_buffer(&target).unwrap();
    (project, buffer, target)
}
pub(super) fn open_form(
    project: &Project,
    buffer: &WritingBuffer,
    target: &TargetRef,
) -> ViewState {
    let projection = project.project_dialogue_buffer(buffer, target).unwrap();
    let statement = &projection.statements[0];
    let mut view = ViewState::default();
    let ctx = egui::Context::default();
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            view.dialogue.begin_context(
                ui.ctx(),
                buffer,
                &projection,
                DialogueOperation::Update {
                    statement_id: statement.id.clone(),
                    draft: statement.draft.clone(),
                },
                "测试编辑".into(),
            );
        });
    });
    view
}
#[test]
fn typed_pending_inputs_join_all_shared_retention_guards_before_buffer_changes() {
    let (project, buffer, target) = fixture();
    let mut view = open_form(&project, &buffer, &target);
    assert!(!view.has_retained_input());
    let form = view.dialogue.forms.values_mut().next().unwrap();
    if let DialogueOperation::Update { draft, .. } = &mut form.request.operation {
        draft.parts = vec![DialoguePart::Literal {
            text: "未纳入正文\n中文😀".into(),
        }];
        draft.speaker = Some(TargetRef::new("character", "b"));
    }
    assert!(view.has_dialogue_input());
    assert!(view.has_retained_input());
    assert!(view.has_retained_for(buffer.path()));
    assert!(!buffer.is_changed());
    assert!(view
        .retained_runtime_drafts(&project.root)
        .values()
        .any(|value| value.contains("未纳入正文")));
    view.discard_retained_for(buffer.path());
    assert!(!view.has_retained_input());
    let _ = std::fs::remove_dir_all(project.root);
}
#[test]
fn typed_failed_plan_keeps_fields_and_never_marks_them_applied() {
    let (project, buffer, target) = fixture();
    let mut view = open_form(&project, &buffer, &target);
    let form = view.dialogue.forms.values_mut().next().unwrap();
    if let DialogueOperation::Update { draft, .. } = &mut form.request.operation {
        draft.direction = Some("保留全部备注".into());
    }
    view.dialogue_plan_result(buffer.path(), &target, Some("基线过期".into()));
    assert!(view.has_retained_input());
    assert!(view
        .dialogue
        .forms
        .values()
        .next()
        .unwrap()
        .error
        .as_deref()
        .unwrap()
        .contains("基线过期"));
    assert!(!buffer.is_changed());
    let _ = std::fs::remove_dir_all(project.root);
}

#[test]
fn dialogue_form_uses_all_five_layouts_seventeen_members_and_three_densities() {
    use crate::theme::{AppearancePreferences, Density, PaletteId, StylePreset, ThemeMode};
    let (project, buffer, target) = fixture();
    let mut count = 0;
    for palette in PaletteId::ALL {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            if palette.effective_mode(mode, None) != mode {
                continue;
            }
            for style in [
                StylePreset::Studio,
                StylePreset::Manuscript,
                StylePreset::Technical,
                StylePreset::Focus,
                StylePreset::Ledger,
            ] {
                for density in [Density::Compact, Density::Standard, Density::Spacious] {
                    let ctx = egui::Context::default();
                    let mut view = open_form(&project, &buffer, &target);
                    let preferences = AppearancePreferences {
                        palette,
                        theme: mode,
                        style,
                        density,
                        reduce_motion: true,
                        ..Default::default()
                    };
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(800.0, 600.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            let _theme = theme::configure_appearance(ctx, &preferences);
                            egui::CentralPanel::default().show(ctx, |ui| {
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    draw(
                                        ui,
                                        &project,
                                        &buffer,
                                        &target,
                                        &mut view,
                                        Typography {
                                            compact: true,
                                            size: 17.0,
                                            spacing: 1.6,
                                            width: 680.0,
                                            source_size: 13.0,
                                        },
                                        &mut Action::default(),
                                    );
                                });
                            });
                        },
                    );
                    fn collect(shape: &egui::Shape, text: &mut String) {
                        match shape {
                            egui::Shape::Text(value) => text.push_str(value.galley.text()),
                            egui::Shape::Vec(values) => {
                                for value in values {
                                    collect(value, text);
                                }
                            }
                            _ => {}
                        }
                    }
                    let mut text = String::new();
                    for shape in &output.shapes {
                        collect(&shape.shape, &mut text);
                    }
                    assert!(
                        text.contains("原话"),
                        "{palette:?}/{mode:?}/{style:?}/{density:?}: {text}"
                    );
                    assert!(!buffer.is_changed());
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 17 * 5 * 3);
    let _ = std::fs::remove_dir_all(project.root);
}
