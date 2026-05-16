use crate::body::meshable::{
    EmbodiedBounds, EmbodiedPoint3, EmbodiedTriangle, EmbodiedVector3, Meshable,
};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// A memoizing wrapper around a boxed `Embodied` implementation that caches
/// `Embodied` method results for each resolution.
///
/// This allows expensive geometry computations to be reused when the same
/// resolution (and optional per-vertex parameters) are requested multiple times.
///
/// # Example
/// ```ignore
/// let body: Box<dyn Embodied<...>> = Box::new(RevolutionBody::new(...)?);
/// let memo = MemoBody::new(body);
/// // First call computes and caches
/// let points1 = memo.sample_points(256);
/// // Second call with same resolution reuses cache
/// let points2 = memo.sample_points(256);
/// // Different resolution creates new cache entry
/// let points3 = memo.sample_points(512);
/// ```
pub struct MemoMesh {
    inner: Box<
        dyn Meshable<
            Vertex = EmbodiedPoint3,
            Index = EmbodiedTriangle,
            Bounds = EmbodiedBounds,
            Vector = EmbodiedVector3,
        >,
    >,
    cache: RwLock<HashMap<usize, ResolutionCache>>,
    opt_resolution_cache: OnceLock<usize>,
}

#[derive(Default)]
struct ResolutionCache {
    sample_points: Option<Vec<EmbodiedPoint3>>,
    mesh_indices: Option<Vec<EmbodiedTriangle>>,
    bounding_box: Option<EmbodiedBounds>,
    material_volume_m3: Option<f64>,
    cavity_volume_m3: Option<Option<f64>>,
    surface_normals: HashMap<usize, Option<EmbodiedVector3>>,
    thickness_by_vertex_and_direction: HashMap<(usize, [u64; 3]), Option<f64>>,
}

fn direction_key(direction: EmbodiedVector3) -> [u64; 3] {
    [
        direction.x.to_bits(),
        direction.y.to_bits(),
        direction.z.to_bits(),
    ]
}

impl MemoMesh {
    /// Wrap a boxed `Embodied` implementation with memoization.
    pub fn new(
        inner: Box<
            dyn Meshable<
                Vertex = EmbodiedPoint3,
                Index = EmbodiedTriangle,
                Bounds = EmbodiedBounds,
                Vector = EmbodiedVector3,
            >,
        >,
    ) -> Self {
        MemoMesh {
            inner,
            cache: RwLock::new(HashMap::new()),
            opt_resolution_cache: OnceLock::new(),
        }
    }

    /// Clear all cached resolutions.
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Get the number of cached resolutions.
    pub fn cache_size(&self) -> usize {
        self.cache.read().map(|cache| cache.len()).unwrap_or(0)
    }

    /// Check if a resolution is currently cached.
    pub fn is_cached(&self, resolution: usize) -> bool {
        self.cache
            .read()
            .map(|cache| cache.contains_key(&resolution))
            .unwrap_or(false)
    }
}

impl Meshable for MemoMesh {
    type Vertex = EmbodiedPoint3;
    type Index = EmbodiedTriangle;
    type Bounds = EmbodiedBounds;
    type Vector = EmbodiedVector3;

    fn sample_points(&self, resolution: usize) -> Vec<Self::Vertex> {
        if let Ok(cache) = self.cache.read() {
            if let Some(points) = cache
                .get(&resolution)
                .and_then(|entry| entry.sample_points.as_ref())
            {
                return points.clone();
            }
        }

        let points = self.inner.sample_points(resolution);
        if let Ok(mut cache) = self.cache.write() {
            let entry = cache.entry(resolution).or_default();
            entry.sample_points = Some(points.clone());
        }
        points
    }

