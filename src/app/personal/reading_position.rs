//! 临时资料的返回身份；不保存或恢复对象内容。
use crate::app::WorldeditApp;
use std::path::PathBuf;
use worldline_core::TargetRef;

#[derive(Clone, PartialEq)]
pub(in crate::app) struct ReadingPosition {
    target: TargetRef,
    history: Vec<TargetRef>,
    source_return: Option<(PathBuf, usize)>,
}

impl WorldeditApp {
    pub(in crate::app) fn capture_reading_position(&self) -> Option<ReadingPosition> {
        Some(ReadingPosition {
            target: self.reading_target.clone()?,
            history: self.reading_history.clone(),
            source_return: self.reading_return.clone(),
        })
    }

    pub(in crate::app) fn restore_reading_position(
        &mut self,
        saved: Option<ReadingPosition>,
        baseline: Option<&str>,
    ) {
        self.close_transient_reading();
        self.reading_return = None;
        let Some(saved) = saved else {
            return;
        };
        let current = |target: &TargetRef| {
            self.snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.result.analysis.catalog.object(target).is_some())
        };
        if !current(&saved.target) {
            self.message = Some("上次阅读的对象已不存在，未按同名替换".into());
            return;
        }
        self.reading_history = saved.history.into_iter().filter(current).collect();
        self.reading_target = Some(saved.target);
        self.reading_return = saved.source_return.filter(|(path, _)| {
            self.source_position_document(path)
                .is_some_and(|(_, source)| {
                    baseline == Some(super::super::writing_workspace::fingerprint(source).as_str())
                })
        });
    }
}
