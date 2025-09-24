use keyframe::CanTween;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReducedMotionState {
    pub total_keyframes: usize,
    pub current_keyframe: usize,
    pub accumulated_time: f64,
}

/// Generic vector tweening utility that handles interpolation between vectors of different lengths
pub fn tween_vectors<T>(from: &[T], to: &[T], time: impl keyframe::num_traits::Float) -> Vec<T>
where
    T: CanTween + Clone,
{
    let target_len = (from.len() as f32 * (1.0 - time.to_f32().unwrap())
        + to.len() as f32 * time.to_f32().unwrap())
    .round() as usize;

    let mut result = Vec::with_capacity(target_len);

    for i in 0..target_len {
        let from_elem = if i < from.len() {
            from[i].clone()
        } else {
            // Interpolate based on relative index when from vector is shorter
            let rel_idx = i as f32 / target_len as f32 * from.len() as f32;
            let idx1 = rel_idx.floor() as usize;
            let idx2 = (rel_idx.ceil() as usize).min(from.len().saturating_sub(1));
            let t = rel_idx - idx1 as f32;

            if idx1 == idx2 {
                from[idx1].clone()
            } else {
                CanTween::ease(from[idx1].clone(), from[idx2].clone(), t)
            }
        };

        let to_elem = if i < to.len() {
            to[i].clone()
        } else {
            // Interpolate based on relative index when to vector is shorter
            let rel_idx = i as f32 / target_len as f32 * to.len() as f32;
            let idx1 = rel_idx.floor() as usize;
            let idx2 = (rel_idx.ceil() as usize).min(to.len().saturating_sub(1));
            let t = rel_idx - idx1 as f32;

            if idx1 == idx2 {
                to[idx1].clone()
            } else {
                CanTween::ease(to[idx1].clone(), to[idx2].clone(), t)
            }
        };

        result.push(CanTween::ease(from_elem, to_elem, time));
    }

    result
}

/// Generic tweening for vectors of 2-tuples where both elements implement CanTween
pub fn tween_tuple_vectors<A, B>(
    from: &[(A, B)],
    to: &[(A, B)],
    time: impl keyframe::num_traits::Float,
) -> Vec<(A, B)>
where
    A: CanTween + Clone,
    B: CanTween + Clone,
{
    let target_len = (from.len() as f32 * (1.0 - time.to_f32().unwrap())
        + to.len() as f32 * time.to_f32().unwrap())
    .round() as usize;

    let mut result = Vec::with_capacity(target_len);

    for i in 0..target_len {
        let from_elem = if i < from.len() {
            from[i].clone()
        } else {
            // Interpolate based on relative index when from vector is shorter
            let rel_idx = i as f32 / target_len as f32 * from.len() as f32;
            let idx1 = rel_idx.floor() as usize;
            let idx2 = (rel_idx.ceil() as usize).min(from.len().saturating_sub(1));
            let t = rel_idx - idx1 as f32;

            if idx1 == idx2 {
                from[idx1].clone()
            } else {
                (
                    CanTween::ease(from[idx1].0.clone(), from[idx2].0.clone(), t),
                    CanTween::ease(from[idx1].1.clone(), from[idx2].1.clone(), t),
                )
            }
        };

        let to_elem = if i < to.len() {
            to[i].clone()
        } else {
            // Interpolate based on relative index when to vector is shorter
            let rel_idx = i as f32 / target_len as f32 * to.len() as f32;
            let idx1 = rel_idx.floor() as usize;
            let idx2 = (rel_idx.ceil() as usize).min(to.len().saturating_sub(1));
            let t = rel_idx - idx1 as f32;

            if idx1 == idx2 {
                to[idx1].clone()
            } else {
                (
                    CanTween::ease(to[idx1].0.clone(), to[idx2].0.clone(), t),
                    CanTween::ease(to[idx1].1.clone(), to[idx2].1.clone(), t),
                )
            }
        };

        result.push((
            CanTween::ease(from_elem.0, to_elem.0, time),
            CanTween::ease(from_elem.1, to_elem.1, time),
        ));
    }

    result
}
