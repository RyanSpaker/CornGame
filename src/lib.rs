pub mod assets;
pub mod core;
pub mod flycam;
pub mod ecs;

pub mod prelude{
    pub use crate::core::*;
    pub use crate::assets::*;
}