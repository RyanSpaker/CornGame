//! CODEX (GPT-6): paper mechanics, like hold.rs. The lobby.corn script owns the note lifecycle.
use avian3d::prelude::*;
use bevy::{input::keyboard::KeyboardInput, prelude::*};
use corn_scripting::ScriptEventsExt;
use crate::{controller::{Player, PlayerBody}, hold::{Held, Holdable}, interaction::Interact, post_it_text};
use crate::post_it_editor::PaperStyle;
use crate::post_it_material::{PaperMaterial, PaperGlow, ink_mask};

// CODEX: marker used by corn to find the stack, without treating the whole stack as Holdable.
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
pub struct PostItStack;

// CODEX: geometry measured by CopyPaper; the authored front is local +Y.
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
#[require(PaperStyle)]
pub struct Paper {
    pub min: Vec3,
    pub max: Vec3,
}

// CODEX: shared with the isolated text-editing lab; scripts decide what actions mean.
pub use crate::post_it_text::PaperText;

// CODEX: reflected tuning for dev console iteration; no rebuild to adjust placement or typography.
#[derive(Resource, Reflect)]
#[reflect(Resource, Default)]
pub struct PaperSettings {
    pub range: f32,
    pub surface_gap: f32,
    // CODEX: minimum sweep thickness for flat authored sheets; curled sheets use their full bounds.
    pub cast_thickness: f32,
    // CODEX: ignore tiny sweep errors at the candidate's endpoint, without masking occluding logs.
    pub visibility_tolerance: f32,
    // CODEX: require a little open space above the sheet so log overhangs cannot bury it in shadow.
    pub overhang_probe: f32,
    pub ink_gap: f32,
    pub texture_size: u32,
    pub margin: f32,
    pub font_size: f32,
    pub max_chars: usize,
    pub cursor_seconds: f32,
    // CODEX: maximum linear RGB editing glow; the shader fades it out on sufficiently lit paper.
    pub editing_emissive: LinearRgba,
    // CODEX: low defaults restrict assistance to deep shadows; brightness is measured after exposure.
    pub editing_darkness_threshold: f32,
    pub editing_darkness_fade: f32,
    // CODEX: cap assisted pixel luminance at the naturally readable reference note, after exposure.
    pub editing_brightness_ceiling: f32,
}

// CODEX: defaults suit the authored 76 mm notes; .corn supplies the hold size and screen offset.
impl Default for PaperSettings {
    // CODEX: reflection uses these defaults when the developer changes individual tuning fields.
    fn default() -> Self {
        Self { range: 1.0, surface_gap: 0.0015, cast_thickness: 0.0005, visibility_tolerance: 0.0005,
            overhang_probe: 0.076,
            ink_gap: 0.00015, texture_size: 512,
            margin: 0.12, font_size: 1024.0, max_chars: 240, cursor_seconds: 0.5,
            editing_emissive: LinearRgba::rgb(0.15, 0.13, 0.06),
            editing_darkness_threshold: 0.001, editing_darkness_fade: 0.002,
            editing_brightness_ceiling: 0.0046 }
    }
}

// CODEX: input ownership only; the script handles taking, completion, discard and rejection.
#[derive(Resource, Reflect, Default)]
#[reflect(Resource, Default)]
pub struct PostItSession {
    pub held: Option<Entity>,
    pub editing: Option<Entity>,
}

// CODEX: retain the chosen path for regression tests and read-only inspection of live placements.
#[derive(Component, Reflect, Default, Debug)]
#[reflect(Component, Default)]
pub struct PaperPlacement {
    pub used_square_fallback: bool,
    // CODEX: preserve the placement view so a reported failure can be replayed after the player moves.
    pub camera: Transform,
}

// CODEX: controller and interaction systems share the editor's keyboard lock.
impl PostItSession {
    // CODEX: the text editor owns gameplay input until corn handles its requested action.
    pub fn blocks_input(&self) -> bool { self.editing.is_some() }
}

