use crate::theme;
use worldline_core::manuscript::{
    ManuscriptBookDestination, ManuscriptChapterCreatePlan, ManuscriptChapterCreateRequest,
};

pub(super) fn draw(
    ui: &mut egui::Ui,
    request: &ManuscriptChapterCreateRequest,
    plan: &ManuscriptChapterCreatePlan,
    existing_book_title: &str,
) {
    ui.label(theme::muted("2 · 核对后开始写作"));
    ui.add_space(theme::SPACE_MD);
    let book_title = match &request.book {
        ManuscriptBookDestination::New { title, .. } => title,
        ManuscriptBookDestination::Existing { .. } => existing_book_title,
    };
    ui.label(egui::RichText::new(book_title).font(theme::body_font(18.0)));
    ui.label(
        egui::RichText::new(&request.chapter.title)
            .font(theme::body_font(24.0))
            .strong(),
    );
    ui.add_space(theme::SPACE_MD);
    ui.label(
        egui::RichText::new(format!("正式来源：{}:{}", plan.target.kind, plan.target.id)).strong(),
    );
    ui.label(format!(
        "正文文件：{}{}",
        plan.source_path.display(),
        if plan.new_source {
            "（新建）"
        } else {
            "（已有）"
        }
    ));
    ui.add_space(theme::SPACE_MD);
    if plan.entry_before == plan.entry_after {
        ui.label(format!("运行入口保持不变：{}", plan.entry_before));
    } else {
        ui.colored_label(
            theme::WARNING(),
            format!(
                "运行入口将从 {} 改为 {}",
                plan.entry_before, plan.entry_after
            ),
        );
    }
    if plan.runtime_fingerprint_before == plan.runtime_fingerprint_after {
        ui.label("运行指纹保持不变；本次仅创建阅读编排。");
    } else {
        ui.colored_label(
            theme::WARNING(),
            "新事件会改变运行指纹；旧运行保存或检查点不能直接用于新指纹。没有自动串接事件路线。",
        );
    }
    ui.add_space(theme::SPACE_LG);
    ui.label(egui::RichText::new("本次全部改动文件").strong());
    for path in &plan.changed_files {
        ui.add(egui::Label::new(path.to_string_lossy()).wrap());
    }
    ui.add_space(theme::SPACE_MD);
    egui::CollapsingHeader::new("运行与稳定身份")
        .id_salt("chapter-create-plan-technical")
        .show(ui, |ui| {
            ui.label(format!(
                "书稿：manuscript:{} · 章节：{}",
                plan.book_id, plan.chapter_id
            ));
            ui.label(format!("正文对象：{}:{}", plan.target.kind, plan.target.id));
            ui.label(format!("书稿编排：{}", plan.manuscript_path.display()));
            ui.label(format!(
                "分节：{} · 插入位置：{}",
                request
                    .chapter
                    .parent_section_id
                    .as_deref()
                    .unwrap_or("根目录"),
                request
                    .chapter
                    .after_sibling_id
                    .as_deref()
                    .map(|id| format!("{id} 后"))
                    .unwrap_or_else(|| "同级末尾".into())
            ));
            ui.label(format!(
                "入口前后：{} → {}",
                plan.entry_before, plan.entry_after
            ));
            ui.label(format!(
                "运行指纹：{:016x} → {:016x}",
                plan.runtime_fingerprint_before, plan.runtime_fingerprint_after
            ));
        });
    for diagnostic in &plan.diagnostics {
        ui.label(format!("{}：{}", diagnostic.code, diagnostic.message));
    }
    if !plan.can_apply {
        ui.colored_label(theme::ERROR(), "此计划不能应用，请核对诊断并修改输入。");
    }
    ui.add_space(theme::SPACE_MD);
    ui.label(theme::muted("创建会先应用到当前工程；保存后写入文件。"));
}
