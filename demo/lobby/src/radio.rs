//! CODEX — GPT-6.1: radio controls and visual knobs; scene configuration lives in radio.corn.
use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    text::{FontSize, FontSource},
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use crate::{computer::ComputerSession, controller::Player, hold::Held, interaction::InteractionPrompt};

// CODEX: Audio and placement remain isolated from interaction and the adjustable dial.
#[path = "radio_audio.rs"]
mod audio;
#[path = "radio_placement.rs"]
mod placement;
// CODEX: Pickup and tuning orientation share one transition, separate from audio and placement.
#[path = "radio_pickup.rs"]
mod pickup;
// CODEX — GPT-6: Independent marker geometry defines linear reception between offset ellipses.
#[path = "radio_region.rs"]
mod region;
// CODEX: Optional regional attenuation overlay, with no additional UI framework.
#[path = "radio_dev_ui.rs"]
mod dev_ui;

// CODEX: A normal broadcast is global; regional broadcasts use RadioSignal on Blender empties.
#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default)]
pub struct RadioStation {
    pub frequency: f32,
    pub sound: String,
    pub gain: f32,
}

// CODEX: Reflected knobs make dial, mixing, volume and direction tunable without recompiling.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
pub struct Radio {
    pub on: bool,
    pub tuning: f32,
    pub stations: Vec<RadioStation>,
    pub static_sound: String,
    pub volume: f32,
    /// CODEX (GPT-6): Shared level for normal stations and regional broadcasts, separate from hiss.
    pub broadcast_gain: f32,
    pub static_gain: f32,
    pub station_band: f32,
    /// CODEX (GPT-6): Proximity thresholds [starts acquiring, full gain], independent of hiss.
    pub acquisition_thresholds: Vec2,
    /// CODEX (GPT-6): Proximity thresholds [starts clearing, fully clean]; wider clean plateau at lower Y.
    pub clarity_thresholds: Vec2,
    pub fade_seconds: f32,
    pub knob_degrees: f32,
    pub viewport_sweep: f32,
    /// CODEX: Source model axes: Blender -Y faces forward; convert once to glTF +Z in audio.
    pub source_forward: Vec3,
    pub rear_gain: f32,
    pub distance_reference: f32,
    pub audio_range: f32,
    /// CODEX: Only pickups tilted farther than this from world up are corrected (also upside down).
    pub pickup_tilt_degrees: f32,
    /// CODEX: Duration of the deliberate upright-and-face-player rotation on pickup or tuning.
    pub pickup_turn_seconds: f32,
    /// CLAUDE: Latched by `near_light_unlock`; shows every `RadioNearLight`. Networked and reset by network.corn.
    pub near_light: bool,
}

// CODEX: The radio is already playing the centered station when it first appears.
impl Default for Radio {
    fn default() -> Self {
        Self {
            on: true,
            tuning: 0.5,
            stations: Vec::new(),
            static_sound: "sounds/radio/static.ogg".into(),
            // CODEX (GPT-6): Fixed headroom for additive station/secret/hiss peaks; never duck by reception.
            volume: 0.5,
            broadcast_gain: 1.0,
            static_gain: 0.55,
            station_band: 0.05,
            acquisition_thresholds: Vec2::new(0.10, 0.45),
            clarity_thresholds: Vec2::new(0.25, 0.75),
            fade_seconds: 0.09,
            knob_degrees: 150.0,
            viewport_sweep: 0.95,
            source_forward: Vec3::NEG_Y,
            rear_gain: 0.3,
            distance_reference: 3.0,
            audio_range: 25.0,
            pickup_tilt_degrees: 50.0,
            pickup_turn_seconds: 0.5,
            near_light: false,
        }
    }
}

// CODEX: The root's full transform defines the inner ellipsoid; its child defines the outer boundary.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct RadioSignal {
    pub frequency: f32,
    pub sound: String,
    pub gain: f32,
}
// CODEX: Attach only to the circle parented under its corresponding RadioSignal empty.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct RadioFalloff;

