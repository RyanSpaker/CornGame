
use bevy::prelude::*;
use bevy_xpbd_3d::prelude::*;
use serde::{Deserialize, Serialize};

pub struct MyPhysicsPlugin;
impl Plugin for MyPhysicsPlugin {
    fn build(&self, app: &mut App) {
        // init physics plugins
        app.add_plugins((
            PhysicsPlugins::default(), 
            PhysicsDebugPlugin::default()
        ));

        // This maybe should be moved
        app.register_type::<DebugRender>();
        app.init_resource::<DebugRender>();
        app.add_systems(Update, toggle_gizmos);
    }
}

#[derive(Debug, Default, Resource, Reflect, Serialize, Deserialize)]
#[reflect(Resource)]
struct DebugRender(bool);

// display colliders, TODO refactor as command so that it can work with keybinds and console
fn toggle_gizmos(d: Res<DebugRender>, mut store: ResMut<GizmoConfigStore>) {
    if d.is_changed(){
        let (config, _) = store.config_mut::<PhysicsGizmos>();
        config.enabled = d.0;
    }
}