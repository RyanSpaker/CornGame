//! CODEX (GPT-6) 2026-11-05: temporary standalone post-it editing lab, without lobby assets or gameplay.
//! Run: cargo run -p lobby --bin post_it_lab. Up/Down: size; Shift: padding; Ctrl: wrap count.
use bevy::{input::keyboard::KeyboardInput, prelude::*};
use corn_scripting::ScriptEventsExt;

// CODEX: exercise the exact editor, fitter and ink geometry used by the lobby.
#[path = "../post_it_text.rs"]
mod post_it_text;
// CODEX: keep third-party font assets and their ordering outside the editor mechanics.
#[path = "../post_it_fonts.rs"]
mod post_it_fonts;
// CODEX: use the lobby's per-note typography and controls directly.
#[path = "../post_it_editor.rs"]
mod post_it_editor;
use post_it_text::PaperText;
use post_it_fonts::FONTS;
use post_it_editor::PaperStyle;

// CODEX: a small reflected control surface also supports remote-console checks and screenshots.
#[derive(Resource, Reflect)]
#[reflect(Resource)]
struct Lab {
    style: PaperStyle,
    description: String,
    max_chars: usize,
    image: Handle<Image>,
    previous: String,
    blink: bool,
}

// CODEX: remote tests feed the same key-edit function as real keyboard messages.
#[derive(Event, Reflect)]
struct LabKey {
    text: String,
    key: KeyCode,
    // CODEX: exercise Ctrl+Enter from the test console as well as the real keyboard.
    ctrl: bool,
    shift: bool,
}

// CODEX: show the current face and padding so every arrow press gives visible feedback.
#[derive(Component)]
struct LabStatus;

// CODEX: only UI, a paper mesh and the shared editor; no room, physics or microphone permissions.
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Post-it text lab — centered, automatic text fit".into(),
                resolution: (900, 800).into(), ..default() }), ..default()
        }))
        .add_plugins(corn_scripting::plugin)
        .register_type::<PaperText>()
        .register_type::<PaperStyle>()
        .register_type::<Lab>()
        .register_script_event::<LabKey>()
        .add_observer(|on: On<LabKey>, mut notes: Query<&mut PaperText>, mut lab: ResMut<Lab>| {
            for mut note in &mut notes {
                if lab.style.adjust(&note.text, 512, on.key, on.shift, on.ctrl) { continue; }
                post_it_text::edit_key(&mut note, &KeyboardInput {
                    key_code: on.key, logical_key: bevy::input::keyboard::Key::Character(on.text.clone().into()),
                    text: Some(on.text.clone().into()), state: bevy::input::ButtonState::Pressed,
                    repeat: false, window: Entity::PLACEHOLDER,
                }, lab.max_chars, on.ctrl);
            }
        })
        .add_systems(Startup, setup)
        .add_systems(Update, edit)
        .run();
}

// CODEX: keep procedural test paper isolated; the game uses the authored postit1–4 meshes.
fn paper_mesh() -> Mesh { Plane3d::default().mesh().size(0.076, 0.076).build() }

// CODEX: one clean view makes wrapping, fit, cursor blink and clipping easy to inspect.
fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((Camera3d::default(), Camera { clear_color: Color::srgb(0.08, 0.09, 0.11).into(), ..default() },
        Transform::from_xyz(0.0, 0.13, 0.0).looking_at(Vec3::ZERO, Vec3::NEG_Z)));
    let paper = paper_mesh();
    let ink = post_it_text::ink_mesh(&paper, Vec3::splat(-0.038), Vec3::splat(0.038), 0.00015).unwrap();
    let image = images.add(post_it_text::rasterize("", true, true, 512, 0.12, 1024.0));
    commands.spawn((Name::new("test post-it"), PaperText { editing: true, ..default() },
        Mesh3d(meshes.add(paper)), MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.91, 0.39), unlit: true, ..default()
        }))))
        .with_child((Mesh3d(meshes.add(ink)), MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(image.clone()), alpha_mode: AlphaMode::Blend, unlit: true, ..default()
        }))));
    commands.spawn((LabStatus, Text::new(""),
        TextFont::from_font_size(19.0), TextLayout::justify(Justify::Center),
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), bottom: Val::Px(30.0), ..default() }));
    commands.insert_resource(Lab { style: PaperStyle::default(), description: String::new(), max_chars: 240, image, previous: String::new(), blink: true });
}

// CODEX: the lab interprets actions only to freeze/clear the preview; lobby.corn owns game behavior.
fn edit(mut input: MessageReader<KeyboardInput>, keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut lab: ResMut<Lab>,
    mut notes: Query<&mut PaperText>, mut images: ResMut<Assets<Image>>, mut status: Query<&mut Text, With<LabStatus>>) {
    let Some(mut note) = notes.iter_mut().next() else { return };
    if keys.just_pressed(KeyCode::Tab) { *note = PaperText { editing: true, ..default() }; }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    for key in [KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight] {
        if keys.just_pressed(key) { lab.style.adjust(&note.text, 512, key, shift, ctrl); }
    }
    for key in input.read() { post_it_text::edit_key(&mut note, key, lab.max_chars, ctrl); }
    if !note.action.is_empty() {
        note.editing = false;
        if note.action == "discard" { note.text.clear(); }
    }
    let blink = (time.elapsed_secs() * 2.0) as u64 % 2 == 0;
    let signature = format!("{}:{}:{:?}", note.text, note.editing, lab.style);
    if signature != lab.previous {
        lab.description = lab.style.description(&note.text, 512);
    }
    if signature != lab.previous || (note.editing && blink != lab.blink) {
        if let Some(mut image) = images.get_mut(&lab.image) {
            *image = lab.style.image(&note.text, note.editing, blink, 512);
        }
        lab.previous = signature;
        lab.blink = blink;
    }
    let label = format!("Font {}/{}: {}\nUp/Down: size | Shift+Up/Down: padding | Ctrl+Up/Down: line count\nLeft/Right: font | Shift+Left/Right: position | Ctrl+Enter: new line\nEnter: finish | Del: discard | Tab: reset | {} characters",
        lab.style.font_index + 1, FONTS.len(), lab.description, lab.max_chars);
    for mut text in &mut status { if text.0 != label { text.0 = label.clone(); } }
}
