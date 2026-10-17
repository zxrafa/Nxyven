// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 NXYVEN contributors

//! Professional timeline editing operations (`RollEdit`, `SlipClip`, `SlideClip`,
//! `InsertClip`, `PasteAttributes`, `LinkClips`, `UnlinkClips`, `AddMarker`,
//! `RemoveMarker`, `AddMediaFolder`) operating directly on [`Project`].

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::commands::{CommandError, IdMint, Outcome};
use crate::model::{
    Clip, MediaFolder, Project, TimelineMarker,
    ranges::{MIN_CLIP_DURATION, MAX_TIME},
};

/// Which attribute groups [`paste_attributes`] copies from a source clip onto targets.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeMask {
    /// Copy scale, offsets, rotation, stretch, flip, blend, crop, and transform keys.
    pub transform: bool,
    /// Copy video effects and their parameter keys.
    pub video_effects: bool,
    /// Copy volume, pan, fades, audio filters, and volume keys.
    pub audio: bool,
    /// Copy speed and speed curve.
    pub speed: bool,
}

impl Default for AttributeMask {
    fn default() -> Self {
        Self {
            transform: true,
            video_effects: true,
            audio: true,
            speed: true,
        }
    }
}

/// Performs a **Roll Edit** on the shared cut between `outgoing_id` and `incoming_id` by `delta` seconds.
///
/// The cut point moves by `delta` while `outgoing.start` and `incoming.start + incoming.duration`
/// stay anchored in place.
pub fn roll_edit(
    project: &mut Project,
    outgoing_id: &str,
    incoming_id: &str,
    delta: f64,
) -> Result<Outcome, CommandError> {
    if !delta.is_finite() {
        return Err(CommandError::NotANumber);
    }
    let tl = project.active_mut();
    if tl.track_is_locked_for_clip(outgoing_id) || tl.track_is_locked_for_clip(incoming_id) {
        return Ok(Outcome::default());
    }
    let (Some(out_clip), Some(in_clip)) = (tl.clip(outgoing_id).cloned(), tl.clip(incoming_id).cloned()) else {
        return Ok(Outcome::default());
    };
    if out_clip.track_id != in_clip.track_id {
        return Ok(Outcome::default());
    }
    let min_delta = MIN_CLIP_DURATION - out_clip.duration;
    let max_by_in_dur = in_clip.duration - MIN_CLIP_DURATION;
    let max_by_in_src = -in_clip.source_start / in_clip.speed.max(1e-6);
    let clamped = delta.clamp(min_delta.max(max_by_in_src), max_by_in_dur);
    if clamped.abs() < 1e-9 {
        return Ok(Outcome::default());
    }

    if let Some(out_mut) = tl.clip_mut(outgoing_id) {
        let old = out_mut.duration;
        out_mut.duration = (old + clamped).max(MIN_CLIP_DURATION);
        out_mut.rewindow_keys(old, 0.0, out_mut.duration);
    }
    if let Some(in_mut) = tl.clip_mut(incoming_id) {
        let old = in_mut.duration;
        in_mut.start = (in_mut.start + clamped).max(0.0);
        in_mut.duration = (old - clamped).max(MIN_CLIP_DURATION);
        in_mut.source_start = (in_mut.source_start + clamped * in_mut.speed).max(0.0);
        in_mut.rewindow_keys(old, clamped, old);
    }
    Ok(Outcome { created_id: None, applied: true })
}

/// Performs a **Slip Edit** on `clip_id`: shifts `source_start` by `delta_source` seconds while
/// keeping timeline `start` and `duration` unchanged.
pub fn slip_clip(
    project: &mut Project,
    clip_id: &str,
    delta_source: f64,
) -> Result<Outcome, CommandError> {
    if !delta_source.is_finite() {
        return Err(CommandError::NotANumber);
    }
    if project.active().track_is_locked_for_clip(clip_id) {
        return Ok(Outcome::default());
    }
    let Some(clip) = project.active().clip(clip_id).cloned() else {
        return Ok(Outcome::default());
    };
    let media_dur = project
        .media_by_id(&clip.media_id)
        .and_then(|m| m.duration)
        .unwrap_or(MAX_TIME);
    let needed_source = clip.duration * clip.speed;
    let max_source_start = (media_dur - needed_source).max(0.0);
    let next_source_start = (clip.source_start + delta_source).clamp(0.0, max_source_start);
    if (next_source_start - clip.source_start).abs() < 1e-9 {
        return Ok(Outcome::default());
    }
    if let Some(clip_mut) = project.active_mut().clip_mut(clip_id) {
        clip_mut.source_start = next_source_start;
    }
    Ok(Outcome { created_id: None, applied: true })
}

