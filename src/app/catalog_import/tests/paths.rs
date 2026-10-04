use super::super::mapping::portable_destination;
use std::path::Path;

#[test]
fn catalog_destination_serializes_normal_components_without_loss() {
    assert_eq!(
        portable_destination(Path::new("资料/第一章.wl"))
            .unwrap()
            .to_str(),
        Some("资料/第一章.wl")
    );
    assert_eq!(
        portable_destination(Path::new("chapter#1%.wl"))
            .unwrap()
            .to_str(),
        Some("chapter#1%.wl")
    );
    for unsafe_path in ["", "/world.wl", "../world.wl", "chapter/../world.wl"] {
        assert!(
            portable_destination(Path::new(unsafe_path)).is_err(),
            "{unsafe_path}"
        );
    }
}

#[cfg(windows)]
#[test]
fn windows_nested_catalog_destination_becomes_forward_slash_dto() {
    let root = Path::new(r"C:\work\novel");
    let native = root.join(r"events\chapter.wl");
    assert_eq!(
        portable_destination(native.strip_prefix(root).unwrap())
            .unwrap()
            .to_str(),
        Some("events/chapter.wl")
    );
}

#[cfg(not(windows))]
#[test]
fn non_windows_literal_backslash_cannot_redirect_to_another_source() {
    assert!(portable_destination(Path::new(r"events\chapter.wl")).is_err());
}

#[cfg(unix)]
#[test]
fn non_utf8_catalog_destination_is_rejected_instead_of_lossy_selected() {
    use std::os::unix::ffi::OsStrExt;
    let path = Path::new(std::ffi::OsStr::from_bytes(b"events/\xff.wl"));
    assert!(portable_destination(path).is_err());
}
