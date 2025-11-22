use bevy::{prelude::*, render::{extract_component::{ExtractComponent, ExtractComponentPlugin}, view::RenderLayers}};
use bevy::scene::SceneInstanceReady;
use serde::{Deserialize, Serialize};

pub const THIRD_PERSON_RENDER_LAYER : usize = 10;


/// MOVEME
/// FROM: https://github.com/bevyengine/bevy/issues/12461
/// Currently [`RenderLayers`] are not applied to children of a scene.
/// This [`SceneInstanceReady`] observer applies the [`RenderLayers`]
/// of a [`SceneRoot`] to all children with a [`Transform`] and without a [`RenderLayers`].
/// 
/// See [#12461](https://github.com/bevyengine/bevy/issues/12461) for current status.
fn apply_render_layers_to_children(
  trigger: Trigger<SceneInstanceReady>,
  mut commands: Commands,
  children: Query<&Children>,
  transforms: Query<&Transform, Without<RenderLayers>>,
  query: Query<(Entity, &RenderLayers)>,
) {
  let Ok((parent, render_layers)) = query.get(trigger.target()) else {
    return;
  };
  children.iter_descendants(parent).for_each(|entity| {
    if transforms.contains(entity) {
      commands.entity(entity).insert(render_layers.clone());
    }
  });
}


#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component, ExtractComponent, Serialize, Deserialize)]
#[reflect(Component)]
pub struct MainCamera;
impl MainCamera{
    // TODO consolidate with caller
    pub fn spawn_main_camera(commands: &mut Commands, simple: bool) -> Entity{
        commands.spawn((
            Self, 
            Camera3d::default(), 
            Camera{order: 0, hdr: !simple, ..Default::default()},
            Name::from("Main Camera"),
            RenderLayers::layer(0),
        )).id()
    }
    pub fn disable_main_camera(mut query: Query<&mut Camera, With<Self>>){
        for camera in query.iter_mut(){camera.into_inner().is_active = false;}
    }
    pub fn enable_main_camera(mut query: Query<&mut Camera, With<Self>>){
        for camera in query.iter_mut(){camera.into_inner().is_active = true;}
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component, ExtractComponent, Serialize, Deserialize)]
#[reflect(Component)]
pub struct UICamera;
impl UICamera{
    pub fn spawn_ui_camera(commands: &mut Commands) -> Entity{
        commands.spawn((
            Self, 
            Camera2d, 
            Camera{order: 1, ..Default::default()},
            Name::from("UI Camera")
        )).id()
    }
    pub fn disable_ui_camera(mut query: Query<&mut Camera, With<Self>>){
        for camera in query.iter_mut(){camera.into_inner().is_active = false;}
    }
    pub fn enable_ui_camera(mut query: Query<&mut Camera, With<Self>>){
        for camera in query.iter_mut(){camera.into_inner().is_active = true;}
    }
}

pub struct CamerasPlugin;
impl Plugin for CamerasPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<MainCamera>()
            .register_type::<UICamera>()
            .add_plugins((
                ExtractComponentPlugin::<MainCamera>::default(),
                ExtractComponentPlugin::<UICamera>::default()
            ));

        app.add_observer(apply_render_layers_to_children);
    }
}