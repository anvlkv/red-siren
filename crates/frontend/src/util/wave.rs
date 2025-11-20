/// Renders a single waveform path from the chronological samples along `x` axis.
pub fn waveform_path_x(
    samples: &[f32],
    total_len: f32,
    center_x: f32,
    center_y: f32,
    amp: f32,
) -> String {
    if samples.len() < 2 {
        return String::new();
    }
    let points = samples.len();
    let dx = total_len / (points - 1) as f32;
    let start_x = center_x - total_len * 0.5;
    let mut s = String::with_capacity(points * 12);
    for (i, &src) in samples.iter().enumerate() {
        let x = start_x + dx * i as f32;
        let y = center_y - src * amp;
        if i == 0 {
            s.push_str(&format!("M{:.2} {:.2}", x, y));
        } else {
            s.push_str(&format!("L{:.2} {:.2}", x, y));
        }
    }
    s
}

/// Renders a waveform along an arbitrary line segment from `start` to `end`.
/// Samples are displaced perpendicular to the line by `amp * -sample`.
pub fn waveform_path_along_dbe(
    samples: &[f32],
    start: mint::Point2<f64>,
    end: mint::Point2<f64>,
    amp: f64,
) -> String {
    if samples.len() < 2 {
        return String::new();
    }

    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f64::EPSILON {
        return String::new();
    }

    // Unit normal to the line (perpendicular)
    let nx = -dy / len;
    let ny = dx / len;

    let points = samples.len();
    let mut s = String::with_capacity(points * 16);

    for (i, &src) in samples.iter().enumerate() {
        let t = i as f64 / (points - 1) as f64;
        let px = start.x + dx * t;
        let py = start.y + dy * t;

        // Match existing convention: invert sample for displacement direction.
        let off = -src as f64 * amp;
        let x = px + off * nx;
        let y = py + off * ny;

        if i == 0 {
            s.push_str(&format!("M{:.2} {:.2}", x, y));
        } else {
            s.push_str(&format!("L{:.2} {:.2}", x, y));
        }
    }

    s
}
