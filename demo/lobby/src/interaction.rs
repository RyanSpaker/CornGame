//! CLAUDE (claude-opus-5-5), replacing CODEX (GPT-6)'s typed prompt: press E to interact.
//! Two labels: a "press E to …" prompt centred near the bottom of the screen, and an optional name
//! tooltip over the target that types itself in like the three_corn flower demo's known-flower names.
use bevy::{
    camera::{
        primitives::Aabb,
        visibility::{RenderLayers, VisibilitySystems},
        CameraUpdateSystems,
    },
    picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings},
    prelude::*,
    text::{FontSize, FontSource},
    ui::UiSystems,
};
use avian3d::prelude::{Collider, ShapeCastConfig, SpatialQuery, SpatialQueryFilter};
use corn_scripting::ScriptEventsExt;
use crate::{
    computer::ComputerSession,
    controller::Player,
    hold::{Held, Holdable},
};

pub const INTERACT_KEY: KeyCode = KeyCode::KeyE;

// CLAUDE: three_corn/demo/flowers `knownNameDelay` and `knownTextLettersPerSecond` defaults.
pub(crate) const NAME_DELAY: f32 = 0.6;
pub(crate) const NAME_LETTERS_PER_SECOND: f32 = 18.0;
// CODEX (GPT-6): viewport-height limits for label placement; keep tuning beside teletype timing.
const NAME_CENTER_LIMIT: f32 = 0.20;
const NAME_LARGE_OBJECT_HEIGHT: f32 = 0.50;
// CLAUDE: Gelasio is metric-compatible with Georgia, the flower demo's font (Georgia isn't redistributable).
const FONT: &str = "fonts/Gelasio-Italic.ttf";
// CLAUDE: the flower label's colour and shadow.
pub(crate) const INK: Color = Color::srgba(250.0 / 255.0, 246.0 / 255.0, 230.0 / 255.0, 0.92);
// CODEX (GPT-6): shared two-level typography for every bottom control hint, including other modules.
pub(crate) const PRIMARY_HINT_SIZE: f32 = 48.0;
pub(crate) const SECONDARY_HINT_SIZE: f32 = 26.0;
// CODEX: pixel anchors keep a six-pixel gap between the two text rows at any window height.
pub(crate) const SECONDARY_HINT_BOTTOM: f32 = 40.0;
pub(crate) const PRIMARY_HINT_BOTTOM: f32 = SECONDARY_HINT_BOTTOM + SECONDARY_HINT_SIZE * 1.2 + 6.0;
// CODEX: dialogue has its own higher band, separate from the bottom control rows.
const MESSAGE_BOTTOM_PERCENT: f32 = 24.0;
const PROMPT_SIZE: f32 = PRIMARY_HINT_SIZE;
const KEY_SIZE: f32 = PRIMARY_HINT_SIZE;
/// CLAUDE: "press R to drop"; CODEX (GPT-6): also shows the separate F action for held props.
const DROP_HINT_SIZE: f32 = SECONDARY_HINT_SIZE;
/// CLAUDE: the interaction ray (and crosshair) sit this fraction of the screen height below its centre
pub const CURSOR_DROP: f32 = 0.05;
/// CLAUDE: crosshair shows once the cursor is this many screen heights from a reachable target's outline
const CROSSHAIR_NEAR: f32 = 0.1;
const CROSSHAIR_FADE_SECONDS: f32 = 0.7;
pub(crate) const CROSSHAIR_OPACITY: f32 = 0.7;
const CROSSHAIR_COLOR: Color = Color::srgb(0.78, 0.78, 0.78);
/// CLAUDE: an interaction is available: instantly this
pub(crate) const CROSSHAIR_ACTIVE: Color = Color::WHITE;
const CROSSHAIR_IMAGE: &str = "menu/crosshair.png";
/// CLAUDE: percent of the viewport height
const CROSSHAIR_DIAMETER: f32 = 1.4;

// CODEX (GPT-6): reflected placement reasons belong to runtime diagnostics, separate from item config.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TooltipPlacement {
    ColliderBounds,
    Cursor,
    OriginFallback,
}

// CODEX (GPT-6): inserted on the item when its tooltip is displayed; retains the last result while
// hidden. Keep live presentation state out of the scene-authored InteractionText configuration.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component)]
pub struct InteractionTooltipDebug {
    pub placement: TooltipPlacement,
    pub vertically_clamped: bool,
    /// Final screen-space bottom-center anchor in logical pixels.
    pub position: Vec2,
}

// CLAUDE: what aiming at an entity offers, authored in lobby.corn.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
pub struct InteractionText {
    /// identifying tooltip over the target; empty = none
    pub name: String,
    /// shown as "press E to {prompt}". empty: not interactable (scripts set it as things change)
    pub prompt: String,
    /// false: no "press E to …" at the bottom, only the crosshair; E still works
    pub hint: bool,
    /// shown as "press E to {active_prompt}" while this target's computer session is active
    pub active_prompt: String,
    /// name tooltip size
    pub font_size: f32,
    /// CODEX (GPT-6): optional world-space adjustment to the collider bounds (origin if no collider).
    pub name_offset: Vec3,
    /// CODEX (GPT-6): logical pixels between the collider's screen-space top and the text's bottom.
    pub name_gap: f32,
    /// max ray distance to the hit mesh
    pub range: f32,
    /// CLAUDE: also targeted when any part of the cursor (its circle, cast as a cylinder along the ray)
    /// touches its colliders, not just the cursor's centre
    pub cursor_cast: bool,
}

impl Default for InteractionText {
    fn default() -> Self {
        Self {
            name: String::new(),
            prompt: "use".into(),
            hint: true,
            active_prompt: String::new(),
            font_size: 36.0,
            name_offset: Vec3::ZERO,
            name_gap: 12.0,
            range: 2.5,
            cursor_cast: false,
        }
    }
}

/// CLAUDE: E was pressed on `entity` (the one holding [`InteractionText`]), or with nothing aimed at, on
/// the held item that has a [`Holdable::use_prompt`]. Scripts listen with `on Interact: f;`.
#[derive(EntityEvent, Reflect, Clone, Debug)]
pub struct Interact {
    pub entity: Entity,
}

