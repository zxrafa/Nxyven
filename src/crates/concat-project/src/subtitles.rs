// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 NXYVEN contributors

//! Subtitle and caption engine: SubRip (`.srt`) and WebVTT (`.vtt`) parsing,
//! formatting, overlap detection/repair, and conversion to/from timeline [`Clip`]s.

use serde::{Deserialize, Serialize};

use crate::model::{Clip, ClipKind, TextStyle, Timeline};

/// One timed subtitle cue.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleCue {
    /// Cue identifier (e.g. `"sub1"`).
    pub id: String,
    /// Start timestamp in timeline seconds.
    pub start: f64,
    /// End timestamp in timeline seconds (`end > start`).
    pub end: f64,
    /// Caption text (may contain `\n`).
    pub text: String,
    /// Optional speaker label.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub speaker: String,
}

impl SubtitleCue {
    /// Duration in seconds.
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(1.0 / 60.0)
    }
}

fn parse_timestamp(raw: &str) -> Option<f64> {
    let token = raw.trim().split_whitespace().next()?;
    let normalized = token.replace(',', ".");
    let parts: Vec<&str> = normalized.split(':').collect();
    let (h, m, s) = match parts.as_slice() {
        [h, m, s] => (h.parse::<f64>().ok()?, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        [m, s] => (0.0, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };
    if !h.is_finite() || !m.is_finite() || !s.is_finite() {
        return None;
    }
    Some((h * 3600.0 + m * 60.0 + s).max(0.0))
}

fn format_timestamp(seconds: f64, comma_ms: bool) -> String {
    let total_ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_s = total_ms / 1000;
    let s = total_s % 60;
    let total_m = total_s / 60;
    let m = total_m % 60;
    let h = total_m / 60;
    let sep = if comma_ms { ',' } else { '.' };
    format!("{h:02}:{m:02}:{s:02}{sep}{ms:03}")
}

fn strip_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Parses `.srt` or `.vtt` subtitle text into sorted [`SubtitleCue`] entries.
pub fn parse_subtitles(input: &str) -> Vec<SubtitleCue> {
    let cleaned = input.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut cues = Vec::new();
    for block in cleaned.split("\n\n") {
        let lines: Vec<&str> = block.lines().map(str::trim_end).collect();
        if lines.is_empty() || lines[0].starts_with("WEBVTT") || lines[0].starts_with("NOTE") {
            continue;
        }
        let Some(arrow_idx) = lines.iter().position(|line| line.contains("-->")) else {
            continue;
        };
        let Some((lhs, rhs)) = lines[arrow_idx].split_once("-->") else {
            continue;
        };
        let (Some(start), Some(end)) = (parse_timestamp(lhs), parse_timestamp(rhs)) else {
            continue;
        };
        if end <= start {
            continue;
        }
        let raw_text = lines[arrow_idx + 1..].join("\n");
        let text = strip_tags(raw_text.trim());
        if text.is_empty() {
            continue;
        }
        cues.push(SubtitleCue {
            id: format!("sub{}", cues.len() + 1),
            start,
            end,
            text,
            speaker: String::new(),
        });
    }
    cues.sort_by(|a, b| a.start.total_cmp(&b.start).then_with(|| a.end.total_cmp(&b.end)));
    cues
}

/// Formats cues as SubRip (`.srt`).
pub fn to_srt(cues: &[SubtitleCue]) -> String {
    let mut out = String::new();
    for (i, cue) in cues.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n",
            i + 1,
            format_timestamp(cue.start, true),
            format_timestamp(cue.end, true),
            cue.text.trim()
        ));
    }
    out
}

/// Formats cues as WebVTT (`.vtt`).
pub fn to_vtt(cues: &[SubtitleCue]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for (i, cue) in cues.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n",
            i + 1,
            format_timestamp(cue.start, false),
            format_timestamp(cue.end, false),
            cue.text.trim()
        ));
    }
    out
}

/// Returns pairs of cue indices `(i, i + 1)` that overlap in time, and repairs them in-place if requested.
pub fn resolve_overlaps(cues: &mut [SubtitleCue], repair: bool) -> Vec<(usize, usize)> {
    cues.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut overlaps = Vec::new();
    for i in 0..cues.len().saturating_sub(1) {
        if cues[i].end > cues[i + 1].start + 1e-6 {
            overlaps.push((i, i + 1));
            if repair {
                cues[i].end = cues[i + 1].start.max(cues[i].start + 1.0 / 60.0);
            }
        }
    }
    overlaps
}

/// Extracts all [`ClipKind::Text`] clips on `timeline` (optionally filtered to `track_id`) as sorted [`SubtitleCue`]s.
pub fn extract_timeline_subtitles(timeline: &Timeline, track_id: Option<&str>) -> Vec<SubtitleCue> {
    let mut cues: Vec<SubtitleCue> = timeline
        .clips
        .iter()
        .filter(|clip| {
            clip.kind == ClipKind::Text
                && track_id.is_none_or(|wanted| clip.track_id == wanted)
        })
        .filter_map(|clip| {
            let content = clip.text.as_ref()?.content.trim();
            (!content.is_empty()).then(|| SubtitleCue {
                id: clip.id.clone(),
                start: clip.start,
                end: clip.start + clip.duration,
                text: content.to_owned(),
                speaker: String::new(),
            })
        })
        .collect();
    cues.sort_by(|a, b| a.start.total_cmp(&b.start));
    cues
}

/// Converts [`SubtitleCue`]s into timeline [`Clip`]s on `track_id` using `base_style`.
pub fn cues_to_text_clips(
    cues: &[SubtitleCue],
    track_id: &str,
    base_style: Option<&TextStyle>,
    offset_y: f64,
) -> Vec<Clip> {
    let style_template = base_style.cloned().unwrap_or_default();
    cues.iter()
        .enumerate()
        .map(|(idx, cue)| {
            let mut style = style_template.clone();
            style.content = cue.text.clone();
            let label = cue.text.lines().next().unwrap_or("Subtitle").trim();
            let mut clip = Clip::blank(
                if cue.id.is_empty() { format!("sub{}", idx + 1) } else { cue.id.clone() },
                track_id,
                ClipKind::Text,
                if label.is_empty() { "Subtitle" } else { label },
                cue.start,
                cue.duration(),
            );
            clip.offset_y = offset_y;
            clip.text = Some(style);
            clip
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srt_and_vtt_round_trip_and_overlap_repair() {
        let sample_srt = "1\n00:00:01,250 --> 00:00:03,500\nHello <b>world</b>\n\n2\n00:00:03,000 --> 00:00:05,000\nSecond line\n";
        let mut cues = parse_subtitles(sample_srt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text, "Hello world");
        assert!((cues[0].start - 1.25).abs() < 1e-6);

        let overlaps = resolve_overlaps(&mut cues, true);
        assert_eq!(overlaps, vec![(0, 1)]);
        assert!((cues[0].end - 3.0).abs() < 1e-6);

        let vtt = to_vtt(&cues);
        assert!(vtt.starts_with("WEBVTT"));
        let reparsed = parse_subtitles(&vtt);
        assert_eq!(reparsed.len(), 2);
        assert!((reparsed[0].end - 3.0).abs() < 1e-6);
    }
}
