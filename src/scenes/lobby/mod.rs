use avian3d::prelude::{Collider, PhysicsTime, RigidBody};
use bevy::{pbr::FogVolume, prelude::*, scene::SceneLoader};
// use blenvy::{BlueprintInfo, GameWorldTag, SpawnBlueprint};
use crate::{
    ecs::{cameras::MainCamera, sunlight::{Moon, NoRotationChild, Sun}, test_cube::TestCube},
    systems::{
        scenes::{CornScene, CurrentScene, OnSpawnScene, SceneTransitionApp},
        util::default_resources::{SimpleMaterials, SimpleMeshes},
    },
    Cli,
};

use super::LoadScene;

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct LobbyScene;
impl CornScene for LobbyScene {
    // NOTE: could use required components
    fn get_bundle(self) -> impl Bundle {
        (self, Name::from("Lobby Scene"))
    }
}
impl LobbyScene {
    fn spawn_scene(
        mut commands: Commands,
        parent: Res<CurrentScene>,
        shapes: Res<SimpleMeshes>,
        materials: Res<SimpleMaterials>,
        mut ambient: ResMut<AmbientLight>,
        cli: Res<Cli>,
        mut time: ResMut<Time<avian3d::prelude::Physics>>,
    ) {
        //TODO can we make ambient not a resource
        ambient.brightness = 0.2;

        time.pause();
        commands.spawn(TestCube);
        commands.entity(parent.0).with_children(|parent| {
            parent.spawn((
                Name::from("Floor"),
                Transform::from_scale(Vec3::new(1000.0, 0.0, 1000.0)),
                Collider::cuboid(1.0, 0.1, 1.0),
                // Mesh3d(shapes.plane.clone()),
                // MeshMaterial3d(materials.white.clone()),
                RigidBody::Static,
            ));
            // parent.spawn((
            //     Name::from("Box"),
            //     Mesh3d(shapes.cube.clone()),
            //     MeshMaterial3d(materials.red.clone()),
            // ));
            parent.spawn((
                Sun, 
                DirectionalLight{
                    illuminance: 1000.0,  
                    ..default()
                },
                Transform::from_translation(Vec3::new(-1.0,-1.0,-1.0) * 2000.0).looking_at(Vec3::ZERO, Vec3::Y),
            )).with_child((
                Transform::from_scale(Vec3::splat(45.0)),
                NoRotationChild, //TODO should make directional light the child instead
                LoadScene::new("models/sky.glb#sun")
            ));
            parent.spawn((
                Moon, 
                DirectionalLight{
                    illuminance: 30.0,  
                    ..default()
                },
                Transform::from_translation(Vec3::new(-1.0, 1.0, -2.0).normalize() * 1000.0).looking_at(Vec3::ZERO, Vec3::Y).with_scale(Vec3::splat(30.0)),//scale does weird things here
                LoadScene::new("models/sky.glb#moon")
            ));
            parent.spawn((Transform::from_xyz(0.0, 500.0, 0.0).with_scale(Vec3::splat(10.0)), LoadScene::new("models/sky.glb#sky")));


            parent.spawn((
                // TODO keep centered on player
                Name::from("Fog Volume"),
                FogVolume {
                    density_factor: 0.0001,
                    ..default()
                },
                Transform::from_scale(Vec3::splat(35.0)),
            ));

            for path in cli.scenes.iter() {
                let path = path.strip_prefix("assets/").unwrap_or(&path);

                // if path.extension() == Some(OsStr::new("glb")) {
                // TODO: Try inserting on parent instead to get rid of extra nesting level when loading 1 scene
                parent.spawn((
                    Name::from("Level from: ".to_string() + &path.to_str().unwrap()),
                    LoadScene::new(path.to_str().unwrap()),
                    // RigidBody::Static // weird things happen if there are colliders with no rigid body
                    // EDIT: weirder things happen with nested RigidBodys
                ));
            }
        });
    }
}

pub fn position_camera(mut query: Query<&mut Transform, With<MainCamera>>) {
    for mut trans in query.iter_mut() {
        *trans = Transform::from_xyz(0.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);
    }
}

#[derive(Debug, Default, Clone)]
pub struct LobbyPlugin;
impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<LobbyScene>()
            .init_scene::<LobbyScene>()
            .add_systems(
                OnSpawnScene(LobbyScene),
                (LobbyScene::spawn_scene, position_camera),
            );
    }
}
