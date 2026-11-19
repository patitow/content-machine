//! Visão computacional do content-machine: rosto e webcam.

mod face;
mod webcam;

pub use face::{Face, FaceDetector, VisionError};
pub use webcam::{WebcamKind, WebcamOutcome, detect_webcam};
