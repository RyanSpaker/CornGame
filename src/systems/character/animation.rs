// reference https://github.com/bevyengine/bevy/blob/main/examples/animation/animation_graph.rs
// https://github.com/idanarye/bevy-tnua/blob/main/demos/src/character_animating_systems/platformer_animating_systems.rs

// TODO
// [ ] Inverse kinematics
//   [ ] camera direction and held item
// [ ] animation graph assets
// [ ] animation transitions and tweening
// [ ] networking animation state/inputs
// [ ] sharing code between NPC's and player character controller

// simplest way to do animation?
// just base it on the

use std::time::Duration;

pub use bevy::prelude::*;
use blenvy::{BlueprintAnimationPlayerLink, BlueprintAnimations, BlueprintInfo, SpawnBlueprint};
use lightyear::prelude::{AppComponentExt, ChannelDirection};
use serde::{Deserialize, Serialize};
use wgpu::core::error;

#[derive(Debug, Clone, Component, Reflect, PartialEq, Serialize, Deserialize)]
#[reflect(Component)]
pub enum MyAnimationState {
    Idle,
    Walk(Vec2),
}

impl MyAnimationState {
    fn update_animation(
        query: Query<(Entity, &MyAnimationState, Option<&Children>), Changed<MyAnimationState>>,
        blueprint: Query<(Entity, &BlueprintAnimationPlayerLink, &BlueprintAnimations)>,
        mut animation_players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>, //TODO should be more general without case
    ) {
        for (id, state, children) in query.iter() {
            // a bunch of ugly code to let player model be a child of the controller
            let mut ids = vec![id];
            if let Some(children) = children {
                ids.extend(children.iter())
            }

            for id in ids {
                let Ok((id, link, animations)) = blueprint.get(id) else {
                    continue;
                };

                let Ok((mut animation_player, mut animation_transitions)) =
                    animation_players.get_mut(link.0)
                else {
                    error!("no AnimationPlayer for {}", link.0);
                    continue;
                };

                let anim_name = match state {
                    MyAnimationState::Idle => "idle",
                    MyAnimationState::Walk(_vec2) => "walk",
                };

                let Some(animation) = animations.named_indices.get(anim_name) else {
                    error!("animation {} does not exist for {}", anim_name, id);
                    return;
                };

                debug!("start animation for {}", id);
                animation_transitions
                    .play(
                        &mut animation_player,
                        *animation,
                        Duration::from_millis(200),
                    )
                    .repeat();
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app.register_type::<MyAnimationState>();
    app.add_systems(Update, MyAnimationState::update_animation);
    app.register_component::<MyAnimationState>(ChannelDirection::Bidirectional);
    //app.register_component::<SpawnBlueprint>(ChannelDirection::Bidirectional);
    //app.register_component::<BlueprintInfo>(ChannelDirection::Bidirectional);
}
