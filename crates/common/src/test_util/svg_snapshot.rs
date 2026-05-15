use mint::Point2;

/// Converts 2D point paths into deterministic SVG bytes for binary snapshot tests.
pub fn assert_paths_points_2d(paths: &[&[Point2<f64>]]) -> Vec<u8> {
    let (min_x, min_y, width, height) = svg_bounds(paths);
    let stroke_width = stroke_width_for_bounds(width, height);
    let marker_radius = marker_radius_for_bounds(width, height);

    let mut svg = String::new();
    svg.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" ");
    svg.push_str("viewBox=\"");
    svg.push_str(&fmt_f64(min_x));
    svg.push(' ');
    svg.push_str(&fmt_f64(min_y));
    svg.push(' ');
    svg.push_str(&fmt_f64(width));
    svg.push(' ');
    svg.push_str(&fmt_f64(height));
    svg.push_str("\">\n");

    for (path_index, path) in paths.iter().enumerate() {
        if path.is_empty() {
            continue;
        }

        let first = sanitize(path[0]);
        let closed = if path.len() > 1 {
            let last = sanitize(path[path.len() - 1]);
            (last.x - first.x).abs() <= f64::EPSILON && (last.y - first.y).abs() <= f64::EPSILON
        } else {
            false
        };
        let draw_len = if closed { path.len() - 1 } else { path.len() };

        svg.push_str("  <path d=\"");

        svg.push_str("M ");
        svg.push_str(&fmt_f64(first.x));
        svg.push(' ');
        svg.push_str(&fmt_f64(first.y));

        for point in &path[1..draw_len] {
            let point = sanitize(*point);
            svg.push_str(" L ");
            svg.push_str(&fmt_f64(point.x));
            svg.push(' ');
            svg.push_str(&fmt_f64(point.y));
        }

        if closed {
            svg.push_str(" Z");
        }

        svg.push_str("\" fill=\"");
        if closed {
            svg.push_str("rgba(0,0,0,0.12)");
        } else {
            svg.push_str("none");
        }
        svg.push_str("\" stroke=\"");
        if path_index == 0 {
            svg.push_str("black");
        } else {
            svg.push_str("#1f77b4");
        }
        svg.push_str("\" stroke-width=\"");
        svg.push_str(&fmt_f64(stroke_width));
        svg.push_str("\" />\n");

        // Add a deterministic marker at the first point to make path orientation/debugging obvious.
        svg.push_str("  <circle fill=\"red\" stroke=\"none\" cx=\"");
        svg.push_str(&fmt_f64(first.x));
        svg.push_str("\" cy=\"");
        svg.push_str(&fmt_f64(first.y));
        svg.push_str("\" r=\"");
        svg.push_str(&fmt_f64(marker_radius));
        svg.push_str("\" />\n");
    }

    svg.push_str("</svg>\n");
    svg.into_bytes()
}

#[macro_export]
macro_rules! assert_paths_points_2d_snapshot {
    ($paths:expr) => {
        insta::assert_binary_snapshot!($crate::test_util::assert_paths_points_2d($paths));
    };
    ($name:expr, $paths:expr) => {
        insta::assert_binary_snapshot!(
            &$crate::test_util::svg_snapshot_name($name),
            $crate::test_util::assert_paths_points_2d($paths)
        );
    };
}

pub use crate::assert_paths_points_2d_snapshot;

pub fn svg_snapshot_name(name: impl AsRef<str>) -> String {
    let name = name.as_ref();
    if name.rsplit_once('.').is_some() {
        name.to_string()
    } else {
        format!("{name}.svg")
    }
}

fn svg_bounds(paths: &[&[Point2<f64>]]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for path in paths {
        for point in *path {
            let point = sanitize(*point);
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }
    }

    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return (0.0, 0.0, 100.0, 100.0);
    }

    let pad = 1.0;
    let span_x = (max_x - min_x).max(1.0);
    let span_y = (max_y - min_y).max(1.0);

    (
        min_x - pad,
        min_y - pad,
        span_x + (pad * 2.0),
        span_y + (pad * 2.0),
    )
}

fn stroke_width_for_bounds(width: f64, height: f64) -> f64 {
    let min_span = width.min(height).max(1.0);
    // Keep line thickness visible across tiny and very large coordinate spaces.
    (min_span * 0.0025).max(f64::EPSILON)
}

fn marker_radius_for_bounds(width: f64, height: f64) -> f64 {
    let min_span = width.min(height).max(1.0);
    (min_span * 0.01).max(f64::EPSILON)
}

fn sanitize(point: Point2<f64>) -> Point2<f64> {
    Point2 {
        x: if point.x.is_finite() { point.x } else { 0.0 },
        y: if point.y.is_finite() { point.y } else { 0.0 },
    }
}

fn fmt_f64(value: f64) -> String {
    let mut out = format!("{value:.6}");
    while out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.pop();
    }
    if out.is_empty() || out == "-0" {
        return "0".to_string();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_output_contains_each_path() {
        let path_a = [Point2 { x: 0.0, y: 0.0 }, Point2 { x: 1.0, y: 1.0 }];
        let path_b = [Point2 { x: 2.0, y: 3.0 }, Point2 { x: 4.0, y: 5.0 }];
        let paths: &[&[Point2<f64>]] = &[&path_a, &path_b];

        let bytes = assert_paths_points_2d(paths);
        let svg = String::from_utf8(bytes).expect("valid utf8");

        assert!(svg.contains("<path d=\"M 0 0 L 1 1\" fill=\"none\" stroke=\"black\""));
        assert!(svg.contains("<path d=\"M 2 3 L 4 5\" fill=\"none\" stroke=\"#1f77b4\""));
        assert!(svg.contains("<circle fill=\"red\" stroke=\"none\" cx=\"0\" cy=\"0\""));
    }

    #[test]
    fn svg_output_closes_path_when_start_matches_end() {
        let closed = [
            Point2 { x: 0.0, y: 0.0 },
            Point2 { x: 1.0, y: 0.0 },
            Point2 { x: 1.0, y: 1.0 },
            Point2 { x: 0.0, y: 0.0 },
        ];
        let paths: &[&[Point2<f64>]] = &[&closed];

        let bytes = assert_paths_points_2d(paths);
        let svg = String::from_utf8(bytes).expect("valid utf8");

        assert!(svg.contains("<path d=\"M 0 0 L 1 0 L 1 1 Z\" fill=\"rgba(0,0,0,0.12)\""));
    }

    #[test]
    fn snapshot_name_adds_svg_extension_when_missing() {
        assert_eq!(svg_snapshot_name("node_outlines"), "node_outlines.svg");
        assert_eq!(
            svg_snapshot_name("node_outlines_shape_variations.svg"),
            "node_outlines_shape_variations.svg"
        );
    }
}
