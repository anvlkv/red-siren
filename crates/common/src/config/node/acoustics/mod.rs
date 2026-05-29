//! Path-separated acoustics computations for node analysis.
//!
//! Scope:
//! - `strike`: strike-path-only structural/acoustic formulas
//! - `jet`: jet-path-only structural/acoustic formulas
//! - `slide`: slide-path-only structural/acoustic formulas
//! - `shared`: path-agnostic helpers and reusable physical components
//!
//! Contract:
//! - Path modules may depend on `shared` and node context types.
//! - Path modules must not import each other.

pub mod jet;
pub mod shared;
pub mod slide;
pub mod strike;