    fn mesh_indices(&self, resolution: usize) -> Vec<Self::Index> {
        if let Ok(cache) = self.cache.read() {
            if let Some(indices) = cache
                .get(&resolution)
                .and_then(|entry| entry.mesh_indices.as_ref())
            {
                return indices.clone();
            }
        }

        let indices = self.inner.mesh_indices(resolution);
        if let Ok(mut cache) = self.cache.write() {
            let entry = cache.entry(resolution).or_default();
            entry.mesh_indices = Some(indices.clone());
        }
        indices
    }

    fn bounding_box(&self, resolution: usize) -> Self::Bounds {
        if let Ok(cache) = self.cache.read() {
            if let Some(bounds) = cache
                .get(&resolution)
                .and_then(|entry| entry.bounding_box.as_ref())
            {
                return *bounds;
            }
        }

        let bounds = self.inner.bounding_box(resolution);
        if let Ok(mut cache) = self.cache.write() {
            let entry = cache.entry(resolution).or_default();
            entry.bounding_box = Some(bounds);
        }
        bounds
    }

    fn material_volume_m3(&self, resolution: usize) -> f64 {
        if let Ok(cache) = self.cache.read() {
            if let Some(volume) = cache
                .get(&resolution)
                .and_then(|entry| entry.material_volume_m3)
            {
                return volume;
            }
        }

        let volume = self.inner.material_volume_m3(resolution);
        if let Ok(mut cache) = self.cache.write() {
            let entry = cache.entry(resolution).or_default();
            entry.material_volume_m3 = Some(volume);
        }
        volume
    }

    fn cavity_volume_m3(&self, resolution: usize) -> Option<f64> {
        if let Ok(cache) = self.cache.read() {
            if let Some(volume) = cache
                .get(&resolution)
                .and_then(|entry| entry.cavity_volume_m3)
            {
                return volume;
            }
        }

        let volume = self.inner.cavity_volume_m3(resolution);
        if let Ok(mut cache) = self.cache.write() {
            let entry = cache.entry(resolution).or_default();
            entry.cavity_volume_m3 = Some(volume);
        }
        volume
    }

    fn surface_normal_at_vertex(
        &self,
        resolution: usize,
        vertex_index: usize,
    ) -> Option<Self::Vector> {
        if let Ok(cache) = self.cache.read() {
            if let Some(normal) = cache
                .get(&resolution)
                .and_then(|entry| entry.surface_normals.get(&vertex_index).copied())
            {
                return normal;
            }
        }

        let normal = self
            .inner
            .surface_normal_at_vertex(resolution, vertex_index);
        if let Ok(mut cache) = self.cache.write() {
            let entry = cache.entry(resolution).or_default();
            entry.surface_normals.insert(vertex_index, normal);
        }
        normal
    }

    fn material_thickness_at_vertex(
        &self,
        resolution: usize,
        vertex_index: usize,
        direction: Self::Vector,
    ) -> Option<f64> {
        let key = (vertex_index, direction_key(direction));
        if let Ok(cache) = self.cache.read() {
            if let Some(thickness) = cache
                .get(&resolution)
                .and_then(|entry| entry.thickness_by_vertex_and_direction.get(&key).copied())
            {
                return thickness;
            }
        }

        let thickness =
            self.inner
                .material_thickness_at_vertex(resolution, vertex_index, direction);
        if let Ok(mut cache) = self.cache.write() {
            let entry = cache.entry(resolution).or_default();
            entry
                .thickness_by_vertex_and_direction
                .insert(key, thickness);
        }
        thickness
    }

