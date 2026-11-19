//! Tipos de domínio puros do content-machine (sem I/O).

mod recording;

pub use recording::{AudioTrack, Recording, TrackRole, assign_roles};
