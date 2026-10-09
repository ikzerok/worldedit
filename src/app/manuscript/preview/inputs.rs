use super::super::WorkbenchState;
use std::path::PathBuf;

impl WorkbenchState {
    /// 便宜的会话失效观察；不克隆/散列源文本，不读取磁盘。
    pub(in crate::app) fn writing_buffer_stamps(&self) -> Vec<(PathBuf, u64, bool)> {
        let mut values: Vec<_> = self
            .writing_buffers
            .values()
            .map(|buffer| {
                (
                    buffer.path().to_owned(),
                    buffer.generation(),
                    buffer.is_changed(),
                )
            })
            .collect();
        values.sort_by(|left, right| left.0.cmp(&right.0));
        values
    }
    pub(in crate::app) fn has_retained_writing_input(&self) -> bool {
        self.writing_view.has_retained_input()
    }
}
