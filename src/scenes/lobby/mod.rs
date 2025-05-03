use bevy::prelude::*;
use crate::systems::{scenes::{CornScene, CurrentScene, OnSpawnScene, SceneTransitionApp}, util::{camera::{MainCamera, UICamera}, default_resources::{SimpleMaterials, SimpleMeshes}}};


#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct LobbyScene;
impl CornScene for LobbyScene{
    fn get_bundle(self) -> impl Bundle {
        (self, Name::from("Lobby Scene"))
    }
}
impl LobbyScene{
    fn spawn_scene(
        mut commands: Commands,
        parent: Res<CurrentScene>,
        shapes: Res<SimpleMeshes>,
        materials: Res<SimpleMaterials>
    ){
        commands.entity(parent.0).with_children(|parent| {
            parent.spawn((
                Name::from("Floor"),
                Transform::from_scale(Vec3::new(10.0, 0.0, 10.0)),
                Mesh3d(shapes.plane.clone()),
                MeshMaterial3d(materials.white.clone())
            ));
            parent.spawn((
                Name::from("Box"),
                Mesh3d(shapes.cube.clone()),
                MeshMaterial3d(materials.red.clone())
            ));
            parent.spawn((
                DirectionalLight::default(), 
                Transform::from_translation(Vec3::ONE).looking_at(Vec3::ZERO, Vec3::Y)
            ));
        });
    }
}

pub fn position_camera(mut query: Query<&mut Transform, With<MainCamera>>) {
    for mut trans in query.iter_mut(){
        *trans = Transform::from_xyz(0.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);
    }
}

#[derive(Debug, Default, Clone)]
pub struct LobbyPlugin;
impl Plugin for LobbyPlugin{
    fn build(&self, app: &mut App) {
        app.register_type::<LobbyScene>()
            .init_scene::<LobbyScene>()
            .add_systems(OnSpawnScene(LobbyScene), (
                LobbyScene::spawn_scene,
                position_camera,
                MainCamera::enable_main_camera,
                UICamera::disable_ui_camera
            ));
    }
}