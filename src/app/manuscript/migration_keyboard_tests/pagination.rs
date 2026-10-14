use super::*;

fn read_labels(h: &mut Harness, expected: &[String]) {
    h.tab_to(READ);
    for _ in 0..1000 {
        if h.offset() < 0.1 {
            break;
        }
        h.key(Key::ArrowUp, Modifiers::NONE);
    }
    assert!(h.offset() < 0.1);
    let mut seen = std::collections::BTreeSet::new();
    let mut stopped = 0;
    for _ in 0..1000 {
        let out = h.settle();
        for label in expected {
            if visible_label(&out, label).is_some() {
                seen.insert(label.clone());
            }
        }
        if seen.len() == expected.len() {
            break;
        }
        let old = h.offset();
        h.key(Key::ArrowDown, Modifiers::NONE);
        stopped = if (h.offset() - old).abs() < 0.1 {
            stopped + 1
        } else {
            0
        };
        if stopped == 3 {
            break;
        }
    }
    assert_eq!(
        seen,
        expected
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        "every exact page item must be fully visible"
    );
}
fn page(text: &str, n: usize) -> &str {
    let boundary = |mut at: usize| {
        at = at.min(text.len());
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        at
    };
    &text[boundary(n * 16 * 1024)..boundary((n + 1) * 16 * 1024)]
}
fn read_segment(h: &mut Harness, segment: &str) {
    h.tab_to(READ);
    for _ in 0..1000 {
        if h.offset() < 0.1 {
            break;
        }
        h.key(Key::ArrowUp, Modifiers::NONE);
    }
    let mut expected = std::collections::BTreeSet::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut stopped = 0;
    for _ in 0..1600 {
        let out = h.settle();
        for (full, index, row, visible) in galley_rows(&out) {
            if full == segment {
                expected.insert((index, row.clone()));
                if visible {
                    seen.insert((index, row));
                }
            }
        }
        if !expected.is_empty() && seen == expected {
            break;
        }
        let old = h.offset();
        h.key(Key::ArrowDown, Modifiers::NONE);
        stopped = if (h.offset() - old).abs() < 0.1 {
            stopped + 1
        } else {
            0
        };
        if stopped == 3 {
            break;
        }
    }
    assert!(!expected.is_empty(), "exact core byte page must be painted");
    assert_eq!(
        seen, expected,
        "all wrapped rows of this exact UTF-8 page must be readable"
    );
}

#[test]
fn migration_keyboard_real_diagnostic_and_utf8_pages_are_keyboard_reachable() {
    let mut h = Harness::paged();
    h.preview();
    let state = h.state();
    let fields = h.fields();
    let request: worldline_core::manuscript::DialogueEditRequest =
        serde_json::from_str(fields.values().next().unwrap()).unwrap();
    let buffer = h.app.manuscript.writing_buffers().pop().unwrap();
    let plan = h
        .app
        .project
        .preview_dialogue_edit(&buffer, &request)
        .unwrap();
    let migration = plan.migration.as_ref().unwrap();
    assert_eq!(migration.diagnostics_after.len(), 21);
    assert!(migration.diagnostics_after.iter().all(|d| d.code == "A201"));
    assert_eq!(
        plan.changes.len(),
        2,
        "20-item wrapper is exercised by real diagnostics, not fictitious files"
    );
    let diagnostic_labels: Vec<_> = migration
        .diagnostics_after
        .iter()
        .map(|d| format!("{} · {}:{} · {}", d.code, d.file, d.span.line, d.message))
        .collect();
    let header = format!(
        "全文候选诊断 · 21项（新增{}项）",
        migration.new_diagnostics.len()
    );
    h.enter(&header);
    read_labels(&mut h, &diagnostic_labels[..20]);
    h.tab_near("后20项", Some("1–20 / 21"));
    h.key(Key::Enter, Modifiers::NONE);
    read_labels(&mut h, &diagnostic_labels[20..]);
    let out = h.settle();
    assert!(labels(&out).contains("21–21 / 21"));
    assert!(!texts(&out)
        .iter()
        .any(|(t, _, _)| diagnostic_labels[..20].contains(t)));
    h.tab_near("前20项", Some("21–21 / 21"));
    h.key(Key::Enter, Modifiers::NONE);
    read_labels(&mut h, &diagnostic_labels[..20]);
    h.enter(&header); // Collapse only the already-read diagnostic section.
    h.enter("本次一次提交的完整文件 · 2个");
    h.expand_distinct_file_bytes(2);
    let change = plan
        .changes
        .iter()
        .find(|change| change.path == buffer.path())
        .unwrap();
    for text in [change.before.as_ref().unwrap(), &change.after] {
        assert!(text.len() > 16 * 1024 && text.len() < 32 * 1024);
        let first = format!("第1 / 2段 · {} UTF-8字节", text.len());
        let second = format!("第2 / 2段 · {} UTF-8字节", text.len());
        read_segment(&mut h, page(text, 0));
        h.tab_near("后一段字节", Some(&first));
        h.key(Key::Enter, Modifiers::NONE);
        read_segment(&mut h, page(text, 1));
        h.tab_near("前一段字节", Some(&second));
        h.key(Key::Enter, Modifiers::NONE);
        read_segment(&mut h, page(text, 0));
        assert_eq!(h.state(), state);
        assert_eq!(h.fields(), fields);
    }
    h.tab_to(CONFIRM);
    assert_eq!(h.state(), state);
    assert_eq!(h.fields(), fields);
}
