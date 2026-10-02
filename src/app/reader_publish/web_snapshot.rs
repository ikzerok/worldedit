use super::super::WorldeditApp;

impl WorldeditApp {
    pub(super) fn reader_worker_snapshot(
        &self,
    ) -> Result<
        (
            worldline_core::project::SnapshotState,
            crate::archive::Files,
            std::path::PathBuf,
        ),
        String,
    > {
        let state = self.project.snapshot_state()?;
        let retained_count = state
            .documents
            .iter()
            .filter(|document| document.retained_bytes.is_some())
            .count();
        let retained_bytes = state
            .documents
            .iter()
            .filter_map(|document| document.retained_bytes.as_ref())
            .try_fold(0usize, |total, bytes| {
                total.checked_add(bytes.len()).ok_or("后台墓碑大小溢出")
            })?;
        let files = self.project.snapshot_files_limited(
            crate::reader_zip::MAX_FILES
                .checked_sub(retained_count)
                .ok_or("后台快照文件数超过预算")?,
            crate::reader_zip::MAX_BYTES
                .checked_sub(retained_bytes)
                .ok_or("后台快照字节数超过预算")?,
        )?;
        let entry = self
            .project
            .entry
            .strip_prefix(&self.project.root)
            .map_err(|_| "工程入口不在工作区内")?
            .to_path_buf();
        Ok((state, files, entry))
    }
}