    fn opt_resolution(&self) -> usize {
        *self
            .opt_resolution_cache
            .get_or_init(|| self.inner.opt_resolution())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Default)]
    struct Counters {
        sample_points: AtomicUsize,
        mesh_indices: AtomicUsize,
        bounding_box: AtomicUsize,
        material_volume_m3: AtomicUsize,
        cavity_volume_m3: AtomicUsize,
        surface_normal_at_vertex: AtomicUsize,
        material_thickness_at_vertex: AtomicUsize,
        opt_resolution: AtomicUsize,
    }

    struct CountingEmbodied {
        counters: Arc<Counters>,
    }

    impl CountingEmbodied {
        fn new(counters: Arc<Counters>) -> Self {
            Self { counters }
        }
    }

    impl Meshable for CountingEmbodied {
        type Vertex = EmbodiedPoint3;
        type Index = EmbodiedTriangle;
        type Bounds = EmbodiedBounds;
        type Vector = EmbodiedVector3;

        fn sample_points(&self, resolution: usize) -> Vec<Self::Vertex> {
            self.counters.sample_points.fetch_add(1, Ordering::Relaxed);
            vec![EmbodiedPoint3::new(resolution as f64, 0.0, 0.0)]
        }

        fn mesh_indices(&self, resolution: usize) -> Vec<Self::Index> {
            self.counters.mesh_indices.fetch_add(1, Ordering::Relaxed);
            vec![[resolution as u32, 0, 0]]
        }

        fn bounding_box(&self, resolution: usize) -> Self::Bounds {
            self.counters.bounding_box.fetch_add(1, Ordering::Relaxed);
            (
                EmbodiedPoint3::new(resolution as f64, 0.0, 0.0),
                EmbodiedPoint3::new(resolution as f64 + 1.0, 1.0, 1.0),
            )
        }

        fn material_volume_m3(&self, resolution: usize) -> f64 {
            self.counters
                .material_volume_m3
                .fetch_add(1, Ordering::Relaxed);
            resolution as f64 * 0.5
        }

        fn cavity_volume_m3(&self, resolution: usize) -> Option<f64> {
            self.counters
                .cavity_volume_m3
                .fetch_add(1, Ordering::Relaxed);
            Some(resolution as f64 * 0.25)
        }

        fn surface_normal_at_vertex(
            &self,
            resolution: usize,
            vertex_index: usize,
        ) -> Option<Self::Vector> {
            self.counters
                .surface_normal_at_vertex
                .fetch_add(1, Ordering::Relaxed);
            Some(EmbodiedVector3::new(
                resolution as f64,
                vertex_index as f64,
                1.0,
            ))
        }

        fn material_thickness_at_vertex(
            &self,
            resolution: usize,
            vertex_index: usize,
            direction: Self::Vector,
        ) -> Option<f64> {
            self.counters
                .material_thickness_at_vertex
                .fetch_add(1, Ordering::Relaxed);
            Some(resolution as f64 + vertex_index as f64 + direction.x + direction.y + direction.z)
        }

        fn opt_resolution(&self) -> usize {
            self.counters.opt_resolution.fetch_add(1, Ordering::Relaxed);
            42
        }
    }

    fn make_memo_body() -> (MemoMesh, Arc<Counters>) {
        let counters = Arc::new(Counters::default());
        let body = CountingEmbodied::new(Arc::clone(&counters));
        (MemoMesh::new(Box::new(body)), counters)
    }

    #[test]
    fn caches_resolution_scoped_methods() {
        let (memo, counters) = make_memo_body();

        assert_eq!(memo.sample_points(8), memo.sample_points(8));
        assert_eq!(memo.mesh_indices(8), memo.mesh_indices(8));
        assert_eq!(memo.bounding_box(8), memo.bounding_box(8));
        assert_eq!(memo.material_volume_m3(8), memo.material_volume_m3(8));
        assert_eq!(memo.cavity_volume_m3(8), memo.cavity_volume_m3(8));

        assert_eq!(counters.sample_points.load(Ordering::Relaxed), 1);
        assert_eq!(counters.mesh_indices.load(Ordering::Relaxed), 1);
        assert_eq!(counters.bounding_box.load(Ordering::Relaxed), 1);
        assert_eq!(counters.material_volume_m3.load(Ordering::Relaxed), 1);
        assert_eq!(counters.cavity_volume_m3.load(Ordering::Relaxed), 1);

        let _ = memo.sample_points(16);
        let _ = memo.mesh_indices(16);
        let _ = memo.bounding_box(16);
        let _ = memo.material_volume_m3(16);
        let _ = memo.cavity_volume_m3(16);

        assert_eq!(counters.sample_points.load(Ordering::Relaxed), 2);
        assert_eq!(counters.mesh_indices.load(Ordering::Relaxed), 2);
        assert_eq!(counters.bounding_box.load(Ordering::Relaxed), 2);
        assert_eq!(counters.material_volume_m3.load(Ordering::Relaxed), 2);
        assert_eq!(counters.cavity_volume_m3.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn caches_surface_normal_by_resolution_and_vertex() {
        let (memo, counters) = make_memo_body();

        let a = memo.surface_normal_at_vertex(8, 3);
        let b = memo.surface_normal_at_vertex(8, 3);
        assert_eq!(a, b);
        assert_eq!(counters.surface_normal_at_vertex.load(Ordering::Relaxed), 1);

        let _ = memo.surface_normal_at_vertex(8, 4);
        assert_eq!(counters.surface_normal_at_vertex.load(Ordering::Relaxed), 2);

        let _ = memo.surface_normal_at_vertex(16, 3);
        assert_eq!(counters.surface_normal_at_vertex.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn caches_thickness_by_resolution_vertex_and_direction() {
        let (memo, counters) = make_memo_body();

        let dir_a = EmbodiedVector3::new(1.0, 0.0, 0.0);
        let dir_b = EmbodiedVector3::new(0.0, 1.0, 0.0);

        let t1 = memo.material_thickness_at_vertex(8, 2, dir_a);
        let t2 = memo.material_thickness_at_vertex(8, 2, dir_a);
        assert_eq!(t1, t2);
        assert_eq!(
            counters
                .material_thickness_at_vertex
                .load(Ordering::Relaxed),
            1
        );

        let _ = memo.material_thickness_at_vertex(8, 2, dir_b);
        assert_eq!(
            counters
                .material_thickness_at_vertex
                .load(Ordering::Relaxed),
            2
        );

        let _ = memo.material_thickness_at_vertex(8, 3, dir_a);
        assert_eq!(
            counters
                .material_thickness_at_vertex
                .load(Ordering::Relaxed),
            3
        );

        let _ = memo.material_thickness_at_vertex(16, 2, dir_a);
        assert_eq!(
            counters
                .material_thickness_at_vertex
                .load(Ordering::Relaxed),
            4
        );
    }

    #[test]
    fn caches_opt_resolution_once() {
        let (memo, counters) = make_memo_body();

        assert_eq!(memo.opt_resolution(), 42);
        assert_eq!(memo.opt_resolution(), 42);
        assert_eq!(memo.opt_resolution(), 42);

        assert_eq!(counters.opt_resolution.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn clear_cache_forces_recompute_for_resolution_entries() {
        let (memo, counters) = make_memo_body();

        let _ = memo.sample_points(8);
        let _ = memo.bounding_box(8);
        let _ = memo.surface_normal_at_vertex(8, 0);
        let _ = memo.material_thickness_at_vertex(8, 0, EmbodiedVector3::new(1.0, 0.0, 0.0));

        assert_eq!(memo.cache_size(), 1);
        assert!(memo.is_cached(8));

        memo.clear_cache();

        assert_eq!(memo.cache_size(), 0);
        assert!(!memo.is_cached(8));

        let _ = memo.sample_points(8);
        let _ = memo.bounding_box(8);
        let _ = memo.surface_normal_at_vertex(8, 0);
        let _ = memo.material_thickness_at_vertex(8, 0, EmbodiedVector3::new(1.0, 0.0, 0.0));

        assert_eq!(counters.sample_points.load(Ordering::Relaxed), 2);
        assert_eq!(counters.bounding_box.load(Ordering::Relaxed), 2);
        assert_eq!(counters.surface_normal_at_vertex.load(Ordering::Relaxed), 2);
        assert_eq!(
            counters
                .material_thickness_at_vertex
                .load(Ordering::Relaxed),
            2
        );
    }
}
