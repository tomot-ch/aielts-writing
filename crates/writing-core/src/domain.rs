pub use crate::models::*;
use crate::Result;
use uuid::Uuid;
pub(crate) fn id(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4().simple())
}
pub(crate) fn safe_name(s: &str) -> String {
    let v: String = s
        .chars()
        .filter(|c| !"\\/:*?\"<>|".contains(*c) && !c.is_control())
        .take(40)
        .collect();
    let v = v.trim().trim_end_matches('.').to_string();
    if v.is_empty() {
        "未命名".into()
    } else {
        v
    }
}
pub(crate) fn word_count(runs: &[TextRun]) -> usize {
    let text = runs.iter().map(|r| r.text.as_str()).collect::<String>();
    let mut count = 0;
    let mut latin = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_alphanumeric() {
            if !latin {
                count += 1;
                latin = true;
            }
        } else if (c == '\'' || c == '’')
            && latin
            && chars
                .peek()
                .is_some_and(|next| next.is_ascii_alphanumeric())
        {
            // An apostrophe inside an English word does not start a new word.
        } else if ('\u{4e00}'..='\u{9fff}').contains(&c) {
            count += 1;
            latin = false;
        } else {
            latin = false;
        }
    }
    count
}
pub(crate) fn validate_runs(runs: &[TextRun]) -> Result<()> {
    if runs.len() > 100_000 {
        return Err("文字片段过多".into());
    }
    let mut len = 0;
    for r in runs {
        if !["black", "blue", "green", "red"].contains(&r.color.as_str()) {
            return Err("无效文字颜色".into());
        };
        len += r.text.len();
    }
    if len > 5_000_000 {
        return Err("作文内容过长".into());
    }
    Ok(())
}
