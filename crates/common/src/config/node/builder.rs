use serde::{Deserialize, Serialize};

use crate::body::materials::Material;
use crate::{
    thickness_map_from_axial_samples, Body, MemoMesh, RevolutionAxis, RevolutionMesh, Segment,
    ThickMesh,
};

use super::shape_profile::ShapeProfileBuilder;
use super::{Node, MODAL_EPSILON};

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
    ) -> Result<Node, String> {
        let bowl_profile = self.active_profile()?;
        let bowl_base =
            RevolutionMesh::new(bowl_profile, RevolutionAxis::Y).map_err(|err| err.to_string())?;
        let axial_samples = bowl_base.profile_sample_positions(self.thickness.sample_budget.max(8));
        let thickness_map = thickness_map_from_axial_samples(
            &axial_samples,
            self.thickness.sample_budget.max(8),
            |u| {
                let u = u.clamp(0.0, 1.0);
                let face = self.thickness.inner_base_thickness_m
                    + (self.thickness.inner_lip_thickness_m
                        - self.thickness.inner_base_thickness_m)
                        * u;
                let back = self.thickness.outer_base_thickness_m
                    + (self.thickness.outer_lip_thickness_m
                        - self.thickness.outer_base_thickness_m)
                        * u;
                (face.max(MODAL_EPSILON), back.max(MODAL_EPSILON))
            },
        )
        .ok_or_else(|| "failed to build bowl thickness map".to_string())?;

        let bowl = Body::new(
            MemoMesh::new(ThickMesh::new(bowl_base, thickness_map)),
            bowl_material,
        );

        let clapper_profile = self.clapper.build_profile()?;
        let clapper_mesh = RevolutionMesh::new(clapper_profile, RevolutionAxis::Y)
            .map_err(|err| err.to_string())?;
        let clapper = Body::new(MemoMesh::new(clapper_mesh), clapper_material);

        Ok(Node::new(bowl, clapper, clapper_to_bowl_friction))
    }
}
