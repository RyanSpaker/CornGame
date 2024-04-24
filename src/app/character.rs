use std::{f32::consts::FRAC_PI_2, ops::Deref};

/// This will implement the character controller and animations.
/// 
/// using a library called tnua because it had a working demo with animations. https://idanarye.github.io/bevy-tnua/demos/platformer_3d-xpbd
/// might want to rip it out eventually
/// 
/// TODO:
/// [ ] character controller
/// [ ] animations
/// [ ] networking
///   [ ] animation
///   [ ] interpolation
/// [ ] item holding (ex flashlight)
/// [ ] sight map (for out of sight changes)

use macaw::prelude::*;
use bevy::{ecs::bundle::DynamicBundle, prelude::*};
use bevy_tnua::{builtins::TnuaBuiltinWalk, controller::TnuaController, math::AdjustPrecision};
use bevy_xpbd_3d::prelude::*;

pub struct CharacterPlugin;
impl Plugin for CharacterPlugin{
    fn build(&self, app: &mut App) {
        // button input plugin
        app.add_plugins(InputManagerPlugin::<Action>::default());

        // init physics plugins
        app.add_plugins(PhysicsPlugins::default());

        // init character controller plugin
        app.add_plugins(bevy_tnua_xpbd3d::TnuaXpbd3dPlugin::default());
        app.add_plugins(bevy_tnua::controller::TnuaControllerPlugin::default());
        
        // This plugin supports `TnuaCrouchEnforcer`, which prevents the character from standing up
        // while obstructed by an obstacle.
        // app.add_plugins(bevy_tnua::control_helpers::TnuaCrouchEnforcerPlugin::default());

        // app.add_systems(Startup, setup_player);
        app.add_systems(
            Update,
            input_handler.in_set(bevy_tnua::TnuaUserControlsSystemSet),
        );
        // app.add_systems(Update, animation_patcher_system);
        // app.add_systems(Update, animate_platformer_character);
    }
}

#[derive(Debug)]
pub enum AnimationState {
    Standing,
    Running(f32),
    Jumping,
    Falling,
    Crouching,
    Crawling(f32),
    Dashing,
}

/// Anything that moves via tnua character controller -- might be NPC, might be other players
pub struct Player;
impl Player {
    pub fn bundle(&self) -> impl Bundle {
        (
            RigidBody::Dynamic,
            Collider::capsule(1.0, 0.5),
            bevy_tnua::controller::TnuaControllerBundle::default(),
            bevy_tnua::TnuaAnimatingState::<AnimationState>::default(),
                
            InputManagerBundle::with_map(
                Action::default_input_map(),
            )
            // NOTE: not bothering with this yet
            // `TnuaCrouchEnforcer` can be used to prevent the character from standing up when obstructed.
            // bevy_tnua::control_helpers::TnuaCrouchEnforcer::new(0.5 * Vec3::Y, |cmd| {
            //     cmd.insert(bevy_tnua_xpbd3d::TnuaXpbd3dSensorShape(
            //         Collider::cylinder(0.0, 0.5)));
            // })
        )
    }
}

fn input_handler(
    mut query : Query<( &ActionState<Action>, &mut TnuaController)>,
    mut camera : Query<&mut Transform, With<crate::ecs::main_camera::MainCamera>>
){
    let Ok(mut camera) = camera.get_single_mut() else { return };

    for (input, mut controller) in query.iter_mut() {
        let pan = input.axis_pair(&Action::Pan).unwrap();

        let yaw = Quat::from_rotation_y(-0.01 * pan.x()).adjust_precision();
        camera.rotation *= yaw;

        // TODO add math helpers
        let forward = camera.forward().normalize() * (Vec3::X + Vec3::Z);

        let mut pitch = forward.angle_between(*camera.forward());
        pitch += 0.005 * pan.y();
        pitch = pitch.clamp(-FRAC_PI_2, FRAC_PI_2);

        let axis = camera.left().normalize();
        camera.rotation *= Quat::from_axis_angle(axis, pitch);

        let speed = match input.pressed(&Action::Run) {
            true => 2.0,
            false => 1.0,
        };

        let direction = input.clamped_axis_pair(&Action::Move).unwrap();
        let direction : Vec2 = direction.into();

        let direction = direction.extend(0.0).xzy();

        controller.basis( TnuaBuiltinWalk{
            desired_velocity: direction * speed,
            desired_forward: forward,
            ..default()
        });
    }
}


// abstracts keyboard input
use leafwing_input_manager::prelude::*;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
enum Action {
    Crouch,
    Run,
    Move,
    Pan
}

impl Action {
    /// Define the default bindings to the input
    fn default_input_map() -> InputMap<Self> {
        let mut input_map = InputMap::default();

        // Default kbm input bindings
        input_map.insert(Self::Move, VirtualDPad::wasd());
        input_map.insert(Self::Crouch, KeyCode::ControlLeft);
        input_map.insert(Self::Run, KeyCode::ShiftLeft);
        input_map.insert(Self::Pan, DualAxis::mouse_motion());

        input_map
    }
}