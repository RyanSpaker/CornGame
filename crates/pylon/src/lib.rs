//! CLAUDE (claude-opus-5-5): port of three_corn's pylon demo (`demo/pylon`), the use case for corn files
//! and scripting: most of it is `.corn` and script (assets/), rust only where it has to be:
//! - [`range`]: interaction ranges, fire enter/exit/move for scripts
//! - [`door`]: the sliding door transition
//! - [`interpolate`]: fixed-step code's targets, eased to between steps
//! - [`material`]: one material type for every custom shader
//! - [`player`]: walking, looking, blockers
//! - [`portal`]: the two realms, crossing between them, cameras that see through portals and windows
//! - [`precompile`]: render pipelines built as meshes are spawned, not as they're first seen
//! - [`sound`]: sounds that start at a set time, to the sample

use bevy::prelude::*;

pub mod door;
pub mod interpolate;
pub mod material;
pub mod player;
pub mod portal;
pub mod precompile;
pub mod range;
pub mod sound;

pub fn plugin(app: &mut App) {
    app.add_plugins((range::plugin, door::plugin, interpolate::plugin, material::plugin, player::plugin, portal::plugin, precompile::plugin, sound::plugin));
}
