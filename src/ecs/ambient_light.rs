/// Allow ambient light to be controlled by component

use bevy::prelude::*;

#[derive(Debug, Default, Clone, Reflect, Component)]
#[reflect(Component)]
pub struct AmbientLight(pub bevy::prelude::AmbientLight);

// impl Default for AmbientLight {
//     fn default() -> Self {
//         Self(
//             bevy::prelude::AmbientLight::NONE
//         )
//     }
// }

pub fn plugin(app: &mut App){
    app.register_type::<AmbientLight>();
    // app.insert_resource(bevy::prelude::AmbientLight::NONE);
    app.add_systems(PostUpdate, sync);
}

/// when AmbientLight component changes, set resource.
/// NOTE: This might get weird if multiple scenes are loaded with different ambient light. Don't do that.
/// TODO: Could do something clever like linearly interpolate by global position and or bounding box 
fn sync(
    mut resource: ResMut<bevy::prelude::AmbientLight>,
    query: Query<&AmbientLight, Changed<AmbientLight>>  
){
    for c in query {
        *resource = c.0.clone();
    }
}