use serde::{Deserialize, Serialize};
use thiserror::Error;

const CONTINUITY_EPSILON: f64 = 1e-9;
const ARC_COS_EPSILON: f64 = 1e-9;
const HYPERBOLA_POLE_EPSILON: f64 = 1e-9;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub sampling_density: f64,
    pub curve: SegmentCurve,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SegmentCurve {
    Line { m: f64, b: f64 },
    Constant { value: f64 },
    Parabolic { a: f64, b: f64, c: f64 },
    Hyperbolic { a: f64, b: f64, c: f64 },
    ArcSweep {
        center_y: f64,
        radius: f64,
        start_angle: f64,
        sweep_angle: f64,
    },
}

#[derive(Debug, Error, PartialEq)]
pub enum SegmentError {
    #[error("segment end must be greater than start")]
    InvalidRange,
    #[error("sampling density must be finite and positive")]
    InvalidSamplingDensity,
    #[error("{0} must be finite")]
    NonFiniteParameter(&'static str),
    #[error("{0} cannot preserve C1 continuity")]
    CannotPreserveC1(&'static str),
    #[error("hyperbolic pole must stay outside the segment interval")]
    InvalidHyperbolicPole,
    #[error("arc sweep requires a finite non-zero angle")]
    InvalidArcSweep,
    #[error("arc radius must be finite and greater than zero")]
    InvalidRadius,
    #[error("requested arc slope is unreachable for the provided radius and sweep")]
    ArcSlopeUnreachable,
}

impl Segment {
    pub fn start_curve(end: f64, curve: SegmentCurve) -> Result<Self, SegmentError> {
        let sampling_density = derived_sampling_density(end, &curve);
        let segment = Self {
            start: 0.0,
            end,
            sampling_density,
            curve,
        };
        segment.validate()?;
        Ok(segment)
    }

    pub fn start_line(end: f64, m: f64, b: f64) -> Result<Self, SegmentError> {
        Self::start_curve(end, SegmentCurve::Line { m, b })
    }

    pub fn start_constant(end: f64, value: f64) -> Result<Self, SegmentError> {
        Self::start_curve(end, SegmentCurve::Constant { value })
    }

    pub fn start_parabolic(end: f64, a: f64, b: f64, c: f64) -> Result<Self, SegmentError> {
        Self::start_curve(end, SegmentCurve::Parabolic { a, b, c })
    }

    pub fn start_hyperbolic(end: f64, a: f64, b: f64, c: f64) -> Result<Self, SegmentError> {
        Self::start_curve(end, SegmentCurve::Hyperbolic { a, b, c })
    }

    pub fn start_arc_sweep(
        end: f64,
        center_y: f64,
        radius: f64,
        start_angle: f64,
        sweep_angle: f64,
    ) -> Result<Self, SegmentError> {
        Self::start_curve(
            end,
            SegmentCurve::ArcSweep {
                center_y,
                radius,
                start_angle,
                sweep_angle,
            },
        )
    }

    pub fn continue_constant(
        prev: &Segment,
        to_t: f64,
        sampling_density: f64,
    ) -> Result<Self, SegmentError> {
        let state = prev.end_state();

        if !approx_eq(state.derivative, 0.0) {
            return Err(SegmentError::CannotPreserveC1("constant continuation"));
        }

        Self::from_curve_with_density(
            prev.end,
            to_t,
            sampling_density,
            SegmentCurve::Constant { value: state.value },
        )
    }

    pub fn continue_line(
        prev: &Segment,
        to_t: f64,
        sampling_density: f64,
    ) -> Result<Self, SegmentError> {
        let state = prev.end_state();
        let m = state.derivative;
        let b = state.value - m * prev.end;

        Self::from_curve_with_density(
            prev.end,
            to_t,
            sampling_density,
            SegmentCurve::Line { m, b },
        )
    }

    pub fn continue_parabolic(
        prev: &Segment,
        to_t: f64,
        to_value: f64,
        sampling_density: f64,
    ) -> Result<Self, SegmentError> {
        require_finite(to_value, "to_value")?;

        let state = prev.end_state();
        let dt = validated_span(prev.end, to_t)?;

        let a_c2 = state.second_derivative * 0.5;
        let b_c2 = state.derivative - 2.0 * a_c2 * prev.end;
        let c_c2 = state.value - a_c2 * prev.end * prev.end - b_c2 * prev.end;
        let predicted_end = a_c2 * to_t * to_t + b_c2 * to_t + c_c2;

        if approx_eq(predicted_end, to_value) {
            return Self::from_curve_with_density(
                prev.end,
                to_t,
                sampling_density,
                SegmentCurve::Parabolic {
                    a: a_c2,
                    b: b_c2,
                    c: c_c2,
                },
            );
        }

        let local_a = (to_value - state.value - state.derivative * dt) / (dt * dt);
        let a = local_a;
        let b = state.derivative - 2.0 * local_a * prev.end;
        let c = state.value - local_a * prev.end * prev.end - b * prev.end;

        Self::from_curve_with_density(
            prev.end,
            to_t,
            sampling_density,
            SegmentCurve::Parabolic { a, b, c },
        )
    }

    pub fn continue_hyperbolic(
        prev: &Segment,
        to_t: f64,
        pole_t: f64,
        sampling_density: f64,
    ) -> Result<Self, SegmentError> {
        require_finite(pole_t, "pole_t")?;

        let start = prev.end;
        validated_span(start, to_t)?;
        let min_t = start.min(to_t);
        let max_t = start.max(to_t);
        if pole_t > min_t - HYPERBOLA_POLE_EPSILON && pole_t < max_t + HYPERBOLA_POLE_EPSILON {
            return Err(SegmentError::InvalidHyperbolicPole);
        }

        let state = prev.end_state();
        let delta = start - pole_t;
        if delta.abs() <= HYPERBOLA_POLE_EPSILON {
            return Err(SegmentError::InvalidHyperbolicPole);
        }

        let a = -state.derivative * delta * delta;
        let c = state.value - a / delta;

        Self::from_curve_with_density(
            start,
            to_t,
            sampling_density,
            SegmentCurve::Hyperbolic { a, b: pole_t, c },
        )
    }

    pub fn continue_arc_sweep(
        prev: &Segment,
        to_t: f64,
        radius: f64,
        sweep_angle: f64,
        concave_up: bool,
        sampling_density: f64,
    ) -> Result<Self, SegmentError> {
        require_finite(radius, "radius")?;
        require_finite(sweep_angle, "sweep_angle")?;

        if radius <= 0.0 {
            return Err(SegmentError::InvalidRadius);
        }
        if sweep_angle.abs() <= CONTINUITY_EPSILON {
            return Err(SegmentError::InvalidArcSweep);
        }

        let start = prev.end;
        let dt = validated_span(start, to_t)?;
        let state = prev.end_state();
        let cos_theta = state.derivative * dt / (radius * sweep_angle);
        if cos_theta.abs() > 1.0 + ARC_COS_EPSILON {
            return Err(SegmentError::ArcSlopeUnreachable);
        }

        let clamped_cos = cos_theta.clamp(-1.0, 1.0);
        let sin_magnitude = (1.0 - clamped_cos * clamped_cos).max(0.0).sqrt();
        let sin_theta = if concave_up {
            -sin_magnitude
        } else {
            sin_magnitude
        };
        let start_angle = sin_theta.atan2(clamped_cos);
        let center_y = state.value - radius * start_angle.sin();

        Self::from_curve_with_density(
            start,
            to_t,
            sampling_density,
            SegmentCurve::ArcSweep {
                center_y,
                radius,
                start_angle,
                sweep_angle,
            },
        )
    }

    fn from_curve_with_density(
        start: f64,
        end: f64,
        sampling_density: f64,
        curve: SegmentCurve,
    ) -> Result<Self, SegmentError> {
        let segment = Self {
            start,
            end,
            sampling_density,
            curve,
        };
        segment.validate()?;
        Ok(segment)
    }

    pub fn evaluate(&self, t: f64) -> f64 {
        match self.curve {
            SegmentCurve::Line { m, b } => m * t + b,
            SegmentCurve::Constant { value } => value,
            SegmentCurve::Parabolic { a, b, c } => a * t * t + b * t + c,
            SegmentCurve::Hyperbolic { a, b, c } => a / (t - b) + c,
            SegmentCurve::ArcSweep {
                center_y,
                radius,
                start_angle,
                sweep_angle,
            } => {
                let angle = start_angle + sweep_angle * self.normalized_position(t);
                center_y + radius * angle.sin()
            }
        }
    }

    pub fn derivative_at(&self, t: f64) -> f64 {
        match self.curve {
            SegmentCurve::Line { m, .. } => m,
            SegmentCurve::Constant { .. } => 0.0,
            SegmentCurve::Parabolic { a, b, .. } => 2.0 * a * t + b,
            SegmentCurve::Hyperbolic { a, b, .. } => -a / (t - b).powi(2),
            SegmentCurve::ArcSweep {
                radius,
                start_angle,
                sweep_angle,
                ..
            } => {
                let angle = start_angle + sweep_angle * self.normalized_position(t);
                radius * angle.cos() * sweep_angle / self.span()
            }
        }
    }

    pub fn second_derivative_at(&self, t: f64) -> f64 {
        match self.curve {
            SegmentCurve::Line { .. } => 0.0,
            SegmentCurve::Constant { .. } => 0.0,
            SegmentCurve::Parabolic { a, .. } => 2.0 * a,
            SegmentCurve::Hyperbolic { a, b, .. } => 2.0 * a / (t - b).powi(3),
            SegmentCurve::ArcSweep {
                radius,
                start_angle,
                sweep_angle,
                ..
            } => {
                let angle = start_angle + sweep_angle * self.normalized_position(t);
                -radius * angle.sin() * (sweep_angle / self.span()).powi(2)
            }
        }
    }

    pub fn start_value(&self) -> f64 {
        self.evaluate(self.start)
    }

    pub fn end_value(&self) -> f64 {
        self.evaluate(self.end)
    }

    pub fn start_slope(&self) -> f64 {
        self.derivative_at(self.start)
    }

    pub fn end_slope(&self) -> f64 {
        self.derivative_at(self.end)
    }

    pub fn start_curvature(&self) -> f64 {
        self.second_derivative_at(self.start)
    }

    pub fn end_curvature(&self) -> f64 {
        self.second_derivative_at(self.end)
    }

    fn validate(&self) -> Result<(), SegmentError> {
        require_finite(self.start, "start")?;
        require_finite(self.end, "end")?;
        if self.end <= self.start {
            return Err(SegmentError::InvalidRange);
        }
        if !self.sampling_density.is_finite() || self.sampling_density <= 0.0 {
            return Err(SegmentError::InvalidSamplingDensity);
        }

        match self.curve {
            SegmentCurve::Line { m, b } => {
                require_finite(m, "m")?;
                require_finite(b, "b")?;
            }
            SegmentCurve::Constant { value } => {
                require_finite(value, "value")?;
            }
            SegmentCurve::Parabolic { a, b, c } => {
                require_finite(a, "a")?;
                require_finite(b, "b")?;
                require_finite(c, "c")?;
            }
            SegmentCurve::Hyperbolic { a, b, c } => {
                require_finite(a, "a")?;
                require_finite(b, "b")?;
                require_finite(c, "c")?;
                let min_t = self.start.min(self.end);
                let max_t = self.start.max(self.end);
                if b > min_t - HYPERBOLA_POLE_EPSILON && b < max_t + HYPERBOLA_POLE_EPSILON {
                    return Err(SegmentError::InvalidHyperbolicPole);
                }
            }
            SegmentCurve::ArcSweep {
                center_y,
                radius,
                start_angle,
                sweep_angle,
            } => {
                require_finite(center_y, "center_y")?;
                require_finite(radius, "radius")?;
                require_finite(start_angle, "start_angle")?;
                require_finite(sweep_angle, "sweep_angle")?;
                if radius <= 0.0 {
                    return Err(SegmentError::InvalidRadius);
                }
                if sweep_angle.abs() <= CONTINUITY_EPSILON {
                    return Err(SegmentError::InvalidArcSweep);
                }
            }
        }

        Ok(())
    }

    fn span(&self) -> f64 {
        self.end - self.start
    }

    fn normalized_position(&self, t: f64) -> f64 {
        (t - self.start) / self.span()
    }

    fn end_state(&self) -> SegmentState {
        SegmentState {
            value: self.end_value(),
            derivative: self.end_slope(),
            second_derivative: self.end_curvature(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SegmentState {
    value: f64,
    derivative: f64,
    second_derivative: f64,
}

fn require_finite(value: f64, name: &'static str) -> Result<(), SegmentError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(SegmentError::NonFiniteParameter(name))
    }
}

fn validated_span(start: f64, end: f64) -> Result<f64, SegmentError> {
    require_finite(start, "start")?;
    require_finite(end, "end")?;
    if end <= start {
        return Err(SegmentError::InvalidRange);
    }
    Ok(end - start)
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= CONTINUITY_EPSILON
}

fn derived_sampling_density(end: f64, curve: &SegmentCurve) -> f64 {
    let span = end.abs().max(1e-9);
    let base = 8.0 + span * 4.0;
    let complexity = match *curve {
        SegmentCurve::Line { .. } | SegmentCurve::Constant { .. } => 0.0,
        SegmentCurve::Parabolic { a, b, c } => a.abs() * 6.0 + b.abs() * 2.0 + c.abs(),
        SegmentCurve::Hyperbolic { a, b, c } => a.abs() * 8.0 + b.abs() * 3.0 + c.abs() + 4.0,
        SegmentCurve::ArcSweep {
            radius,
            sweep_angle,
            ..
        } => radius.abs() * 2.0 + sweep_angle.abs() * 4.0 + 4.0,
    };

    (base + complexity).clamp(4.0, 128.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(left: f64, right: f64) {
        assert!(
            (left - right).abs() <= 1e-7,
            "left={left}, right={right}, diff={}",
            (left - right).abs()
        );
    }

    #[test]
    fn direct_line_evaluates_endpoints() {
        let segment = Segment::start_line(2.0, 0.0, 1.0).unwrap();

        assert_eq!(segment.start, 0.0);
        assert!(segment.sampling_density > 0.0);
        assert_close(segment.evaluate(0.0), 1.0);
        assert_close(segment.evaluate(2.0), 1.0);
    }

    #[test]
    fn parabolic_continuation_keeps_c2_when_terminal_value_matches() {
        let prev = Segment::start_parabolic(1.0, 1.0, 0.0, 0.0).unwrap();
        let next = Segment::continue_parabolic(&prev, 2.0, 4.0, 4.0).unwrap();

        assert_close(prev.end_value(), next.start_value());
        assert_close(prev.end_slope(), next.start_slope());
        assert_close(prev.end_curvature(), next.start_curvature());
        assert_close(next.end_value(), 4.0);
    }

    #[test]
    fn parabolic_continuation_silently_downgrades_to_c1_when_needed() {
        let prev = Segment::start_parabolic(1.0, 1.0, 0.0, 0.0).unwrap();
        let next = Segment::continue_parabolic(&prev, 2.0, 5.0, 4.0).unwrap();

        assert_close(prev.end_value(), next.start_value());
        assert_close(prev.end_slope(), next.start_slope());
        assert!((prev.end_curvature() - next.start_curvature()).abs() > 1e-7);
        assert_close(next.end_value(), 5.0);
    }

    #[test]
    fn constant_continuation_requires_flat_slope() {
        let prev = Segment {
            start: 0.0,
            end: 1.0,
            sampling_density: 8.0,
            curve: SegmentCurve::Line { m: 2.0, b: 0.0 },
        };
        let error = Segment::continue_constant(&prev, 2.0, 4.0).unwrap_err();

        assert_eq!(
            error,
            SegmentError::CannotPreserveC1("constant continuation")
        );
    }

    #[test]
    fn hyperbolic_continuation_matches_value_and_slope() {
        let prev = Segment {
            start: 0.0,
            end: 1.0,
            sampling_density: 8.0,
            curve: SegmentCurve::Line { m: 1.0, b: 0.0 },
        };
        let next = Segment::continue_hyperbolic(&prev, 2.0, -1.0, 4.0).unwrap();

        assert_close(prev.end_value(), next.start_value());
        assert_close(prev.end_slope(), next.start_slope());
        assert!((prev.end_curvature() - next.start_curvature()).abs() > 1e-7);
    }

    #[test]
    fn arc_continuation_matches_value_and_slope() {
        let prev = Segment::start_constant(1.0, 1.0).unwrap();
        let next =
            Segment::continue_arc_sweep(&prev, 2.0, 1.0, std::f64::consts::FRAC_PI_2, false, 4.0)
                .unwrap();

        assert_close(prev.end_value(), next.start_value());
        assert_close(prev.end_slope(), next.start_slope());
        assert!(next.end_value().is_finite());
    }

    #[test]
    fn arc_continuation_rejects_non_positive_radius() {
        let prev = Segment::start_constant(1.0, 1.0).unwrap();
        let error =
            Segment::continue_arc_sweep(&prev, 2.0, 0.0, std::f64::consts::FRAC_PI_2, false, 4.0)
                .unwrap_err();

        assert_eq!(error, SegmentError::InvalidRadius);
    }

    #[test]
    fn arc_continuation_rejects_unreachable_slope() {
        let prev = Segment {
            start: 0.0,
            end: 1.0,
            sampling_density: 8.0,
            curve: SegmentCurve::Line { m: 10.0, b: 0.0 },
        };
        let error =
            Segment::continue_arc_sweep(&prev, 2.0, 1.0, std::f64::consts::FRAC_PI_4, true, 4.0)
                .unwrap_err();

        assert_eq!(error, SegmentError::ArcSlopeUnreachable);
    }
}
