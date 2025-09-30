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
    // Early-out: both empty
    if from.is_empty() && to.is_empty() {
        return Vec::new();
    }

    let t_f32 = time.to_f32().unwrap();
    let target_len = (from.len() as f32 * (1.0 - t_f32) + to.len() as f32 * t_f32).round() as usize;

    if target_len == 0 {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(target_len);

    for i in 0..target_len {
        // Derive an element index helper for gradual length morph.
        let idx_for = |len: usize| -> usize {
            if len == 0 {
                0
            } else {
                ((i * len) / target_len).min(len - 1)
            }
        };

        // Safely obtain or synthesize from_elem
        let from_elem = if !from.is_empty() {
            if i < from.len() {
                from[i].clone()
            } else {
                // Interpolate inside 'from' when we conceptually stretch it
                let rel_idx = i as f32 / target_len as f32 * from.len() as f32;
                let idx1 = rel_idx.floor() as usize;
                let idx2 = (rel_idx.ceil() as usize).min(from.len().saturating_sub(1));
                let local_t = rel_idx - idx1 as f32;
                if idx1 == idx2 {
                    from[idx1].clone()
                } else {
                    CanTween::ease(from[idx1].clone(), from[idx2].clone(), local_t)
                }
            }
        } else {
            // Mirror an element from 'to' so easing is stable
            let ti = idx_for(to.len());
            to[ti].clone()
        };

        // Safely obtain or synthesize to_elem
        let to_elem = if !to.is_empty() {
            if i < to.len() {
                to[i].clone()
            } else {
                let rel_idx = i as f32 / target_len as f32 * to.len() as f32;
                let idx1 = rel_idx.floor() as usize;
                let idx2 = (rel_idx.ceil() as usize).min(to.len().saturating_sub(1));
                let local_t = rel_idx - idx1 as f32;
                if idx1 == idx2 {
                    to[idx1].clone()
                } else {
                    CanTween::ease(to[idx1].clone(), to[idx2].clone(), local_t)
                }
            }
        } else {
            // Mirror element from 'from'
            let fi = idx_for(from.len());
            from[fi].clone()
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
    if from.is_empty() && to.is_empty() {
        return Vec::new();
    }

    let t_f32 = time.to_f32().unwrap();
    let target_len = (from.len() as f32 * (1.0 - t_f32) + to.len() as f32 * t_f32).round() as usize;

    if target_len == 0 {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(target_len);

    for i in 0..target_len {
        let idx_for = |len: usize| -> usize {
            if len == 0 {
                0
            } else {
                ((i * len) / target_len).min(len - 1)
            }
        };

        let from_elem = if !from.is_empty() {
            if i < from.len() {
                from[i].clone()
            } else {
                let rel_idx = i as f32 / target_len as f32 * from.len() as f32;
                let idx1 = rel_idx.floor() as usize;
                let idx2 = (rel_idx.ceil() as usize).min(from.len().saturating_sub(1));
                let local_t = rel_idx - idx1 as f32;
                if idx1 == idx2 {
                    from[idx1].clone()
                } else {
                    (
                        CanTween::ease(from[idx1].0.clone(), from[idx2].0.clone(), local_t),
                        CanTween::ease(from[idx1].1.clone(), from[idx2].1.clone(), local_t),
                    )
                }
            }
        } else {
            // Mirror to element
            let ti = idx_for(to.len());
            to[ti].clone()
        };

        let to_elem = if !to.is_empty() {
            if i < to.len() {
                to[i].clone()
            } else {
                let rel_idx = i as f32 / target_len as f32 * to.len() as f32;
                let idx1 = rel_idx.floor() as usize;
                let idx2 = (rel_idx.ceil() as usize).min(to.len().saturating_sub(1));
                let local_t = rel_idx - idx1 as f32;
                if idx1 == idx2 {
                    to[idx1].clone()
                } else {
                    (
                        CanTween::ease(to[idx1].0.clone(), to[idx2].0.clone(), local_t),
                        CanTween::ease(to[idx1].1.clone(), to[idx2].1.clone(), local_t),
                    )
                }
            }
        } else {
            // Mirror from element
            let fi = idx_for(from.len());
            from[fi].clone()
        };

        result.push((
            CanTween::ease(from_elem.0, to_elem.0, time),
            CanTween::ease(from_elem.1, to_elem.1, time),
        ));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tween_vectors_both_empty() {
        let v: Vec<f32> = tween_vectors::<f32>(&[], &[], 0.5f32);
        assert!(
            v.is_empty(),
            "Expected empty result when both inputs are empty"
        );
    }

    #[test]
    fn tween_vectors_empty_to_nonempty_no_panic() {
        let to = vec![1.0f32, 2.0, 3.0];
        let v = tween_vectors::<f32>(&[], &to, 0.5f32);
        assert!(
            !v.is_empty(),
            "Result should gain elements when target vector is non-empty"
        );
        for &x in &v {
            assert!(
                (1.0..=3.0).contains(&x),
                "Interpolated value {x} should lie within bounds of target slice"
            );
        }
    }

    #[test]
    fn tween_vectors_nonempty_to_empty_no_panic() {
        let from = vec![10.0f32, 20.0, 30.0];
        let v = tween_vectors::<f32>(&from, &[], 0.5f32);
        assert!(
            !v.is_empty(),
            "Result should retain some elements while morphing toward empty"
        );
        for &x in &v {
            assert!(
                (10.0..=30.0).contains(&x),
                "Interpolated value {x} should lie within bounds of source slice"
            );
        }
    }

    #[test]
    fn tween_tuple_vectors_empty_mismatch() {
        let to = vec![(1.0f32, 10.0f32), (2.0, 20.0), (3.0, 30.0)];
        let v = tween_tuple_vectors::<f32, f32>(&[], &to, 0.5f32);
        assert!(
            !v.is_empty(),
            "Result should gain elements when tuple target vector is non-empty"
        );
        assert!(
            v.len() <= to.len(),
            "Length should not exceed target length; got {} > {}",
            v.len(),
            to.len()
        );
    }
}