// CLAUDE: Hidden until a radio stays tuned within `tolerance` of `frequency` for `seconds` in a row.
// Put it on the light's node; its Visibility is managed here.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
pub struct RadioNearLight {
    pub frequency: f32,
    pub tolerance: f32,
    pub seconds: f32,
}
impl Default for RadioNearLight {
    fn default() -> Self {
        Self { frequency: 0.9, tolerance: 0.038, seconds: 5.0 }
    }
}

// CODEX: Shared input ownership prevents the mouse from also moving the player's camera.
#[derive(Resource, Reflect, Default)]
#[reflect(Resource, Default)]
pub struct RadioTuning {
    pub entity: Option<Entity>,
    #[reflect(ignore)]
    previous_grab: Option<CursorGrabMode>,
    #[reflect(ignore)]
    previous_visible: bool,
}

// CODEX: Preserve each knob's exported rest pose instead of replacing its rotation basis.
#[derive(Component)]
struct KnobRest(Quat);
// CODEX: Separate UI gives the primary F action and smaller E/T/R controls a stable layout.
#[derive(Component)]
struct RadioHint(u8);

// CODEX: The generic Unseen event owns randomization, not special camera logic in the radio.
pub fn plugin(app: &mut App) {
    app.init_resource::<RadioTuning>()
        .register_type::<RadioTuning>()
        .register_type::<RadioStation>()
        .register_type::<Radio>()
        .register_type::<RadioSignal>()
        .register_type::<RadioFalloff>()
        .register_type::<RadioNearLight>()
        .add_plugins((audio::plugin, placement::plugin, pickup::plugin, dev_ui::plugin))
        .add_observer(randomize_hidden_knob)
        .add_systems(
            Update,
            input
                .after(crate::interaction::update_prompt)
                .before(crate::computer::update_session),
        )
        .add_systems(Update, knobs.after(input))
        .add_systems(Update, (near_light_unlock.after(input), show_near_light).chain())
        .add_systems(
            PostUpdate,
            hints
                .after(crate::post_it::editor_prompt)
                .before(bevy::ui::UiSystems::Prepare),
        );
}

// CODEX: F works held or placed. T toggles mouse tuning; T/Escape/pause releases it.
fn input(
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    prompt: Res<InteractionPrompt>,
    computer: Res<ComputerSession>,
    paper: Res<crate::post_it::PostItSession>,
    held: Query<Entity, With<Held>>,
    mut radios: Query<&mut Radio>,
    mut tuning: ResMut<RadioTuning>,
    mut window: Single<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut commands: Commands,
) {
    let playing = crate::browser::captured()
        && computer.active.is_none()
        && !computer.returning
        && !paper.blocks_input();
    let target = held
        .iter()
        .find(|e| radios.contains(*e))
        .or_else(|| prompt.target.filter(|e| radios.contains(*e)));
    let previous = tuning.entity;
    if !playing
        || tuning.entity.is_some_and(|e| !radios.contains(e))
        || keys.just_pressed(KeyCode::Escape)
    {
        tuning.entity = None;
    } else if keys.just_pressed(KeyCode::KeyT) {
        tuning.entity = if tuning.entity.is_some() {
            None
        } else {
            target
        };
    }
    if previous.is_none() && tuning.entity.is_some() {
        // CODEX: Placed radios remain tunable, but only held radios turn to face the player.
        if let Some(entity) = tuning.entity.filter(|e| held.contains(*e)) {
            commands.trigger(pickup::FaceRadio { entity });
        }
        tuning.previous_grab = Some(window.1.grab_mode);
        tuning.previous_visible = window.1.visible;
        window.1.grab_mode = CursorGrabMode::Locked;
        window.1.visible = false;
    } else if previous.is_some() && tuning.entity.is_none() {
        // CLAUDE: on web the DOM owns pointer lock and the window flag may read None while locked;
        // restoring None would exit the lock and raise the pause menu, so keep Locked there.
        let previous_grab = tuning.previous_grab.take();
        window.1.grab_mode = if cfg!(target_arch = "wasm32") && crate::browser::captured() {
            CursorGrabMode::Locked
        } else {
            previous_grab.unwrap_or(CursorGrabMode::Locked)
        };
        window.1.visible = tuning.previous_visible;
    }
    if !playing {
        return;
    }
    if let Some(entity) = tuning.entity.or(target) {
        if let Ok(mut radio) = radios.get_mut(entity) {
            if keys.just_pressed(KeyCode::KeyF) {
                radio.on = !radio.on;
                commands.trigger(crate::foley::Switched { entity, on: radio.on });
            }
            if tuning.entity == Some(entity) {
                radio.tuning = (radio.tuning
                    + motion.delta.x / (window.0.width() * radio.viewport_sweep).max(1.0))
                .clamp(0.0, 1.0);
            }
        }
    }
}

