use std::{collections::HashMap, time::Duration};
use avian3d::prelude::RigidBodyDisabled;
use bevy::{
    animation, audio::Volume, diagnostic::FrameCount, input::keyboard::{Key, KeyboardInput}, picking::{self, backend::HitData, hover::HoverMap, pointer::{Location, PointerId, PointerLocation}}, platform::time::Instant, prelude::*, render::{camera::RenderTarget, primitives::Aabb}, window::{CursorGrabMode, NormalizedWindowRef, PrimaryWindow}
};
use bevy_easings::EasingsPlugin;
use clap::Parser;
use frunk::labelled::chars::{N, Q};
use lightyear::{
    connection::{identity::{is_client, is_host_server}, server::is_server},
    prelude::{
        ActionsChannel, AppMessageExt, Client, MessageSender, NetworkDirection, NetworkTarget, ServerMultiMessageSender
    },
};
use serde::{Deserialize, Serialize};

use crate::{
    ecs::cameras::MainCamera, scenes::resolver::{EntityPointer, EntityResolver}, systems::{animation_context::AnimationParams, network::uid::{Uid, UidUsePath}}, Cli
};

use super::character::Player;

pub struct InteractPlugin;
impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MeshPickingPlugin);
        app.add_plugins(EasingsPlugin::default());
        app.add_observer(on_over);
        app.add_observer(on_out);
        app.add_systems(
            Update,
            (
                display_tooltip,
                handle_key,
                ToggleInteractionBlender::handle_animation_done,
            ),
        );
        app.add_systems(PostUpdate, DebugForMissingReflect::system);
        app.register_type::<DebugForMissingReflect>();
        app.init_resource::<DebugForMissingReflect>();

        app.register_type::<Interactable>();
        app.register_type::<ToggleInteractionBlender>();
        app.register_type::<ToggleInteractionState>();
        app.register_type::<FlipVisible>();
        app.register_type::<Hover>();
        app.register_type::<InteractionText>();
        app.register_type::<Pickup>();
        app.register_type::<Held>();

        //app.register_trigger::<Interaction>(ChannelDirection::Bidirectional);

        // for debugging a blenvy issue
        app.register_type::<HashMapTest>();
        app.register_type::<HashMapTest2>();
        app.register_type::<HashMapTest3>();

        app.add_observer(ToggleInteractionBlender::observer);
        app.add_observer(Pickup::observer);
        app.add_observer(ToggleInteractionBlender::handle_flip);
        app.add_observer(ToggleInteractionBlender::flip_init);

        app.register_type::<DehydratedController>();
        app.add_systems(PostUpdate, DehydratedController::hydrate);
        // app.add_observer(ToggleInteractionBlender::handle_flip);

        // deal with cursor lock bug
        // NOTE: redundant with force_pointer_center
        app.add_systems(Main, center_cursor_on_camera_viewport.before(bevy::picking::PickSet::Input));

        // XXX Why does this not work?? I am guessing bc Main?
        // app.add_systems(Main, force_pointer_center
        //     .after(bevy::picking::pointer::PointerInput::receive)
        //     .before(bevy::picking::backend::ray::RayMap::repopulate)
        //     .before(bevy::picking::PickSet::Backend)
        // );
        app.add_systems(PreUpdate, force_pointer_center
            .after(bevy::picking::pointer::PointerInput::receive)
            .before(bevy::picking::backend::ray::RayMap::repopulate)
            .in_set(bevy::picking::PickSet::ProcessInput)
        );
    }
}

