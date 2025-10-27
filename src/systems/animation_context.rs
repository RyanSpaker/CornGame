use std::time::Duration;

use bevy::animation::{ActiveAnimation, AnimationTarget, AnimationTargetId};
use bevy::ecs::query::QueryData;
use bevy::prelude::*;
use bevy::platform::collections::{HashMap, HashSet};
use bevy_editor_pls::bevy_inspector_egui::inspector_egui_impls::InspectorPrimitive;
use bevy_editor_pls::egui::Checkbox;

use crate::scenes::SceneGltf;

#[derive(QueryData)]
#[query_data(mutable)]
pub struct AnimationParams {
    pub entity: Entity,
    pub context: &'static AnimationContext,
    pub player: &'static mut AnimationPlayer,
    pub graph: &'static AnimationGraphHandle,
    pub transition: Option<&'static mut AnimationTransitions>,
}

impl<'a> AnimationParamsItem<'a> {
    pub fn play(&mut self, name: &str, fade: Duration) -> Result<&mut ActiveAnimation> {
        let Some(handle) = self.context.named_animations.get(name) else {
            return Err(format!("{} has no animation {}", self.entity, name).into());
        };

        let mut active = match &mut self.transition {
            Some(t) => t.play(&mut *self.player, *handle, fade),
            None => {
                if !fade.is_zero() {
                    error!(entity = %self.entity, "cannot fade animation");
                }
                self.player.stop_all();
                self.player.play(*handle)
            }
        };

        Ok(active)
    }

    // syncronize to time
    // hyper annoying since it needs stuff from asset server, and we want to call this from a system
    // pub fn synchronize(
    //     &mut self,
    //     time: Time,
    // ) -> self {
    //     if let Some(a) = self.player.playing_animations_mut().next() {
    //         time.elapsed().as_secs_f64().rem(a.1)
    //         a.1.seek_time()
    //     }
    // }
}

// TODO better unification between commands and systems
fn animation_sync(
    
){
    
}

// added to the root of a object which needs to be able to play animations.
// finds children with AnimationTarget or AnimationPlayer and grabs applicable animations from parent scene and assets
#[derive(Debug, Clone, Component, Default, Reflect)]
#[reflect(Component)]
pub struct AnimationContext {
    named_animations: HashMap<String, AnimationNodeIndex>,
}

impl AnimationContext {
    fn init_system(
        mut animation_context: Query<(Entity, &mut Self), Added<Self>>,
        gltf_assets: Res<Assets<Gltf>>,
        clip_assets: Res<Assets<AnimationClip>>,
        mut clip_graph: ResMut<Assets<AnimationGraph>>,
        tree: Query<&ChildOf>,
        children: Query<&Children>,
        mut animation_targets: Query<&mut AnimationTarget>,
        // animation_players: Query<&AnimationPlayer>,
        scenes: Query<(&SceneRoot, &SceneGltf)>,
        mut commands: Commands,
    ) -> Result {
        for (entity, mut animation_context) in animation_context.iter_mut() {
            let Some(scene) = std::iter::once(entity).chain(tree
                .iter_ancestors(entity))
                .filter_map(|e| scenes.get(e).ok())
                .next()
            else {
                continue;
            };

            let gltf = scene.1;
            let gltf = gltf_assets.get(&gltf.0).unwrap(); // TODO how to reconcile fallibility and `for in` + how to get entity context into error log

            // get all animation_targets
            // TODO stop at child AnimationContext
            let targets: Vec<Entity> = std::iter::once(entity).chain(children
                .iter_descendants(entity))
                .filter(|e| animation_targets.get(*e).is_ok())
                .collect();

            let mut removed: HashSet<Entity> = default();
            let mut target_ids: HashSet<AnimationTargetId> = default();
            for e in targets.iter() {
                let mut t = animation_targets.get_mut(*e).unwrap();
                target_ids.insert(t.id);
                if t.player != entity {
                    if removed.insert(t.player) {
                        debug!(from = %t.player, to = %entity, "moving AnimationPlayer");
                        commands.entity(t.player).remove::<AnimationPlayer>();
                    }
                    t.player = entity;
                }
            }

            // create animation graph with the animation clips
            // store name->index
            // TODO this should be done per blueprint not per instance.
            let mut graph = AnimationGraph::new();
            for (name, handle) in gltf.named_animations.iter() {
                let clip = clip_assets.get(handle).unwrap();
                if clip.curves().iter().any(|c| target_ids.contains(c.0)) {
                    // This animation clip targets a child of the AnimationContext
                    let index = graph.add_clip(handle.clone(), 1.0, graph.root);
                    animation_context
                        .named_animations
                        .insert(name.to_string(), index);
                }
            }

            let graph = clip_graph.add(graph);
            commands
                .entity(entity)
                // these are not required components because I want their presense to indicate this system actually ran
                // TODO consider a marker WantsAnimationContext instead
                .insert(AnimationPlayer::default())
                .insert(AnimationTransitions::default())
                .insert(AnimationGraphHandle(graph));

        }

        Ok(())
    }

