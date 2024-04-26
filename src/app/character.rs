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

use bevy::prelude::*;
use bevy_tnua::prelude::*;
use bevy_tnua::builtins::{TnuaBuiltinCrouch, TnuaBuiltinDash}; 
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
            ),

            Name::new("Player")
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
    mut query : Query<(&Transform, &ActionState<Action>, &mut TnuaController), Without<crate::ecs::main_camera::MainCamera>>,
    mut camera : Query<&mut Transform, With<crate::ecs::main_camera::MainCamera>>
){
    let Ok(mut camera) = camera.get_single_mut() else { return };

    for (transform, input, mut controller) in query.iter_mut() {
        let mouse = input.axis_pair(&Action::Pan).unwrap();

        let sensitivity = 0.002;
        let (mut yaw, mut pitch, _) = camera.rotation.to_euler(EulerRot::YXZ);
        yaw   -= sensitivity * mouse.x();
        pitch -= sensitivity * mouse.y();
        pitch = pitch.clamp(-1.54, 1.54);

        let yaw_rot = Quat::from_rotation_y(yaw);
        camera.rotation = yaw_rot*Quat::from_rotation_x(pitch);
        camera.translation = transform.translation + Vec3::new(0., 0.0, 0.);

        let direction = input.clamped_axis_pair(&Action::Move).unwrap();
        let direction : Vec2 = direction.into();

        let mut direction = direction.extend(0.0).xzy();
        direction.z = -direction.z;

        let speed = match input.pressed(&Action::Run) {
            true => 5.0,
            false => 2.5,
        };

        // TODO add math helpers
        let forward = camera.forward().normalize() * (Vec3::X + Vec3::Z);

        controller.basis( TnuaBuiltinWalk{
            desired_velocity: yaw_rot * direction * speed,
            desired_forward: forward,
            float_height: 2.0,
            acceleration: 150.0,
            ..default()
        });

        if input.pressed(&Action::Crouch){
            controller.action(TnuaBuiltinCrouch {
                float_offset: -0.9,
                ..Default::default()
            });
        } else if input.pressed(&Action::Run){
            controller.action(TnuaBuiltinDash::default());
        }
    
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