//! Tray icon drawn in code so the repository needs no binary assets.
//!
//! macOS renders status items from *template* images: only the alpha channel is
//! used and the OS tints the glyph for light/dark menu bars. Windows uses the
//! colours as-is, where a nearly white glyph is readable on the default dark
//! taskbar.

/// RGBA pixels for the tray icon: a rounded frame with a filled top-left
/// quadrant, i.e. the "snap to top-left" gesture this tool is about.
pub fn rgba(size: u32, color: [u8; 3]) -> Vec<u8> {
    let size = size.max(8);
    let size_f = size as f32;

    // Frame geometry in unit space.
    let outer: f32 = 0.06; // inset from the bitmap edge
    let stroke: f32 = 0.10; // frame thickness
    let radius: f32 = 0.22;
    let inner: f32 = outer + stroke;
    let quadrant_cut: f32 = 0.50; // quadrant covers the top-left half of the inner box

    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            // Sample at the pixel centre in 0..1 space.
            let px = (x as f32 + 0.5) / size_f;
            let py = (y as f32 + 0.5) / size_f;

            let frame = ring_coverage(px, py, outer, radius, stroke);
            let quadrant = if px <= 0.5 && py <= 0.5 {
                box_coverage(px, py, inner, 0.5 - quadrant_cut * 0.0, radius * 0.5)
            } else {
                0.0
            };

            let alpha = frame.max(quadrant);
            let a = (alpha * 255.0).round() as u8;
            pixels.extend_from_slice(&[color[0], color[1], color[2], a]);
        }
    }
    pixels
}

/// Coverage of a rounded rectangle outline.
fn ring_coverage(px: f32, py: f32, inset: f32, radius: f32, stroke: f32) -> f32 {
    let distance = rounded_box_distance(px, py, inset, 1.0 - inset, radius);
    let half = stroke * 0.5;
    // |d| <= half inside the stroke, anti-aliased over one pixel worth of space.
    let aa = 0.5 * stroke;
    let outer_edge = smoothstep(half + aa, half - aa, distance);
    let inner_edge = smoothstep(-half - aa, -half + aa, distance);
    (outer_edge * inner_edge).clamp(0.0, 1.0)
}

/// Coverage of a filled rounded rectangle spanning `inset..right` / `inset..bottom`.
fn box_coverage(px: f32, py: f32, inset: f32, right: f32, radius: f32) -> f32 {
    let distance = rounded_box_distance(px, py, inset, right.max(inset + 0.01), radius);
    let aa = 0.03;
    smoothstep(aa, -aa, distance).clamp(0.0, 1.0)
}

/// Signed distance to a rounded box; negative inside.
fn rounded_box_distance(px: f32, py: f32, left: f32, right: f32, radius: f32) -> f32 {
    let bottom = right;
    let half_w = (right - left) * 0.5;
    let half_h = (bottom - left) * 0.5;
    let cx = left + half_w;
    let cy = left + half_h;
    let r = radius.min(half_w).min(half_h);

    let qx = (px - cx).abs() - (half_w - r);
    let qy = (py - cy).abs() - (half_h - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < f32::EPSILON {
        return if x <= edge0 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::rgba;

    #[test]
    fn produces_rgba_of_expected_length() {
        let pixels = rgba(32, [255, 255, 255]);
        assert_eq!(pixels.len(), 32 * 32 * 4);
    }

    #[test]
    fn centre_is_transparent_and_quadrant_is_opaque() {
        let size = 32usize;
        let pixels = rgba(32, [255, 255, 255]);
        let alpha_at = |x: usize, y: usize| pixels[(y * size + x) * 4 + 3];

        // Middle of the bitmap falls in the empty bottom-right area.
        assert_eq!(alpha_at(size / 2, size / 2), 0);
        // Top-left quadrant is filled.
        assert!(
            alpha_at(size / 4, size / 4) > 200,
            "{}",
            alpha_at(size / 4, size / 4)
        );
        // Corner pixel is outside the rounded frame.
        assert_eq!(alpha_at(0, 0), 0);
    }

    #[test]
    fn very_small_icons_do_not_panic() {
        for size in 0..12 {
            let pixels = rgba(size, [0, 0, 0]);
            let expected = size.max(8) as usize;
            assert_eq!(pixels.len(), expected * expected * 4);
        }
    }
}