/// CLAUDE: teletype `text` above the action prompt for a few seconds. Scripts: `trigger(null, "Say", { text })`.
#[derive(Event, Reflect, Clone, Debug, Default)]
pub struct Say {
    pub text: String,
    // CODEX (GPT-6): optional per-line hold; helpers tune this in corn without changing door dialogue.
    #[reflect(default)]
    pub hold_seconds: Option<f32>,
}

// CODEX: same teletype and font as Say, with its line at the center of the screen.
#[derive(Event, Reflect, Clone, Debug, Default)]
pub struct SayCentered {
    pub text: String,
}

/// CLAUDE: the current [`Say`] line and how long it has shown
#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
struct Message {
    text: String,
    since: f32,
    // CODEX: resolved from Say's optional override, or MESSAGE_HOLD for existing dialogue.
    hold_seconds: f32,
    // CODEX: keep normal Say placement for existing door/key messages.
    centered: bool,
    // CODEX (GPT-6): scripts wait for the actual typing, hold and fade instead of duplicating timing.
    finished: bool,
}

/// CLAUDE: the [`Say`] label
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct MessageLabel;

// CLAUDE: a message types in at the name tooltip's rate, stays, then fades
const MESSAGE_HOLD: f32 = 2.5;
const MESSAGE_FADE: f32 = 0.6;

// CODEX: the current target and a one-shot activation request consumed by the computer session.
// CLAUDE: reflected for scripts (dev.rs)
#[derive(Resource, Default, Reflect)]
#[reflect(Resource, Default)]
pub struct InteractionPrompt {
    pub target: Option<Entity>,
    /// seconds the current target has been aimed at, drives the name teletype
    pub since: f32,
    pub activate: Option<Entity>,
    /// CLAUDE: pressing E now would do something (a target, or the held item's use)
    pub available: bool,
}

// CODEX (GPT-6): opt-in script telemetry; proximity is attributed to this item, not another prop.
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
pub struct InteractionFeedback {
    pub cursor_visible: bool,
    pub active: bool,
    pub missed_presses: u64,
}

/// CLAUDE: small hollow circle at the interaction cursor, shown while it is on or near a target.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Crosshair {
    /// seconds it has been shown, drives the fade in
    pub shown: f32,
}

/// CLAUDE: the cursor's (crosshair circle's) radius in the world, `distance` along the view
pub fn cursor_radius(camera: &Camera, distance: f32) -> f32 {
    distance * CROSSHAIR_DIAMETER / 100.0 / camera.clip_from_view().y_axis.y.max(1e-3)
}

/// CLAUDE: view-space direction of the interaction ray, CURSOR_DROP of the screen height below centre.
pub fn cursor_view_direction(camera: &Camera) -> Vec3 {
    let point = camera
        .clip_from_view()
        .inverse()
        .project_point3(Vec3::new(0.0, -2.0 * CURSOR_DROP, 0.5));
    // before the first frame's projection is computed
    point.try_normalize().filter(|d| d.z < 0.0).unwrap_or(Vec3::NEG_Z)
}

/// CLAUDE: floating name label over the target.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct InteractionTooltip;

/// CLAUDE: "press E to …", centred near the bottom of the screen. The verb is its last span.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct ActionPrompt;
/// CLAUDE: "press R to drop"
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct DropHint;
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct ActionVerb;
// CODEX: editor can replace the key span while preserving the normal prompt styling.
#[derive(Component)]
pub(crate) struct ActionKey;

// CODEX: targeting runs before focus starts, so pressing E activates in the same frame.
pub fn plugin(app: &mut App) {
    app.register_type::<InteractionText>()
        .register_type::<TooltipPlacement>()
        .register_type::<InteractionTooltipDebug>()
        .register_type::<InteractionFeedback>()
        .register_type::<Message>()
        // CLAUDE: for scripts (dev.rs)
        .register_type::<(MessageLabel, Crosshair, InteractionTooltip, ActionPrompt, ActionVerb, DropHint)>()
        .init_resource::<InteractionPrompt>()
        .init_resource::<Message>()
        .init_resource::<crate::post_it::PostItSession>()
        // CLAUDE: one-off reactions (the door, the keys) are script handlers in lobby.corn
        .register_script_entity_event::<Interact>()
        .register_script_event::<Say>()
        .register_script_event::<SayCentered>()
        .add_observer(|on: On<Say>, mut message: ResMut<Message>| {
            *message = Message {
                text: on.event().text.clone(), since: 0.0, centered: false, finished: false,
                hold_seconds: on.event().hold_seconds.unwrap_or(MESSAGE_HOLD).max(0.0),
            };
        })
        .add_observer(|on: On<SayCentered>, mut message: ResMut<Message>| {
            *message = Message {
                text: on.event().text.clone(), since: 0.0, centered: true, finished: false,
                hold_seconds: MESSAGE_HOLD,
            };
        })
        .add_systems(
            Update,
            (setup_tooltip, update_prompt)
                .chain()
                .before(crate::computer::update_session),
        )
        .add_systems(Update, present_message)
        // CODEX: project the final camera pose before UI measurement/layout and visibility consume it.
        .add_systems(
            PostUpdate,
            present_tooltip
                .after(TransformSystems::Propagate)
                .after(CameraUpdateSystems)
                .before(UiSystems::Prepare)
                .before(VisibilitySystems::VisibilityPropagate),
        )
        .add_systems(
            PostUpdate,
            present_crosshair
                .after(TransformSystems::Propagate)
                .after(CameraUpdateSystems)
                .after(VisibilitySystems::CalculateBounds)
                .after(VisibilitySystems::VisibilityPropagate),
        );
}

// CLAUDE
pub(crate) fn font(assets: &AssetServer, size: f32) -> TextFont {
    TextFont {
        font: FontSource::Handle(assets.load(FONT)),
        font_size: FontSize::Px(size),
        ..default()
    }
}

// CLAUDE
pub(crate) fn shadow() -> TextShadow {
    TextShadow {
        offset: Vec2::new(0.0, 2.0),
        color: Color::BLACK.with_alpha(0.8),
    }
}

