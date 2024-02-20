/// XXX WORK IN PROGRESS, currently unused

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::{app::Plugin, ecs::{bundle::Bundle, component::Component, event::EventWriter, query::With, schedule::States, system::{Query, Res}}, math::Vec3, prelude::default, reflect::Reflect, time::Time, transform::components::Transform};
use bevy_rapier3d::{control::KinematicCharacterController, dynamics::RigidBody, geometry::Collider, plugin::{NoUserData, RapierPhysicsPlugin}};

use crate::ecs::flycam::FlyCamConfig;

pub struct CharacterControllerPlugin;

impl Plugin for CharacterControllerPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        //app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());
        
    }
}

#[derive(Component, Debug, Default, Clone, Reflect)]
pub struct CharacterController;

#[derive(Bundle, Debug, Clone)]
pub struct CharacterControllerBundle(RigidBody, Collider, KinematicCharacterController);

impl Default for CharacterControllerBundle {
    fn default() -> Self {
        Self(
            RigidBody::KinematicPositionBased,
            Collider::ball(0.5),
            KinematicCharacterController::default(),
        )
    }
}

/// Reads in input data, sending an event if there are inputs to process
fn update(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut mouse_events: EventReader<MouseMotion>,
    mut query: Query<&mut Transform, With<CharacterController>>,
    config: Res<FlyCamConfig>,

    time: Res<Time>,
){

    let mut movement: Vec3 = Vec3::ZERO;
    if keyboard_input.pressed(KeyCode::KeyW) {movement.z += 0.5;}
    if keyboard_input.pressed(KeyCode::KeyS) {movement.z -= 0.5;}
    if keyboard_input.pressed(KeyCode::KeyA) {movement.x += 0.5;}
    if keyboard_input.pressed(KeyCode::KeyD) {movement.x -= 0.5;}
    if keyboard_input.pressed(KeyCode::KeyR) {movement.y += 0.1;}
    if keyboard_input.pressed(KeyCode::KeyF) {movement.y -= 0.1;}
    
    movement = movement.normalize();

    let mouse_events: Vec<&MouseMotion> = mouse_events.read().collect();

    let is_mouse = !mouse_events.is_empty(); 
    let total_mouse: Vec2 = mouse_events.into_iter().map(|event| event.delta).sum();
    for mut transform in query.iter_mut(){
        let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        if is_mouse {
            pitch -= (config.sensitivity*total_mouse.y).to_radians();
            yaw -= (config.sensitivity*total_mouse.x).to_radians();
            pitch = pitch.clamp(-1.54, 1.54);
        }
        let yaw_rot = Quat::from_rotation_y(yaw);
        if is_mouse {
            transform.rotation = yaw_rot*Quat::from_rotation_x(pitch);
        }
        let movement: Vec3 = yaw_rot*movement;
        transform.translation += movement*config.movement_speed*time.delta_seconds();
    }
}
