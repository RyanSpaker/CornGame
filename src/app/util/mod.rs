pub mod camera;
pub mod default_resources;
pub mod button;

use bevy::app::Plugin;

#[derive(Default, Debug, Clone)]
pub struct AppUtilPlugin;
impl Plugin for AppUtilPlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins((
            default_resources::DefaultResourcesPlugin,
            button::ButtonPlugin
        ));
    }
}