// CLAUDE: Each client times the (networked) dial itself; any one latching it is enough.
// TODO: revisit networking: every client latching it makes each one claim the radio (network.corn
// pendingRadio) at once; one authority (the radio's owner or the server) should decide it.
fn near_light_unlock(
    time: Res<Time>,
    lights: Query<&RadioNearLight>,
    mut radios: Query<(Entity, &mut Radio)>,
    mut held_for: Local<bevy::platform::collections::HashMap<Entity, f32>>,
) {
    for (entity, mut radio) in &mut radios {
        let timer = held_for.entry(entity).or_default();
        let light = lights
            .iter()
            .find(|l| (radio.tuning - l.frequency).abs() <= l.tolerance);
        let Some(light) = light.filter(|_| !radio.near_light) else {
            *timer = 0.0;
            continue;
        };
        *timer += time.delta_secs();
        if *timer >= light.seconds {
            radio.near_light = true;
        }
    }
}

// CLAUDE: Follows the radio's flag both ways, so a reset turns the light off again.
fn show_near_light(radios: Query<&Radio>, mut lights: Query<&mut Visibility, With<RadioNearLight>>) {
    let lit = radios.iter().any(|r| r.near_light);
    let wanted = if lit { Visibility::Inherited } else { Visibility::Hidden };
    for mut visibility in &mut lights {
        visibility.set_if_neq(wanted);
    }
}

// CODEX: Both pivots retain their base orientation; only the tuning knob tracks frequency.
fn knobs(
    radios: Query<(Entity, &Radio)>,
    children: Query<&Children>,
    mut parts: Query<(Entity, &Name, &mut Transform, Option<&KnobRest>)>,
    mut commands: Commands,
) {
    for (root, radio) in &radios {
        for part in children.iter_descendants(root) {
            let Ok((entity, name, mut transform, rest)) = parts.get_mut(part) else {
                continue;
            };
            if name.as_str() != "KNOB" && name.as_str() != "KNOB2" {
                continue;
            }
            let base = rest.map_or(transform.rotation, |r| r.0);
            if rest.is_none() {
                commands.entity(entity).insert(KnobRest(base));
            }
            if name.as_str() == "KNOB" {
                transform.rotation = base
                    * Quat::from_rotation_y(
                        (1.0 - radio.tuning * 2.0) * radio.knob_degrees.to_radians(),
                    );
            }
        }
    }
}

// CODEX: Exactly once per seen-to-unseen transition, using time/entity entropy without a new RNG crate.
fn randomize_hidden_knob(
    on: On<crate::seen::Unseen>,
    radios: Query<&Radio>,
    children: Query<&Children>,
    mut parts: Query<(&Name, &mut Transform, Option<&KnobRest>)>,
    time: Res<Time>,
) {
    let Ok(radio) = radios.get(on.entity) else {
        return;
    };
    let mut hash =
        (time.elapsed_secs_f64().to_bits() ^ on.entity.to_bits()).wrapping_mul(0x9E3779B97F4A7C15);
    hash ^= hash >> 29;
    let unit = (hash >> 40) as f32 / (1u32 << 24) as f32;
    for part in children.iter_descendants(on.entity) {
        if let Ok((name, mut transform, rest)) = parts.get_mut(part) {
            if name.as_str() == "KNOB2" {
                let base = rest.map_or(Quat::IDENTITY, |r| r.0);
                transform.rotation = base
                    * Quat::from_rotation_y((unit * 2.0 - 1.0) * radio.knob_degrees.to_radians());
            }
        }
    }
}

