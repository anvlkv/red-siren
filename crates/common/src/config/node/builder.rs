use serde::{Deserialize, Serialize};

use fastrand::Rng;

use crate::body::materials::Material;
use crate::{
    Body, ImperfectMesh, MemoMesh, Meshable, RevolutionAxis, RevolutionMesh, Segment, ThickMesh,
    ThicknessMap, ThicknessMapPoint,
};

use super::shape_profile::ShapeProfileBuilder;
use super::{Node, MODAL_EPSILON};

const BOWL_BASE_SHELL_DEVIATION_SCALE_M: f64 = 0.0015;
const BOWL_THICK_SHELL_DEVIATION_SCALE_M: f64 = 0.0025;
const CLAPPER_DEVIATION_SCALE_M: f64 = 0.0025;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ClapperShapeBuilder {
    pub length_m: f64,
    pub head_radius_m: f64,
    pub neck_radius_m: f64,
    pub tip_radius_m: f64,
    pub sampling_density: f64,
}

impl Default for ClapperShapeBuilder {
    fn default() -> Self {
        Self {
            length_m: 0.38,
            head_radius_m: 0.045,
            neck_radius_m: 0.025,
            tip_radius_m: 0.015,
            sampling_density: 20.0,
        }
    }
}

impl ClapperShapeBuilder {
    pub fn build_profile(&self) -> Result<[Segment; 3], String> {
        let length = self.length_m.max(0.1);
        let t1 = length / 3.0;
        let t2 = 2.0 * length / 3.0;
        let slope = (self.neck_radius_m - self.head_radius_m) / t1.max(MODAL_EPSILON);
        let seg0 = Segment::start_line(t1, slope, self.head_radius_m.max(MODAL_EPSILON))
            .map_err(|err| err.to_string())?;
        let seg1 = Segment::continue_parabolic(
            &seg0,
            t2,
            self.neck_radius_m.max(MODAL_EPSILON),
            self.sampling_density.max(2.0),
        )
        .map_err(|err| err.to_string())?;
        let seg2 = Segment::continue_parabolic(
            &seg1,
            length,
            self.tip_radius_m.max(MODAL_EPSILON),
            self.sampling_density.max(2.0),
        )
        .map_err(|err| err.to_string())?;
        Ok([seg0, seg1, seg2])
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ShellThicknessBuilder {
    pub inner_base_thickness_m: f64,
    pub inner_lip_thickness_m: f64,
    pub outer_base_thickness_m: f64,
    pub outer_lip_thickness_m: f64,
    pub sample_budget: usize,
}

impl Default for ShellThicknessBuilder {
    fn default() -> Self {
        Self {
            inner_base_thickness_m: 0.006,
            inner_lip_thickness_m: 0.0048,
            outer_base_thickness_m: 0.0048,
            outer_lip_thickness_m: 0.0038,
            sample_budget: 32,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeModelBuilders {
    pub shape: ShapeProfileBuilder,
    pub clapper: ClapperShapeBuilder,
    pub thickness: ShellThicknessBuilder,
}

impl Default for NodeModelBuilders {
    fn default() -> Self {
        Self {
            shape: ShapeProfileBuilder::default(),
            clapper: ClapperShapeBuilder::default(),
            thickness: ShellThicknessBuilder::default(),
        }
    }
}

impl NodeModelBuilders {
    pub fn active_profile(&self) -> Result<[Segment; 5], String> {
        self.shape.build_profile()
    }

    pub fn build_node(
        &self,
        bowl_material: Material,
        clapper_material: Material,
        clapper_to_bowl_friction: f64,
        seed: Option<u64>,
    ) -> Result<Node, String> {
        log::trace!(
            "node build start seed={seed:?} friction={clapper_to_bowl_friction:.4} thickness_samples={}",
            self.thickness.sample_budget
        );

        let mut seed_rng = seed.map(Rng::with_seed);
        let mut next_rng = || {
            seed_rng
                .as_mut()
                .map(|rng| Rng::with_seed(rng.u64(..)))
                .unwrap_or_else(Rng::new)
        };

        let bowl_profile = self.active_profile()?;
        log::trace!("node build progress: bowl profile generated");
        let bowl_base_mesh =
            RevolutionMesh::new(bowl_profile, RevolutionAxis::Y).map_err(|err| err.to_string())?;
        log::trace!("node build progress: bowl revolution mesh generated");

        // Build inner imperfect mesh first and derive thickness points from its actual
        // sampled vertices (instead of relying on axial profile-position indexing).
        let theta_samples = self.thickness.sample_budget.max(8);
        let bowl_base = ImperfectMesh::new(
            bowl_base_mesh,
            BOWL_BASE_SHELL_DEVIATION_SCALE_M,
            next_rng(),
        );
        let bowl_points = bowl_base.sample_points(theta_samples);
        if bowl_points.is_empty() {
            return Err("failed to sample bowl points for thickness map".to_string());
        }

        let (min_y, max_y) = bowl_points
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min_v, max_v), p| {
                (min_v.min(p.y), max_v.max(p.y))
            });
        let y_span = (max_y - min_y).abs().max(MODAL_EPSILON);

        let mut thickness_points = Vec::with_capacity(bowl_points.len());
        for (i, p) in bowl_points.iter().enumerate() {
            let u = ((p.y - min_y) / y_span).clamp(0.0, 1.0);
            let face = self.thickness.inner_base_thickness_m
                + (self.thickness.inner_lip_thickness_m - self.thickness.inner_base_thickness_m)
                    * u;
            let back = self.thickness.outer_base_thickness_m
                + (self.thickness.outer_lip_thickness_m - self.thickness.outer_base_thickness_m)
                    * u;
            thickness_points.push(ThicknessMapPoint {
                body_vertex_index: i,
                face_thickness: face.max(MODAL_EPSILON),
                backface_thickness: back.max(MODAL_EPSILON),
            });
        }

        let thickness_map = ThicknessMap::new(thickness_points)
            .ok_or_else(|| "failed to build bowl thickness map".to_string())?;
        log::trace!(
            "node build progress: thickness map generated points={} theta_samples={} y_range=[{:.6e},{:.6e}]",
            bowl_points.len(),
            theta_samples,
            min_y,
            max_y
        );

        let bowl_shell = ThickMesh::new(bowl_base, thickness_map);
        let bowl = Body::new(
            MemoMesh::new(ImperfectMesh::new(
                bowl_shell,
                BOWL_THICK_SHELL_DEVIATION_SCALE_M,
                next_rng(),
            )),
            bowl_material,
        );
        log::trace!("node build progress: bowl meshables wrapped (imperfect+thick+memo)");

        let clapper_profile = self.clapper.build_profile()?;
        log::trace!("node build progress: clapper profile generated");
        let clapper_mesh = RevolutionMesh::new(clapper_profile, RevolutionAxis::Y)
            .map_err(|err| err.to_string())?;
        let clapper = Body::new(
            MemoMesh::new(ImperfectMesh::new(
                clapper_mesh,
                CLAPPER_DEVIATION_SCALE_M,
                next_rng(),
            )),
            clapper_material,
        );
        log::trace!("node build progress: clapper meshables wrapped (imperfect+memo)");
        log::trace!("node build complete");

        Ok(Node::new(bowl, clapper, clapper_to_bowl_friction))
    }
}
