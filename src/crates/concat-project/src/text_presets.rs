// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 NXYVEN contributors

//! Built-in text and title animation presets (In, Out, Loop) ported from Drift
//! and mapped onto NXYVEN's keyframe curves ([`ClipKey`], [`KeyEase`]).

use serde::{Deserialize, Serialize};

use crate::model::{Clip, KeyEase, KeyProperty};

/// Phase of a text animation preset.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAnimPhase {
    /// Entry animation at the start of the title clip.
    In,
    /// Exit animation at the end of the title clip.
    Out,
    /// Continuous motion over the title clip.
    Loop,
}

/// Metadata for a built-in text animation preset.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextAnimPreset {
    /// Stable preset id (e.g. `"fade-in"`, `"pop-in"`, `"slide-up-in"`).
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Animation phase.
    pub phase: TextAnimPhase,
    /// Default duration in seconds (`0.0` for full-clip loop).
    pub default_duration: f64,
}

/// Built-in catalog of text animation presets.
pub const PRESETS: &[TextAnimPreset] = &[
    TextAnimPreset { id: "fade-in", name: "Fade In", phase: TextAnimPhase::In, default_duration: 0.5 },
    TextAnimPreset { id: "pop-in", name: "Pop In", phase: TextAnimPhase::In, default_duration: 0.45 },
    TextAnimPreset { id: "slide-up-in", name: "Slide Up", phase: TextAnimPhase::In, default_duration: 0.5 },
    TextAnimPreset { id: "slide-down-in", name: "Slide Down", phase: TextAnimPhase::In, default_duration: 0.5 },
    TextAnimPreset { id: "slide-left-in", name: "Slide Left", phase: TextAnimPhase::In, default_duration: 0.5 },
    TextAnimPreset { id: "slide-right-in", name: "Slide Right", phase: TextAnimPhase::In, default_duration: 0.5 },
    TextAnimPreset { id: "zoom-in", name: "Zoom In", phase: TextAnimPhase::In, default_duration: 0.5 },
    TextAnimPreset { id: "spin-in", name: "Spin In", phase: TextAnimPhase::In, default_duration: 0.6 },
    TextAnimPreset { id: "fade-out", name: "Fade Out", phase: TextAnimPhase::Out, default_duration: 0.5 },
    TextAnimPreset { id: "pop-out", name: "Pop Out", phase: TextAnimPhase::Out, default_duration: 0.45 },
    TextAnimPreset { id: "slide-down-out", name: "Slide Down Out", phase: TextAnimPhase::Out, default_duration: 0.5 },
    TextAnimPreset { id: "zoom-out", name: "Zoom Out", phase: TextAnimPhase::Out, default_duration: 0.5 },
    TextAnimPreset { id: "pulse-loop", name: "Pulse", phase: TextAnimPhase::Loop, default_duration: 0.0 },
    TextAnimPreset { id: "float-loop", name: "Float", phase: TextAnimPhase::Loop, default_duration: 0.0 },
    TextAnimPreset { id: "pendulum-loop", name: "Pendulum", phase: TextAnimPhase::Loop, default_duration: 0.0 },
];

