use std::ffi::OsStr;
use avian3d::prelude::{Collider, RigidBody};
use bevy::{pbr::FogVolume, prelude::*};
use blenvy::{BlueprintInfo, GameWorldTag, SpawnBlueprint};
use crate::{ecs::{cameras::{MainCamera, UICamera}, test_cube::TestCube}, systems::{scenes::{CornScene, CurrentScene, OnSpawnScene, SceneTransitionApp}, util::default_resources::{SimpleMaterials, SimpleMeshes}}, Cli};


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
        materials: Res<SimpleMaterials>,
        cli: Res<Cli>
    ){
        commands.entity(parent.0).with_children(|parent| {
            parent.spawn((
                Name::from("Floor"),
                Transform::from_scale(Vec3::new(10.0, 0.0, 10.0)),
                Collider::cuboid(1.0, 0.1, 1.0),
                Mesh3d(shapes.plane.clone()),
                MeshMaterial3d(materials.white.clone()),
                RigidBody::Static
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
            parent.spawn((
                Name::from("Fog Volume"),
                FogVolume{ density_factor:0.0001, ..default() },
                Transform::from_scale(Vec3::splat(35.0)),
            ));
        
            for path in cli.scenes.iter() {
                let path = path.strip_prefix("assets/").unwrap_or(&path);
        
                if path.extension() == Some(OsStr::new("glb")) {
                    // TODO: Try inserting on parent instead to get rid of extra nesting level when loading 1 scene
                    parent.spawn((
                        Name::from("Level from: ".to_string() + &path.to_str().unwrap()),
                        BlueprintInfo::from_path(&path.to_str().unwrap()), //NOTE: I wish there was a language where I could just do path, and it would give a warning but I wouldn't have to do all this type munching
                        SpawnBlueprint,
                        GameWorldTag,
                        // RigidBody::Static // weird things happen if there are colliders with no rigid body
                        // EDIT: weirder things happen with nested RigidBodys
                    ));
                }
            }
        
            parent.spawn(TestCube);
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