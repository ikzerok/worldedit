//! worldline 语法高亮:为 egui TextEdit 生成 LayoutJob。
//! 规则与 `worldline/spec/syntax.md` 一致:注释/字符串/关键字/跃迁/标签/插值。
//! 全部按字节偏移工作(关键字与标记均为 ASCII,段落边界只落在 ASCII 边界上)。

use egui::text::{LayoutJob, LayoutSection, TextFormat};
use egui::{Color32, FontId};
use worldline_core::LanguageVersion;

const KEYWORDS: &[&str] = &[
    "alias",
    "state",
    "become",
    "has",
    "on",
    "enter",
    "exit",
    "done",
    "event",
    "scene",
    "choice",
    "once",
    "if",
    "else",
    "let",
    "const",
    "set",
    "include",
    "and",
    "or",
    "not",
    "true",
    "false",
    "storyline",
    "character",
    "effect",
    "grant",
    "revoke",
    "meet",
    "part",
    "anchor",
    "to",
    "world",
    "description",
    "property",
    "relation",
    "at",
    "with",
    "as",
    "perm",
    "after",
    "period",
    "tag",
    "asset",
    "mark",
    "attach",
    "during",
    "follows",
];

fn c_default() -> Color32 {
    crate::theme::TEXT()
}
fn c_keyword() -> Color32 {
    crate::theme::ACCENT()
}
fn c_symbol() -> Color32 {
    crate::theme::GOLD()
}
fn c_string() -> Color32 {
    if crate::theme::is_light() {
        Color32::from_rgb(34, 106, 49)
    } else {
        Color32::from_rgb(150, 220, 150)
    }
}
fn c_comment() -> Color32 {
    crate::theme::MUTED()
}
fn c_divert() -> Color32 {
    if crate::theme::is_light() {
        Color32::from_rgb(146, 62, 19)
    } else {
        Color32::from_rgb(255, 150, 90)
    }
}
fn c_tag() -> Color32 {
    if crate::theme::is_light() {
        Color32::from_rgb(106, 49, 155)
    } else {
        Color32::from_rgb(200, 160, 255)
    }
}
fn c_interp() -> Color32 {
    if crate::theme::is_light() {
        Color32::from_rgb(0, 106, 103)
    } else {
        Color32::from_rgb(110, 220, 210)
    }
}

/// 生成整段源码的 LayoutJob(逐行状态机,支持跨行块注释)。
pub fn layout_job(text: &str, size: f32, language_version: LanguageVersion) -> LayoutJob {
    if text.is_empty() {
        return LayoutJob::simple(
            String::new(),
            FontId::monospace(size),
            c_default(),
            f32::INFINITY,
        );
    }
    let mut job = LayoutJob {
        text: text.into(),
        ..Default::default()
    };
    let mut in_block = false;
    let mut pos = 0usize;
    for line in text.split('\n') {
        highlight_line(&mut job, line, pos, &mut in_block, size, language_version);
        let end = pos + line.len();
        let covered = job
            .sections
            .last()
            .map_or(0, |section| section.byte_range.end);
        push(&mut job, covered..end, c_default(), size);
        // LayoutJob 的片段必须完整覆盖字符，空行也需要字体度量。
        push(&mut job, end..(end + 1).min(text.len()), c_default(), size);
        pos = end + 1; // 含换行符
    }
    job
}

fn push(job: &mut LayoutJob, range: std::ops::Range<usize>, color: Color32, size: f32) {
    if range.start >= range.end {
        return;
    }
    let covered = job
        .sections
        .last()
        .map_or(0, |section| section.byte_range.end);
    if covered < range.start {
        push(job, covered..range.start, c_default(), size);
    }
    if let Some(last) = job.sections.last_mut() {
        if last.byte_range.end == range.start
            && last.format.color == color
            && last.format.font_id.size == size
        {
            last.byte_range.end = range.end;
            return;
        }
    }
    job.sections.push(LayoutSection {
        leading_space: 0.0,
        byte_range: range,
        format: TextFormat::simple(FontId::monospace(size), color),
    });
}

fn highlight_line(
    job: &mut LayoutJob,
    line: &str,
    base: usize,
    in_block: &mut bool,
    size: f32,
    language_version: LanguageVersion,
) {
    if *in_block {
        if let Some(end) = line.find("*/") {
            push(job, base..base + end + 2, c_comment(), size);
            *in_block = false;
            classify(
                job,
                &line[end + 2..],
                base + end + 2,
                size,
                language_version,
            );
        } else {
            push(job, base..base + line.len(), c_comment(), size);
        }
        return;
    }
    // 找注释起点(字符串感知)
    let bytes = line.as_bytes();
    let mut in_str = false;
    let mut code_end = line.len();
    let mut block: Option<usize> = None;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if in_str => i += 2,
            b'"' => {
                in_str = !in_str;
                i += 1;
            }
            b'/' if !in_str && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                code_end = i;
                i = bytes.len();
            }
            b'/' if !in_str && i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                code_end = i;
                block = Some(i);
                i = bytes.len();
            }
            _ => i += 1,
        }
    }
    classify(job, &line[..code_end], base, size, language_version);
    if let Some(bs) = block {
        let rest = &line[bs..];
        if let Some(end) = rest.find("*/") {
            push(job, base + bs..base + bs + end + 2, c_comment(), size);
            classify(
                job,
                &rest[end + 2..],
                base + bs + end + 2,
                size,
                language_version,
            );
        } else {
            push(job, base + bs..base + line.len(), c_comment(), size);
            *in_block = true;
        }
    } else if code_end < line.len() {
        push(job, base + code_end..base + line.len(), c_comment(), size);
    }
}