// CODEX: exercise placement through real Avian queries without opening a window.
#[cfg(test)]
#[path = "post_it_tests.rs"]
mod tests;

// CODEX: clone only rendered geometry into the script-created target; never duplicate physics/scripts.
#[derive(Event, Reflect)]
pub struct CopyPaper {
    pub entity: Entity,
    pub source: Entity,
}

// CODEX: image cache belongs to each paper; redraw only on typing, blinking, or tuning changes.
#[derive(Component)]
pub(crate) struct PaperInk {
    image: Handle<Image>,
    text: String,
    cursor: bool,
    editing: bool,
    // CODEX: retain typography with the note and invalidate the texture when its style changes.
    style: PaperStyle,
    description: String,
}

// CODEX (GPT-6): mark paper surfaces so updating assistance never touches the separate ink overlay.
#[derive(Component)]
struct PaperSurface;

// CODEX: unobtrusive keyboard help shown only while a note owns input.
#[derive(Component)]
pub(crate) struct PaperEditorHint;

// CODEX: reusable engine work; no rejection timers, sounds or word lists in Rust.
pub fn plugin(app: &mut App) {
    // CLAUDE: build its pipelines as meshes using it spawn
    crate::precompile::register::<PaperMaterial>(app);
    app.add_plugins(MaterialPlugin::<PaperMaterial>::default())
        .register_type::<PostItStack>()
        .register_type::<Paper>()
        .register_type::<PaperText>()
        .register_type::<PaperStyle>()
        .register_type::<PaperPlacement>()
        .register_type::<PaperSettings>()
        .register_type::<PostItSession>()
        .init_resource::<PaperSettings>()
        .init_resource::<PostItSession>()
        .register_script_event::<CopyPaper>()
        .add_observer(copy_paper)
        .add_observer(place)
        .add_systems(Update, (keyboard, offer_surface).chain().before(crate::interaction::update_prompt))
        .add_systems(Update, (render_text, editor_glow).after(keyboard))
        .add_systems(Update, setup_editor_hint)
        .add_systems(PostUpdate, editor_prompt.after(crate::interaction::present_tooltip).before(bevy::ui::UiSystems::Prepare));
}

// CODEX: copy each primitive under the selected authored postitN, preserving local geometry/materials.
fn copy_paper(
    on: On<CopyPaper>,
    globals: Query<&GlobalTransform>,
    cameras: Query<&Transform, With<Player>>,
    children: Query<&Children>,
    primitives: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>, &GlobalTransform)>,
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    mut paper_materials: ResMut<Assets<PaperMaterial>>,
    mut session: ResMut<PostItSession>,
    mut commands: Commands,
) {
    let (Ok(source), Some(camera)) = (globals.get(on.source), cameras.iter().next()) else { return };
    let mut min = Vec3::MAX;
    let mut max = Vec3::MIN;
    let mut parts = Vec::new();
    for e in std::iter::once(on.source).chain(children.iter_descendants(on.source)) {
        let Ok((mesh, material, global)) = primitives.get(e) else { continue };
        let Some(positions) = meshes.get(&mesh.0).and_then(|m| m.attribute(Mesh::ATTRIBUTE_POSITION)).and_then(|a| a.as_float3()) else { continue };
        let local = global.reparented_to(source);
        for p in positions {
            let p = local.transform_point(Vec3::from(*p));
            min = min.min(p);
            max = max.max(p);
        }
        // CODEX: isolate the taken sheet from the stack and other notes before changing emission.
        let Some(source_material) = materials.get(&material.0).cloned() else { continue };
        let material = MeshMaterial3d(paper_materials.add(crate::post_it_material::paper_material(source_material)));
        parts.push((mesh.clone(), material, local, PaperSurface));
    }
    if parts.is_empty() { return; }
    let scale = source.to_scale_rotation_translation().0;
    let rotation = camera.rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let center = rotation * (scale * (min + max) * 0.5);
    commands.entity(on.entity).insert((
        Paper { min, max },
        Transform { translation: camera.translation, rotation, scale },
        Visibility::Inherited,
        Held { rotation, scale, center, radius: (scale * (max - min) * 0.5).length(), ..default() },
    )).with_children(|parent| {
        for part in parts { parent.spawn(part); }
    });
    session.held = Some(on.entity);
}

