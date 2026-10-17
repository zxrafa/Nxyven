// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 NXYVEN contributors

//! Commercial speed-ramp curve presets (`Montage`, `Hero`, `Bullet`, `JumpCut`,
//! `FlashIn`, `FlashOut`) and export platform presets (`YouTube`, `TikTok/Reels/Shorts`,
//! `Instagram`, `Archival`, `Podcast`).

use serde::{Deserialize, Serialize};

use crate::model::{ColorSpace, SpeedPoint};

/// Named variable speed-ramp preset.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SpeedRampPreset {
    /// Preset id (`"montage"`, `"hero"`, `"bullet"`, `"jump-cut"`, `"flash-in"`, `"flash-out"`).
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Control points `(at, speed)`.
    pub points: &'static [(f64, f64)],
}

/// Built-in CapCut Pro / commercial speed-ramp curves.
pub const SPEED_PRESETS: &[SpeedRampPreset] = &[
    SpeedRampPreset {
        id: "montage",
        name: "Montage",
        points: &[(0.0, 0.8), (0.25, 2.8), (0.55, 0.45), (0.85, 2.2), (1.0, 1.0)],
    },
    SpeedRampPreset {
        id: "hero",
        name: "Hero Time",
        points: &[(0.0, 1.0), (0.3, 3.2), (0.48, 0.25), (0.72, 0.25), (0.88, 2.5), (1.0, 1.0)],
    },
    SpeedRampPreset {
        id: "bullet",
        name: "Bullet",
        points: &[(0.0, 4.0), (0.35, 0.2), (0.65, 0.2), (1.0, 4.0)],
    },
    SpeedRampPreset {
        id: "jump-cut",
        name: "Jump Cut",
        points: &[(0.0, 0.6), (0.4, 0.6), (0.5, 4.5), (0.6, 0.6), (1.0, 0.6)],
    },
    SpeedRampPreset {
        id: "flash-in",
        name: "Flash In",
        points: &[(0.0, 5.0), (0.35, 1.0), (1.0, 1.0)],
    },
    SpeedRampPreset {
        id: "flash-out",
        name: "Flash Out",
        points: &[(0.0, 1.0), (0.65, 1.0), (1.0, 5.0)],
    },
];

/// Returns the [`SpeedPoint`] vector for a named speed-ramp preset.
pub fn speed_preset_points(id: &str) -> Option<Vec<SpeedPoint>> {
    let preset = SPEED_PRESETS.iter().find(|p| p.id == id)?;
    Some(
        preset
            .points
            .iter()
            .map(|&(at, speed)| SpeedPoint { at, speed })
            .collect(),
    )
}

/// Commercial export delivery preset.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreset {
    /// Preset id (e.g. `"youtube-4k"`, `"tiktok-1080p"`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Frame rate numerator.
    pub rate_num: i64,
    /// Frame rate denominator.
    pub rate_den: i64,
    /// Video codec identifier (`"h264"`, `"hevc"`, `"av1"`, `"vp9"`, `"prores"`).
    pub codec: String,
    /// Container extension (`"mp4"`, `"mov"`, `"mkv"`, `"webm"`, `"wav"`).
    pub container: String,
    /// Constant Rate Factor quality (`0..=51`).
    pub crf: u32,
    /// Target bitrate in kbps (`0` uses CRF).
    pub bitrate_kbps: u32,
    /// 10-bit pixel format.
    pub ten_bit: bool,
    /// Output color space.
    pub color_space: ColorSpace,
    /// Audio-only export flag.
    pub audio_only: bool,
}

/// Built-in delivery presets for social, broadcast, web, and archival workflows.
pub fn builtin_export_presets() -> Vec<ExportPreset> {
    vec![
        ExportPreset {
            id: "youtube-4k60".to_owned(),
            name: "YouTube 4K UHD (60 fps)".to_owned(),
            width: 3840,
            height: 2160,
            rate_num: 60,
            rate_den: 1,
            codec: "hevc".to_owned(),
            container: "mp4".to_owned(),
            crf: 18,
            bitrate_kbps: 45_000,
            ten_bit: true,
            color_space: ColorSpace::Sdr,
            audio_only: false,
        },
        ExportPreset {
            id: "youtube-1080p".to_owned(),
            name: "YouTube / Web 1080p Full HD".to_owned(),
            width: 1920,
            height: 1080,
            rate_num: 30,
            rate_den: 1,
            codec: "h264".to_owned(),
            container: "mp4".to_owned(),
            crf: 18,
            bitrate_kbps: 12_000,
            ten_bit: false,
            color_space: ColorSpace::Sdr,
            audio_only: false,
        },
        ExportPreset {
            id: "vertical-9x16".to_owned(),
            name: "TikTok / Reels / Shorts (1080×1920 60fps)".to_owned(),
            width: 1080,
            height: 1920,
            rate_num: 60,
            rate_den: 1,
            codec: "h264".to_owned(),
            container: "mp4".to_owned(),
            crf: 18,
            bitrate_kbps: 15_000,
            ten_bit: false,
            color_space: ColorSpace::Sdr,
            audio_only: false,
        },
        ExportPreset {
            id: "instagram-4x5".to_owned(),
            name: "Instagram Feed 4:5 (1080×1350)".to_owned(),
            width: 1080,
            height: 1350,
            rate_num: 30,
            rate_den: 1,
            codec: "h264".to_owned(),
            container: "mp4".to_owned(),
            crf: 18,
            bitrate_kbps: 10_000,
            ten_bit: false,
            color_space: ColorSpace::Sdr,
            audio_only: false,
        },
        ExportPreset {
            id: "archival-master".to_owned(),
            name: "Master Archival 10-bit HEVC".to_owned(),
            width: 3840,
            height: 2160,
            rate_num: 30,
            rate_den: 1,
            codec: "hevc".to_owned(),
            container: "mov".to_owned(),
            crf: 14,
            bitrate_kbps: 0,
            ten_bit: true,
            color_space: ColorSpace::Sdr,
            audio_only: false,
        },
        ExportPreset {
            id: "podcast-audio".to_owned(),
            name: "Audio Only (WAV Master)".to_owned(),
            width: 1920,
            height: 1080,
            rate_num: 30,
            rate_den: 1,
            codec: "pcm_s16le".to_owned(),
            container: "wav".to_owned(),
            crf: 0,
            bitrate_kbps: 1536,
            ten_bit: false,
            color_space: ColorSpace::Sdr,
            audio_only: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_and_export_presets_resolve_valid_configurations() {
        let hero = speed_preset_points("hero").expect("hero preset");
        assert!(hero.len() >= 4);
        assert!((hero[0].at - 0.0).abs() < 1e-6);
        assert!((hero.last().unwrap().at - 1.0).abs() < 1e-6);

        let exports = builtin_export_presets();
        assert!(exports.iter().any(|p| p.id == "vertical-9x16" && p.width == 1080 && p.height == 1920));
        assert!(exports.iter().any(|p| p.id == "podcast-audio" && p.audio_only));
    }
}