pub struct NetworkInteractPlugin;
impl Plugin for NetworkInteractPlugin {
    fn build(&self, app: &mut App) {

        // direction appears needed for MessageSender to be attached
        app.add_message::<InteractionMessage>().add_direction(NetworkDirection::Bidirectional);


        pub(crate) fn receive(
            mut receiver: Query<
                &mut lightyear::prelude::MessageReceiver<InteractionMessage>,
                With<Client>,
            >,
            uid: Query<(Entity, &Uid)>,
            state: Query<&ToggleInteractionState>,
            mut commands: Commands,
        )  -> Result {

            for message in receiver.single_mut()?.receive() {
                info!("Client received message: {:?}", message);
                let Some((e, _)) = uid.iter().find(|e| *e.1 == message.uid) else {
                    error!(uid = message.uid.0, "invalid uid");
                    break;
                };

                match state.get(e) {
                    Ok(s) => {
                        if s.0 != message.state {
                            info!(entity = %e, "ToggleInteractionState changed to {}", s.0);
                            commands.trigger_targets(Interaction, e);
                        } else {
                            info!(entity = %e, "ToggleInteractionState already set to {}", s.0);
                        }
                    }
                    Err(_) => {
                        error!(entity = %e, "Entity does not have ToggleInteractionState component");
                    }
                }
            }
            Ok(())
        }


        /// Returns true if the peer is a client (host-server counts as a server)
        pub fn is_any_client(query: Query<(), With<Client>>) -> bool {
            !query.is_empty()
        }

        app.add_systems(
            Update,
            receive
                .run_if(is_any_client)
                .in_set(lightyear::prelude::MessageSet::Receive)
        );

        pub(crate) fn server_receive_and_send(
            receiver: Query<(
                &mut lightyear::prelude::RemoteId,
                &mut lightyear::prelude::MessageReceiver<InteractionMessage>,
            )>,
            mut sender: ServerMultiMessageSender,
            server: Single<&lightyear::prelude::Server>,
        ) -> Result {
            for (remote_id, mut receiver) in receiver {
                for message in receiver.receive() {
                    info!(peer = ?*remote_id, "Server received message: {:?}", message);

                    sender.send::<_, ActionsChannel>(
                        &message,
                        &*server,
                        &NetworkTarget::AllExceptSingle(remote_id.0),
                    )?;
                }
            }

            Ok(())
        }

        app.add_systems(
            PostUpdate,
            server_receive_and_send
                // .in_set(lightyear::prelude::MessageSet::Receive)
                .run_if(is_server),
        );

        app.add_plugins(InteractDummy(false));
    }
}

#[derive(Resource, Debug, Clone, Default, Reflect)]
struct InteractDummy(bool);