// CODEX: keep the paper's top edge near camera-up for both point placement and square fallback.
fn paper_rotation(camera: &Transform, normal: Vec3) -> Option<Quat> {
    let normal = normal.try_normalize()?;
    let top = (camera.rotation * Vec3::Y).reject_from_normalized(normal).try_normalize()
        .unwrap_or_else(|| normal.any_orthonormal_vector());
    Some(Quat::from_mat3(&Mat3::from_cols(top.cross(normal).normalize(), normal, -top)))
}

// CODEX: try the original point placement, sweep its square toward that position to check visibility,
// and only use the original square-cast placement when the point candidate is hidden or missing.
fn surface(
    camera: &Transform, view: &Camera, holdable: &Holdable, settings: &PaperSettings,
    paper: &Paper, scale: Vec3,
    spatial: &SpatialQuery, excluded: Vec<Entity>,
    owners: &Query<&ColliderOf>, bodies: &Query<&RigidBody>, sensors: &Query<(), With<Sensor>>,
) -> Option<(Entity, Transform, PaperPlacement)> {
    let point = view.clip_from_view().inverse().project_point3(Vec3::new(0.0, -2.0 * holdable.below, 0.5));
    let direction = Dir3::new(camera.rotation * point).ok()?;
    let filter = SpatialQueryFilter::from_excluded_entities(excluded);
    let allowed = |entity| {
        let body = owners.get(entity).ok().map_or(entity, |owner| owner.body);
        !sensors.contains(entity) && !bodies.get(body).is_ok_and(|body| *body == RigidBody::Dynamic)
    };
    let mut size = scale.abs() * (paper.max - paper.min);
    size.y = size.y.max(settings.cast_thickness.max(0.0001));
    let center = (paper.min + paper.max) * 0.5;
    let range = settings.range.max(0.0);
    if let Some(hit) = spatial.cast_ray(camera.translation, direction, range, true, &filter) {
        // CODEX: unsuitable foreground bodies still block placement; fallback must not bypass them.
        if hit.distance <= 0.0 || !allowed(hit.entity) { return None; }
        let rotation = paper_rotation(camera, hit.normal)?;
        let anchor = Vec3::new(center.x, paper.min.y, center.z);
        let world = Transform {
            translation: camera.translation + direction * hit.distance + hit.normal * settings.surface_gap.max(0.0)
                - rotation * (scale * anchor),
            rotation, scale,
        };
        // CODEX: include the target collider in this sweep: the same bark mesh contains both logs.
        // Sweep the actual oriented sheet volume, stopping just short of its proposed center. A flat
        // desk remains below the whole path, even when the paper overhangs the front edge.
        let to_center = world.transform_point(center) - camera.translation;
        let visibility_direction = Dir3::new(to_center).ok()?;
        let visibility = ShapeCastConfig::from_max_distance(
            (to_center.length() - settings.visibility_tolerance.max(0.0)).max(0.0));
        let shape = Collider::cuboid(size.x, size.y, size.z);
        let view_clear = spatial.cast_shape(&shape, camera.translation, rotation, visibility_direction, &visibility, &filter).is_none();
        // CODEX: the upper live groove can clear the eye-line sweep while an upper log still covers
        // the sheet from above. Sweep the same square upward at its proposed position to detect that
        // overhang. A desk's front face stays behind the sheet throughout this vertical sweep.
        let overhang_clear = settings.overhang_probe <= 0.0 || (view_clear && spatial.cast_shape(
            &shape, world.transform_point(center), rotation, Dir3::Y,
            &ShapeCastConfig::from_max_distance(settings.overhang_probe.max(0.0)), &filter).is_none());
        if view_clear && overhang_clear {
            return Some((hit.entity, world, PaperPlacement { used_square_fallback: false, camera: *camera }));
        }
    }
    // CODEX: preserve the first square fix as the fallback, including its contact angle. No face probes
    // or final clearance recast: the square's first contact determines its parent and travel distance.
    let gap = 2.0 * settings.surface_gap.max(0.0);
    let shape = Collider::cuboid(size.x + gap, size.y + gap, size.z + gap);
    let facing = camera.rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let hit = spatial.cast_shape(&shape, camera.translation, facing, direction,
        &ShapeCastConfig::from_max_distance(range), &filter)?;
    if hit.distance <= 0.0 || !allowed(hit.entity) { return None; }
    let rotation = paper_rotation(camera, hit.normal1)?;
    Some((hit.entity, Transform {
        translation: camera.translation + direction * hit.distance - rotation * (scale * center), rotation, scale,
    }, PaperPlacement { used_square_fallback: true, camera: *camera }))
}

