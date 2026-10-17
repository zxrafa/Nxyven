// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 NXYVEN contributors

//! Native Drift (`.json`) project importer and converter into NXYVEN [`Project`] documents.

use std::sync::Arc;

use serde_json::Value;

use crate::model::{
    AppliedFilter, Clip, ClipKey, ClipKind, ColorSpace, KeyEase, KeyProperty, MediaItem, MediaKind,
    Project, SpeedPoint, TextAlign, TextStyle, Timeline, Track, Transition, VideoSettings,
};

/// Imports a Drift project JSON value into a NXYVEN [`Project`].
pub fn import_drift_project(root: &Value) -> Result<Project, String> {
    let obj = root
        .as_object()
        .ok_or_else(|| "Drift project root must be a JSON object".to_owned())?;

    let width = obj.get("width").and_then(Value::as_u64).unwrap_or(1920) as u32;
    let height = obj.get("height").and_then(Value::as_u64).unwrap_or(1080) as u32;
    let fps = obj.get("fps").and_then(Value::as_f64).unwrap_or(30.0);
    let (rate_num, rate_den) = rational_fps(fps);
    let video = VideoSettings {
        width: width.max(16),
        height: height.max(16),
        rate_num,
        rate_den,
        color_space: ColorSpace::Sdr,
    };

    let mut media = Vec::new();
    if let Some(items) = obj.get("media").or_else(|| obj.get("mediaItems")).and_then(Value::as_array) {
        for (idx, item) in items.iter().enumerate() {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .map_or_else(|| format!("m{}", idx + 1), str::to_owned);
            let path = item.get("path").and_then(Value::as_str).unwrap_or("").to_owned();
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_else(|| path.rsplit(['/', '\\']).next().unwrap_or("media"))
                .to_owned();
            let kind = match item.get("type").or_else(|| item.get("kind")).and_then(Value::as_str) {
                Some("audio") => MediaKind::Audio,
                Some("image" | "still") => MediaKind::Image,
                _ => MediaKind::Video,
            };
            media.push(MediaItem {
                id,
                path,
                name,
                duration: item.get("duration").and_then(Value::as_f64),
                kind,
                width: item.get("width").and_then(Value::as_u64).map(|v| v as u32),
                height: item.get("height").and_then(Value::as_u64).map(|v| v as u32),
                frame_rate: item.get("fps").and_then(Value::as_f64),
                has_audio: item.get("hasAudio").and_then(Value::as_bool).unwrap_or(kind != MediaKind::Image),
                ..MediaItem::default()
            });
        }
    }

    let mut tracks = Vec::new();
    let mut clips = Vec::new();

    if let Some(raw_tracks) = obj.get("tracks").and_then(Value::as_array) {
        for (t_idx, raw_track) in raw_tracks.iter().enumerate() {
            let track_id = raw_track
                .get("id")
                .and_then(Value::as_str)
                .map_or_else(|| format!("T{}", t_idx + 1), str::to_owned);
            let visible = !raw_track.get("hidden").and_then(Value::as_bool).unwrap_or(false)
                && raw_track.get("visible").and_then(Value::as_bool).unwrap_or(true);
            let muted = raw_track.get("muted").and_then(Value::as_bool).unwrap_or(false);
            tracks.push(Track {
                id: track_id.clone(),
                visible,
                muted,
                ..Track::default()
            });

            if let Some(raw_clips) = raw_track.get("clips").and_then(Value::as_array) {
                for (c_idx, rc) in raw_clips.iter().enumerate() {
                    if let Some(clip) = convert_drift_clip(rc, &track_id, c_idx + clips.len() + 1) {
                        clips.push(Arc::new(clip));
                    }
                }
            }
        }
    }

    if let Some(top_clips) = obj.get("clips").and_then(Value::as_array) {
        for (c_idx, rc) in top_clips.iter().enumerate() {
            let track_id = rc
                .get("trackId")
                .and_then(Value::as_str)
                .unwrap_or("T1")
                .to_owned();
            if !tracks.iter().any(|t| t.id == track_id) {
                tracks.push(Track {
                    id: track_id.clone(),
                    ..Track::default()
                });
            }
            if let Some(clip) = convert_drift_clip(rc, &track_id, c_idx + clips.len() + 1) {
                clips.push(Arc::new(clip));
            }
        }
    }

    if tracks.is_empty() {
        tracks.push(Track {
            id: "T1".to_owned(),
            ..Track::default()
        });
    }

    let timeline_name = obj
        .get("name")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Timeline 1")
        .to_owned();

    Ok(Project {
        media,
        folders: Vec::new(),
        fonts: Vec::new(),
        timelines: vec![Arc::new(Timeline {
            id: "TL1".to_owned(),
            name: timeline_name,
            video,
            tracks,
            clips,
            ..Timeline::default()
        })],
        active_timeline_id: "TL1".to_owned(),
        extra: Default::default(),
    })
}