fn setup_tooltip(
    mut commands: Commands,
    assets: Res<AssetServer>,
    tooltips: Query<(), With<InteractionTooltip>>,
    players: Query<Entity, With<Player>>,
) {
    if !tooltips.is_empty() {
        return;
    }
    let Some(camera) = players.iter().next() else {
        return;
    };
    commands.spawn((
        Name::new("Interaction tooltip"),
        InteractionTooltip,
        UiTargetCamera(camera),
        Text::new(""),
        font(&assets, 18.0),
        TextColor(INK),
        shadow(),
        Node {
            position_type: PositionType::Absolute,
            overflow: Overflow::visible(),
            ..default()
        },
        // CODEX (GPT-6): keep the entire text line above the bounds, including during teletype/layout.
        UiTransform::from_translation(Val2::percent(-50.0, -100.0)),
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
    commands.spawn((
        Name::new("Action prompt"),
        ActionPrompt,
        UiTargetCamera(camera),
        Text::new("press "),
        font(&assets, PROMPT_SIZE),
        TextColor(INK),
        shadow(),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            bottom: Val::Px(PRIMARY_HINT_BOTTOM),
            ..default()
        },
        Visibility::Hidden,
        Pickable::IGNORE,
        children![
            (ActionKey, TextSpan::new("E"), font(&assets, KEY_SIZE), TextColor(INK), shadow()),
            (ActionVerb, TextSpan::new(""), font(&assets, PROMPT_SIZE), TextColor(INK), shadow()),
        ],
    ));
    // CLAUDE: "press R to drop", small, under the action prompt while holding something droppable
    commands.spawn((
        Name::new("Drop hint"),
        DropHint,
        UiTargetCamera(camera),
        Text::new("press R to drop"),
        font(&assets, DROP_HINT_SIZE),
        TextColor(INK),
        shadow(),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            bottom: Val::Px(SECONDARY_HINT_BOTTOM),
            ..default()
        },
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
    // CLAUDE: Say lines, just above the action prompt
    commands.spawn((
        Name::new("Message"),
        MessageLabel,
        UiTargetCamera(camera),
        Text::new(""),
        font(&assets, PROMPT_SIZE),
        TextColor(INK.with_alpha(0.0)),
        TextShadow { color: Color::NONE, ..shadow() },
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            bottom: Val::Percent(MESSAGE_BOTTOM_PERCENT),
            ..default()
        },
        Pickable::IGNORE,
    ));
    // CLAUDE: alpha is driven by present_tooltip; transparent = hidden
    commands.spawn((
        Name::new("Crosshair"),
        Crosshair::default(),
        UiTargetCamera(camera),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0 + 100.0 * CURSOR_DROP),
            width: Val::Vh(CROSSHAIR_DIAMETER),
            height: Val::Vh(CROSSHAIR_DIAMETER),
            ..default()
        },
        UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
        ImageNode::new(assets.load(CROSSHAIR_IMAGE)).with_color(CROSSHAIR_COLOR.with_alpha(0.0)),
        Pickable::IGNORE,
    ));
}

// CODEX: a centre-of-view mesh ray respects scenery occlusion; proximity alone never shows a prompt.
pub(crate) fn update_prompt(
    mut ray_cast: MeshRayCast,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<ComputerSession>,
    mut prompt: ResMut<InteractionPrompt>,
    cameras: Query<(&Transform, &Camera), With<Player>>,
    targets: Query<&InteractionText>,
    parents: Query<&ChildOf>,
    layers: Query<&RenderLayers>,
    held: Query<(Entity, &Holdable), With<Held>>,
    holdables: Query<(), With<Holdable>>,
    children: Query<&Children>,
    spatial: SpatialQuery,
    mut commands: Commands,
    paper: Res<crate::post_it::PostItSession>,
    tuning: Option<Res<crate::radio::RadioTuning>>,
) {
    prompt.activate = None;
    // CODEX: T owns the controls until released; E must not pick up or activate scenery mid-turn.
    if tuning.is_some_and(|t| t.entity.is_some()) {
        prompt.target = None;
        prompt.available = false;
        return;
    }
    // CODEX: paper owns E while held and all gameplay keys while its editor is active.
    if paper.blocks_input() || paper.held.is_some() {
        prompt.target = None;
        prompt.available = !paper.blocks_input() && crate::browser::captured() && held.iter().any(|(_, h)| !h.use_prompt.is_empty());
        if prompt.available && keys.just_pressed(INTERACT_KEY) {
            if let Some(entity) = paper.held { commands.trigger(Interact { entity }); }
        }
        return;
    }
    let camera = cameras.iter().next();
    let holding = held.iter().next();
    // CLAUDE: the held item floats in front of the camera; the ray goes through it
    let held_parts: Vec<Entity> = holding
        .map(|(e, _)| std::iter::once(e).chain(children.iter_descendants(e)).collect())
        .unwrap_or_default();
    let playing = crate::browser::captured() && session.active.is_none() && !session.returning;
    // CODEX: menu keystrokes must never trigger an interaction after resuming.
    let target = match camera {
        Some((transform, camera)) if playing => {
            let world_layer = |entity| {
                !held_parts.contains(&entity)
                    && layers
                        .get(entity)
                        .map_or(true, |layer| layer.intersects(&RenderLayers::default()))
            };
            // CLAUDE: an empty prompt is switched off; one hand: nothing else can be picked up while holding
            let offered = |e: Entity, text: &InteractionText| {
                !text.prompt.is_empty() && (holding.is_none() || !holdables.contains(e))
            };
            let direction = Dir3::new(transform.rotation * cursor_view_direction(camera)).unwrap_or(transform.forward());
            let ancestry = |e: Entity| std::iter::once(e).chain(parents.iter_ancestors(e));
            let first = ray_cast
                .cast_ray(Ray3d::new(transform.translation, direction), &MeshRayCastSettings::default().with_filter(&world_layer))
                .first()
                .map(|(entity, hit)| (*entity, hit.distance));
            first
                .and_then(|(entity, distance)| {
                    // CLAUDE: the hit mesh or its nearest ancestor holds the prompt (a glTF prop's root)
                    ancestry(entity)
                        // CLAUDE: one switched off is as if it had none
                        .find_map(|e| targets.get(e).ok().filter(|text| distance <= text.range && offered(e, text)).map(|_| e))
                })
                // CLAUDE: else the cursor's whole circle, cast as a cylinder, against `cursor_cast` targets
                .or_else(|| {
                    let owner = |collider: Entity| {
                        ancestry(collider)
                            .find_map(|e| targets.get(e).ok().map(|text| (e, text)))
                            .filter(|(e, text)| text.cursor_cast && offered(*e, text))
                    };
                    let range = targets.iter().filter(|t| t.cursor_cast).map(|t| t.range).fold(0.0, f32::max);
                    // a thin disc of the cursor's radius (at the farthest reach), facing along the ray
                    let hit = spatial.cast_shape_predicate(
                        &Collider::cylinder(cursor_radius(camera, range), 1e-3),
                        transform.translation,
                        Quat::from_rotation_arc(Vec3::Y, *direction),
                        direction,
                        &ShapeCastConfig { max_distance: range, ..default() },
                        &SpatialQueryFilter::default(),
                        &|e| owner(e).is_some(),
                    )?;
                    let (target, text) = owner(hit.entity)?;
                    let touched = hit.point1 - transform.translation;
                    if hit.distance > text.range {
                        return None;
                    }
                    // and nothing in front of the point it touched
                    let to_touched = Ray3d::new(transform.translation, Dir3::new(touched).ok()?);
                    let hidden = ray_cast
                        .cast_ray(to_touched, &MeshRayCastSettings::default().with_filter(&world_layer))
                        .first()
                        .is_some_and(|(e, hit)| hit.distance < touched.length() - 0.005 && !ancestry(*e).any(|a| a == target));
                    (!hidden).then_some(target)
                })
        }
        _ => None,
    };
    // CLAUDE: aimed at nothing, E uses the held item, if it has a use
    let held_use = holding.filter(|(_, h)| target.is_none() && playing && !h.use_prompt.is_empty());
    prompt.available = target.is_some() || held_use.is_some();
    if let (Some((entity, _)), true) = (held_use, keys.just_pressed(INTERACT_KEY)) {
        commands.trigger(Interact { entity });
    }
    if prompt.target == target {
        prompt.since += time.delta_secs();
    } else {
        prompt.since = 0.0;
    }
    prompt.target = target;
    if let Some(entity) = target {
        if keys.just_pressed(INTERACT_KEY) {
            prompt.activate = Some(entity);
            commands.trigger(Interact { entity });
        }
    }
}

