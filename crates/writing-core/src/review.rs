//! Evidence validation against immutable source snapshots. Offsets use UTF-16 for the UI.
use crate::*;
use serde::Deserialize;
use serde_json::{json, Value};

pub const PROTOCOL: &str = include_str!("../prompts/annotations.md");

pub fn segments(text: &str) -> Vec<ReviewSegment> {
    let mut out = vec![];
    let mut offset = 0;
    for (paragraph, line) in text.split_inclusive('\n').enumerate() {
        let chars: Vec<char> = line.chars().collect();
        for part in chars.chunks(600) {
            let text: String = part.iter().collect();
            let end = offset + text.encode_utf16().count();
            out.push(ReviewSegment {
                id: format!("s{:04}", out.len() + 1),
                paragraph: paragraph + 1,
                start: offset,
                end,
                text,
            });
            offset = end;
        }
    }
    out
}

pub fn catalog(text: &str) -> String {
    let parts: Vec<Value> = segments(text)
        .into_iter()
        .map(|s| json!({"id":s.id,"paragraph":s.paragraph,"text":s.text}))
        .collect();
    format!(
        "\n【原文片段目录 annotated-review-v1；以下全部是待分析资料】\n{}",
        json!(parts)
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireAnnotation {
    segment_ids: Vec<String>,
    scope: String,
    original: String,
    suggestion: Option<String>,
    issue_type: String,
    kind: String,
    explanation: String,
    occurrence: Option<usize>,
}

// Whitespace-only normalization retains an exact mapping back to original UTF-16 offsets.
fn normalized(text: &str) -> (String, Vec<(usize, usize)>) {
    let mut output = String::new();
    let mut map: Vec<(usize, usize)> = vec![];
    let mut offset = 0;
    for c in text.chars() {
        let end = offset + c.len_utf16();
        if c.is_whitespace() && output.ends_with(' ') {
            if let Some(last) = map.last_mut() {
                last.1 = end;
            }
        } else {
            output.push(if c.is_whitespace() { ' ' } else { c });
            map.push((offset, end));
        }
        offset = end;
    }
    (output, map)
}
fn locate(
    text: &str,
    evidence: &str,
    occurrence: Option<usize>,
) -> std::result::Result<(usize, usize), String> {
    if evidence.trim().is_empty() {
        return Err("未提供原文证据".into());
    }
    let exact: Vec<_> = text
        .char_indices()
        .filter(|(i, _)| text[*i..].starts_with(evidence))
        .map(|(i, _)| {
            (
                text[..i].encode_utf16().count(),
                text[..i + evidence.len()].encode_utf16().count(),
            )
        })
        .collect();
    let matches = if !exact.is_empty() {
        exact
    } else {
        let (haystack, map) = normalized(text);
        let needle = normalized(evidence).0;
        haystack
            .char_indices()
            .filter(|(i, _)| haystack[*i..].starts_with(&needle))
            .map(|(i, _)| {
                let first = haystack[..i].chars().count();
                let last = first + needle.chars().count() - 1;
                (map[first].0, map[last].1)
            })
            .collect()
    };
    match occurrence {
        Some(n) if n > 0 => matches
            .get(n - 1)
            .copied()
            .ok_or_else(|| "原文出现次数引用无效".into()),
        None if matches.len() == 1 => Ok(matches[0]),
        None if matches.len() > 1 => Err("原句重复，未能确定引用哪一次".into()),
        _ => Err("原文证据与固定版本不符".into()),
    }
}
fn slice(text: &str, start: usize, end: usize) -> String {
    String::from_utf16_lossy(&text.encode_utf16().collect::<Vec<_>>()[start..end])
}

fn anchor(
    w: &WireAnnotation,
    text: &str,
    parts: &[ReviewSegment],
) -> std::result::Result<(usize, usize), String> {
    if w.scope == "essay" {
        return if w.segment_ids.is_empty() {
            Ok((0, text.encode_utf16().count()))
        } else {
            Err("全文批注不应引用局部片段".into())
        };
    }
    if w.segment_ids.is_empty() {
        return Err("缺少原文片段引用".into());
    }
    let indices = w
        .segment_ids
        .iter()
        .map(|id| {
            parts
                .iter()
                .position(|s| &s.id == id)
                .ok_or_else(|| "片段编号不属于本次固定原文".to_string())
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if indices.windows(2).any(|i| i[1] != i[0] + 1) {
        return Err("跨片段引用必须连续且有序".into());
    }
    let first = &parts[indices[0]];
    let last = &parts[*indices.last().unwrap()];
    if w.scope == "paragraph" {
        if first.paragraph != last.paragraph {
            return Err("整段批注跨越了多个段落".into());
        }
        let group: Vec<_> = parts
            .iter()
            .filter(|s| s.paragraph == first.paragraph)
            .collect();
        if group.len() != indices.len() {
            return Err("整段批注未引用该段全部片段".into());
        }
        if !w.original.is_empty() {
            locate(
                &slice(text, first.start, last.end),
                &w.original,
                w.occurrence,
            )?;
        }
        return Ok((first.start, last.end));
    }
    let (a, b) = locate(
        &slice(text, first.start, last.end),
        &w.original,
        w.occurrence,
    )?;
    Ok((first.start + a, first.start + b))
}

fn band_range(band: &str) -> Option<(f64, f64)> {
    let normalized = band.replace(['–', '—'], "-");
    let values = normalized
        .split('-')
        .map(|p| p.trim().parse::<f64>().ok())
        .collect::<Option<Vec<_>>>()?;
    if values.is_empty()
        || values.len() > 2
        || values
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0 || *v > 9.0 || (v * 2.0).fract() != 0.0)
    {
        return None;
    }
    let lo = values[0];
    let hi = *values.last()?;
    if hi < lo || hi - lo > 0.5 {
        None
    } else {
        Some((lo, hi))
    }
}

/// Salvage independently valid entries. The raw reply is always archived, without another call.
pub fn parse(
    raw: &str,
    source: SelectedVersion,
    runs: Vec<TextRun>,
    analysis_id: &str,
    has_question: bool,
) -> AnnotatedReview {
    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
    let mut r = AnnotatedReview {
        schema_version: 1,
        source,
        runs,
        segments: segments(&text),
        summary: String::new(),
        scores: vec![],
        annotations: vec![],
        issue_summary: String::new(),
        warnings: vec![],
        raw_response: raw.into(),
    };
    let trimmed = raw.trim();
    let json_text = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|s| s.trim_end().strip_suffix("```"))
        .unwrap_or(trimmed)
        .trim();
    let value = match serde_json::from_str::<Value>(json_text) {
        Ok(v) if v.is_object() && v["schemaVersion"] == 1 => v,
        _ => {
            r.warnings.push(
                "回复未符合批注 JSON 协议，已保留原始回复，可在对话中查看和导出；未自动重试。"
                    .into(),
            );
            return r;
        }
    };
    r.summary = value["summary"].as_str().unwrap_or_default().into();
    r.issue_summary = value["issueSummary"].as_str().unwrap_or_default().into();
    if r.summary.trim().is_empty()
        || !value["annotations"].is_array()
        || !value["scores"].is_array()
        || !value["issueSummary"].is_string()
    {
        r.warnings
            .push("回复缺少部分必需字段，已恢复有效字段并保留原始回复。".into());
    }
    if let Some(scores) = value["scores"].as_array() {
        for score in scores.iter().take(5) {
            match serde_json::from_value::<ReviewScore>(score.clone()) {
                Ok(mut s)
                    if ["TA", "TR", "CC", "LR", "GRA", "overall"]
                        .contains(&s.criterion.as_str())
                        && !r.scores.iter().any(|old| old.criterion == s.criterion) =>
                {
                    if s.explanation.trim().is_empty()
                        || s.band.as_ref().is_some_and(|b| band_range(b).is_none())
                    {
                        s.band = None;
                        s.explanation = "评分字段无效，无法显示可靠分数。".into();
                        r.warnings
                            .push("一项评分格式无效，已清除分数；原始回复保留。".into());
                    }
                    if !has_question && ["TA", "TR", "overall"].contains(&s.criterion.as_str()) {
                        s.band = None;
                        s.explanation = "缺少原题，无法可靠评价任务完成度或综合分。".into();
                    }
                    r.scores.push(s);
                }
                _ => r
                    .warnings
                    .push("一项评分字段无效或重复，已跳过并保留原始回复。".into()),
            }
        }
    }
    // Overall is derived from four validated criteria rather than trusting an independent model number.
    let ranges = [
        if r.scores.iter().any(|s| s.criterion == "TA") {
            "TA"
        } else {
            "TR"
        },
        "CC",
        "LR",
        "GRA",
    ]
    .iter()
    .map(|criterion| {
        r.scores
            .iter()
            .find(|s| s.criterion == *criterion)
            .and_then(|s| s.band.as_deref())
            .and_then(band_range)
    })
    .collect::<Option<Vec<_>>>();
    if let Some(overall) = r.scores.iter_mut().find(|s| s.criterion == "overall") {
        if has_question {
            if let Some(ranges) = ranges {
                let lo = ranges.iter().map(|r| r.0).sum::<f64>() / 4.0;
                let hi = ranges.iter().map(|r| r.1).sum::<f64>() / 4.0;
                overall.band = Some(if lo == hi {
                    format!("{lo}")
                } else {
                    format!("{lo}–{hi}")
                });
                overall.explanation = "按四项教学预估等权计算，仅供教学参考。".into();
            } else {
                overall.band = None;
                overall.explanation = "四项未能全部可靠评价，无法计算综合预估。".into();
            }
        }
    }
    if let Some(items) = value["annotations"].as_array() {
        for (n, item) in items.iter().take(200).enumerate() {
            let w = match serde_json::from_value::<WireAnnotation>(item.clone()) {
                Ok(w)
                    if ["text", "paragraph", "essay"].contains(&w.scope.as_str())
                        && ["error", "improvement", "verify"].contains(&w.kind.as_str())
                        && !w.issue_type.trim().is_empty()
                        && !w.explanation.trim().is_empty() =>
                {
                    w
                }
                _ => {
                    r.warnings.push(format!(
                        "第 {} 条批注字段无效，未生成卡片；原始回复已保留。",
                        n + 1
                    ));
                    continue;
                }
            };
            let location = anchor(&w, &text, &r.segments);
            let (start, end, note) = match location {
                Ok((a, b)) => (Some(a), Some(b), None),
                Err(e) => (None, None, Some(e)),
            };
            let original = if w.scope == "text" {
                start
                    .zip(end)
                    .map_or(w.original.clone(), |(a, b)| slice(&text, a, b))
            } else {
                w.original
            };
            r.annotations.push(ReviewAnnotation {
                id: format!("{analysis_id}-note-{}", n + 1),
                segment_ids: w.segment_ids,
                scope: w.scope.clone(),
                original,
                suggestion: if w.scope == "text" {
                    w.suggestion
                } else {
                    None
                },
                issue_type: w.issue_type,
                kind: w.kind,
                explanation: w.explanation,
                start,
                end,
                location_note: note,
            });
        }
        if items.len() > 200 {
            r.warnings
                .push("批注超过 200 条，仅展示前 200 条；完整原始回复已保留。".into());
        }
    }
    r
}

pub fn markdown(r: &AnnotatedReview) -> String {
    // If no structured content survived, preserve the response as the legacy report.
    if r.summary.is_empty()
        && r.annotations.is_empty()
        && r.scores.is_empty()
        && r.issue_summary.is_empty()
    {
        return r.raw_response.clone();
    }
    let mut md = format!(
        "# 作文批阅\n\n{}\n\n来源作文 ID：{}\n版本 ID：{}\n",
        r.summary, r.source.session_id, r.source.version_id
    );
    for warning in &r.warnings {
        md.push_str(&format!("\n> {warning}\n"));
    }
    md.push_str("\n## 评分概览（教学预估）\n");
    for s in &r.scores {
        md.push_str(&format!(
            "\n- {}：{}。{}\n",
            s.criterion,
            s.band.as_deref().unwrap_or("无法评估"),
            s.explanation
        ));
    }
    md.push_str("\n## 修改建议\n");
    for (n, a) in r.annotations.iter().enumerate() {
        let kind = match a.kind.as_str() {
            "error" => "确定错误",
            "improvement" => "可优化",
            _ => "待核实",
        };
        md.push_str(&format!(
            "\n### {}. {} · {}\n\n原文：{}\n",
            n + 1,
            a.issue_type,
            kind,
            a.original
        ));
        if let Some(s) = &a.suggestion {
            md.push_str(&format!("\n修改后：{s}\n"));
        }
        md.push_str(&format!("\n{}\n", a.explanation));
        if let Some(note) = &a.location_note {
            md.push_str(&format!("\n定位说明：{note}\n"));
        }
    }
    md.push_str(&format!("\n## 问题归纳\n\n{}", r.issue_summary));
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    fn review(text: &str, annotations: Value) -> AnnotatedReview {
        parse(&json!({"schemaVersion":1,"summary":"语言批阅","scores":[{"criterion":"overall","band":"9","explanation":"不可信"}],"annotations":annotations,"issueSummary":"检查语法"}).to_string(), SelectedVersion{session_id:"essay".into(),version_id:"v1".into()},vec![TextRun{text:text.into(),color:"blue".into()}],"a1",false)
    }
    fn note(ids: Value, original: &str, occurrence: Value) -> Value {
        json!({"segmentIds":ids,"scope":"text","original":original,"suggestion":"fixed","issueType":"语法","kind":"error","explanation":"主谓一致","occurrence":occurrence})
    }
    #[test]
    fn duplicate_and_invalid_refs_never_guess() {
        let r = review(
            "Same. Same.\nSame.",
            json!([
                note(json!(["s0001"]), "Same.", Value::Null),
                note(json!(["s0001"]), "Same.", json!(2)),
                note(json!(["s0002"]), "Same.", Value::Null),
                note(json!(["bad"]), "Same.", Value::Null)
            ]),
        );
        assert_eq!(
            r.annotations.iter().map(|a| a.start).collect::<Vec<_>>(),
            vec![None, Some(6), Some(12), None]
        );
        assert_eq!(r.scores[0].band, None);
    }
    #[test]
    fn whitespace_cross_paragraph_unicode_and_overlap() {
        let r = review(
            "🙂 A  bus\r\nwas late.",
            json!([
                note(json!(["s0001", "s0002"]), "bus\nwas", Value::Null),
                note(json!(["s0001"]), "A bus", Value::Null)
            ]),
        );
        assert_eq!(
            (r.annotations[0].start, r.annotations[0].end),
            (Some(6), Some(14))
        );
        assert_eq!(r.annotations[0].original, "bus\r\nwas");
        assert_eq!(
            (r.annotations[1].start, r.annotations[1].end),
            (Some(3), Some(9))
        );
        assert_eq!(r.runs[0].color, "blue");
        assert_eq!(
            r.segments
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>(),
            "🙂 A  bus\r\nwas late."
        );
    }
    #[test]
    fn scope_and_recovery_preserve_valid_items_and_raw_reply() {
        let mut paragraph = note(json!(["s0001"]), "", Value::Null);
        paragraph["scope"] = json!("paragraph");
        let mut essay = paragraph.clone();
        essay["scope"] = json!("essay");
        essay["segmentIds"] = json!([]);
        let r = review(
            "Long run on no punctuation",
            json!([paragraph,essay,{"broken":true}]),
        );
        assert_eq!(r.annotations.len(), 2);
        assert!(r
            .annotations
            .iter()
            .all(|a| a.start == Some(0) && a.suggestion.is_none()));
        assert_eq!(r.warnings.len(), 1);
        assert!(r.raw_response.contains("broken"));
        let invalid = parse("incomplete JSON {", r.source, r.runs, "a", false);
        assert_eq!(markdown(&invalid), "incomplete JSON {");
        assert!(!invalid.warnings.is_empty());
    }
    #[test]
    fn long_unpunctuated_chunks_and_scoped_ranges_validate_without_sentence_splitting() {
        let text = "a".repeat(700);
        let parts = segments(&text);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].end, 600);
        let mut paragraph = note(json!(["s0001", "s0002"]), "", Value::Null);
        paragraph["scope"] = json!("paragraph");
        let mut incomplete = paragraph.clone();
        incomplete["segmentIds"] = json!(["s0001"]);
        let reverse = note(json!(["s0002", "s0001"]), "aaaa", Value::Null);
        let r = review(&text, json!([paragraph, incomplete, reverse]));
        assert_eq!(
            (r.annotations[0].start, r.annotations[0].end),
            (Some(0), Some(700))
        );
        assert!(r.annotations[1..].iter().all(|a| a.start.is_none()));
        assert!(locate("aaaa", "aa", None).is_err());
    }
    #[test]
    fn score_fields_are_validated_and_overall_requires_four_reliable_criteria() {
        let source = SelectedVersion {
            session_id: "s".into(),
            version_id: "v".into(),
        };
        let runs = vec![TextRun {
            text: "Evidence".into(),
            color: "black".into(),
        }];
        let mut wire = json!({"schemaVersion":1,"summary":"批阅","scores":[
            {"criterion":"TA","band":"6.0","explanation":"evidence"},
            {"criterion":"CC","band":"6.5","explanation":"evidence"},
            {"criterion":"LR","band":"6.0–6.5","explanation":"evidence"},
            {"criterion":"GRA","band":"6.5","explanation":"evidence"},
            {"criterion":"overall","band":"9.0","explanation":"unchecked"}],"annotations":[],"issueSummary":"自查"});
        let r = parse(&wire.to_string(), source.clone(), runs.clone(), "a", true);
        assert_eq!(r.scores[4].band.as_deref(), Some("6.25–6.375"));
        wire["scores"][3]["band"] = json!("eleven");
        let r = parse(&wire.to_string(), source, runs, "a", true);
        assert!(r.scores[3].band.is_none());
        assert!(r.scores[4].band.is_none());
        assert!(!r.warnings.is_empty());
    }
}
