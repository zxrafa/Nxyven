// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 NXYVEN contributors

//! Intelligent audio & editorial tools: Automatic Audio Ducking under dialogue,
//! Silence Detection & Strip, Beat/Onset Detection, and Loudness Normalization.

use crate::model::{Clip, KeyEase, KeyProperty, TimeRangeSpan};

/// Configuration for automatic audio ducking on background music clips.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DuckingConfig {
    /// Gain multiplier applied during active dialogue (e.g. `0.25` for `-12 dB`).
    pub duck_gain: f64,
    /// Ramp-down transition duration in seconds before dialogue starts.
    pub attack: f64,
    /// Ramp-up transition duration in seconds after dialogue ends.
    pub release: f64,
    /// Minimum gap between dialogue segments before restoring full volume.
    pub hold_gap: f64,
}

impl Default for DuckingConfig {
    fn default() -> Self {
        Self {
            duck_gain: 0.25,
            attack: 0.2,
            release: 0.35,
            hold_gap: 0.4,
        }
    }
}

/// Merges nearby speech intervals `(start, end)` separated by less than `hold_gap`.
pub fn coalesce_intervals(mut spans: Vec<(f64, f64)>, hold_gap: f64) -> Vec<(f64, f64)> {
    spans.retain(|(s, e)| s.is_finite() && e.is_finite() && *e > *s);
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (s, e) in spans {
        if let Some(last) = merged.last_mut()
            && s <= last.1 + hold_gap.max(0.0)
        {
            last.1 = last.1.max(e);
        } else {
            merged.push((s, e));
        }
    }
    merged
}

/// Applies automatic volume ducking keyframes onto `music_clip` whenever any interval in
/// `speech_spans` overlaps the music clip's timeline span.
pub fn apply_audio_ducking(
    music_clip: &mut Clip,
    speech_spans: &[(f64, f64)],
    config: DuckingConfig,
) -> bool {
    let dur = music_clip.duration;
    if dur <= 1.0 / 60.0 || speech_spans.is_empty() {
        return false;
    }
    let clip_start = music_clip.start;
    let clip_end = clip_start + dur;
    let coalesced = coalesce_intervals(speech_spans.to_vec(), config.hold_gap);
    let overlapping: Vec<(f64, f64)> = coalesced
        .into_iter()
        .filter(|&(s, e)| e > clip_start && s < clip_end)
        .collect();
    if overlapping.is_empty() {
        return false;
    }

    let base_vol = music_clip.volume;
    let ducked_vol = (base_vol * config.duck_gain.clamp(0.0, 1.0)).max(0.0);
    music_clip.clear_keys(KeyProperty::Volume);
    music_clip.set_key(KeyProperty::Volume, 0.0, base_vol, KeyEase::LINEAR);

    for (s, e) in overlapping {
        let pre = ((s - config.attack - clip_start) / dur).clamp(0.0, 1.0);
        let down = ((s - clip_start) / dur).clamp(0.0, 1.0);
        let up_start = ((e - clip_start) / dur).clamp(0.0, 1.0);
        let post = ((e + config.release - clip_start) / dur).clamp(0.0, 1.0);

        if pre > 0.0 {
            music_clip.set_key(KeyProperty::Volume, pre, base_vol, KeyEase::LINEAR);
        }
        music_clip.set_key(KeyProperty::Volume, down, ducked_vol, KeyEase::IN_OUT);
        music_clip.set_key(KeyProperty::Volume, up_start, ducked_vol, KeyEase::LINEAR);
        if post < 1.0 {
            music_clip.set_key(KeyProperty::Volume, post, base_vol, KeyEase::IN_OUT);
        }
    }
    true
}

