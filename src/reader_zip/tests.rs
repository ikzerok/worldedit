use super::*;

#[test]
fn reader_zip_roundtrips_five_thousand_files_without_backup_limits() {
    let files = (0..5000)
        .map(|index| {
            (
                PathBuf::from(format!("pages/{index:05}.html")),
                vec![index as u8; 7],
            )
        })
        .collect();
    let bytes = encode(&files, &mut |_, _| true).unwrap();
    let archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    assert_eq!(archive.len(), 5000);
}

#[test]
fn unsafe_paths_and_file_budget_are_rejected_before_encoding() {
    for name in [
        "../private",
        "/absolute",
        "a/../b",
        "a//b",
        "./a",
        "C:/a",
        "a\\b",
    ] {
        assert!(
            encode(
                &BTreeMap::from([(PathBuf::from(name), vec![])]),
                &mut |_, _| true
            )
            .is_err(),
            "{name}"
        );
    }
    let files = (0..=MAX_FILES)
        .map(|index| (PathBuf::from(format!("{index}.html")), Vec::new()))
        .collect();
    assert!(encode(&files, &mut |_, _| true)
        .unwrap_err()
        .contains("10000"));
}

#[test]
fn cancellation_during_encoding_and_readback_returns_no_package() {
    let files = BTreeMap::from([(PathBuf::from("index.html"), vec![b'a'; CHUNK * 3])]);
    assert!(encode(&files, &mut |_, _| false)
        .unwrap_err()
        .contains("READER_CANCELLED"));
    let mut seen = 0;
    assert!(encode(&files, &mut |completed, _| {
        seen += 1;
        completed == 0 && seen < 8
    })
    .unwrap_err()
    .contains("READER_CANCELLED"));
    assert!(encode(&files, &mut |completed, _| completed < files.len())
        .unwrap_err()
        .contains("READER_CANCELLED"));
}

#[test]
fn output_budget_is_checked_before_allocating_beyond_limit() {
    let completed = Cell::new(0);
    let mut control = |_, _| true;
    let control: RefCell<&mut dyn FnMut(usize, usize) -> bool> = RefCell::new(&mut control);
    let failure = Cell::new(None);
    let mut writer = ControlledCursor {
        cursor: Cursor::new(Vec::new()),
        control: &control,
        completed: &completed,
        total: 1,
        position: 0,
        length: 0,
        budget: MAX_BYTES as u64,
        failure: &failure,
    };
    writer.seek(SeekFrom::Start(MAX_BYTES as u64)).unwrap();
    writer.write_all(b"x").unwrap();
    assert!(check_failure(&failure).unwrap_err().contains("ZIP预算"));
    assert!(writer.cursor.get_ref().is_empty());
}

#[test]
fn file_and_directory_aliases_cannot_enter_the_package() {
    let files = BTreeMap::from([
        (PathBuf::from("assets"), vec![]),
        (PathBuf::from("assets/image.png"), vec![]),
    ]);
    assert!(validate(&files).unwrap_err().contains("冲突"));
}

#[test]
fn every_callback_cancellation_boundary_returns_without_panicking_in_zip_cleanup() {
    let files = BTreeMap::from([
        (PathBuf::from("a.html"), vec![b'a'; CHUNK * 2]),
        (PathBuf::from("b.html"), vec![b'b'; CHUNK * 2]),
    ]);
    let mut calls = 0usize;
    encode(&files, &mut |_, _| {
        calls += 1;
        true
    })
    .unwrap();
    for stop in 1..=calls {
        let mut current = 0;
        let error = encode(&files, &mut |_, _| {
            current += 1;
            current < stop
        })
        .unwrap_err();
        assert!(
            error.contains("READER_CANCELLED"),
            "callback {stop}: {error}"
        );
    }
}

#[test]
fn budget_exhaustion_during_zip_headers_and_finish_never_returns_partial_bytes() {
    let files = BTreeMap::from([(PathBuf::from("index.html"), vec![b'x'; CHUNK * 2])]);
    let full = encode(&files, &mut |_, _| true).unwrap();
    for limit in [0, 10, 32, full.len() / 2, full.len() - 1] {
        assert!(
            encode_with_budget(&files, &mut |_, _| true, limit)
                .unwrap_err()
                .contains("ZIP预算"),
            "budget {limit}"
        );
    }
}