    // TODO I want to be able to add it in code, and work when scene loads
    fn update_system(

    ){

    }
}

#[derive(Debug, Component, Reflect)]
#[reflect(Component)]
pub struct AutoPlayAnimation;

// #[derive(Debug, Component, Reflect)]
// #[reflect(Component)]
// pub struct AutoPlaySync;

fn do_autoplay(
    mut query: Query<(Entity, AnimationParams), With<AutoPlayAnimation>>,
    mut commands: Commands
) -> Result{
    for (entity, mut anim) in query.iter_mut(){
        if anim.player.playing_animations().next().is_none() {
            let Some((name,_)) = anim.context.named_animations.iter().next() else { continue };

            anim.play(name, Duration::ZERO)?.repeat();
            commands.entity(entity).remove::<AutoPlayAnimation>();
        }
    }
    Ok(())
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, AnimationContext::init_system);
    app.add_systems(Update, do_autoplay);
    app.register_type::<AnimationContext>();
    app.register_type::<AutoPlayAnimation>();
    app.register_type_data::<AnimationContext, bevy_editor_pls::InspectorEguiImpl>();
}

impl InspectorPrimitive for AnimationContext {
    fn ui(
        &mut self,
        ui: &mut bevy_editor_pls::egui::Ui,
        _options: &dyn std::any::Any,
        _id: bevy_editor_pls::egui::Id,
        env: bevy_editor_pls::bevy_inspector_egui::reflect_inspector::InspectorUi<'_, '_>,
    ) -> bool {
        let mut play = None;
        for (name, index) in self.named_animations.iter() {
            ui.horizontal(|ui| {
                ui.label(name);
                if ui.button("play").clicked() {
                    play = Some(name);
                }
            });
        }
        if let Some(name) = play {
            if let Some(ref mut queue) = env.context.queue {
                if let Some(e) = env.context.entity {
                    let name = name.to_string();
                    queue.push(move |world: &mut World| {
                        // TODO: implement animation play logic here
                        if let Ok(mut params) = world.query::<AnimationParams>().get_mut(world, e) {
                            params.play(&name, Duration::ZERO).unwrap();
                        }
                    });
                }
            }
        }
        false
    }

    fn ui_readonly(
        &self,
        ui: &mut bevy_editor_pls::egui::Ui,
        _options: &dyn std::any::Any,
        _id: bevy_editor_pls::egui::Id,
        _env: bevy_editor_pls::bevy_inspector_egui::reflect_inspector::InspectorUi<'_, '_>,
    ) {
        for (name, index) in self.named_animations.iter() {
            ui.horizontal(|ui| {
                //TODO show currently active
                ui.add_enabled(false, Checkbox::new(&mut false, "name"));
                ui.label(name);
            });
        }
    }
}