impl Plugin for InteractDummy {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.clone());
        app.add_systems(Startup, |cli: Res<Cli>, mut commands: Commands| {
            if cli.dummy {
                commands.insert_resource(InteractDummy(true));
            }
        });

        pub fn dummy_interact_loop(
            mut commands: Commands,
            mut query: Query<(Entity, &Interactable)>,
            mut local: Local<Timer>,
            time: Res<Time>,
            this: Res<InteractDummy>,
            
            mut sender: Query<&mut MessageSender<InteractionMessage>, With<Client>>,
            state: Query<&ToggleInteractionState>,
            uid: Query<&Uid>,
        ) {
            if !this.0 {
                return;
            }

            local.set_duration(Duration::from_millis(3000));
            local.set_mode(bevy::time::TimerMode::Repeating);
            if !local.tick(time.delta()).just_finished() {
                return;
            }
            for (entity, interactable) in query.iter_mut() {
                if interactable.active {
                    continue;
                }
                // TODO this is a dummy loop, should be replaced with actual interaction logic
                commands.trigger_targets(Interaction, entity);
                // Send network message
                if let Ok(mut net) = sender.single_mut() {
                    net.send::<ActionsChannel>(InteractionMessage {
                        uid: uid.get(entity).unwrap().clone(),
                        state: !state.get(entity).unwrap().0,
                    });
                } else {
                    error!("No MessageSender found for InteractionMessage");
                }
            }
        }
        app.add_systems(Update, dummy_interact_loop);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct InteractionMessage {
    pub uid: Uid,
    pub state: bool,
    //client_id
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
struct HashMapTest(HashMap<String, String>);
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
struct HashMapTest2(HashMap<String, Vec<String>>);
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
struct HashMapTest3(HashMap<String, HashMap<String, String>>);

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[require(Interactable)]
#[require(InteractionText = InteractionText::flip())]
pub struct Pickup;

// TODO want to be able to set held object from commandline or scene file
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[component(storage = "SparseSet")]
pub struct Held; /*{
                     // Entity to sync to
                     // entity: Entity,

                     // Offset
                     // TODO hand bone
                     // offset: Transform,
                 }*/

impl Pickup {
    /// This should handle the logic of picking up an item.
    /// It should have special handling for the player (if appropriate)
    /// It should not handle logic of displaying a held item. That should be done with the Held component (which perhaps should be relation)
    fn observer(
        ev: Trigger<Interaction>,
        mut commands: Commands,
        mut query: Query<(Entity, &Self, &mut Interactable, &GlobalTransform)>,

        // current held item to put down
        mut held: Query<(Entity, &Held, &mut Transform)>,

        // player to take item
        player: Query<(Entity, &Player)>,
    ) {
        // HERE need to handle rigidbody, and add damping to outer rocket
        let Ok((entity, _pickup, _interactable, gt)) = query.get_mut(ev.target()) else {
            return;
        };
        debug!("pickup {}", ev.target());

        let player = player.single().unwrap(); //TODO multiplayer
        commands.entity(entity).insert(ChildOf(player.0)).insert((
            Transform {
                translation: Vec3::new(0.1, -0.3, -0.6),
                scale: gt.scale(),
                ..default()
            },
            Held,
            RigidBodyDisabled, // XXX what about child colliders
        ));

        for mut h in held.iter_mut() {
            // TODO better logic for putting down currently held item
            // TODO have everything work off of adding or removing the Held component
            h.2.translation = gt.translation();
            commands
                .entity(h.0)
                .remove::<(ChildOf, Held, RigidBodyDisabled)>();
        }
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct FlipVisible {
    /// currently unused
    vis: bool,
    delay: f32,
}

/// attatched to ex. light switches
/// TODO deal with many-to-many
#[derive(Debug, Clone, Component, Reflect)]
#[relationship_target(relationship = ControlledBy)]
pub struct Controls(Vec<Entity>);

/// attatched to lights controlled by switches
#[derive(Debug, Clone, Component, Reflect)]
#[relationship(relationship_target = Controls)]
pub struct ControlledBy(Entity);

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct DehydratedController {
    reference: EntityPointer,
    this_is_the_controller: bool
}

impl DehydratedController {
    fn hydrate(
        resolve: EntityResolver,
        query: Query<(Entity, &DehydratedController), Added<DehydratedController>>,
        mut commands: Commands,
    ) -> Result {
        for (entity, item) in query.iter() {
            let target = resolve.resolve(entity, &item.reference)?;
            if item.this_is_the_controller {
                commands.entity(target).insert(ControlledBy(entity));
            }else{
                commands.entity(entity).insert(ControlledBy(target));
            }
        }
        Ok(())
    }
}


#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component)]
pub struct ToggleInteractionState(bool);

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component)]
#[require(Interactable, ToggleInteractionState)]
pub struct ToggleInteractionBlender {
    on_animation: String,
    off_animation: String,
    on_sfx: Option<String>,
    off_sfx: Option<String>,
}

impl ToggleInteractionBlender {
    fn handle_animation_done(mut query: Query<(AnimationParams, &mut Interactable)>) {
        for (ref animated, mut state) in query.iter_mut() {
            if animated.player.all_finished() {
                // TODO what if there is an idle animation
                state.active = false;
            }
        }
    }

    /// make sure initial light state matches FlipVisible.vis
    fn flip_init(        
        event: Trigger<OnAdd, ControlledBy>,
        mut vis: Query<(&mut Visibility, &ControlledBy)>,
        flip: Query<(&FlipVisible, &ToggleInteractionState)>,
    ){
        let Ok((mut vis, controlled_by)) = vis.get_mut(event.target()) else { return; };
        if let Ok((flip, _state)) = flip.get(controlled_by.0){
            *vis = match flip.vis {
                false => Visibility::Hidden,
                true => Visibility::Inherited,
            }
        }
    }

    fn handle_flip(
        // NOTE vecs appear broken in blenvy so I can't add AnimationMarkers
        // TODO revert this back to a component on the breaker
        event: Trigger<Interaction>,
        query: Query<(&FlipVisible, &ToggleInteractionState, &Controls)>,
        mut target: Query<&mut Visibility>,
    ) {
        let flip = query.get(event.target());
        if let Ok((flip, _state, controls)) = flip {
            dbg!(event.event(), &flip);
            for c in controls.iter() {
                let Ok(mut vis) = target.get_mut(c) else {continue};
                // TODO deal with state

                if *vis == Visibility::Hidden {
                    *vis = Visibility::Inherited;
                } else {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }

    fn observer(
        ev: Trigger<Interaction>,
        mut commands: Commands,
        asset_server: ResMut<AssetServer>,
        mut query: Query<(&Self, &mut ToggleInteractionState, &mut Interactable)>,
        mut uids: Query<&Uid>,
        mut animate: Query<AnimationParams>,
    ) {
        let Ok((conf, mut state, mut interactable)) = query.get_mut(ev.target()) else {
            return;
        };
        state.0 = !state.0;
        debug!("{} toggle {}", ev.target(), state.0);

        let Ok(mut animate) = animate.get_mut(ev.target()) else {
            error!(entity = %ev.target(), "AnimationContext or required missing");
            return;
        };

        let anim_name = match state.0 {
            true => conf.on_animation.as_str(),
            false => conf.off_animation.as_str(),
        };

        debug!("play {}", anim_name);
        interactable.active = true;
        animate.play(anim_name, Duration::ZERO);

        let sfx = match state.0 {
            true => &conf.on_sfx,
            false => &conf.off_sfx,
        };

        // TODO make sound part of animation framework
        if let Some(mut s) = sfx.clone() {
            if !s.contains("/") {
                s.insert_str(0, "sounds/".into());
            }
            //NOTE originally this attached AudioPlayer to entity instead of spawning a child but for some reason that prevented the sound from being overwritten before it was finished.
            commands.entity(ev.target()).with_child((
                AudioPlayer::<AudioSource>(asset_server.load(s)), //TODO preload
                PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Despawn,
                    volume: Volume::Linear(0.7),
                    ..Default::default()
                },
            ));
        }
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct InteractionText {
    pub string: String,
    pub show: String,
}
impl InteractionText {
    fn flip() -> Self {
        Self {
            string: "pick up".to_string(),
            show: "p--- --".to_string(),
        }
    }
}
//TODO IntereactionText should be required for tooltip based interaction
impl Default for InteractionText {
    fn default() -> Self {
        Self {
            string: "i".to_string(),
            show: "[i]nteract".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component)]
#[require(super::network::uid::UidGen)] //FIXME
pub struct Interactable {
    /// is unfinished interaction occuring
    pub active: bool,
}

#[derive(Debug, Clone, Event, Reflect, Serialize, Deserialize)]
pub struct Interaction;

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct Hover(HitData);

// fn on_hover_debug(ev: Trigger<Pointer<Over>>, item: Query<&Name>) {
//     if let Ok(name) = item.get(ev.entity()){
//         dbg!(name);
//     }
// }

fn on_over(
    mut ev: Trigger<Pointer<Over>>,
    item: Query<&Name, With<Interactable>>,
    mut commands: Commands,
) {
    trace!(entity = %ev.target(), "on");
    ev.propagate(true);
    if let Ok(name) = item.get(ev.target()) {
        debug!("Over: {}", name);
        commands.entity(ev.target()).insert(Hover(ev.hit.clone()));
    }
}

// NOTE example of utility of runtime system disabling for debug
fn on_out(mut ev: Trigger<Pointer<Out>>, item: Query<&Name, With<Hover>>, mut commands: Commands) {
    trace!(entity = %ev.target(), "out");
    ev.propagate(true);
    if let Ok(name) = item.get(ev.target()) {
        debug!("Out: {}", name);
        commands.entity(ev.target()).remove::<Hover>();
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct Tooltip {
    target: Entity,
    extra_frames: Option<Instant>,
    text: String,
}

fn display_tooltip(
    mut commands: Commands,
    item: Query<
        (
            Entity,
            &Hover,
            &Interactable,
            Option<&InteractionText>,
            Option<&Aabb>,
            &GlobalTransform,
            Option<&Name>,
        ),
        Without<Held>,
    >, //TODO really should use the active field of the interaction, or remove interactable
    mut tooltip: Query<(Entity, &Tooltip, &mut Node, &ComputedNode, &mut Visibility)>,
    camera: Query<(&Camera, &GlobalTransform)>, //XXX GlobalTransform will have 1 frame delay, unfortionately
    window: Query<&Window, With<PrimaryWindow>>,
) {
    for t in tooltip.iter() {
        let target = t.1.target;
        if !item.get(target).is_ok_and(|item| !item.2.active) {
            debug!(tooltip = %t.0, entity = %target, "despawn");
            commands.entity(t.0).despawn();
        }
    }

    for item in item.iter().map(frunk::into_generic) {
        if Interactable::of(&item).active {
            continue;
        }

        let item_pos = GlobalTransform::of(&item);
        // let center = Aabb::option(&item).map(|a| a.center).unwrap_or_default();
        // let item_pos = item_pos.transform_point(center.into());
        let (camera, camera_transform) = camera.get(Hover::of(&item).0.camera).unwrap(); //NOTE another example of somewhere where the unhappy path should be pluggable with panic as default
        let pos = camera
            .world_to_ndc(camera_transform, item_pos.compute_transform().translation)
            .unwrap();

        let entity: &Entity = item.get();

        if let Some(mut t) = tooltip.iter_mut().find(|t| t.1.target == *entity) {
            *t.4 = Visibility::Visible;

            let size = t.3.size() / window.single().unwrap().scale_factor();
            let mut res = window.single().unwrap().size();

            if let Some(v) = &camera.viewport {
                // should fix tracking when editor is open.
                res = v.physical_size.as_vec2() / window.single().unwrap().scale_factor();
            }

            let xy = res * ((pos + 1.) * 0.5).xy();

            // dbg!(size, pos, window.single().scale_factor(), window.single().size());

            let pos = xy - size / 2.0;
            // let pos = pos / 2.0; //XXX WHY!

            t.2.left = Val::Px(pos.x);
            t.2.bottom = Val::Px(pos.y);
        } else {
            let text = InteractionText::option(&item).cloned().unwrap_or_default();

            // dbg!(&pos, Name::option(&item));
            trace!(name = %Name::option(&item).map(|n|n.as_str()).unwrap_or_default(), ?pos, "tooltip");
            commands.spawn((
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    overflow: Overflow::visible(),
                    // border: UiRect::all(Val::Px(10.0)),
                    // align_items: AlignItems::Center,
                    // justify_content: JustifyContent::Center,
                    ..default()
                },
                Text::new(text.show),
                TextFont::default().with_font_size(40.0),
                // TODO scaling based on ui size?
                //      actually we need world sized tooltips, they should get bigger as you get closer.
                //      we also need interaction distance.
                //      but before any of that, we ought to try to get avian-based collider picking working
                // https://github.com/blaind/bevy_text_mesh rendered to a top layer
                // TODO MONOSPACE, choose font for corngame
                BackgroundColor(Color::srgba(0.5, 0.5, 0.5, 0.5)),
                TextColor(Color::srgba(0.05, 0.05, 0.05, 0.5)),
                // BorderColor(Color::srgba(0.05, 0.05, 0.05, 0.9)),
                Outline::new(
                    Val::Px(4.0),
                    Val::Px(0.0),
                    Color::srgba(0.05, 0.05, 0.05, 0.9),
                ),
                // looks cool:
                // BackgroundColor(Color::srgba(1.0, 0.5, 0.5, 0.5)).ease_to(
                //     BackgroundColor(Color::srgba(0.15, 1.0, 0.15, 1.0)),
                //     bevy_easings::EaseFunction::QuadraticIn,
                //     bevy_easings::EasingType::Once {
                //         duration: std::time::Duration::from_secs_f32(1.0),
                //     },
                // ),
                Tooltip {
                    target: *entity,
                    text: "".to_string(),
                    extra_frames: None,
                },
                //TODO Easing / tweening
            ));
        }
    }
}

fn handle_key(
    mut keyboard: EventReader<KeyboardInput>,
    hover: Query<(Entity, &Interactable, &Hover, Option<&InteractionText>)>,
    mut tooltip: Query<(Entity, &mut Tooltip, &mut Text)>,
    mut commands: Commands,

    // FIXME
    mut sender: Query<&mut MessageSender<InteractionMessage>, With<Client>>,
    state: Query<&ToggleInteractionState>,
    uid: Query<&Uid>,
) {
    for h in hover.iter() {
        if let Some((id, mut tooltip, mut text)) = tooltip.iter_mut().find(|t| t.1.target == h.0) {
            if let Some(start) = tooltip.extra_frames {
                // then tooltip was already triggered, leave it visible a few frames and then despawn
                if start.elapsed().as_millis() > 10000 {
                    commands.entity(id).despawn();
                }
            }

            'outer: for k in keyboard.read() {
                if k.state.is_pressed() && !k.repeat {
                    match &k.logical_key {
                        Key::Character(s) => {
                            let s = s.as_str();
                            if !s.chars().all(|c| c.is_alphabetic()) {
                                continue 'outer;
                            }

                            if "wasd".contains(s) && tooltip.text.is_empty() {
                                continue 'outer;
                            }

                            tooltip.text += s;
                        }
                        Key::Escape => {
                            tooltip.text.clear();
                        }
                        Key::Backspace => {
                            tooltip.text.pop();
                        }
                        _ => break,
                    }

                    let conf = h.3.cloned().unwrap_or_default();
                    let i = tooltip.text.len();
                    if conf.string.get(i..i + 1) == Some(" ") {
                        // support strings with spaces in them, even though we don't type the space
                        tooltip.text += " ";
                    }

                    text.0 = tooltip.text.clone();

                    // trigger event
                    // TODO disable tooltip during animation.
                    if tooltip.text == conf.string {
                        // Send network message FIXME
                        if let Ok(mut net) = sender.single_mut() {
                            net.send::<ActionsChannel>(InteractionMessage {
                                uid: uid.get(h.0).unwrap().clone(),
                                state: !state.get(h.0).map(|a|a.0).unwrap_or_default(), //XXX  might not have ToggleState
                            });
                        } else {
                            error!("No MessageSender found for InteractionMessage");
                        }

                        commands.trigger_targets(Interaction, h.0);
                        tooltip.extra_frames = Some(Instant::now());
                        commands
                            .entity(id)
                            .insert(BackgroundColor(Color::srgba(0.5, 0.5, 0.5, 0.0)))
                            .remove::<Outline>();

                        // TODO need 0.16 for bugfix
                        // commands.entity(id).insert(
                        //     TextColor(Color::srgba(0.05, 0.05, 0.05, 0.5)).ease_to(
                        //         TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                        //         EaseMethod::Linear,
                        //         EasingType::Once {
                        //             duration: Duration::from_millis(5000),
                        //         },
                        //     ),
                        // );
                        return;
                    }

                    let i = text.0.len();
                    if i < conf.show.len() {
                        text.0 += &conf.show[i..]; //TODO greyout suggestions
                    }
                    if i > conf.show.len() {
                        let i = conf.show.len();
                        text.0 = text.0[..i].to_string() // are there really no ergonomic string manipulation fns in rust?
                    }

                    if tooltip.text.len() == 0 {
                        commands
                            .entity(id)
                            .insert(TextColor(Color::srgba(0.05, 0.05, 0.05, 0.5)));
                    } else {
                        commands
                            .entity(id)
                            .insert(TextColor(Color::srgba(0.05, 0.05, 0.05, 0.9)));
                    }
                }
            }
        }
    }
}

pub trait Of<'a>: Sized {
    fn of<I, T: frunk::hlist::Selector<&'a Self, I>>(src: &T) -> &'a Self;

    fn of_mut<I, H: frunk::hlist::Selector<&'a mut Self, I>>(src: &'a mut H) -> &'a mut Self;

    fn option<I, T: frunk::hlist::Selector<Option<&'a Self>, I>>(src: &T) -> Option<&'a Self>;
}

impl<'a, T> Of<'a> for T
where
    T: Component,
{
    fn of<I, H: frunk::hlist::Selector<&'a Self, I>>(src: &H) -> &'a Self {
        src.get()
    }
    // fn option<I, H: frunk::hlist::Selector<&'a Self, I>>(src: &H) -> &'a Self {
    //     src.get()
    // }
    fn of_mut<I, H: frunk::hlist::Selector<&'a mut Self, I>>(src: &'a mut H) -> &'a mut Self {
        src.get_mut()
    }

    fn option<I, H: frunk::hlist::Selector<Option<&'a Self>, I>>(src: &H) -> Option<&'a Self> {
        let a: &Option<&Self> = src.get();
        a.to_owned()
    }
}

#[derive(Debug, Clone, Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct DebugForMissingReflect {
    pub hover: HashMap<Entity, HitData>,
}
impl DebugForMissingReflect {
    fn system(mut this: ResMut<DebugForMissingReflect>, h: Res<HoverMap>) {
        this.hover = h.0.values().flat_map(|v| v.clone()).collect()
    }
}

fn force_pointer_center(
    mut pointer_inputs: Query<(&mut PointerLocation, &PointerId)>,
    window: Single<(Entity, &Window), With<PrimaryWindow>>,
    camera: Single<&Camera, With<MainCamera>>, // or Camera2d, or a marker
    // frame: Res<FrameCount>
) {
    let (w_e, window) = *window;
    if window.cursor_options.grab_mode == CursorGrabMode::Locked {
        let center = if let Some(viewport) = &camera.viewport {
            // viewport physical position + half its size
            let vp_pos = viewport.physical_position;
            let vp_size = viewport.physical_size;
            Vec2::new(
                (vp_pos.x + vp_size.x / 2) as f32,
                (vp_pos.y + vp_size.y / 2) as f32,
            ) / window.scale_factor()
        } else {
            // no viewport, fallback to full window
            Vec2::new(window.width() / 2.0, window.height() / 2.0)
        };

        // window.set_cursor_position(Some(center));
        if let Some(mut loc) = pointer_inputs.iter_mut().find(|a| *a.1 == PointerId::Mouse) {
            // println!("{} {}", frame.0, &center);
            loc.0.location = Some(Location{
                target: RenderTarget::Window(bevy::window::WindowRef::Primary).normalize(Some(w_e)).unwrap(),
                position: center
            });
        }
    }
}

/// a system to fix locked cursor bug 
/// TODO support soft cursor, for things like menu
fn center_cursor_on_camera_viewport(
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    camera: Single<&Camera, With<MainCamera>>, // or Camera2d, or a marker
) {
    if window.cursor_options.grab_mode == CursorGrabMode::Locked {
        let center = if let Some(viewport) = &camera.viewport {
            // viewport physical position + half its size
            let vp_pos = viewport.physical_position;
            let vp_size = viewport.physical_size;
            Vec2::new(
                (vp_pos.x + vp_size.x / 2) as f32,
                (vp_pos.y + vp_size.y / 2) as f32,
            ) / window.scale_factor()
        } else {
            // no viewport, fallback to full window
            Vec2::new(window.width() / 2.0, window.height() / 2.0)
        };

        window.set_cursor_position(Some(center));
    }
}

// trait WhyMustIWriteIt {
//     fn get<T>(&self) -> &T;
// }

// macro_rules! impl_why {
//     ($($T:ident),*) => {
//         impl<$($T),*> Of for ($($T,)*) {
//             fn of<I, T:frunk::hlist::Selector<Self, I>>(src: &T) -> &Self {
//                 src.get()
//             }
//         }
//     };
// }
// all_tuples!(impl_why,0,15, T);