/// Performs a **Slide Edit** on `clip_id` between optional `left_id` and `right_id` neighbors.
///
/// The clip's own duration and `source_start` remain constant while it slides by `delta` seconds,
/// lengthening/shortening `left_id`'s tail and `right_id`'s head to keep all cuts flush.
pub fn slide_clip(
    project: &mut Project,
    clip_id: &str,
    left_id: Option<&str>,
    right_id: Option<&str>,
    delta: f64,
) -> Result<Outcome, CommandError> {
    if !delta.is_finite() {
        return Err(CommandError::NotANumber);
    }
    let tl = project.active_mut();
    if tl.track_is_locked_for_clip(clip_id) {
        return Ok(Outcome::default());
    }
    let Some(center) = tl.clip(clip_id).cloned() else {
        return Ok(Outcome::default());
    };
    let mut min_delta = -center.start;
    let mut max_delta = MAX_TIME;

    if let Some(lid) = left_id
        && let Some(left) = tl.clip(lid)
    {
        min_delta = min_delta.max(MIN_CLIP_DURATION - left.duration);
    }
    if let Some(rid) = right_id
        && let Some(right) = tl.clip(rid)
    {
        max_delta = max_delta.min(right.duration - MIN_CLIP_DURATION);
    }
    let clamped = delta.clamp(min_delta, max_delta);
    if clamped.abs() < 1e-9 {
        return Ok(Outcome::default());
    }
    if let Some(lid) = left_id
        && let Some(left) = tl.clip_mut(lid)
    {
        left.duration = (left.duration + clamped).max(MIN_CLIP_DURATION);
    }
    if let Some(rid) = right_id
        && let Some(right) = tl.clip_mut(rid)
    {
        right.start = (right.start + clamped).max(0.0);
        right.duration = (right.duration - clamped).max(MIN_CLIP_DURATION);
        right.source_start = (right.source_start + clamped * right.speed).max(0.0);
    }
    if let Some(c) = tl.clip_mut(clip_id) {
        c.start = (c.start + clamped).max(0.0);
    }
    Ok(Outcome { created_id: None, applied: true })
}

/// Copies selected attribute groups from `source_id` onto every clip in `target_ids`.
pub fn paste_attributes(
    project: &mut Project,
    source_id: &str,
    target_ids: &[String],
    mask: AttributeMask,
) -> Result<Outcome, CommandError> {
    let tl = project.active_mut();
    let Some(src) = tl.clip(source_id).cloned() else {
        return Ok(Outcome::default());
    };
    let mut applied = false;
    for tid in target_ids {
        if tid == source_id || tl.track_is_locked_for_clip(tid) {
            continue;
        }
        let Some(dst) = tl.clip_mut(tid) else {
            continue;
        };
        if mask.transform {
            dst.scale = src.scale;
            dst.offset_x = src.offset_x;
            dst.offset_y = src.offset_y;
            dst.rotation = src.rotation;
            dst.stretch_x = src.stretch_x;
            dst.stretch_y = src.stretch_y;
            dst.opacity = src.opacity;
            dst.flip_h = src.flip_h;
            dst.flip_v = src.flip_v;
            dst.blend = src.blend.clone();
            dst.crop = src.crop;
            dst.masks = src.masks.clone();
            applied = true;
        }
        if mask.video_effects {
            dst.video_effects = src.video_effects.clone();
            applied = true;
        }
        if mask.audio {
            dst.volume = src.volume;
            dst.pan = src.pan;
            dst.fade_in = src.fade_in;
            dst.fade_out = src.fade_out;
            dst.filters = src.filters.clone();
            applied = true;
        }
        if mask.speed {
            dst.speed = src.speed;
            dst.speed_curve = src.speed_curve.clone();
            dst.preserve_pitch = src.preserve_pitch;
            applied = true;
        }
    }
    Ok(Outcome { created_id: None, applied })
}

/// Links `clip_ids` together under a shared `link_id` (or unlinks them when `link == false`).
pub fn set_clips_linked(
    project: &mut Project,
    mint: &mut IdMint,
    clip_ids: &[String],
    link: bool,
) -> Outcome {
    let group_id = link.then(|| mint.next("lnk"));
    let tl = project.active_mut();
    let mut applied = false;
    for cid in clip_ids {
        if let Some(clip) = tl.clip_mut(cid)
            && clip.link_id != group_id
        {
            clip.link_id = group_id.clone();
            applied = true;
        }
    }
    Outcome {
        created_id: group_id,
        applied,
    }
}

/// Adds or updates a [`TimelineMarker`] on the active timeline.
pub fn add_timeline_marker(
    project: &mut Project,
    mint: &mut IdMint,
    at: f64,
    name: String,
    color: String,
) -> Outcome {
    let id = mint.next("mk");
    let tl = project.active_mut();
    tl.markers.push(TimelineMarker {
        id: id.clone(),
        at: at.max(0.0),
        duration: 0.0,
        name: if name.trim().is_empty() { "Marker".to_owned() } else { name.trim().to_owned() },
        color: if color.is_empty() { "#FF6A24".to_owned() } else { color },
    });
    tl.markers.sort_by(|a, b| a.at.total_cmp(&b.at));
    Outcome {
        created_id: Some(id),
        applied: true,
    }
}

/// Creates a media bin folder (`MediaFolder`) in `project`.
pub fn add_media_folder(
    project: &mut Project,
    mint: &mut IdMint,
    name: String,
    parent_id: Option<String>,
) -> Outcome {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Outcome::default();
    }
    let id = mint.next("fld");
    project.folders.push(MediaFolder {
        id: id.clone(),
        name: trimmed.to_owned(),
        parent_id,
    });
    Outcome {
        created_id: Some(id),
        applied: true,
    }
}

