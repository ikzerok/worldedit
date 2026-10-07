//! 执行边界只枚举会影响正文/事件的未应用意图，不把纯书稿编排当运行内容。
use super::{creation::SourceChoice, WorkbenchState};
use std::collections::BTreeMap;
use std::path::Path;

impl WorkbenchState {
    /// 展示摘要和完整内容签名同源；不同来源角色使用独立前缀，签名不依赖标题。
    pub(in crate::app) fn runtime_drafts(&self, root: &Path) -> BTreeMap<String, String> {
        let mut inputs = BTreeMap::new();
        for buffer in self
            .writing_buffers
            .values()
            .filter(|buffer| buffer.is_changed())
        {
            let path = buffer.path().strip_prefix(root).unwrap_or(buffer.path());
            inputs.insert(
                format!("正文文件 · {}", path.display()),
                serde_json::json!([
                    "writing_buffer",
                    buffer.path(),
                    buffer.baseline(),
                    buffer.generation(),
                    buffer.source()
                ])
                .to_string(),
            );
        }
        if let Some(form) = self
            .creation
            .as_ref()
            .filter(|form| form.touched && form.source_choice == SourceChoice::NewEvent)
        {
            inputs.insert(
                format!(
                    "待创建正文 · {} · event:{} · {}",
                    form.chapter_title, form.event_id, form.source_path
                ),
                serde_json::json!([
                    "new_event",
                    form.existing_book,
                    form.book_id,
                    form.book_title,
                    form.chapter_id,
                    form.chapter_title,
                    form.parent,
                    form.after,
                    form.event_id,
                    form.source_path,
                    form.new_file,
                    form.storyline,
                    form.preview
                        .as_ref()
                        .map(|(request, plan)| (request, &plan.plan_digest))
                ])
                .to_string(),
            );
        }
        inputs.extend(self.writing_view.retained_runtime_drafts(root));
        inputs
    }
}