// CODEX: input stays in Update so activation retains its timing; presentation uses final globals.
pub(crate) fn present_tooltip(
    session: Res<ComputerSession>,
    prompt: Res<InteractionPrompt>,
    cameras: Query<(&GlobalTransform, &Camera), With<Player>>,
    targets: Query<(&InteractionText, &GlobalTransform)>,
    children: Query<&Children>,
    colliders: Query<(&Collider, &GlobalTransform)>,
    mut tooltips: Query<(&mut Node, &mut Text, &mut TextFont, &mut Visibility), With<InteractionTooltip>>,
    mut actions: Query<&mut Visibility, (With<ActionPrompt>, Without<InteractionTooltip>)>,
    mut verbs: Query<&mut TextSpan, With<ActionVerb>>,
    mut keys: Query<&mut TextSpan, (With<ActionKey>, Without<ActionVerb>)>,
    held: Query<&Holdable, With<Held>>,
    mut drop_hints: Query<(&mut Visibility, &mut Text), (With<DropHint>, Without<ActionPrompt>, Without<InteractionTooltip>)>,
    mut commands: Commands,
) {
    // CODEX: independent held controls remain visible while E targets other things. Keep DropHint's
    // existing marker/layout so the paper editor can still replace this label with its editing hints.
    let item = held.iter().next().filter(|_| session.active.is_none() && !session.returning && crate::browser::captured());
    // CLAUDE: the bottom prompt; while a session runs it offers leaving it. With nothing aimed at, the
    // held item's use.
    let playing = crate::browser::captured() && !session.returning;
    let verb = match (session.active, prompt.target) {
        (Some(active), _) => targets.get(active).ok().map(|(text, _)| &text.active_prompt),
        (None, Some(target)) if !session.returning => {
            targets.get(target).ok().filter(|(text, _)| text.hint).map(|(text, _)| &text.prompt)
        }
        (None, None) if playing => held.iter().next().map(|h| &h.use_prompt),
        _ => None,
    }
    .filter(|verb| !verb.is_empty());
    // CODEX: held F is primary; a reachable E action stays available on the secondary control line.
    let toggle = item.map(|h| &h.toggle_prompt).filter(|text| !text.is_empty());
    let mut controls = Vec::new();
    if toggle.is_some() {
        if let Some(verb) = verb { controls.push(format!("press E to {verb}")); }
    }
    if item.is_some_and(|h| h.droppable) { controls.push("press R to drop".into()); }
    for (mut visibility, mut label) in &mut drop_hints {
        visibility.set_if_neq(if controls.is_empty() { Visibility::Hidden } else { Visibility::Inherited });
        label.set_if_neq(Text::new(controls.join(" · ")));
    }
    let verb = toggle.or(verb);
    let key_label = if toggle.is_some() { "F" } else { "E" };
    for mut key in &mut keys { if key.0 != key_label { key.0 = key_label.into(); } }
    for mut visibility in &mut actions {
        *visibility = if verb.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    }
    if let Some(verb) = verb {
        for mut span in &mut verbs {
            let text = format!(" to {verb}");
            if span.0 != text {
                span.0 = text;
            }
        }
    }

    for (mut node, mut label, mut font, mut visibility) in &mut tooltips {
        *visibility = Visibility::Hidden;
        // CODEX: activation may have started a session after update_prompt selected this target.
        if session.active.is_some() || session.returning || prompt.since < NAME_DELAY {
            continue;
        }
        let Some((global, camera)) = cameras.iter().next() else {
            continue;
        };
        let Some(entity) = prompt.target else {
            continue;
        };
        let Ok((text, anchor)) = targets.get(entity) else {
            continue;
        };
        if text.name.is_empty() {
            continue;
        }
        let collider_bounds = collider_screen_bounds(entity, text.name_offset, global, camera, &children, &colliders);
        let bounds = collider_bounds.or_else(|| camera.world_to_viewport(global, anchor.translation() + text.name_offset)
                .ok().map(|at| Rect::from_corners(at, at)));
        let (Some(bounds), Some(size)) = (bounds, camera.logical_viewport_size()) else {
            continue;
        };
        let Some(placement) = tooltip_position(bounds, size, text.name_gap, text.font_size) else {
            continue;
        };
        // CODEX: record the actual result rather than recomputing placement rules for debug output.
        commands.entity(entity).insert(InteractionTooltipDebug {
            placement: if collider_bounds.is_some() { placement.kind } else { TooltipPlacement::OriginFallback },
            vertically_clamped: placement.clamped,
            position: placement.position,
        });
        node.left = Val::Px(placement.position.x);
        node.top = Val::Px(placement.position.y);
        // CLAUDE: first letter at the delay, as in the flower demo
        let letters = ((prompt.since - NAME_DELAY) * NAME_LETTERS_PER_SECOND) as usize + 1;
        label.0 = text.name.chars().take(letters).collect();
        font.font_size = FontSize::Px(text.font_size);
        *visibility = Visibility::Visible;
    }
}

