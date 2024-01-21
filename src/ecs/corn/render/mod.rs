pub mod scan_prepass;
pub mod render;

use bevy::prelude::*;
use self::{render::CornRenderPlugin, scan_prepass::CornPrepassPlugin};

/// A Plugin that adds all corn rendering functionality
pub struct CornRenderPipelinePlugin;
impl Plugin for CornRenderPipelinePlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins((CornPrepassPlugin, CornRenderPlugin));
    }
}
