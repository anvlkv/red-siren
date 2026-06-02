use std::time::{Duration, Instant};

use eframe::egui;

pub struct VisualizationThrottle {
    interval: Duration,
    last_refresh: Option<Instant>,
    debug_tag: &'static str,
}

impl VisualizationThrottle {
    pub const SPECTRUM_INTERVAL_MS: u64 = 100;
    pub const OUTPUT_INTERVAL_MS: u64 = 16;

    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_refresh: None,
            debug_tag: "unspecified",
        }
    }

    pub fn with_tag(mut self, tag: &'static str) -> Self {
        self.debug_tag = tag;
        self
    }

    pub fn spectrum_default() -> Self {
        Self::new(Duration::from_millis(Self::SPECTRUM_INTERVAL_MS))
    }

    pub fn spectrum_default_tagged(tag: &'static str) -> Self {
        Self::spectrum_default().with_tag(tag)
    }

    pub fn output_default() -> Self {
        Self::new(Duration::from_millis(Self::OUTPUT_INTERVAL_MS))
    }

    pub fn output_default_tagged(tag: &'static str) -> Self {
        Self::output_default().with_tag(tag)
    }

    pub fn should_refresh(&mut self) -> bool {
        let now = Instant::now();
        let (should_refresh, elapsed_ms) = match self.last_refresh {
            None => {
                self.last_refresh = Some(now);
                (true, -1.0)
            }
            Some(last_refresh) if now.duration_since(last_refresh) >= self.interval => {
                self.last_refresh = Some(now);
                (
                    true,
                    now.duration_since(last_refresh).as_secs_f64() * 1000.0,
                )
            }
            Some(last_refresh) => (
                false,
                now.duration_since(last_refresh).as_secs_f64() * 1000.0,
            ),
        };

        if log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "visualization_throttle_diag tag={} interval_ms={:.1} elapsed_ms={:.1} should_refresh={}",
                self.debug_tag,
                self.interval.as_secs_f64() * 1000.0,
                elapsed_ms,
                should_refresh,
            );
        }

        should_refresh
    }

    pub fn reset(&mut self) {
        self.last_refresh = None;
        if log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "visualization_throttle_reset tag={} interval_ms={:.1}",
                self.debug_tag,
                self.interval.as_secs_f64() * 1000.0,
            );
        }
    }
}

pub fn show_buffer_line_chart(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    samples: &[f32],
    empty_label: &str,
    color: egui::Color32,
) {
    if samples.is_empty() {
        ui.small(empty_label);
        return;
    }

    ui.push_id(id_salt, |ui| {
        let chart_height = 180.0;
        let chart_size = egui::vec2(ui.available_width(), chart_height);
        let (rect, response) = ui.allocate_exact_size(chart_size, egui::Sense::hover());
        let painter = ui.painter_at(rect);

        let max_abs = samples
            .iter()
            .copied()
            .map(f32::abs)
            .fold(f32::EPSILON, f32::max);

        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color),
            egui::StrokeKind::Middle,
        );

        let mid_y = rect.center().y;
        painter.line_segment(
            [
                egui::pos2(rect.left(), mid_y),
                egui::pos2(rect.right(), mid_y),
            ],
            egui::Stroke::new(1.0_f32, ui.visuals().weak_text_color()),
        );

        let sample_count = samples.len();
        let half_height = rect.height() * 0.5;
        let points = samples
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                let x_t = if sample_count <= 1 {
                    0.0
                } else {
                    index as f32 / (sample_count - 1) as f32
                };
                let y_t = (sample / max_abs).clamp(-1.0, 1.0);
                let x = egui::lerp(rect.left()..=rect.right(), x_t);
                let y = mid_y - y_t * half_height;
                egui::pos2(x, y)
            })
            .collect::<Vec<_>>();

        painter.line(points, egui::Stroke::new(2.0_f32, color));

        let text_color = ui.visuals().weak_text_color();
        painter.text(
            egui::pos2(rect.left() + 6.0, rect.top() + 6.0),
            egui::Align2::LEFT_TOP,
            format!("{} samples", sample_count),
            egui::FontId::monospace(11.0),
            text_color,
        );
        painter.text(
            egui::pos2(rect.right() - 6.0, rect.top() + 6.0),
            egui::Align2::RIGHT_TOP,
            format!("peak {:.3e}", max_abs),
            egui::FontId::monospace(11.0),
            text_color,
        );

        response.on_hover_text("Live output waveform snapshot");
    });
}