// CODEX: Primary F action stays large; pickup/turn/drop are a smaller second line underneath.
fn hints(
    mut commands: Commands,
    assets: Res<AssetServer>,
    players: Query<Entity, With<Player>>,
    prompt: Res<InteractionPrompt>,
    tuning: Res<RadioTuning>,
    computer: Res<ComputerSession>,
    paper: Res<crate::post_it::PostItSession>,
    radios: Query<&Radio>,
    held: Query<Entity, With<Held>>,
    mut preload: ResMut<audio::RadioAudioPreload>,
    mut labels: Query<(&RadioHint, &mut Text, &mut Visibility)>,
    mut ordinary: Query<
        &mut Visibility,
        (
            Or<(
                With<crate::interaction::ActionPrompt>,
                With<crate::interaction::DropHint>,
            )>,
            Without<RadioHint>,
        ),
    >,
) {
    let held_radio = held.iter().find(|e| radios.contains(*e));
    let target = tuning
        .entity
        .or(held_radio)
        .or_else(|| prompt.target.filter(|e| radios.contains(*e)));
    let active = crate::browser::captured()
        && computer.active.is_none()
        && !computer.returning
        && !paper.blocks_input();
    if labels.is_empty() {
        let Some(camera) = players.iter().next() else {
            return;
        };
        let font = FontSource::Handle(assets.load("fonts/Gelasio-Italic.ttf"));
        // CODEX: radio uses the same primary/secondary sizes and spacing as every other prop.
        for (line, size, bottom) in [
            (
                0,
                crate::interaction::PRIMARY_HINT_SIZE,
                crate::interaction::PRIMARY_HINT_BOTTOM,
            ),
            (
                1,
                crate::interaction::SECONDARY_HINT_SIZE,
                crate::interaction::SECONDARY_HINT_BOTTOM,
            ),
        ] {
            commands.spawn((
                Name::new(format!("Radio controls {line}")),
                RadioHint(line),
                crate::interaction::HudSize { font: size, bottom: Some(bottom) },
                UiTargetCamera(camera),
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(size),
                    ..default()
                },
                TextColor(crate::interaction::INK),
                TextShadow::default(),
                TextLayout::justify(Justify::Center),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    bottom: Val::Px(bottom),
                    ..default()
                },
                Visibility::Hidden,
                Pickable::IGNORE,
            ));
        }
    }
    let radio = target.filter(|_| active).and_then(|e| radios.get(e).ok());
    for (hint, mut text, mut visibility) in &mut labels {
        *visibility = if radio.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if let Some(radio) = radio {
            if hint.0 == 0 {
                text.0 = format!("press F to turn {}", if radio.on { "off" } else { "on" });
            } else {
                // CODEX (GPT-6): Latch only when the secondary tuning controls are shown to the player.
                preload.unlocked = true;
                text.0 = if tuning.entity.is_some() {
                    "move mouse to tune · press T to finish".into()
                } else if held_radio.is_some() {
                    "press T to turn · press R to drop".into()
                } else {
                    "press E to pick up · press T to turn".into()
                };
            }
        }
    }
    if radio.is_some() {
        for mut visibility in &mut ordinary {
            *visibility = Visibility::Hidden;
        }
    }
}

