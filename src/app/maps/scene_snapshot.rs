//! 工程任务先计墓碑，再以剩余额度读取/复制活动文件。
use worldline_core::project::SnapshotState;

#[cfg(target_arch = "wasm32")]
pub(super) fn capture(
    project: &worldline_core::project::Project,
) -> Result<(crate::archive::Files, SnapshotState), String> {
    use crate::worker_protocol::{MAX_BYTES, MAX_FILES};
    let state = project.snapshot_state()?;
    let (files_left, bytes_left) = remaining(&state, MAX_FILES, MAX_BYTES)?;
    let files = project.snapshot_files_limited(files_left, bytes_left)?;
    Ok((files, state))
}

fn remaining(
    state: &SnapshotState,
    max_files: usize,
    max_bytes: usize,
) -> Result<(usize, usize), String> {
    let mut files = 0usize;
    let mut bytes = 0usize;
    for document in &state.documents {
        if !document.deleted {
            continue;
        }
        let retained = document
            .retained_bytes
            .as_ref()
            .ok_or("删除态缺少精确保留字节")?;
        files = files.checked_add(1).ok_or("墓碑数量溢出")?;
        bytes = bytes.checked_add(retained.len()).ok_or("墓碑字节数溢出")?;
    }
    Ok((
        max_files
            .checked_sub(files)
            .ok_or("墓碑超过后台文件数预算")?,
        max_bytes.checked_sub(bytes).ok_or("墓碑超过后台字节预算")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldline_core::project::SnapshotDocument;
    #[test]
    fn tombstones_reserve_item_and_byte_budget_before_active_files_are_copied() {
        let state = SnapshotState {
            schema_version: 1,
            documents: vec![
                SnapshotDocument {
                    path: "deleted.wl".into(),
                    authoring: false,
                    deleted: true,
                    read_only: false,
                    retained_bytes: Some(vec![0; 7]),
                },
                SnapshotDocument {
                    path: "active.wl".into(),
                    authoring: false,
                    deleted: false,
                    read_only: false,
                    retained_bytes: None,
                },
                SnapshotDocument {
                    path: "empty.json".into(),
                    authoring: true,
                    deleted: true,
                    read_only: true,
                    retained_bytes: Some(Vec::new()),
                },
            ],
        };
        assert_eq!(remaining(&state, 10, 20).unwrap(), (8, 13));
        assert!(remaining(&state, 1, 20).is_err());
        assert!(remaining(&state, 10, 6).is_err());
    }
}