/// Pre-composes `clip_ids` on the active timeline into a new compound [` crate::model::Timeline`],
/// returning the new timeline's id.
pub fn create_compound_timeline(
    project: &mut Project,
    mint: &mut IdMint,
    clip_ids: &[String],
    name: String,
) -> Outcome {
    let active = project.active().clone();
    let selected: Vec<Arc<Clip>> = active
        .clips
        .iter()
        .filter(|c| clip_ids.contains(&c.id))
        .cloned()
        .collect();
    if selected.is_empty() {
        return Outcome::default();
    }
    let min_start = selected.iter().map(|c| c.start).fold(f64::INFINITY, f64::min);
    let shifted: Vec<Arc<Clip>> = selected
        .into_iter()
        .map(|arc_c| {
            let mut c = (*arc_c).clone();
            c.start = (c.start - min_start).max(0.0);
            Arc::new(c)
        })
        .collect();
    let tl_id = mint.next("tl");
    let compound = crate::model::Timeline {
        id: tl_id.clone(),
        name: if name.trim().is_empty() { "Compound Clip".to_owned() } else { name.trim().to_owned() },
        video: active.video,
        tracks: active.tracks.clone(),
        clips: shifted,
        markers: Vec::new(),
        subtitles: Vec::new(),
        extra: Default::default(),
    };
    project.timelines.push(Arc::new(compound));
    Outcome {
        created_id: Some(tl_id),
        applied: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Clip, ClipKind, MediaItem, MediaKind};

    #[test]
    fn roll_slip_slide_paste_link_marker_folder_and_compound_work() {
        let mut proj = Project::new();
        proj.media.push(MediaItem {
            id: "m1".to_owned(),
            path: "/a.mp4".to_owned(),
            name: "a.mp4".to_owned(),
            duration: Some(30.0),
            kind: MediaKind::Video,
            has_audio: true,
            ..MediaItem::default()
        });
        let mut c1 = Clip::blank("c1", "T1", ClipKind::Video, "Shot 1", 0.0, 5.0);
        c1.media_id = "m1".to_owned();
        c1.scale = 1.5;
        let mut c2 = Clip::blank("c2", "T1", ClipKind::Video, "Shot 2", 5.0, 5.0);
        c2.media_id = "m1".to_owned();
        c2.source_start = 5.0;
        let mut c3 = Clip::blank("c3", "T1", ClipKind::Video, "Shot 3", 10.0, 5.0);
        c3.media_id = "m1".to_owned();
        c3.source_start = 10.0;

        proj.active_mut().clips.push(Arc::new(c1));
        proj.active_mut().clips.push(Arc::new(c2));
        proj.active_mut().clips.push(Arc::new(c3));

        assert!(roll_edit(&mut proj, "c1", "c2", 1.0).unwrap().applied);
        assert!((proj.active().clip("c1").unwrap().duration - 6.0).abs() < 1e-6);
        assert!((proj.active().clip("c2").unwrap().start - 6.0).abs() < 1e-6);
        assert!((proj.active().clip("c2").unwrap().duration - 4.0).abs() < 1e-6);

        assert!(slip_clip(&mut proj, "c2", 2.0).unwrap().applied);
        assert!((proj.active().clip("c2").unwrap().source_start - 8.0).abs() < 1e-6);

        assert!(slide_clip(&mut proj, "c2", Some("c1"), Some("c3"), -1.0).unwrap().applied);
        assert!((proj.active().clip("c1").unwrap().duration - 5.0).abs() < 1e-6);
        assert!((proj.active().clip("c2").unwrap().start - 5.0).abs() < 1e-6);

        assert!(
            paste_attributes(
                &mut proj,
                "c1",
                &["c2".to_owned()],
                AttributeMask::default()
            )
            .unwrap()
            .applied
        );
        assert!((proj.active().clip("c2").unwrap().scale - 1.5).abs() < 1e-6);

        let mut mint = IdMint::default();
        let link_out = set_clips_linked(&mut proj, &mut mint, &["c1".to_owned(), "c2".to_owned()], true);
        assert!(link_out.applied);
        assert_eq!(
            proj.active().clip("c1").unwrap().link_id,
            proj.active().clip("c2").unwrap().link_id
        );

        let mk_out = add_timeline_marker(&mut proj, &mut mint, 3.5, "Beat".to_owned(), String::new());
        assert!(mk_out.applied);
        assert_eq!(proj.active().markers.len(), 1);

        let fld_out = add_media_folder(&mut proj, &mut mint, "B-Roll".to_owned(), None);
        assert!(fld_out.applied);
        assert_eq!(proj.folders.len(), 1);

        let comp_out = create_compound_timeline(
            &mut proj,
            &mut mint,
            &["c1".to_owned(), "c2".to_owned()],
            "Intro Precomp".to_owned(),
        );
        assert!(comp_out.applied);
        assert_eq!(proj.timelines.len(), 2);
    }
}
