use bevy::{math::VectorSpace, prelude::*};

use crate::app::{state::{AppStage, InGame}, util::default_resources::{SimpleMaterials, SimpleMeshes}};

#[derive(Debug, Default, Clone)]
pub struct LobbyPlugin;
impl Plugin for LobbyPlugin{
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppStage::Lobby), spawn_lobby);
    }
}

fn spawn_lobby(
    mut commands: Commands,
    shapes: Res<SimpleMeshes>,
    materials: Res<SimpleMaterials>
){
    commands.spawn((
        DirectionalLight::default(), 
        Transform::from_translation(Vec3::ONE).looking_at(Vec3::ZERO, Vec3::Y),
        StateScoped(InGame)
    ));
    commands.spawn((
        Mesh3d(shapes.plane.clone()),
        MeshMaterial3d(materials.white.clone()),
        StateScoped(AppStage::Lobby)
    ));
}