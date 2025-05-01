use bevy::{ecs::system::SystemId, prelude::*};
use crate::systems::util::{camera::MainCamera, default_resources::{SimpleMaterials, SimpleMeshes}};
use super::{AppStage, InGame};

#[derive(Debug, Default, Clone)]
pub struct LobbyPlugin;
impl Plugin for LobbyPlugin{
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppStage::Lobby), spawn_lobby);
        let id = app.register_system(PositionCameraSystem::system());
        app.insert_resource(PositionCameraSystem(id));
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct PositionCameraSystem(SystemId);
impl PositionCameraSystem{
    pub fn system() -> impl FnMut(Query<&mut Transform, With<MainCamera>>) {
        |mut query: Query<&mut Transform, With<MainCamera>>| {
            for mut t in query.iter_mut(){
                *t = Transform::from_xyz(0.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);
            }
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Reflect, Component)]
pub struct Lobby;

fn spawn_lobby(
    mut commands: Commands,
    shapes: Res<SimpleMeshes>,
    materials: Res<SimpleMaterials>,
    camera_system: Res<PositionCameraSystem>
){
    commands.spawn((
        DirectionalLight::default(), 
        Transform::from_translation(Vec3::ONE).looking_at(Vec3::ZERO, Vec3::Y),
        StateScoped(InGame)
    ));

    commands.run_system(camera_system.0);

    let parent = commands.spawn((
        Lobby, 
        Name::from("Lobby Scene"),
        Visibility::Visible,
        Transform::default(),
        StateScoped(AppStage::Lobby),
    )).id();

    commands.spawn((
        Name::from("Floor"),
        Transform::from_scale(Vec3::new(10.0, 0.0, 10.0)),
        Mesh3d(shapes.plane.clone()),
        MeshMaterial3d(materials.white.clone())
    )).set_parent(parent);
    commands.spawn((
        Name::from("Box"),
        Mesh3d(shapes.cube.clone()),
        MeshMaterial3d(materials.red.clone())
    )).set_parent(parent);
}