// CODEX (GPT-6): project collider-local bounds with the final render transform. shape() is unscaled;
// GlobalTransform applies scale once. Mesh-only details (e.g. the radio antenna) never contribute.
fn collider_screen_bounds(
    entity: Entity,
    offset: Vec3,
    view: &GlobalTransform,
    camera: &Camera,
    children: &Query<&Children>,
    colliders: &Query<(&Collider, &GlobalTransform)>,
) -> Option<Rect> {
    std::iter::once(entity)
        .chain(children.iter_descendants(entity))
        .filter_map(|e| colliders.get(e).ok())
        .flat_map(|(collider, global)| {
            let aabb = collider.shape().compute_local_aabb();
            let center = Vec3::from(aabb.center());
            let half = Vec3::from(aabb.half_extents());
            (0..8).filter_map(move |i| {
                let sign = Vec3::new((i & 1) as f32, (i >> 1 & 1) as f32, (i >> 2 & 1) as f32) * 2.0 - 1.0;
                // CODEX: a large collider can cross the camera plane; only front-facing corners project.
                camera.world_to_viewport(view, global.transform_point(center + half * sign) + offset).ok()
            })
        })
        .fold(None, |rect: Option<Rect>, at| Some(rect.map_or(Rect::from_corners(at, at), |r| r.union_point(at))))
}

// CODEX (GPT-6): keep labels in the central 40% vertical band. Large objects use the interaction
// cursor instead of their distant top edge; normal objects use their full projected bounds' center/top.
fn tooltip_position(bounds: Rect, viewport: Vec2, gap: f32, font_size: f32) -> Option<TooltipPosition> {
    let visible_min = bounds.min.max(Vec2::ZERO);
    let visible_max = bounds.max.min(viewport);
    if visible_min.x > visible_max.x || visible_min.y > visible_max.y {
        return None;
    }
    let margin = gap.max(0.0);
    let large = bounds.height() > viewport.y * NAME_LARGE_OBJECT_HEIGHT;
    let mut at = if large {
        viewport * Vec2::new(0.5, 0.5 + CURSOR_DROP) - Vec2::Y * margin
    } else {
        // CODEX: clipping against the viewport must not shift the collider's horizontal center.
        Vec2::new(bounds.center().x, bounds.min.y - margin)
    };
    // CODEX: anchor is the text's bottom; reserve its line height so the whole line stays in-band.
    let bottom = viewport.y * (0.5 + NAME_CENTER_LIMIT);
    let top = (viewport.y * (0.5 - NAME_CENTER_LIMIT) + font_size * 1.2).min(bottom);
    let clamped_y = at.y.clamp(top, bottom);
    let clamped = clamped_y != at.y;
    at.y = clamped_y;
    Some(TooltipPosition {
        position: at,
        kind: if large { TooltipPlacement::Cursor } else { TooltipPlacement::ColliderBounds },
        clamped,
    })
}

// CODEX (GPT-6): carry placement diagnostics alongside the final anchor to keep debug state accurate.
struct TooltipPosition {
    position: Vec2,
    kind: TooltipPlacement,
    clamped: bool,
}

/// CLAUDE: the crosshair fades in over CROSSHAIR_FADE_SECONDS to CROSSHAIR_OPACITY while the cursor is over,
/// or within CROSSHAIR_NEAR screen heights of, a reachable target's on-screen outline. Never while the
/// player isn't in control.
pub(crate) fn present_crosshair(
    time: Res<Time>,
    session: Res<ComputerSession>,
    prompt: Res<InteractionPrompt>,
    cameras: Query<(&GlobalTransform, &Camera), With<Player>>,
    targets: Query<(Entity, &InteractionText)>,
    children: Query<&Children>,
    meshes: Query<(&Aabb, &GlobalTransform, &InheritedVisibility)>,
    mut crosshairs: Query<(&mut Crosshair, &mut ImageNode)>,
    held: Query<(), With<Held>>,
    holdables: Query<(), With<Holdable>>,
    paper: Res<crate::post_it::PostItSession>,
    keys: Res<ButtonInput<KeyCode>>,
    mut feedback: Query<(Entity, &mut InteractionFeedback)>,
) {
    let playing = crate::browser::captured() && session.active.is_none() && !session.returning && !paper.blocks_input();
    let holding = !held.is_empty();
    // CODEX (GPT-6): share the exact proximity test with the cursor and opt-in script feedback.
    let near_item = |entity: Entity, text: &InteractionText| {
        if !playing || text.prompt.is_empty() || (holding && holdables.contains(entity)) {
            return false;
        }
        let Some((view, camera)) = cameras.iter().next() else { return false };
        let Some(size) = camera.logical_viewport_size() else {
            return false;
        };
        let cursor = size * Vec2::new(0.5, 0.5 + CURSOR_DROP);
        screen_outline(entity, text.range, view, camera, &children, &meshes).is_some_and(|outline| {
            (cursor - cursor.clamp(outline.min, outline.max)).length() < CROSSHAIR_NEAR * size.y
        })
    };
    let near = (playing && prompt.target.is_some())
        || targets.iter().any(|(entity, text)| near_item(entity, text));
    // CODEX: count only unused E presses; another prop or a held-item action owns a successful E.
    for (entity, mut state) in &mut feedback {
        state.active = playing && prompt.target == Some(entity);
        state.cursor_visible = state.active
            || targets.get(entity).is_ok_and(|(_, text)| near_item(entity, text));
        if state.cursor_visible && !prompt.available && keys.just_pressed(INTERACT_KEY) {
            state.missed_presses = state.missed_presses.saturating_add(1);
        }
    }
    for (mut crosshair, mut image) in &mut crosshairs {
        crosshair.shown = if near { crosshair.shown + time.delta_secs() } else { 0.0 };
        let opacity = (crosshair.shown / CROSSHAIR_FADE_SECONDS).min(1.0) * CROSSHAIR_OPACITY;
        // CLAUDE: an interaction available: bright at once
        let color = if prompt.available && playing { CROSSHAIR_ACTIVE } else { CROSSHAIR_COLOR.with_alpha(opacity) };
        if image.color != color {
            image.color = color;
        }
    }
}