// CODEX: prompt and activation share exactly the same collider projection.
fn offer_surface(
    settings: Res<PaperSettings>, session: Res<PostItSession>, spatial: SpatialQuery,
    camera: Query<(&Transform, &Camera), With<Player>>,
    mut held: Query<(Entity, &mut Holdable, &Held, &Paper)>,
    children: Query<&Children>, player: Query<Entity, With<PlayerBody>>,
    owners: Query<&ColliderOf>, bodies: Query<&RigidBody>, sensors: Query<(), With<Sensor>>,
) {
    let Some(note) = session.held else { return };
    let (Ok((entity, mut holdable, held, paper)), Some((camera, view))) = (held.get_mut(note), camera.iter().next()) else { return };
    let excluded = std::iter::once(entity).chain(children.iter_descendants(entity)).chain(player.iter()).collect();
    let available = surface(camera, view, &holdable, &settings, paper, held.scale, &spatial, excluded, &owners, &bodies, &sensors).is_some();
    holdable.use_prompt = if available { "post-it".into() } else { String::new() };
}

// CODEX: E on carried paper attaches it to the actual hit collider, retaining moving-surface transforms.
fn place(
    on: On<Interact>, settings: Res<PaperSettings>, spatial: SpatialQuery,
    camera: Query<(&Transform, &Camera), With<Player>>,
    held: Query<(&Held, &Holdable, &Paper)>, globals: Query<&GlobalTransform>,
    children: Query<&Children>, player: Query<Entity, With<PlayerBody>>,
    owners: Query<&ColliderOf>, bodies: Query<&RigidBody>, sensors: Query<(), With<Sensor>>,
    mut session: ResMut<PostItSession>, mut commands: Commands,
) {
    let (Ok((held, holdable, paper)), Some((camera, view))) = (held.get(on.entity), camera.iter().next()) else { return };
    let excluded = std::iter::once(on.entity).chain(children.iter_descendants(on.entity)).chain(player.iter()).collect();
    let Some((parent, world, placement)) = surface(camera, view, holdable, &settings, paper, held.scale, &spatial, excluded, &owners, &bodies, &sensors) else { return };
    let Ok(parent_global) = globals.get(parent) else { return };
    commands.entity(on.entity).remove::<Held>().remove::<Holdable>()
        .insert((ChildOf(parent), GlobalTransform::from(world).reparented_to(parent_global),
            PaperText { editing: true, ..default() },
            PaperStyle { font_size: settings.font_size, margin: settings.margin, ..default() }, placement));
    session.held = None;
    session.editing = Some(on.entity);
}

