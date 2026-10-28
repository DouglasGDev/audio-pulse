//! Local pinna personalization.
//!
//! A monocular depth model estimates the shape of the outer ear from a photo
//! the user frames themselves. Geometry measured on that depth map is mapped
//! to a small corrective EQ curve. Everything runs on the machine: the photo
//! is never uploaded, only the model file is downloaded once.

pub mod commands;
pub mod geometry;
pub mod inference;
pub mod model;
pub mod overlay;
pub mod pipeline;