/// Finds a preset by `id`.
pub fn find_preset(id: &str) -> Option<&'static TextAnimPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Applies a text animation preset `preset_id` over `anim_duration` seconds onto `clip`.
pub fn apply_text_preset(clip: &mut Clip, preset_id: &str, anim_duration: Option<f64>) -> bool {
    let Some(preset) = find_preset(preset_id) else {
        return false;
    };
    let clip_dur = clip.duration.max(1.0 / 60.0);
    let dur = anim_duration.unwrap_or(preset.default_duration).clamp(0.05, clip_dur);
    let span = (dur / clip_dur).clamp(0.05, 1.0);
    let base_scale = clip.scale;
    let base_x = clip.offset_x;
    let base_y = clip.offset_y;
    let base_rot = clip.rotation;
    let base_op = clip.opacity;

    match preset.id {
        "fade-in" => {
            clip.set_key(KeyProperty::Opacity, 0.0, 0.0, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Opacity, span, base_op, KeyEase::OUT);
        }
        "pop-in" => {
            clip.set_key(KeyProperty::Scale, 0.0, base_scale * 0.2, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Scale, span * 0.7, base_scale * 1.12, KeyEase::OUT);
            clip.set_key(KeyProperty::Scale, span, base_scale, KeyEase::IN_OUT);
            clip.set_key(KeyProperty::Opacity, 0.0, 0.0, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Opacity, span * 0.4, base_op, KeyEase::OUT);
        }
        "slide-up-in" => {
            clip.set_key(KeyProperty::OffsetY, 0.0, base_y + 0.15, KeyEase::LINEAR);
            clip.set_key(KeyProperty::OffsetY, span, base_y, KeyEase::OUT);
            clip.set_key(KeyProperty::Opacity, 0.0, 0.0, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Opacity, span, base_op, KeyEase::OUT);
        }
        "slide-down-in" => {
            clip.set_key(KeyProperty::OffsetY, 0.0, base_y - 0.15, KeyEase::LINEAR);
            clip.set_key(KeyProperty::OffsetY, span, base_y, KeyEase::OUT);
            clip.set_key(KeyProperty::Opacity, 0.0, 0.0, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Opacity, span, base_op, KeyEase::OUT);
        }
        "slide-left-in" => {
            clip.set_key(KeyProperty::OffsetX, 0.0, base_x + 0.25, KeyEase::LINEAR);
            clip.set_key(KeyProperty::OffsetX, span, base_x, KeyEase::OUT);
            clip.set_key(KeyProperty::Opacity, 0.0, 0.0, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Opacity, span, base_op, KeyEase::OUT);
        }
        "slide-right-in" => {
            clip.set_key(KeyProperty::OffsetX, 0.0, base_x - 0.25, KeyEase::LINEAR);
            clip.set_key(KeyProperty::OffsetX, span, base_x, KeyEase::OUT);
            clip.set_key(KeyProperty::Opacity, 0.0, 0.0, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Opacity, span, base_op, KeyEase::OUT);
        }
        "zoom-in" => {
            clip.set_key(KeyProperty::Scale, 0.0, base_scale * 1.6, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Scale, span, base_scale, KeyEase::OUT);
            clip.set_key(KeyProperty::Opacity, 0.0, 0.0, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Opacity, span, base_op, KeyEase::OUT);
        }
        "spin-in" => {
            clip.set_key(KeyProperty::Rotation, 0.0, base_rot - 180.0, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Rotation, span, base_rot, KeyEase::OUT);
            clip.set_key(KeyProperty::Scale, 0.0, base_scale * 0.1, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Scale, span, base_scale, KeyEase::OUT);
        }
        "fade-out" => {
            clip.set_key(KeyProperty::Opacity, 1.0 - span, base_op, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Opacity, 1.0, 0.0, KeyEase::IN);
        }
        "pop-out" => {
            clip.set_key(KeyProperty::Scale, 1.0 - span, base_scale, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Scale, 1.0 - span * 0.65, base_scale * 1.1, KeyEase::OUT);
            clip.set_key(KeyProperty::Scale, 1.0, base_scale * 0.1, KeyEase::IN);
            clip.set_key(KeyProperty::Opacity, 1.0 - span * 0.5, base_op, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Opacity, 1.0, 0.0, KeyEase::IN);
        }
        "slide-down-out" => {
            clip.set_key(KeyProperty::OffsetY, 1.0 - span, base_y, KeyEase::LINEAR);
            clip.set_key(KeyProperty::OffsetY, 1.0, base_y + 0.15, KeyEase::IN);
            clip.set_key(KeyProperty::Opacity, 1.0 - span, base_op, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Opacity, 1.0, 0.0, KeyEase::IN);
        }
        "zoom-out" => {
            clip.set_key(KeyProperty::Scale, 1.0 - span, base_scale, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Scale, 1.0, base_scale * 1.6, KeyEase::IN);
            clip.set_key(KeyProperty::Opacity, 1.0 - span, base_op, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Opacity, 1.0, 0.0, KeyEase::IN);
        }
        "pulse-loop" => {
            clip.set_key(KeyProperty::Scale, 0.0, base_scale, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Scale, 0.25, base_scale * 1.08, KeyEase::IN_OUT);
            clip.set_key(KeyProperty::Scale, 0.5, base_scale, KeyEase::IN_OUT);
            clip.set_key(KeyProperty::Scale, 0.75, base_scale * 1.08, KeyEase::IN_OUT);
            clip.set_key(KeyProperty::Scale, 1.0, base_scale, KeyEase::IN_OUT);
        }
        "float-loop" => {
            clip.set_key(KeyProperty::OffsetY, 0.0, base_y, KeyEase::LINEAR);
            clip.set_key(KeyProperty::OffsetY, 0.25, base_y - 0.03, KeyEase::IN_OUT);
            clip.set_key(KeyProperty::OffsetY, 0.5, base_y, KeyEase::IN_OUT);
            clip.set_key(KeyProperty::OffsetY, 0.75, base_y + 0.03, KeyEase::IN_OUT);
            clip.set_key(KeyProperty::OffsetY, 1.0, base_y, KeyEase::IN_OUT);
        }
        "pendulum-loop" => {
            clip.set_key(KeyProperty::Rotation, 0.0, base_rot - 4.0, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Rotation, 0.5, base_rot + 4.0, KeyEase::IN_OUT);
            clip.set_key(KeyProperty::Rotation, 1.0, base_rot - 4.0, KeyEase::IN_OUT);
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ClipKind;

    #[test]
    fn text_presets_populate_clip_keyframes() {
        let mut clip = Clip::blank("c1", "T1", ClipKind::Text, "Title", 0.0, 2.0);
        assert!(apply_text_preset(&mut clip, "pop-in", Some(0.5)));
        assert!(clip.is_keyed(KeyProperty::Scale));
        assert!(clip.is_keyed(KeyProperty::Opacity));
        assert!((clip.value_at(KeyProperty::Opacity, 0.0) - 0.0).abs() < 1e-6);
        assert!((clip.value_at(KeyProperty::Scale, 0.5) - 1.0).abs() < 1e-6);
    }
}
