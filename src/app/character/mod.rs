use core::panic;
use std::cell::RefCell;
use std::time::Duration;

use bevy::ecs::bundle::DynamicBundle;
use bevy::ecs::query::{QueryData, QuerySingleError, WorldQuery};
use bevy::ecs::system::SystemParam;
use bevy::gizmos::GizmoRenderSystem;
use bevy::pbr::LightEntity;
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

use bevy::{prelude::*, transform};
use avian3d::prelude::*;
use controller::CornGameCharController;
use leafwing_input_manager::plugin::InputManagerPlugin;
use leafwing_input_manager::InputManagerBundle;

use crate::ecs::main_camera::MainCamera;

use self::input::Action;

mod input;
mod controller;
mod animation;

pub struct MyCharacterPlugin;
impl Plugin for MyCharacterPlugin{
    fn build(&self, app: &mut App) {
        // button input plugin
        app.add_plugins(InputManagerPlugin::<self::input::Action>::default());
        
        app.register_type::<CornGameCharController>();
        app.register_type::<SpawnLocation>();
        app.register_type::<SpawnPlayerEvent>();

        // init character controller plugin
        app.add_plugins(bevy_tnua_avian3d::TnuaAvian3dPlugin::new(Update));//NOTE: FixedUpdate?
        app.add_plugins(bevy_tnua::controller::TnuaControllerPlugin::default());

        // This plugin supports `TnuaCrouchEnforcer`, which prevents the character from standing up
        // while obstructed by an obstacle.
        // app.add_plugins(bevy_tnua::control_helpers::TnuaCrouchEnforcerPlugin::default());

        // app.add_systems(Startup, setup_player);
        app.add_systems(
            FixedUpdate, // Update was causing jitter. but it might have just been gizmos.
            self::controller::input_handler.in_set(bevy_tnua::TnuaUserControlsSystemSet),
        );
        app.add_systems(Update, animation_test);
        app.add_observer(spawn_dehydrated_child_obs);

        // app.add_systems(Update, animation_patcher_system);
        // app.add_systems(Update, animate_platformer_character);
        app.add_observer(move_player_to_spawn_obs);
        app.add_observer(|
            t: Trigger<OnAdd, SpawnLocation>, 
            query: Query<&SpawnLocation>,
            mut commands: Commands
        | {
            // problematic for multiplayer
            if query.get(t.entity()).unwrap().default {
                commands.trigger(SpawnPlayerEvent::default());
            }
        });
    }
}

/// Anything that moves via tnua character controller -- might be NPC, might be other players
#[derive(Debug, Component)]
pub struct Player;
impl Player {
    pub fn bundle() -> impl Bundle {
        (
            Player,
            Name::new("Player"),
            RigidBody::Dynamic,
            bevy_tnua::controller::TnuaController::default(),
            bevy_tnua::TnuaAnimatingState::<self::animation::AnimationState>::default(),
            bevy_tnua_avian3d::TnuaAvian3dSensorShape(Collider::cylinder(0.2, 0.0)), //XXX configure this in CornGameCharacterController
            CornGameCharController{
                // height: 2.0,
                // crouch_height: 2.0,
                // float: 0.5,
                // crouch_float:0.1,
                ..default()
            },
            
            InputManagerBundle::with_map(
                Action::default_input_map(),
            ),

            // Hack to add spot light as child,
            // TODO make it so I can specify this as a asset and load it with scene / from commandline (also shpuld be able to disable scene items from command line or console)
            // DehydratedChild::new((
            //     BlueprintInfo::from_path("blueprints/flashlight.glb"),
            //     SpawnBlueprint, // manditory
            //     Transform{
            //         translation: Vec3::new(0.2, -0.2, -0.3),
            //         ..default()
            //     },
            //     super::interactions::Held, // TODO rework so this auto attaches to player
            //     super::interactions::Pickup,
            //     RigidBodyDisabled,
            //     RigidBody::Dynamic
            // )),

            // NOTE: not bothering with this yet
            // `TnuaCrouchEnforcer` can be used to prevent the character from standing up when obstructed.
            // bevy_tnua::control_helpers::TnuaCrouchEnforcer::new(0.5 * Vec3::Y, |cmd| {
            //     cmd.insert(bevy_tnua_xpbd3d::TnuaXpbd3dSensorShape(
            //         Collider::cylinder(0.0, 0.5)));
            // })
        )
    }
}

#[derive(Debug, Default, Reflect, Component)]
#[reflect(Default)]
#[reflect(Component)]
pub struct SpawnLocation{
    pub default: bool,
}