// CODEX: platform text input supplies shifted keys, punctuation and Unicode; E/R/WASD are ordinary text.
fn keyboard(
    mut input: MessageReader<KeyboardInput>, keys: Res<ButtonInput<KeyCode>>, settings: Res<PaperSettings>,
    mut session: ResMut<PostItSession>, mut notes: Query<(Entity, &mut PaperText, &mut PaperStyle)>,
) {
    session.editing = notes.iter().find(|(_, text, _)| text.editing).map(|(e, _, _)| e);
    if !crate::browser::captured() { input.clear(); return; }
    let Some(entity) = session.editing else { input.clear(); return };
    let Ok((_, mut note, mut style)) = notes.get_mut(entity) else { return };
    if !note.action.is_empty() { input.clear(); return; }
    // CODEX: Ctrl+Enter is text input; plain Enter still requests completion from corn.
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    // CODEX: the lobby and lab use the same typography shortcuts; editing consumes gameplay arrows.
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for key in [KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight] {
        if keys.just_pressed(key) { style.adjust(&note.text, settings.texture_size, key, shift, ctrl); }
    }
    if keys.just_pressed(KeyCode::Delete) { note.action = "discard".into(); }
    else if !ctrl && (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter)) { note.action = "finish".into(); }
    for key in input.read() { post_it_text::edit_key(&mut note, key, settings.max_chars, ctrl); }
}

// CODEX (GPT-6): send live tuning to the paper shader; authored material properties stay untouched.
fn editor_glow(
    settings: Res<PaperSettings>, notes: Query<(Entity, &PaperText), With<Paper>>,
    children: Query<&Children>, surfaces: Query<&MeshMaterial3d<PaperMaterial>, With<PaperSurface>>,
    mut materials: ResMut<Assets<PaperMaterial>>,
) {
    for (entity, text) in &notes {
        let glow = settings.editing_emissive;
        let extension = PaperGlow {
            glow: Vec4::new(glow.red, glow.green, glow.blue, if text.editing { 1.0 } else { 0.0 }),
            darkness: Vec4::new(settings.editing_darkness_threshold.max(0.0),
                settings.editing_darkness_fade.max(0.0001), settings.editing_brightness_ceiling.max(0.0), 0.0),
        };
        for part in children.iter_descendants(entity) {
            let Ok(handle) = surfaces.get(part) else { continue };
            if let Some(mut material) = materials.get_mut(&handle.0) {
                if material.extension != extension { material.extension = extension.clone(); }
            }
        }
    }
}

// CODEX: build conforming ink once per placed note, then update its image only when needed.
fn render_text(
    mut commands: Commands, settings: Res<PaperSettings>, time: Res<Time>,
    notes: Query<(Entity, &Paper, &PaperText, &PaperStyle, Option<&PaperInk>)>, children: Query<&Children>,
    parts: Query<(&Mesh3d, &Transform), Without<PaperInk>>, mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>,
    authored: Query<(&bevy::gltf::GltfMaterialName, &MeshMaterial3d<StandardMaterial>)>,
) {
    let blink = ((time.elapsed_secs() / settings.cursor_seconds.max(0.1)) as u64) % 2 == 0;
    for (entity, paper, text, style, ink) in &notes {
        if let Some(ink) = ink {
            let content_changed = ink.text != text.text || ink.style != *style || settings.is_changed();
            if !content_changed && (!text.editing || ink.cursor == blink) && ink.editing == text.editing { continue; }
            if let Some(mut image) = images.get_mut(&ink.image) {
                *image = ink_mask(style.image(&text.text, text.editing, blink, settings.texture_size));
            }
            let description = if content_changed { style.description(&text.text, settings.texture_size) } else { ink.description.clone() };
            commands.entity(entity).insert(PaperInk { image: ink.image.clone(), text: text.text.clone(), cursor: blink, editing: text.editing,
                style: style.clone(), description });
        } else {
            // CODEX: copy the actual authored handwriting material, so future asset edits carry through.
            // Wait for the glTF primitive rather than silently substituting a different material.
            let Some(material) = authored.iter()
                .find(|(name, _)| name.0 == "postit_handwritten_ink")
                .and_then(|(_, handle)| materials.get(&handle.0)).cloned() else { continue };
            let image = images.add(ink_mask(style.image(&text.text, text.editing, blink, settings.texture_size)));
            let material = materials.add(crate::post_it_material::ink_material(material, image.clone()));
            for part in children.iter_descendants(entity) {
                let Ok((handle, transform)) = parts.get(part) else { continue };
                let Some(source) = meshes.get(&handle.0) else { continue };
                if let Some(mesh) = post_it_text::ink_mesh(source, paper.min, paper.max, settings.ink_gap) {
                    commands.spawn((ChildOf(entity), Mesh3d(meshes.add(mesh)), MeshMaterial3d(material.clone()), *transform));
                }
            }
            commands.entity(entity).insert(PaperInk { image, text: text.text.clone(), cursor: blink, editing: text.editing,
                style: style.clone(), description: style.description(&text.text, settings.texture_size) });
        }
    }
}