/// CLAUDE: types the current [`Say`] line in, holds it, fades it out.
fn present_message(
    time: Res<Time>,
    mut message: ResMut<Message>,
    mut labels: Query<(&mut Text, &mut TextColor, &mut TextShadow, &mut Node), With<MessageLabel>>,
) {
    message.since += time.delta_secs();
    let letters = (message.since * NAME_LETTERS_PER_SECOND) as usize + 1;
    let typed = message.text.chars().count() as f32 / NAME_LETTERS_PER_SECOND;
    let alpha = 1.0 - ((message.since - typed - message.hold_seconds) / MESSAGE_FADE).clamp(0.0, 1.0);
    let alpha = if message.text.is_empty() { 0.0 } else { alpha };
    // CODEX: includes the hold and fade, so repeated helper presses cannot restart a visible line.
    message.finished = alpha <= 0.0;
    for (mut text, mut color, mut shadow, mut node) in &mut labels {
        node.bottom = Val::Percent(if message.centered { 48.0 } else { MESSAGE_BOTTOM_PERCENT });
        let shown: String = message.text.chars().take(letters).collect();
        if text.0 != shown {
            text.0 = shown;
        }
        color.0 = INK.with_alpha(INK.alpha() * alpha);
        shadow.color = Color::BLACK.with_alpha(0.8 * alpha);
    }
}

/// CLAUDE: viewport rect around the visible meshes of `entity` and its descendants, if within `range` of the
/// camera and entirely in front of it. Ignores occlusion.
fn screen_outline(
    entity: Entity,
    range: f32,
    view: &GlobalTransform,
    camera: &Camera,
    children: &Query<&Children>,
    meshes: &Query<(&Aabb, &GlobalTransform, &InheritedVisibility)>,
) -> Option<Rect> {
    let corners: Vec<Vec3> = std::iter::once(entity)
        .chain(children.iter_descendants(entity))
        .filter_map(|e| meshes.get(e).ok())
        .filter(|(_, _, visibility)| visibility.get())
        .flat_map(|(aabb, global, _)| {
            let (center, half) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
            (0..8).map(move |i| {
                let sign = Vec3::new((i & 1) as f32, (i >> 1 & 1) as f32, (i >> 2 & 1) as f32) * 2.0 - 1.0;
                global.transform_point(center + half * sign)
            })
        })
        .collect();
    let bounds = corners.iter().fold(None, |acc: Option<(Vec3, Vec3)>, &c| {
        Some(acc.map_or((c, c), |(min, max)| (min.min(c), max.max(c))))
    })?;
    let eye = view.translation();
    if eye.distance(eye.clamp(bounds.0, bounds.1)) > range {
        return None;
    }
    corners.iter().try_fold(Rect::EMPTY, |rect, &c| {
        let at = camera.world_to_viewport(view, c).ok()?;
        Some(rect.union_point(at))
    })
}

