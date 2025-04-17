use std::{str::Chars, time::Duration};

use bevy::{
    ecs::{
        entity,
        query::{self, QueryData},
    },
    input::keyboard::{Key, KeyboardInput},
    picking::backend::HitData,
    prelude::*,
    render::primitives::Aabb,
    text::FontStyle,
    utils::all_tuples, window::PrimaryWindow,
};
use bevy_editor_pls::egui::TextStyle;
use blenvy::{BlueprintAnimationPlayerLink, BlueprintAnimations};
use frunk::{hlist::HList, Generic};

pub struct InteractPlugin;
impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MeshPickingPlugin);
        app.add_observer(on_over);
        app.add_observer(on_out);
        app.add_systems(Update, (display_tooltip, handle_key));
        app.register_type::<Interactable>();
        app.register_type::<ToggleInteractionBlender>();
        app.add_observer(ToggleInteractionBlender::observer);
    }
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component)]
pub struct ToggleInteractionState(bool);

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[require(Interactable, ToggleInteractionState)]
pub struct ToggleInteractionBlender {
    on_animation: String,
    off_animation: String,
}

impl ToggleInteractionBlender {
    fn observer(
        ev: Trigger<Interaction>,
        mut query: Query<(&Self, &mut ToggleInteractionState)>,
        animated: Query<(&BlueprintAnimationPlayerLink, &BlueprintAnimations)>,
        mut animation_players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    ) {
        dbg!(&ev, &query);
        let Ok((conf, mut state)) = query.get_mut(ev.entity()) else {
            return;
        };
        state.0 = !state.0;
        debug!("{} toggle {}", ev.entity(), state.0);

        if let Ok((link, animations)) = animated.get(ev.entity()) {
            dbg!();
            let (mut animation_player, mut animation_transitions) =
                animation_players.get_mut(link.0).unwrap();

            let anim_name = match state.0 {
                true => conf.on_animation.as_str(),
                false => conf.off_animation.as_str(),
            };

            let Some(animation) = animations.named_indices.get(anim_name) else {
                error!("animation {} does not exist for {}", anim_name, link.0);
                return
            };
            animation_transitions
                .play(&mut animation_player, animation.clone(), Duration::from_secs(0));
        }
    }
}

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component)]
pub struct Interactable;

#[derive(Debug, Clone, Event, Reflect)]
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
    ev.propagate(true);
    if let Ok(name) = item.get(ev.entity()) {
        debug!("Over: {}", name);
        commands.entity(ev.entity()).insert(Hover(ev.hit.clone()));
    }
}

// NOTE example of utility of runtime system disabling for debug
fn on_out(ev: Trigger<Pointer<Out>>, item: Query<&Name, With<Hover>>, mut commands: Commands) {
    if let Ok(name) = item.get(ev.entity()) {
        debug!("Out: {}", name);
        commands.entity(ev.entity()).remove::<Hover>();
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct Tooltip {
    target: Entity,
    text: String,
}

fn display_tooltip(
    mut commands: Commands,
    item: Query<(Entity, &Hover, &Aabb, &GlobalTransform, Option<&Name>)>,
    mut tooltip: Query<(Entity, &Tooltip, &mut Node, &ComputedNode, &mut Visibility)>,
    camera: Query<(&Camera, &GlobalTransform)>, //XXX GlobalTransform will have 1 frame delay, unfortionately
    window: Query<&Window, With<PrimaryWindow>>
) {
    for t in tooltip.iter() {
        let target = t.1.target;
        if !item.contains(target) {
            commands.entity(t.0).despawn();
        }
    }

    for item in item.iter().map(frunk::into_generic) {
        let item_pos = GlobalTransform::of(&item);
        let item_pos = item_pos.transform_point(Aabb::of(&item).center.into());
        let (camera, camera_transform) = camera.get(Hover::of(&item).0.camera).unwrap(); //NOTE another example of somewhere where the unhappy path should be pluggable with panic as default
        let pos = camera
            .world_to_ndc(camera_transform, item_pos.into())
            .unwrap();

        let entity: &Entity = item.get();

        if let Some(mut t) = tooltip.iter_mut().find(|t| t.1.target == *entity) {
            *t.4 = Visibility::Visible;

            let size = t.3.size() / window.single().scale_factor();
            
            let res = window.single().size();
            let xy = res * ((pos + 1.) * 0.5).xy();

            dbg!(size, pos, window.single().scale_factor(), window.single().size());

            let pos = xy - size / 2.0;
            let pos = pos / 2.0; //XXX WHY!

            t.2.left = Val::Px(pos.x);
            t.2.bottom = Val::Px(pos.y);
        } else {
            dbg!(&pos, Name::option(&item));
            commands.spawn((
                PickingBehavior::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    overflow: Overflow::visible(),
                    // border: UiRect::all(Val::Px(10.0)),
                    // align_items: AlignItems::Center,
                    // justify_content: JustifyContent::Center,
                    
                    ..default()
                },
                Text::new("f---"),
                // TODO MONOSPACE, choose font for corngame
                BackgroundColor(Color::srgba(0.5, 0.5, 0.5, 0.5)),
                TextColor(Color::srgba(0.05, 0.05, 0.05, 0.5)),
                // BorderColor(Color::srgba(0.05, 0.05, 0.05, 0.9)),
                Outline::new(
                    Val::Px(2.0),
                    Val::Px(0.0),
                    Color::srgba(0.05, 0.05, 0.05, 0.9),
                ),
                // .ease_to(
                //     BackgroundColor(Color::srgba(0.15, 0.15, 0.15, 0.8)),
                //     bevy_easings::EaseFunction::QuadraticIn,
                //     bevy_easings::EasingType::Once {
                //         duration: std::time::Duration::from_secs_f32(0.4),
                //     },
                // ),
                Tooltip {
                    target: *entity,
                    text: "".to_string(),
                },
                //TODO Easing / tweening
            ));
        }
    }
}

fn handle_key(
    mut keyboard: EventReader<KeyboardInput>,
    hover: Query<(Entity, &Interactable, &Hover)>,
    mut tooltip: Query<(Entity, &mut Tooltip, &mut Text)>,
    mut commands: Commands,
) {
    let txt = "flip";
    let pattern = "f---";

    'outer: for k in keyboard.read() { 
        if k.state.is_pressed() && !k.repeat{
            for h in hover.iter() {
                if let Some((id, mut tooltip, mut text)) = tooltip.iter_mut().find(|t|t.1.target == h.0){
                    match &k.logical_key {
                        Key::Character(s) => {
                            let s = s.as_str();
                            if ! s.chars().all(|c|c.is_alphabetic()){
                                break 'outer;
                            }
                            tooltip.text += s;
                        }
                        Key::Backspace => {
                            tooltip.text.pop();
                        },
                        _ => break
                    }

                    // trigger event
                    // TODO disable tooltip during animation.
                    if tooltip.text == txt {
                        commands.trigger_targets(Interaction, h.0);
                        commands.entity(id).despawn();
                        return;
                    }

                    text.0 = tooltip.text.clone();
                    if text.0.len() < pattern.len() {
                        let i = text.0.len();
                        text.0 += &pattern[i..];
                    }
                

                    if tooltip.text.len() == 0 {
                        commands.entity(id).insert(TextColor(Color::srgba(0.05, 0.05, 0.05, 0.5)));
                    }else{
                        commands.entity(id).insert(TextColor(Color::srgba(0.05, 0.05, 0.05, 0.9)));
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