// TODO (CLAUDE): font size, color, placement and text here and in editor_prompt should all be set in a postit.corn
// (cornscript case study, see .agents/doc/lobby_port.md).
// CODEX: keep the expanded editor instructions above finish/discard, using shared hint typography.
fn setup_editor_hint(mut commands: Commands, assets: Res<AssetServer>, hints: Query<(), With<PaperEditorHint>>,
    players: Query<Entity, With<Player>>) {
    if !hints.is_empty() { return; }
    let Some(camera) = players.iter().next() else { return };
    commands.spawn((Name::new("Post-it editor controls"), PaperEditorHint, UiTargetCamera(camera), Text::new(""),
        TextFont { font: assets.load("fonts/Gelasio-Italic.ttf").into(), font_size: crate::interaction::SECONDARY_HINT_SIZE.into(), ..default() },
        TextColor(crate::interaction::INK), TextShadow::default(), TextLayout::justify(Justify::Center),
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), bottom: Val::Percent(24.0), ..default() },
        Visibility::Hidden, Pickable::IGNORE));
}

// CODEX: editor hints reuse the normal font/prompt; the smaller secondary line replaces the drop hint.
pub(crate) fn editor_prompt(
    session: Res<PostItSession>,
    mut actions: Query<&mut Visibility, With<crate::interaction::ActionPrompt>>,
    mut spans: Query<&mut TextSpan, With<crate::interaction::ActionVerb>>,
    mut keys: Query<&mut TextSpan, (With<crate::interaction::ActionKey>, Without<crate::interaction::ActionVerb>)>,
    mut hints: Query<(&mut Text, &mut Visibility), (With<crate::interaction::DropHint>, Without<crate::interaction::ActionPrompt>)>,
    mut controls: Query<(&mut Text, &mut Visibility), (With<PaperEditorHint>, Without<crate::interaction::DropHint>, Without<crate::interaction::ActionPrompt>)>,
    ink: Query<&PaperInk>,
) {
    let editing = session.blocks_input() && crate::browser::captured();
    if editing {
        for mut visible in &mut actions { *visible = Visibility::Inherited; }
        for mut text in &mut spans { text.0 = " to finish".into(); }
    }
    // CODEX: normal E/F selection belongs to present_tooltip; paper overrides it only while editing.
    if editing { for mut text in &mut keys { text.0 = "Enter".into(); } }
    for (mut text, mut visible) in &mut hints {
        // CODEX (GPT-6): normal held controls (including F) belong to present_tooltip; override only
        // while editing, or the paper system would erase other props' secondary action hints.
        if editing {
            text.0 = "press Del to discard".into();
            *visible = Visibility::Inherited;
        }
    }
    for (mut text, mut visible) in &mut controls {
        *visible = if editing { Visibility::Inherited } else { Visibility::Hidden };
        if editing {
            let description = session.editing.and_then(|e| ink.get(e).ok()).map_or("", |ink| ink.description.as_str());
            let label = format!("{description}\nUp/Down: size | Shift+Up/Down: padding | Ctrl+Up/Down: line count\nLeft/Right: font | Shift+Left/Right: position | Ctrl+Enter: new line");
            if text.0 != label { text.0 = label; }
        }
    }
}