#[derive(Debug, Default, Reflect, Event)]
pub struct SpawnPlayerEvent{
    pub target: Option<String>,
}

#[derive(Debug, QueryData)]
struct SpawnQuery {
    gt: &'static GlobalTransform,
    name: Option<&'static Name>,
    info: Option<&'static SpawnLocation>
}

fn move_player_to_spawn_obs(
    trigger: Trigger<SpawnPlayerEvent>,
    mut camera: Query<(Entity, &mut Transform, &GlobalTransform), (With<MainCamera>, Without<Player>)>,
    mut player: Query<(Entity, &mut Transform, &GlobalTransform), With<Player>>,
    spawn: Query<SpawnQuery>,
    mut commands: Commands,
){
    // spawn a player or move existing player
    // TODO narrow by scene

    // get matching names (or SpawnLocations if name not specified)
    let mut spawn : Vec<_> = spawn.iter().filter(|s| 
        trigger.target.as_ref().is_none_or(|target| 
            s.name.is_some_and(|n2| target == n2.as_str())
        &&
        ( trigger.target.is_some() || s.info.is_some() )
    )).collect();

    // prefer default spawn_location, followed by other spawn_locations, fallback to simple name match (XXX overengineered)
    spawn.sort_by_key(|s|{
        match s.info {
            Some(v) => match v.default {
                true => 2,
                false => 1,
            },
            None => 0,
        }
    });

    let spawn = spawn.pop().map(|s|*s.gt).unwrap_or_default();

    let mut transform : Transform = spawn.compute_transform();
    transform.scale = Vec3::ONE;

    match player.get_single_mut() {
        Ok((id, mut t, _gt)) => {
            // todo use gt
            info!("moving player to spawn {}", &transform.translation);
            *t = transform;
            commands.entity(id).insert((
                LinearVelocity::default(),
                AngularVelocity::default()
            ));
        },
        Err(QuerySingleError::NoEntities(_)) => {
            info!("creating player at spawn {}", &transform.translation);
            commands.spawn((
                Player::bundle(),
                transform
            ));
        },
        Err(QuerySingleError::MultipleEntities(_)) => todo!(),
    }

    if let Ok(mut camera) = camera.get_single_mut(){
        *camera.1 = transform;
    }
    
}


#[derive(Component)]
pub struct DehydratedChild {
    bundle_factory: Option<Box<dyn FnOnce(&mut Commands) -> Entity + Send + Sync>>,
}

impl DehydratedChild {
    pub fn new<B: Bundle + Send + Sync + 'static>(bundle: B) -> Self {
        Self {
            bundle_factory: Some(Box::new( move |commands: &mut Commands| {
                            commands.spawn(bundle).id()
                        })),
        }
    }
}

/// System that spawns a new child entity when `DynamicChild` is added
fn spawn_dehydrated_child_obs(
    trigger: Trigger<OnAdd, DehydratedChild>,
    mut commands: Commands,
    mut query: Query<&mut DehydratedChild, Added<DehydratedChild>>,
) {
    let mut dehydrated_child = query.get_mut(trigger.entity()).expect("bad trigger?");
    let bundle_factory = dehydrated_child.bundle_factory.take().expect("should have callback");
    let child = bundle_factory(&mut commands);
    commands.entity(trigger.entity()).add_child(child).remove::<DehydratedChild>();
}


use blenvy::{BlueprintAnimationPlayerLink, BlueprintInfo, SpawnBlueprint};
use blenvy::BlueprintAnimations;

use super::interactions::Interactable;

/// KEEP this, default behavior should be to play animations
pub fn animation_test(
    animated_robots: Query<(Entity, &BlueprintAnimationPlayerLink, &BlueprintAnimations), Without<Interactable>>,

    mut animation_players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>, //TODO should be more general without case

) {
    // robots
    for (id, link, animations) in animated_robots.iter() {
        let Ok((mut animation_player, mut animation_transitions)) =
            animation_players.get_mut(link.0) else {continue};
        if animation_player.playing_animations().next().is_some(){
            // don't start animation if one is playing
            break;   
        }
        debug!("start animation for {}", id);
        animation_transitions
            .play(
                &mut animation_player,
                *animations
                    .named_indices.iter().next()
                    .expect("there should be an animation").1,
                Duration::from_secs(1),
            )
            .repeat();
    }
}

// TODO system to prevent rendering player model for current player
// TODO system to rehydrate player on client