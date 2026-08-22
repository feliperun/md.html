//! Closed runtime selection (SPEC §17): strict committed manifest and font
//! catalog loading with size/hash/concatenation verification, plus
//! deterministic fragment and face selection from accepted analysis and
//! scanner evidence.

pub mod fonts;
pub mod manifest;

mod json;
mod sha256;

pub use fonts::{Catalog, Face, Family, Preset, SelectedFace, WghtRange, select_faces};
pub use manifest::{
    Fragment, FragmentId, Manifest, SelectionError, SelectionProblem, load, select_fragments,
};
