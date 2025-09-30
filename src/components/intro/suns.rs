use leptos::prelude::*;
use mint::{Point2, Vector2};

#[component]
pub fn SplitSuns(
    pos: Point2<f32>,
    radius: f32,
    split: Vector2<f32>,
    stroke_width: f32,
) -> impl IntoView {
    // Calculate points for the two semi-circles
    let center_x = pos.x;
    let center_y = pos.y;
    let split_x = split.x;
    let split_y = split.y;

    // Calculate the angle of the split vector
    let angle = split_y.atan2(split_x);
    let start_angle_1 = angle;
    let end_angle_1 = angle + std::f32::consts::PI;
    let start_angle_2 = angle + std::f32::consts::PI;
    let end_angle_2 = angle + 2.0 * std::f32::consts::PI;

    // Generate arc paths for semi-circles
    let arc_1 = generate_arc_path(center_x, center_y, radius, start_angle_1, end_angle_1);
    let arc_2 = generate_arc_path(center_x, center_y, radius, start_angle_2, end_angle_2);

    // Calculate connection line endpoints
    let line_start_x = center_x + radius * start_angle_1.cos();
    let line_start_y = center_y + radius * start_angle_1.sin();
    let line_end_x = center_x + radius * end_angle_1.cos();
    let line_end_y = center_y + radius * end_angle_1.sin();

    view! {
        <>
            <path d=arc_1 />
            <path d=arc_2 />
            <path
                d=format!("M{} {}L{} {}", line_start_x, line_start_y, line_end_x, line_end_y)
                stroke-width=stroke_width
            />
        </>
    }
}

fn generate_arc_path(cx: f32, cy: f32, r: f32, start_angle: f32, end_angle: f32) -> String {
    let start_x = cx + r * start_angle.cos();
    let start_y = cy + r * start_angle.sin();
    let end_x = cx + r * end_angle.cos();
    let end_y = cy + r * end_angle.sin();

    let large_arc_flag = if (end_angle - start_angle).abs() > std::f32::consts::PI {
        1
    } else {
        0
    };
    let sweep_flag = if end_angle > start_angle { 1 } else { 0 };

    format!(
        "M{} {}A{} {} 0 {} {} {} {}",
        start_x, start_y, r, r, large_arc_flag, sweep_flag, end_x, end_y
    )
}
