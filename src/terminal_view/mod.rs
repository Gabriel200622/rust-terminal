//! Geometry, preparation and painting consume project-owned terminal snapshots.

mod cache;
pub mod geometry;
mod links;
mod paint;

pub use cache::Cache;
pub use paint::{PaintResult, SelectionInteraction};
