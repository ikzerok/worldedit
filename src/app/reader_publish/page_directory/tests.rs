use super::*;
use worldline_core::reader_export::ReaderContentPreview;

fn preview(count: usize) -> ReaderExportPreview {
    ReaderExportPreview {
        schema_version: 3,
        plan_digest: "verified-first".into(),
        content_baseline: "baseline".into(),
        included: vec![],
        exclusions: vec![],
        content: (0..count)
            .map(|index| ReaderContentPreview {
                title: "同名页 ÉCLAIR".into(),
                output_path: format!("objects/public-{index:04}.html"),
                text: format!("公开正文 {index} MiXeD\n正文保留  两个空格"),
                empty_content: false,
            })
            .collect(),
    }
}

#[test]
fn public_directory_reaches_thousandth_result_and_keeps_duplicate_titles_distinct() {
    let preview = preview(1000);
    let mut directory = PageDirectory::default();
    directory.sync(&preview);
    assert_eq!(directory.matches.len(), 1000);
    assert_eq!(directory.page_count(), 84);
    let mut paths = Vec::new();
    for page in 0..directory.page_count() {
        directory.set_page(page);
        assert!(directory.range().len() <= PAGE_SIZE);
        paths.extend(
            directory
                .range()
                .map(|offset| directory.index[directory.matches[offset]].path.clone()),
        );
    }
    assert_eq!(
        paths,
        preview
            .content
            .iter()
            .map(|page| page.output_path.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(directory.range().len(), 4);
    directory.open(paths.last().unwrap());
    assert_eq!(directory.opened_index(), Some(999));
    assert_eq!(
        directory.opened_path.as_deref(),
        Some("objects/public-0999.html")
    );
}

#[test]
fn public_directory_matches_unicode_lowercase_chinese_paths_and_only_trims_query() {
    let preview = preview(4);
    let mut directory = PageDirectory::default();
    directory.sync(&preview);
    for query in [
        "éClAiR",
        "  ÉCLAIR  ",
        "公开正文",
        "mixed",
        "\t \n",
        "保留  两个",
    ] {
        directory.query = query.into();
        directory.sync(&preview);
        assert_eq!(directory.matches.len(), 4, "{query:?}");
    }
    for query in [
        "正文保留 两个",
        "HIDDEN_SECRET_SENTINEL",
        "no-such-public-content",
    ] {
        directory.query = query.into();
        directory.sync(&preview);
        assert!(directory.matches.is_empty(), "{query}");
        assert!(directory.candidate.is_none());
        assert!(directory.range().is_empty());
    }
    directory.query = "PUBLIC-0003.HTML".into();
    directory.sync(&preview);
    assert_eq!(directory.matches, vec![3]);
}

#[test]
fn public_directory_query_resets_candidate_and_page_without_opening_or_reindexing_each_frame() {
    let preview = preview(26);
    let mut directory = PageDirectory::default();
    directory.sync(&preview);
    directory.set_page(2);
    directory.open("objects/public-0025.html");
    directory.show();
    for _ in 0..10 {
        directory.sync(&preview);
    }
    assert_eq!(directory.searches, 1);
    assert_eq!(directory.page, 2);
    directory.query = "0004".into();
    directory.sync(&preview);
    assert_eq!(directory.searches, 2);
    assert_eq!(directory.page, 0);
    assert_eq!(
        directory.candidate.as_deref(),
        Some("objects/public-0004.html")
    );
    assert_eq!(directory.opened_index(), Some(25));
    assert!(directory.visible);
    directory.open_in_public_order(24);
    assert_eq!(
        directory.opened_index(),
        Some(24),
        "reader navigation must ignore the filter"
    );
}

#[test]
fn public_directory_navigation_clamps_and_crosses_page_boundaries() {
    let preview = preview(25);
    let mut directory = PageDirectory::default();
    directory.sync(&preview);
    directory.move_candidate(false);
    assert_eq!(directory.page, 0);
    for _ in 0..12 {
        directory.move_candidate(true);
    }
    assert_eq!(directory.page, 1);
    assert_eq!(
        directory.candidate.as_deref(),
        Some("objects/public-0012.html")
    );
    for _ in 0..20 {
        directory.move_candidate(true);
    }
    assert_eq!(directory.page, 2);
    assert_eq!(
        directory.candidate.as_deref(),
        Some("objects/public-0024.html")
    );
    for _ in 0..30 {
        directory.move_candidate(false);
    }
    assert_eq!(directory.page, 0);
    assert_eq!(
        directory.candidate.as_deref(),
        Some("objects/public-0000.html")
    );
    directory.query = "unmatched".into();
    directory.sync(&preview);
    directory.move_candidate(true);
    assert!(directory.candidate.is_none());
    assert_eq!(directory.page, 0);
}

#[test]
fn public_directory_bundle_replacement_clears_query_cache_and_old_path_identity() {
    let mut preview = preview(30);
    let mut directory = PageDirectory::default();
    directory.sync(&preview);
    directory.set_page(2);
    directory.open("objects/public-0029.html");
    directory.query = "0029".into();
    directory.sync(&preview);
    preview.plan_digest = "verified-second".into();
    preview.content.reverse();
    preview.content.truncate(3);
    preview.content[0].output_path = "objects/replacement.html".into();
    directory.sync(&preview);
    assert!(directory.query.is_empty());
    assert_eq!(directory.page, 0);
    assert_eq!(directory.matches.len(), 3);
    assert_eq!(
        directory.opened_path.as_deref(),
        Some("objects/replacement.html")
    );
    directory.open("objects/public-0000.html");
    assert_eq!(
        directory.opened_path.as_deref(),
        Some("objects/replacement.html")
    );
}

#[test]
fn invalidating_review_clears_directory_and_retains_publication_choices() {
    let mut state = crate::app::reader_publish::ReaderPublishState::new();
    state
        .objects
        .insert(worldline_core::catalog::TargetRef::new("event", "public"));
    let selection = state.selection();
    state.page_directory.sync(&preview(30));
    state.page_directory.query = "public".into();
    state.page_directory.set_page(2);
    state.invalidate_review();
    assert!(state.page_directory.query.is_empty());
    assert!(state.page_directory.opened_path.is_none());
    assert!(state.page_directory.index.is_empty());
    assert!(state.page_directory.matches.is_empty());
    assert_eq!(state.selection(), selection);
}

mod keyboard;