fn convert_drift_clip(rc: &Value, track_id: &str, seq: usize) -> Option<Clip> {
    let id = rc
        .get("id")
        .and_then(Value::as_str)
        .map_or_else(|| format!("c{seq}"), str::to_owned);
    let start = rc
        .get("timelineIn")
        .or_else(|| rc.get("start"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let duration = rc
        .get("duration")
        .and_then(Value::as_f64)
        .or_else(|| {
            let end = rc.get("timelineOut")?.as_f64()?;
            Some((end - start).max(1.0 / 60.0))
        })
        .unwrap_or(5.0);
    let kind = match rc.get("clipType").or_else(|| rc.get("kind")).and_then(Value::as_str) {
        Some("audio") => ClipKind::Audio,
        Some("image" | "still") => ClipKind::Image,
        Some("text" | "title" | "subtitle") => ClipKind::Text,
        Some("adjustment" | "layer") => ClipKind::Layer,
        Some("shape") => ClipKind::Shape,
        _ => ClipKind::Video,
    };
    let name = rc.get("name").and_then(Value::as_str).unwrap_or("Clip").to_owned();
    let mut clip = Clip::blank(id, track_id, kind, name, start, duration);
    clip.media_id = rc.get("mediaId").and_then(Value::as_str).unwrap_or("").to_owned();
    clip.source_start = rc
        .get("sourceIn")
        .or_else(|| rc.get("sourceStart"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    clip.volume = rc.get("volume").and_then(Value::as_f64).unwrap_or(1.0);
    clip.fade_in = rc.get("fadeIn").and_then(Value::as_f64).unwrap_or(0.0);
    clip.fade_out = rc.get("fadeOut").and_then(Value::as_f64).unwrap_or(0.0);
    clip.scale = rc.get("scaleX").or_else(|| rc.get("scale")).and_then(Value::as_f64).unwrap_or(1.0);
    clip.offset_x = rc.get("positionX").or_else(|| rc.get("offsetX")).and_then(Value::as_f64).unwrap_or(0.0);
    clip.offset_y = rc.get("positionY").or_else(|| rc.get("offsetY")).and_then(Value::as_f64).unwrap_or(0.0);
    clip.rotation = rc.get("rotation").and_then(Value::as_f64).unwrap_or(0.0);
    clip.opacity = rc.get("opacity").and_then(Value::as_f64).unwrap_or(1.0);
    clip.speed = rc.get("speed").and_then(Value::as_f64).unwrap_or(1.0);
    clip.flip_h = rc.get("flipH").and_then(Value::as_bool).unwrap_or(false);
    clip.flip_v = rc.get("flipV").and_then(Value::as_bool).unwrap_or(false);
    if let Some(blend) = rc.get("blendMode").or_else(|| rc.get("blend")).and_then(Value::as_str) {
        if blend != "normal" {
            clip.blend = blend.to_owned();
        }
    }
    if let Some(tr) = rc.get("transitionIn").and_then(Value::as_object) {
        if let Some(tr_id) = tr.get("id").or_else(|| tr.get("type")).and_then(Value::as_str) {
            clip.transition_in = Some(Transition {
                id: tr_id.to_owned(),
                duration: tr.get("duration").and_then(Value::as_f64).unwrap_or(1.0),
            });
        }
    }
    if kind == ClipKind::Text {
        let content = rc
            .get("textContent")
            .or_else(|| rc.get("text"))
            .and_then(Value::as_str)
            .unwrap_or(&clip.name)
            .to_owned();
        clip.text = Some(TextStyle {
            content,
            align: TextAlign::Center,
            ..TextStyle::default()
        });
    }
    if let Some(effects) = rc.get("effects").and_then(Value::as_array) {
        for eff in effects {
            if let Some(eff_id) = eff.get("id").and_then(Value::as_str) {
                let mut filter = AppliedFilter::new(eff_id);
                filter.enabled = eff.get("enabled").and_then(Value::as_bool).unwrap_or(true);
                if let Some(params) = eff.get("params").and_then(Value::as_object) {
                    for (k, v) in params {
                        if let Some(num) = v.as_f64() {
                            filter.params.insert(k.clone(), num);
                        }
                    }
                }
                clip.video_effects.push(filter);
            }
        }
    }
    if let Some(curve) = rc.get("speedCurve").and_then(Value::as_array) {
        let pts: Vec<SpeedPoint> = curve
            .iter()
            .filter_map(|pt| {
                Some(SpeedPoint {
                    at: pt.get("at").or_else(|| pt.get("t"))?.as_f64()?,
                    speed: pt.get("speed")?.as_f64()?,
                })
            })
            .collect();
        if !pts.is_empty() {
            clip.speed_curve = Some(pts);
        }
    }
    if let Some(keys) = rc.get("keyframes").and_then(Value::as_array) {
        for k in keys {
            if let (Some(prop_str), Some(at), Some(val)) = (
                k.get("property").and_then(Value::as_str),
                k.get("at").or_else(|| k.get("time")).and_then(Value::as_f64),
                k.get("value").and_then(Value::as_f64),
            ) && let Some(property) = KeyProperty::from_name(prop_str)
            {
                clip.keys.push(ClipKey {
                    property,
                    at: at.clamp(0.0, 1.0),
                    value: val,
                    ease: KeyEase::LINEAR,
                });
            }
        }
    }
    Some(clip.tidy())
}

fn rational_fps(fps: f64) -> (i64, i64) {
    if (fps - 23.976).abs() < 0.05 {
        (24_000, 1001)
    } else if (fps - 29.97).abs() < 0.05 {
        (30_000, 1001)
    } else if (fps - 59.94).abs() < 0.05 {
        (60_000, 1001)
    } else {
        ((fps.round() as i64).clamp(1, 240), 1)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use super::*;

    #[test]
    fn imports_drift_project_with_tracks_clips_effects_and_transitions() {
        let drift_doc = json!({
            "name": "Drift Commercial Cut",
            "width": 1080,
            "height": 1920,
            "fps": 29.97,
            "media": [
                { "id": "m1", "path": "/tmp/a.mp4", "name": "a.mp4", "duration": 12.0, "type": "video", "hasAudio": true }
            ],
            "tracks": [
                {
                    "id": "T1",
                    "muted": false,
                    "clips": [
                        {
                            "id": "c1",
                            "mediaId": "m1",
                            "timelineIn": 0.0,
                            "timelineOut": 4.5,
                            "sourceIn": 1.0,
                            "scaleX": 1.25,
                            "transitionIn": { "id": "cross-fade", "duration": 0.5 },
                            "effects": [{ "id": "concat.vignette", "params": { "strength": 40.0 } }]
                        }
                    ]
                }
            ]
        });
        let project = import_drift_project(&drift_doc).expect("valid drift project");
        assert_eq!(project.media.len(), 1);
        let tl = project.active();
        assert_eq!(tl.video.width, 1080);
        assert_eq!(tl.video.height, 1920);
        assert_eq!(tl.video.rate_num, 30_000);
        assert_eq!(tl.video.rate_den, 1001);
        assert_eq!(tl.clips.len(), 1);
        let c = &tl.clips[0];
        assert!((c.duration - 4.5).abs() < 1e-6);
        assert!((c.scale - 1.25).abs() < 1e-6);
        assert_eq!(c.video_effects.len(), 1);
        assert_eq!(c.transition_in.as_ref().map(|t| t.id.as_str()), Some("cross-fade"));
    }
}