/// 代码段分类:关键字行 / 跃迁行 / 正文行。
fn classify(
    job: &mut LayoutJob,
    code: &str,
    base: usize,
    size: f32,
    language_version: LanguageVersion,
) {
    let bytes = code.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    if i >= bytes.len() {
        push(job, base..base + code.len(), c_default(), size);
        return;
    }
    // 跃迁行
    if bytes[i] == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'>' {
        push(job, base + i..base + i + 2, c_divert(), size);
        let rest = &code[i + 2..];
        let ws = rest.len() - rest.trim_start().len();
        let t_start = base + i + 2 + ws;
        if t_start < base + code.len() {
            push(job, t_start..base + code.len(), c_symbol(), size);
        }
        return;
    }
    // 首词
    let mut j = i;
    while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
        j += 1;
    }
    let word = &code[i..j];
    let after_word_boundary = j >= bytes.len() || bytes[j] == b' ' || bytes[j] == b'"';
    let keyword = (language_version.supports_language_111()
        && matches!(
            word,
            "rule" | "fragment" | "local" | "call" | "return" | "say"
        ))
        || KEYWORDS.contains(&word)
        || (i == 0
            && ((language_version.supports_entities() && word == "entity")
                || (language_version.supports_relations()
                    && matches!(word, "relation_type" | "relation_def"))));
    if keyword && after_word_boundary {
        push(job, base + i..base + j, c_keyword(), size);
        inline(job, code, j, base, size);
    } else {
        inline(job, code, i, base, size);
    }
}

/// 行内细节:字符串、#标签、{插值}、行尾 ~ 粘接。
fn inline(job: &mut LayoutJob, code: &str, from: usize, base: usize, size: f32) {
    let bytes = code.as_bytes();
    let mut i = from;
    let mut seg = from;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                push(job, base + seg..base + i, c_default(), size);
                let mut j = i + 1;
                while j < bytes.len() {
                    if bytes[j] == b'\\' {
                        j += 2;
                        continue;
                    }
                    if bytes[j] == b'"' {
                        j += 1;
                        break;
                    }
                    j += 1;
                }
                let end = j.min(bytes.len());
                push(job, base + i..base + end, c_string(), size);
                i = end;
                seg = i;
            }
            b'#' if i == 0 || bytes[i - 1] == b' ' => {
                push(job, base + seg..base + i, c_default(), size);
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != b' ' {
                    j += 1;
                }
                push(job, base + i..base + j, c_tag(), size);
                i = j;
                seg = i;
            }
            b'{' => {
                push(job, base + seg..base + i, c_default(), size);
                let mut j = i + 1;
                let mut depth = 1;
                while j < bytes.len() && depth > 0 {
                    if bytes[j] == b'{' {
                        depth += 1;
                    } else if bytes[j] == b'}' {
                        depth -= 1;
                    }
                    j += 1;
                }
                push(job, base + i..base + j, c_interp(), size);
                i = j;
                seg = i;
            }
            _ => i += 1,
        }
    }
    push(job, base + seg..base + code.len(), c_default(), size);
    // 行尾未转义的 ~ 标记为粘接
    let trimmed = code.trim_end();
    if trimmed.ends_with('~') && !trimmed.ends_with("\\~") {
        let pos = base + trimmed.len() - 1;
        // 重新着色已有片段，不能重复追加同一个字符。
        if let Some(index) = job
            .sections
            .iter()
            .rposition(|section| section.byte_range.contains(&pos))
        {
            let section = job.sections[index].clone();
            let mut replacement = Vec::with_capacity(3);
            if section.byte_range.start < pos {
                replacement.push(LayoutSection {
                    byte_range: section.byte_range.start..pos,
                    ..section.clone()
                });
            }
            replacement.push(LayoutSection {
                byte_range: pos..pos + 1,
                format: TextFormat::simple(FontId::monospace(size), c_divert()),
                leading_space: 0.0,
            });
            if pos + 1 < section.byte_range.end {
                replacement.push(LayoutSection {
                    byte_range: pos + 1..section.byte_range.end,
                    ..section
                });
            }
            job.sections.splice(index..=index, replacement);
        }
    }
}