// CODEX: headless regressions exercise the real presentation schedule without rendering or assets.
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        camera::{CameraProjection, ComputedCameraValues, RenderTargetInfo},
        ecs::system::RunSystemOnce,
    };

    // CODEX (GPT-6): old script events may omit the new field; helper overrides control completion.
    #[test]
    fn say_hold_override_preserves_default_and_finishes_earlier() {
        let mut fields = bevy::reflect::structs::DynamicStruct::default();
        fields.insert("text", "test".to_string());
        let original = Say::from_reflect(&fields).unwrap();
        assert_eq!(original.hold_seconds, None);
        fields.insert("hold_seconds", Some(2.0_f32));
        let helper = Say::from_reflect(&fields).unwrap();

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), corn_scripting::plugin));
        plugin(&mut app);
        app.world_mut().trigger(original);
        assert_eq!(app.world().resource::<Message>().hold_seconds, MESSAGE_HOLD);
        app.world_mut().trigger(helper);
        assert_eq!(app.world().resource::<Message>().hold_seconds, 2.0);
        let done = 4.0 / NAME_LETTERS_PER_SECOND + 2.0 + MESSAGE_FADE + 0.01;
        app.world_mut().resource_mut::<Message>().since = done;
        app.world_mut().run_system_once(present_message).unwrap();
        assert!(app.world().resource::<Message>().finished);
        app.world_mut().trigger(Say { text: "test".into(), hold_seconds: None });
        app.world_mut().resource_mut::<Message>().since = done;
        app.world_mut().run_system_once(present_message).unwrap();
        assert!(!app.world().resource::<Message>().finished);
    }

    // CODEX (GPT-6): verify the standalone helper's selectors, reflected components and handler syntax.
    #[test]
    fn key_helper_corn_parses() {
        let mut registry = bevy::reflect::TypeRegistry::default();
        registry.register::<Name>();
        registry.register::<bevy::ecs::lifecycle::AddEvent>();
        registry.register::<InteractionFeedback>();
        registry.register::<crate::seen::SeenTracker>();
        let doc = corn_scene::parse_doc(
            include_str!("../assets/stuck_player_key_helper.corn"),
            "stuck_player_key_helper.corn",
            &registry,
            &[],
        ).unwrap();
        assert_eq!(doc.scripts.len(), 1);
    }

    // CODEX: use Bevy's projection implementation so test matrices follow the engine's conventions.
    fn projection(fov: f32) -> Mat4 {
        PerspectiveProjection {
            fov,
            aspect_ratio: 4.0 / 3.0,
            ..default()
        }
        .get_clip_from_view()
    }

    // CODEX: camera/target motion and a camera-matrix update must reach presentation in the same pass.
    #[test]
    fn tooltip_uses_final_camera_and_hides_when_activation_starts() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, AssetPlugin::default()))
            .init_asset::<Font>()
            .init_asset::<Image>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ComputerSession>();
        plugin(&mut app);
        let player = app
            .world_mut()
            .spawn((
                Player::default(),
                Transform::IDENTITY,
                Camera {
                    computed: ComputedCameraValues {
                        clip_from_view: projection(0.9),
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(800, 600),
                            scale_factor: 1.0,
                        }),
                        ..default()
                    },
                    ..default()
                },
            ))
            .id();
        let screen = app
            .world_mut()
            .spawn((
                InteractionText {
                    name: "voice box".into(),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, -4.0),
            ))
            .id();
        {
            let mut prompt = app.world_mut().resource_mut::<InteractionPrompt>();
            prompt.target = Some(screen);
            // CLAUDE: three letters in
            prompt.since = NAME_DELAY + 2.5 / NAME_LETTERS_PER_SECOND;
        }
        app.world_mut().run_system_once(setup_tooltip).unwrap();
        // CODEX: emulate the renderer updating the computed matrix, using the actual camera system label.
        app.add_systems(
            PostUpdate,
            (|mut cameras: Query<&mut Camera>| {
                for mut camera in &mut cameras {
                    camera.computed.clip_from_view = projection(1.2);
                }
            })
            .in_set(CameraUpdateSystems),
        );
        app.world_mut().run_schedule(PostUpdate);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation
            .x = 1.0;
        app.world_mut()
            .get_mut::<Transform>(screen)
            .unwrap()
            .translation
            .y = 0.5;
        app.world_mut()
            .get_mut::<Camera>(player)
            .unwrap()
            .computed
            .clip_from_view = projection(0.9);
        app.world_mut().run_schedule(PostUpdate);
        let world = app.world_mut();
        let camera = world.get::<Camera>(player).unwrap();
        let expected = camera
            .world_to_viewport(
                world.get::<GlobalTransform>(player).unwrap(),
                world.get::<GlobalTransform>(screen).unwrap().translation(),
            )
            .unwrap();
        let mut tooltips = world.query_filtered::<(&Node, &Text, &Visibility), With<InteractionTooltip>>();
        let (node, text, visibility) = tooltips.single(world).unwrap();
        assert_eq!(node.left, Val::Px(expected.x));
        assert_eq!(node.top, Val::Px(expected.y - InteractionText::default().name_gap));
        assert_eq!(text.0, "voi");
        assert_eq!(*visibility, Visibility::Visible);
        // CODEX: runtime diagnostics live on the item and match the rendered bottom-center anchor.
        let debug = world.get::<InteractionTooltipDebug>(screen).unwrap();
        assert_eq!(debug.placement, TooltipPlacement::OriginFallback);
        assert!(!debug.vertically_clamped);
        assert_eq!(node.left, Val::Px(debug.position.x));
        assert_eq!(node.top, Val::Px(debug.position.y));
        world.entity_mut(screen).insert(Collider::cuboid(0.2, 0.2, 0.2));
        world.run_schedule(PostUpdate);
        assert_eq!(world.get::<InteractionTooltipDebug>(screen).unwrap().placement, TooltipPlacement::ColliderBounds);
        world.entity_mut(screen).insert(Collider::cuboid(1.0, 10.0, 1.0));
        world.run_schedule(PostUpdate);
        assert_eq!(world.get::<InteractionTooltipDebug>(screen).unwrap().placement, TooltipPlacement::Cursor);
        let mut actions = world.query_filtered::<&Visibility, With<ActionPrompt>>();
        assert_eq!(*actions.single(world).unwrap(), Visibility::Inherited);
        // CODEX: input selected a target, but update_session consumed activation before presentation.
        world.resource_mut::<ComputerSession>().active = Some(screen);
        world.run_schedule(PostUpdate);
        assert_eq!(*tooltips.single(world).unwrap().2, Visibility::Hidden);
        assert_eq!(world.get::<InteractionTooltipDebug>(screen).unwrap().placement, TooltipPlacement::Cursor);
        // CLAUDE: the session target has no active_prompt here, so nothing is offered
        assert_eq!(*actions.single(world).unwrap(), Visibility::Hidden);
    }

    // CODEX (GPT-6): child collider rotation/scale must affect labels, while mesh-only antennas don't.
    #[test]
    fn tooltip_bounds_follow_child_collider_and_ignore_mesh_only_details() {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let transform = GlobalTransform::from(
            Transform::from_xyz(0.3, 0.1, -4.0)
                .with_rotation(Quat::from_rotation_z(0.4))
                .with_scale(Vec3::new(2.0, 0.5, 1.0)),
        );
        world.spawn((ChildOf(root), Collider::cuboid(0.4, 0.2, 0.1), transform));
        world.spawn((
            ChildOf(root),
            Aabb { center: Vec3::Y.into(), half_extents: Vec3::splat(10.0).into() },
            GlobalTransform::IDENTITY,
        ));
        let camera = Camera {
            computed: ComputedCameraValues {
                clip_from_view: projection(0.9),
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(800, 600),
                    scale_factor: 1.0,
                }),
                ..default()
            },
            ..default()
        };
        let offset = Vec3::new(0.01, 0.03, 0.0);
        let expected = [-0.2, 0.2].into_iter().flat_map(|x| {
            [-0.1, 0.1].into_iter().flat_map(move |y| {
                [-0.05, 0.05].into_iter().map(move |z| Vec3::new(x, y, z))
            })
        }).fold(Rect::EMPTY, |rect, corner| {
            rect.union_point(camera.world_to_viewport(
                &GlobalTransform::IDENTITY, transform.transform_point(corner) + offset,
            ).unwrap())
        });
        let actual = world.run_system_once(move |children: Query<&Children>, colliders: Query<(&Collider, &GlobalTransform)>| {
            collider_screen_bounds(root, offset, &GlobalTransform::IDENTITY, &camera, &children, &colliders)
        }).unwrap().unwrap();
        assert!((actual.min - expected.min).length() < 0.001);
        assert!((actual.max - expected.max).length() < 0.001);
    }

    // CODEX (GPT-6): verify the 50%-height switch and 20%-from-center limits, including offscreen tops.
    #[test]
    fn tooltip_placement_caps_height_and_uses_cursor_for_large_objects() {
        let viewport = Vec2::new(800.0, 600.0);
        let at = |min, max| tooltip_position(Rect::from_corners(min, max), viewport, 12.0, 36.0).unwrap().position;
        assert_eq!(at(Vec2::new(200.0, 270.0), Vec2::new(400.0, 350.0)), Vec2::new(300.0, 258.0));
        // CODEX: partly offscreen props retain the full collider's center, not the visible portion's.
        assert_eq!(at(Vec2::new(-100.0, 270.0), Vec2::new(300.0, 350.0)), Vec2::new(100.0, 258.0));
        // CODEX: exactly 50% still uses the object top; only larger objects switch to the cursor.
        assert_eq!(at(Vec2::new(200.0, 270.0), Vec2::new(400.0, 570.0)), Vec2::new(300.0, 258.0));
        assert_eq!(at(Vec2::new(-500.0, -900.0), Vec2::new(700.0, 500.0)), Vec2::new(400.0, 318.0));
        assert_eq!(at(Vec2::new(200.0, 500.0), Vec2::new(400.0, 550.0)).y, 420.0);
        let high = at(Vec2::new(200.0, 10.0), Vec2::new(400.0, 50.0));
        assert!((high.y - 223.2).abs() < 0.001);
        assert!(tooltip_position(Rect::from_corners(Vec2::new(200.0, 10.0), Vec2::new(400.0, 50.0)), viewport, 12.0, 36.0).unwrap().clamped);
        assert!(tooltip_position(Rect::from_corners(Vec2::splat(-100.0), Vec2::splat(-10.0)), viewport, 12.0, 36.0).is_none());
    }

    // CLAUDE: crosshair strength follows the cursor's screen gap to reachable targets, and the fade in.
    #[test]
    fn crosshair_follows_screen_gap_to_reachable_targets() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, AssetPlugin::default()))
            .init_asset::<Font>()
            .init_asset::<Image>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ComputerSession>();
        plugin(&mut app);
        app.world_mut().spawn((
            Player::default(),
            Transform::IDENTITY,
            Camera {
                computed: ComputedCameraValues {
                    clip_from_view: projection(0.9),
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::new(800, 600),
                        scale_factor: 1.0,
                    }),
                    ..default()
                },
                ..default()
            },
        ));
        // a 0.2m cube 2m ahead; at x = 0.25 its outline is ~0.07 screen heights right of the cursor, at
        // x = 0.4 ~0.15
        let cube = app
            .world_mut()
            .spawn((
                InteractionText::default(),
                InteractionFeedback::default(),
                Transform::from_xyz(0.25, 0.0, -2.0),
                Aabb::from_min_max(Vec3::splat(-0.1), Vec3::splat(0.1)),
                InheritedVisibility::VISIBLE,
            ))
            .id();
        app.world_mut().run_system_once(setup_tooltip).unwrap();
        let world = app.world_mut();
        let strength = |world: &mut World, at: Vec3, shown: f32| {
            world.get_mut::<Transform>(cube).unwrap().translation = at;
            let mut crosshairs = world.query::<(&mut Crosshair, &ImageNode)>();
            crosshairs.single_mut(world).unwrap().0.shown = shown;
            world.run_schedule(PostUpdate);
            crosshairs.single(world).unwrap().1.color.alpha()
        };
        let near = Vec3::new(0.25, 0.0, -2.0);
        let full = CROSSHAIR_OPACITY;
        assert!((strength(world, near, CROSSHAIR_FADE_SECONDS / 2.0) - full / 2.0).abs() < 1e-4, "half faded");
        assert_eq!(strength(world, near, 1.0), full, "near: same opacity as over it");
        assert_eq!(strength(world, Vec3::new(0.0, -0.2, -2.0), 1.0), full, "cursor over the outline");
        assert_eq!(strength(world, Vec3::new(0.4, 0.0, -2.0), 1.0), 0.0, "just beyond the angular reach");
        assert_eq!(strength(world, Vec3::new(0.0, -0.4, -4.0), 1.0), 0.0, "out of reach");
        // CODEX (GPT-6): opt-in feedback counts only near, unused E presses for this target.
        world.resource_mut::<ButtonInput<KeyCode>>().press(INTERACT_KEY);
        strength(world, near, 1.0);
        let feedback = world.get::<InteractionFeedback>(cube).unwrap();
        assert!(feedback.cursor_visible && !feedback.active);
        assert_eq!(feedback.missed_presses, 1);
        strength(world, Vec3::new(0.4, 0.0, -2.0), 1.0);
        assert!(!world.get::<InteractionFeedback>(cube).unwrap().cursor_visible);
        assert_eq!(world.get::<InteractionFeedback>(cube).unwrap().missed_presses, 1);
        // over the target: shown; during a computer session: none
        {
            let mut prompt = world.resource_mut::<InteractionPrompt>();
            prompt.target = Some(cube);
            prompt.available = true;
        }
        assert_eq!(strength(world, Vec3::new(2.0, 0.0, -2.0), 0.0), 1.0, "an interaction available: bright at once");
        assert!(world.get::<InteractionFeedback>(cube).unwrap().active);
        assert_eq!(world.get::<InteractionFeedback>(cube).unwrap().missed_presses, 1);
        // CODEX: an available action on another prop suppresses missed-E feedback on the near key.
        world.resource_mut::<InteractionPrompt>().target = Some(Entity::PLACEHOLDER);
        strength(world, near, 1.0);
        assert!(!world.get::<InteractionFeedback>(cube).unwrap().active);
        assert_eq!(world.get::<InteractionFeedback>(cube).unwrap().missed_presses, 1);
        world.resource_mut::<ComputerSession>().active = Some(cube);
        assert_eq!(strength(world, near, 1.0), 0.0);
        assert!(!world.get::<InteractionFeedback>(cube).unwrap().cursor_visible);
    }
}
