//! Tipos de domínio puros do content-machine (sem I/O).

mod geometry;
mod recording;

pub use geometry::{Rect, WebcamDetection, WebcamSource};
pub use recording::{AudioTrack, Recording, TrackRole, assign_roles};