/// Detects silent intervals `(start_sec, end_sec)` from mono/interleaved PCM samples.
pub fn detect_silence_in_samples(
    samples: &[f32],
    sample_rate: u32,
    channels: usize,
    threshold_db: f64,
    min_silence_duration: f64,
) -> Vec<TimeRangeSpan> {
    let ch = channels.max(1);
    let sr = sample_rate.max(1) as f64;
    let window_frames = ((sr * 0.02).round() as usize).max(1);
    let linear_threshold = 10.0_f64.powf(threshold_db / 20.0) as f32;
    let total_frames = samples.len() / ch;
    let mut silent_spans = Vec::new();
    let mut silent_start: Option<usize> = None;

    for frame_idx in (0..total_frames).step_by(window_frames) {
        let end_idx = (frame_idx + window_frames).min(total_frames);
        let slice = &samples[frame_idx * ch..end_idx * ch];
        let sum_sq: f32 = slice.iter().map(|&x| x * x).sum();
        let rms = (sum_sq / (slice.len().max(1) as f32)).sqrt();
        if rms <= linear_threshold {
            silent_start.get_or_insert(frame_idx);
        } else if let Some(start_f) = silent_start.take() {
            let s = start_f as f64 / sr;
            let e = frame_idx as f64 / sr;
            if e - s >= min_silence_duration {
                silent_spans.push(TimeRangeSpan { start: s, end: e });
            }
        }
    }
    if let Some(start_f) = silent_start {
        let s = start_f as f64 / sr;
        let e = total_frames as f64 / sr;
        if e - s >= min_silence_duration {
            silent_spans.push(TimeRangeSpan { start: s, end: e });
        }
    }
    silent_spans
}

/// Detects musical beat / onset timestamps (in seconds) using energy-flux peak picking.
pub fn detect_beats_in_samples(
    samples: &[f32],
    sample_rate: u32,
    channels: usize,
    min_beat_interval: f64,
) -> Vec<f64> {
    let ch = channels.max(1);
    let sr = sample_rate.max(1) as f64;
    let hop = ((sr * 0.01).round() as usize).max(1);
    let total_frames = samples.len() / ch;
    if total_frames < hop * 4 {
        return Vec::new();
    }
    let mut energies = Vec::with_capacity(total_frames / hop + 1);
    for f in (0..total_frames).step_by(hop) {
        let end = (f + hop).min(total_frames);
        let slice = &samples[f * ch..end * ch];
        let e: f32 = slice.iter().map(|&x| x * x).sum::<f32>() / (slice.len().max(1) as f32);
        energies.push(e);
    }
    let mut flux = vec![0.0f32; energies.len()];
    for i in 1..energies.len() {
        flux[i] = (energies[i] - energies[i - 1]).max(0.0);
    }
    let mean_flux: f32 = flux.iter().sum::<f32>() / (flux.len().max(1) as f32);
    let threshold = (mean_flux * 2.5).max(1e-4);
    let min_hops = ((min_beat_interval.max(0.1) * sr) / hop as f64).round() as usize;

    let mut beats = Vec::new();
    let mut last_hop: Option<usize> = None;
    for i in 1..flux.len().saturating_sub(1) {
        if flux[i] > threshold && flux[i] >= flux[i - 1] && flux[i] >= flux[i + 1] {
            if last_hop.is_none_or(|prev| i >= prev + min_hops) {
                beats.push((i * hop) as f64 / sr);
                last_hop = Some(i);
            }
        }
    }
    beats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ClipKind;

    #[test]
    fn audio_ducking_silence_and_beat_detection_work_end_to_end() {
        let mut music = Clip::blank("m1", "T2", ClipKind::Audio, "Music", 0.0, 10.0);
        assert!(apply_audio_ducking(&mut music, &[(2.0, 5.0)], DuckingConfig::default()));
        assert!((music.value_at(KeyProperty::Volume, 0.35) - 0.25).abs() < 1e-4);
        assert!((music.value_at(KeyProperty::Volume, 0.9) - 1.0).abs() < 1e-4);

        let sr = 1000u32;
        let mut pcm = vec![0.5f32; 1000];
        pcm.extend(vec![0.0f32; 1000]);
        pcm.extend(vec![0.5f32; 1000]);
        let silences = detect_silence_in_samples(&pcm, sr, 1, -40.0, 0.5);
        assert_eq!(silences.len(), 1);
        assert!((silences[0].start - 1.0).abs() < 0.05);
        assert!((silences[0].end - 2.0).abs() < 0.05);

        let mut pulse = vec![0.0f32; 3000];
        for beat_ms in [500usize, 1000, 1500, 2000, 2500] {
            for k in 0..20 {
                pulse[beat_ms + k] = 0.95;
            }
        }
        let beats = detect_beats_in_samples(&pulse, sr, 1, 0.3);
        assert_eq!(beats.len(), 5);
    }
}
