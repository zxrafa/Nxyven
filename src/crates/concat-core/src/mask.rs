// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 NXYVEN contributors

//! Geometric and polygonal clip masks (`Rectangle`, `Ellipse`, `Polygon`) with
//! soft edge feathering, expansion, inversion, and opacity blending.

use crate::frame::Frame;

/// Shape of a clip mask.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MaskShape {
    /// Axis-aligned or rotated rectangle with optional corner rounding.
    #[default]
    Rectangle,
    /// Elliptical / circular mask.
    Ellipse,
    /// Closed polygon defined by normalized `[x, y]` vertices (`0..=1`).
    Polygon,
}

/// A resolved geometric or polygonal mask ready to evaluate over normalized UV `(0..=1, 0..=1)`.
#[derive(Clone, PartialEq, Debug)]
pub struct MaskSpec {
    /// Shape primitive.
    pub shape: MaskShape,
    /// Center X in normalized frame coordinates (`0.5` is center).
    pub center_x: f64,
    /// Center Y in normalized frame coordinates (`0.5` is center).
    pub center_y: f64,
    /// Half-width or full width fraction (`0..=2.0`, default `0.5`).
    pub width: f64,
    /// Height fraction (`0..=2.0`, default `0.5`).
    pub height: f64,
    /// Clockwise rotation in degrees around `(center_x, center_y)`.
    pub rotation: f64,
    /// Corner radius fraction for [`MaskShape::Rectangle`] (`0..=1`).
    pub radius: f64,
    /// Edge softness fraction (`0..=0.5`).
    pub feather: f64,
    /// Signed expansion offset (`-0.5..=0.5`).
    pub expansion: f64,
    /// Mask strength (`0..=1`).
    pub opacity: f64,
    /// When true, cuts out the inside instead of keeping it.
    pub invert: bool,
    /// Normalized vertices `[x, y]` when `shape == MaskShape::Polygon`.
    pub points: Vec<[f64; 2]>,
}

impl Default for MaskSpec {
    fn default() -> Self {
        Self {
            shape: MaskShape::Rectangle,
            center_x: 0.5,
            center_y: 0.5,
            width: 0.5,
            height: 0.5,
            rotation: 0.0,
            radius: 0.0,
            feather: 0.0,
            expansion: 0.0,
            opacity: 1.0,
            invert: false,
            points: Vec::new(),
        }
    }
}

impl MaskSpec {
    /// Evaluates the mask coverage in `0.0..=1.0` at normalized `(u, v)`.
    pub fn sample(&self, u: f64, v: f64) -> f64 {
        let dist = match self.shape {
            MaskShape::Rectangle => self.sdf_rect(u, v),
            MaskShape::Ellipse => self.sdf_ellipse(u, v),
            MaskShape::Polygon => self.sdf_polygon(u, v),
        } - self.expansion;

        let fw = self.feather.max(0.0);
        let raw = if fw <= 1e-6 {
            if dist <= 0.0 { 1.0 } else { 0.0 }
        } else {
            let t = ((fw - dist) / (2.0 * fw)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let shaped = if self.invert { 1.0 - raw } else { raw };
        1.0 - self.opacity.clamp(0.0, 1.0) * (1.0 - shaped)
    }

    fn local_coords(&self, u: f64, v: f64) -> (f64, f64) {
        let dx = u - self.center_x;
        let dy = v - self.center_y;
        let rad = -self.rotation.to_radians();
        let (s, c) = rad.sin_cos();
        (dx * c - dy * s, dx * s + dy * c)
    }

    fn sdf_rect(&self, u: f64, v: f64) -> f64 {
        let (lx, ly) = self.local_coords(u, v);
        let hx = (self.width * 0.5).max(1e-4);
        let hy = (self.height * 0.5).max(1e-4);
        let r = (self.radius.clamp(0.0, 1.0) * hx.min(hy)).min(hx.min(hy));
        let qx = lx.abs() - (hx - r);
        let qy = ly.abs() - (hy - r);
        let out = (qx.max(0.0).hypot(qy.max(0.0))) + qx.max(qy).min(0.0) - r;
        out
    }

    fn sdf_ellipse(&self, u: f64, v: f64) -> f64 {
        let (lx, ly) = self.local_coords(u, v);
        let rx = (self.width * 0.5).max(1e-4);
        let ry = (self.height * 0.5).max(1e-4);
        let norm = (lx / rx).hypot(ly / ry);
        (norm - 1.0) * rx.min(ry)
    }

    fn sdf_polygon(&self, u: f64, v: f64) -> f64 {
        if self.points.len() < 3 {
            return 1.0;
        }
        let mut inside = false;
        let mut min_d2 = f64::INFINITY;
        let n = self.points.len();
        for i in 0..n {
            let [x0, y0] = self.points[i];
            let [x1, y1] = self.points[(i + 1) % n];
            let ex = x1 - x0;
            let ey = y1 - y0;
            let wx = u - x0;
            let wy = v - y0;
            let len2 = (ex * ex + ey * ey).max(1e-12);
            let t = ((wx * ex + wy * ey) / len2).clamp(0.0, 1.0);
            let dx = wx - ex * t;
            let dy = wy - ey * t;
            min_d2 = min_d2.min(dx * dx + dy * dy);
            if ((y0 > v) != (y1 > v)) && (u < (x1 - x0) * (v - y0) / (y1 - y0) + x0) {
                inside = !inside;
            }
        }
        let d = min_d2.sqrt();
        if inside { -d } else { d }
    }
}

/// Multiplies `frame`'s alpha channel in place by every active [`MaskSpec`] in `masks`.
pub fn apply_masks(frame: &mut Frame, masks: &[MaskSpec]) {
    if masks.is_empty() {
        return;
    }
    let w = frame.width().max(1) as f64;
    let h = frame.height().max(1) as f64;
    let width = frame.width() as usize;
    for (idx, px) in frame.pixels_mut().chunks_exact_mut(4).enumerate() {
        let x = (idx % width) as f64 + 0.5;
        let y = (idx / width) as f64 + 0.5;
        let (u, v) = (x / w, y / h);
        let mut alpha = 1.0;
        for mask in masks {
            alpha *= mask.sample(u, v);
        }
        px[3] = ((f64::from(px[3]) * alpha.clamp(0.0, 1.0)).round() as u32).min(255) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_ellipse_and_polygon_masks_gate_alpha() {
        let rect = MaskSpec::default();
        assert!((rect.sample(0.5, 0.5) - 1.0).abs() < 1e-6);
        assert!(rect.sample(0.05, 0.05) < 1e-6);

        let mut inv = rect.clone();
        inv.invert = true;
        assert!(inv.sample(0.5, 0.5) < 1e-6);
        assert!((inv.sample(0.05, 0.05) - 1.0).abs() < 1e-6);

        let poly = MaskSpec {
            shape: MaskShape::Polygon,
            points: vec![[0.2, 0.2], [0.8, 0.2], [0.5, 0.8]],
            ..MaskSpec::default()
        };
        assert!((poly.sample(0.5, 0.4) - 1.0).abs() < 1e-6);
        assert!(poly.sample(0.1, 0.1) < 1e-6);
    }
}
