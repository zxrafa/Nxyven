// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 NXYVEN contributors

//! Real-time video scopes: RGB/Luma Histogram, RGB Parade, Luma Waveform,
//! and 2D Vectorscope computed directly from an RGBA8 [`Frame`].

use crate::frame::Frame;

/// 256-bin histogram for Red, Green, Blue, and Rec. 709 Luma ($Y'$).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Histogram {
    /// Red channel bin counts (`0..=255`).
    pub r: [u32; 256],
    /// Green channel bin counts (`0..=255`).
    pub g: [u32; 256],
    /// Blue channel bin counts (`0..=255`).
    pub b: [u32; 256],
    /// Rec. 709 luma bin counts (`0..=255`).
    pub luma: [u32; 256],
    /// Maximum count across all channels (for normalizing scope display height).
    pub peak: u32,
}

/// Column-wise waveform / RGB parade density grid (`columns × 256` levels).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaveformScope {
    /// Horizontal bucket count.
    pub columns: usize,
    /// Luma density (`columns * 256`, row-major by `[col * 256 + level]`).
    pub luma: Vec<u32>,
    /// Red parade density (`columns * 256`).
    pub r: Vec<u32>,
    /// Green parade density (`columns * 256`).
    pub g: Vec<u32>,
    /// Blue parade density (`columns * 256`).
    pub b: Vec<u32>,
}

/// 2D chrominance Vectorscope ($Cb \times Cr$ density grid).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vectorscope {
    /// Grid side in bins (e.g. `128` for `128×128`).
    pub size: usize,
    /// Chrominance hit counts (`size * size`, indexed `[v_bin * size + u_bin]`).
    pub grid: Vec<u32>,
    /// Maximum bin density in `grid`.
    pub peak: u32,
}

#[inline]
fn rec709_luma(r: u8, g: u8, b: u8) -> u8 {
    ((54 * u32::from(r) + 183 * u32::from(g) + 19 * u32::from(b) + 128) >> 8) as u8
}

#[inline]
fn rec709_cb_cr(r: u8, g: u8, b: u8) -> (u8, u8) {
    let rf = f32::from(r);
    let gf = f32::from(g);
    let bf = f32::from(b);
    let cb = (128.0 + (-0.114572 * rf - 0.385428 * gf + 0.5 * bf)).clamp(0.0, 255.0) as u8;
    let cr = (128.0 + (0.5 * rf - 0.454153 * gf - 0.045847 * bf)).clamp(0.0, 255.0) as u8;
    (cb, cr)
}

/// Computes a 256-bin RGB + Rec. 709 Luma [`Histogram`] from `frame`.
pub fn compute_histogram(frame: &Frame) -> Histogram {
    let mut r = [0u32; 256];
    let mut g = [0u32; 256];
    let mut b = [0u32; 256];
    let mut luma = [0u32; 256];
    for px in frame.pixels().chunks_exact(4) {
        r[usize::from(px[0])] += 1;
        g[usize::from(px[1])] += 1;
        b[usize::from(px[2])] += 1;
        luma[usize::from(rec709_luma(px[0], px[1], px[2]))] += 1;
    }
    let peak = r
        .iter()
        .chain(&g)
        .chain(&b)
        .chain(&luma)
        .copied()
        .max()
        .unwrap_or(0);
    Histogram { r, g, b, luma, peak }
}

/// Computes a Luma Waveform and RGB Parade with `columns` horizontal buckets.
pub fn compute_waveform(frame: &Frame, columns: usize) -> WaveformScope {
    let cols = columns.max(1);
    let mut luma = vec![0u32; cols * 256];
    let mut r = vec![0u32; cols * 256];
    let mut g = vec![0u32; cols * 256];
    let mut b = vec![0u32; cols * 256];
    let w = (frame.width() as usize).max(1);
    let h = frame.height() as usize;
    let data = frame.pixels();
    for y in 0..h {
        let row = &data[y * w * 4..(y + 1) * w * 4];
        for (x, px) in row.chunks_exact(4).enumerate() {
            let col: usize = (x * cols / w).min(cols - 1);
            let base = col * 256;
            r[base + usize::from(px[0])] += 1;
            g[base + usize::from(px[1])] += 1;
            b[base + usize::from(px[2])] += 1;
            luma[base + usize::from(rec709_luma(px[0], px[1], px[2]))] += 1;
        }
    }
    WaveformScope { columns: cols, luma, r, g, b }
}

/// Computes a 2D [`Vectorscope`] of `size × size` chrominance bins.
pub fn compute_vectorscope(frame: &Frame, size: usize) -> Vectorscope {
    let side = size.clamp(16, 256);
    let mut grid = vec![0u32; side * side];
    for px in frame.pixels().chunks_exact(4) {
        let (cb, cr) = rec709_cb_cr(px[0], px[1], px[2]);
        let u_bin = (usize::from(cb) * side / 256).min(side - 1);
        let v_bin = ((255 - usize::from(cr)) * side / 256).min(side - 1);
        grid[v_bin * side + u_bin] += 1;
    }
    let peak = grid.iter().copied().max().unwrap_or(0);
    Vectorscope { size: side, grid, peak }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_analyze_pure_colors_and_neutral_grey_accurately() {
        let frame = Frame::from_rgba(4, 4, [128u8, 128, 128, 255].repeat(16)).expect("frame");
        let hist = compute_histogram(&frame);
        assert_eq!(hist.r[128], 16);
        assert_eq!(hist.g[128], 16);
        assert_eq!(hist.b[128], 16);
        assert_eq!(hist.luma[128], 16);
        assert_eq!(hist.peak, 16);

        let wf = compute_waveform(&frame, 2);
        assert_eq!(wf.columns, 2);
        assert_eq!(wf.luma[128], 8);
        assert_eq!(wf.luma[256 + 128], 8);

        let vs = compute_vectorscope(&frame, 64);
        assert_eq!(vs.peak, 16);
    }
}