// CODEX: Exercise the actual input system without a renderer or audio device.
#[cfg(test)]
mod tests {
    use super::*;
    // CLAUDE: 5 s in the band in a row lights it; leaving the band starts the count again.
    #[test]
    fn near_light_needs_five_consecutive_seconds_in_band() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs(1),
            ))
            .add_systems(Update, (near_light_unlock, show_near_light).chain());
        // Virtual time clamps each step to 0.25 s by default.
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_max_delta(std::time::Duration::from_secs(2));
        let light = app
            .world_mut()
            .spawn((RadioNearLight::default(), Visibility::Hidden))
            .id();
        let radio = app.world_mut().spawn(Radio { tuning: 0.93, ..default() }).id();
        let set = |app: &mut App, tuning: f32| app.world_mut().get_mut::<Radio>(radio).unwrap().tuning = tuning;
        let lit = |app: &App| app.world().get::<Radio>(radio).unwrap().near_light;
        for _ in 0..4 {
            app.update();
        }
        assert!(!lit(&app));
        set(&mut app, 0.95);
        app.update();
        set(&mut app, 0.87);
        for _ in 0..4 {
            app.update();
        }
        assert!(!lit(&app));
        app.update();
        assert!(lit(&app));
        assert_eq!(app.world().get::<Visibility>(light), Some(&Visibility::Inherited));
        set(&mut app, 0.5);
        app.update();
        assert!(lit(&app), "latched until a reset");
        app.world_mut().get_mut::<Radio>(radio).unwrap().near_light = false;
        app.update();
        assert_eq!(app.world().get::<Visibility>(light), Some(&Visibility::Hidden));
    }
    // CODEX: Viewed from the exported front (+Z), the marker moves clockwise as tuning increases.
    #[test]
    fn dial_turns_clockwise_from_front_with_increasing_frequency() {
        let mut app = App::new();
        app.add_systems(Update, knobs);
        let rest = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let knob = app
            .world_mut()
            .spawn((Name::new("KNOB"), Transform::from_rotation(rest)))
            .id();
        let radio = app.world_mut().spawn(Radio::default()).add_child(knob).id();
        for tuning in [0.0, 0.25, 0.5, 0.75, 1.0] {
            app.world_mut().get_mut::<Radio>(radio).unwrap().tuning = tuning;
            app.update();
            let rotation = app.world().get::<Transform>(knob).unwrap().rotation;
            // CODEX: At center a reference marker points right; clockwise turns move it downward.
            let marker = rotation * Vec3::X;
            let expected_angle = (tuning * 2.0 - 1.0) * 150.0_f32.to_radians();
            assert!(
                marker.distance(Vec3::new(expected_angle.cos(), -expected_angle.sin(), 0.0)) < 1e-5
            );
        }
    }

    // CODEX: Full-width motion covers the dial, limits clamp, F preserves tuning, and T restores capture.
    #[test]
    fn tuning_owns_mouse_and_power_preserves_frequency() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<AccumulatedMouseMotion>()
            .init_resource::<InteractionPrompt>()
            .init_resource::<ComputerSession>()
            .init_resource::<crate::post_it::PostItSession>()
            .init_resource::<RadioTuning>()
            .init_resource::<Time>()
            .add_plugins(pickup::plugin)
            .add_systems(Update, input);
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: (1000, 800).into(),
                    ..default()
                },
                PrimaryWindow,
                CursorOptions {
                    grab_mode: CursorGrabMode::None,
                    visible: true,
                    ..default()
                },
            ))
            .id();
        app.world_mut().spawn((
            Player::default(),
            GlobalTransform::from_translation(Vec3::Z),
        ));
        let radio = app
            .world_mut()
            .spawn((Radio::default(), Transform::default()))
            .id();
        app.world_mut().resource_mut::<InteractionPrompt>().target = Some(radio);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyT);
        app.update();
        assert_eq!(app.world().resource::<RadioTuning>().entity, Some(radio));
        assert!(
            app.world()
                .get::<crate::hold::HeldRotation>(radio)
                .is_none(),
            "entering T tuning on a placed radio must preserve its orientation"
        );
        assert_eq!(
            app.world().get::<CursorOptions>(window).unwrap().grab_mode,
            CursorGrabMode::Locked
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta
            .x = 950.0;
        app.update();
        assert_eq!(app.world().get::<Radio>(radio).unwrap().tuning, 1.0);
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta
            .x = -1900.0;
        app.update();
        assert_eq!(app.world().get::<Radio>(radio).unwrap().tuning, 0.0);
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::ZERO;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyF);
        app.update();
        assert!(!app.world().get::<Radio>(radio).unwrap().on);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyF);
        app.update();
        assert_eq!(app.world().get::<Radio>(radio).unwrap().tuning, 0.0);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyT);
        app.update();
        assert_eq!(app.world().resource::<RadioTuning>().entity, None);
        assert_eq!(
            app.world().get::<CursorOptions>(window).unwrap().grab_mode,
            CursorGrabMode::None
        );
        assert!(app.world().get::<CursorOptions>(window).unwrap().visible);
    }
}
