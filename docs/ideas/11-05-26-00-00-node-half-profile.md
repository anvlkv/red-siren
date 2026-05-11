# Node Half-Profile Preview (Point2)

## Problem Framing
You want a 2D half-section preview for a bell/gong/bowl-like node using the existing NodePhysics fields.
Coordinate convention (confirmed): x = radius, y = height, y increases downward.
Scope: only property review + mapping of properties to one half-profile in mint::Point2<f64>.

## Questions Asked
1. Axis convention: x=radius, y=height, y-down.
2. Curvature interpretation: propose both curved and linear options.
3. Thickness: include both outer and inner contour.
4. Density role: non-geometric.
5. Include Add semantics review: yes.

## Codebase Inspiration
- NodePhysics currently carries geometric + physical properties, but only config-level usage is visible.
- Existing fields naturally split into:
  - Geometric preview drivers: size_m.y, r1, r2, wall_thickness_m, concavity flags.
  - Physical metadata: mass_kg, material_density_kg_per_m3.
- Add implementation sums all scalar fields and combines concavity flags by sign parity, which is mathematically neat but semantically unclear for shape composition.

## Web Inspiration
- Bell profile quality is strongly shape-driven; meridian section is a standard representation in bell design references.
- Circular-segment geometry gives a direct way to build controlled convex/concave arcs from endpoints + sagitta.
- Superellipse family is useful as a smooth, controllable fallback when you want one-parameter shape stylization.

## Idea Options (Half-Profile as Point2 Sequence)

### Option 1: Endpoint + Piecewise Linear (minimum semantics)
Concept:
Use only endpoint radii and total height; define a polyline from base to rim.

Mapping:
- H = size_m.y
- Outer base point: P0 = Point2 { x: r1, y: 0.0 }
- Outer rim point: P1 = Point2 { x: r2, y: H }
- Optional mid control for visual taper only: Pm = Point2 { x: (r1 + r2)/2, y: H/2 }
- Inner contour by radial offset:
  - inner_r1 = max(0, r1 - wall_thickness_m)
  - inner_r2 = max(0, r2 - wall_thickness_m)
  - Qi0 = Point2 { x: inner_r1, y: 0.0 }
  - Qi1 = Point2 { x: inner_r2, y: H }

Fit:
Best if you want deterministic preview without adding new properties.

Feasibility:
Very high.

Risks:
Cannot express bowl-vs-bell character beyond endpoint taper.

Effort:
Very low.

### Option 2: Circular-Arc Sidewall with Signed Sagitta (recommended)
Concept:
Treat outer and inner wall as arc segments from base to rim; concavity bool chooses sagitta sign.

Mapping:
- Endpoints:
  - A = (r1, 0)
  - B = (r2, H)
- Chord midpoint M = ((r1+r2)/2, H/2)
- Unit normal n to chord AB (choose orientation pointing toward larger radius side for consistency).
- Sagitta magnitude s from existing fields only:
  - s_outer = min(H, abs(r2-r1) + min(r1, r2)) * k
  - where k is fixed style constant (for preview only), e.g. 0.15
- Signed sagitta:
  - sign_outer = if is_r1_concave xor is_r2_concave then -1 else +1
  - C_outer = M + sign_outer * s_outer * n
- Sample quadratic/circular-like curve points (t in [0,1]):
  - P(t) = (1-t)^2 A + 2(1-t)t C_outer + t^2 B
- Inner contour repeats with radii reduced by wall_thickness_m.

Fit:
Matches your existing bool idea of inward/outward curvature while keeping current schema.

Feasibility:
High.

Risks:
Concavity booleans at both ends are ambiguous (what if one true/one false).

Effort:
Low to medium.

### Option 3: Two-Zone Profile (base zone + rim zone)
Concept:
Interpret r1 and is_r1_concave as lower-half behavior, r2 and is_r2_concave as upper-half behavior.

Mapping:
- Split height at y = H/2.
- Lower segment points:
  - A0 = (r1, 0)
  - A1 = (rm, H/2)
  - control sign from is_r1_concave
- Upper segment points:
  - B0 = (rm, H/2)
  - B1 = (r2, H)
  - control sign from is_r2_concave
- rm can be fixed blend rm = 0.5*(r1+r2) or derived from thickness-safe bounds.
- Inner contour mirrors with radial offset by wall_thickness_m.

Fit:
Uses both concavity flags with clear local meaning.

Feasibility:
Medium.

Risks:
Without one extra parameter (e.g., split ratio or separate curvatures), artistic control is limited.

Effort:
Medium.

## Property Completeness Review
Likely missing for expressive, unambiguous preview:
- A dedicated curvature magnitude parameter (single value or per-zone).
- A lip/thickness profile parameter (constant wall thickness is often too simple visually).
- Optional foot radius or base fillet parameter for gong/bowl distinction.

Not strictly required for a first preview:
- mass_kg and material_density_kg_per_m3 (non-geometric).

## Confusion Review
Potentially confusing today:
- size_m.x/size_m.z described as base width/depth while r1/r2 are radii; relation between these is unspecified.
- r1/r2 naming: sounds like radii only, but concavity booleans suggest they also imply local curvature behavior.
- is_r1_concave/is_r2_concave semantics are unclear for a continuous wall.
- Add semantics for booleans (parity logic) is surprising for geometry composition.

## Recommendation
Choose Option 2 now (arc-like with signed sagitta), because it gives a clear bowl/bell feel using current fields and supports both outer and inner contours with no schema changes.
If you want stronger artistic control later, add one curvature magnitude parameter; keep mass/density out of rendering.

## References
- https://en.wikipedia.org/wiki/Circular_segment
- https://en.wikipedia.org/wiki/Superellipse
- https://en.wikipedia.org/wiki/Bell
