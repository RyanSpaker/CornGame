//! CODEX (GPT-6): render-to-texture and local monitor input, adapted from ecs/menu_lobby.rs.
use bevy::{
    camera::{
        visibility::RenderLayers,
        NormalizedRenderTarget,
        RenderTarget,
        ScalingMode,
        Viewport,
    },
    input::{mouse::AccumulatedMouseMotion, InputSystems},
    picking::{
        pointer::{
            Location,
            PointerAction,
            PointerButton,
            PointerId,
            PointerInput,
            PointerLocation,
            PointerPressState,
        },
        PickingSystems,
    },
    prelude::*,
    render::render_resource::TextureFormat,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use crate::{
    controller::Player,
    monitor_material::{CrtSettings, MonitorMaterial},
    ui,
};

#[path = "previews.rs"]
mod previews;


// CODEX: settings replace SpawnComputerMenu; all appearance/input values can be authored in corn.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
pub struct Monitor {
    pub texture_size: u32,
    pub cursor_speed: f32,
    pub transition_seconds: f32,
    pub crt: CrtSettings,
}

// CODEX: keep the original render resolution and cursor feel.
impl Default for Monitor {
    // CODEX: corn can override these fields individually.
    fn default() -> Self {
        Self {
            texture_size: 512,
            cursor_speed: 0.4,
            transition_seconds: 0.4,
            crt: default(),
        }
    }
}

// CODEX: a corn-authored transform, relative to the screen, the main camera transitions toward.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct MonitorView;

// CODEX: marker on the authored bounds node; its parent mouse moves within this local XZ rectangle.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct MouseBounds;

// CODEX: runtime entities and image belong to one monitor, separate from its reflected settings.
// CLAUDE: reflected for scripts (dev.rs): `position` is the monitor's pointer
#[derive(Component, Reflect)]
#[reflect(Component)]
pub(crate) struct ComputerCanvas {
    image: Handle<Image>,
    pointer: PointerId,
    cursor: Entity,
    position: Vec2,
}

// CODEX: capture the authored rest position once, so mouse motion never accumulates or hardcodes it.
#[derive(Component)]
pub(crate) struct PhysicalMouse {
    rest: Vec3,
    bounds: Rect,
}

// CODEX: one local computer session; the original camera pose is restored on Escape.
// CLAUDE: reflected for scripts (dev.rs)
#[derive(Resource, Default, Reflect)]
#[reflect(Resource, Default)]
pub struct ComputerSession {
    pub active: Option<Entity>,
    pub returning: bool,
    original: Option<Transform>,
    from: Transform,
    elapsed: f32,
    duration: f32,
    release_on_return: bool,
}

// CODEX: process the synthetic pointer before Bevy picking, after actual input has arrived.
pub fn plugin(app: &mut App) {
    app.init_resource::<ComputerSession>()
        .register_type::<ComputerCanvas>()
        .register_type::<crate::ui::StartStatus>()
        .register_type::<Monitor>()
        .register_type::<MonitorView>()
        .register_type::<MouseBounds>()
        .add_systems(Update, (setup, setup_mice, update_session).chain())
        .add_systems(
            PreUpdate,
            pointer_input
                .after(InputSystems)
                .before(PickingSystems::ProcessInput),
        );
    previews::plugin(app);
}

// CODEX: create a render target and desktop once the corn-built screen primitive is in the world.
fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<MonitorMaterial>>,
    monitors: Query<(Entity, &Monitor), Without<ComputerCanvas>>,
) {
    for (entity, settings) in &monitors {
        let size = settings.texture_size.max(64);
        let image = images.add(Image::new_target_texture(
            size,
            size,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        // The pointer is private to the image target. No window events are forwarded recursively.
        let pointer = PointerId::Custom(Default::default());
        commands.spawn((Name::new("Computer pointer"), pointer, DespawnWith(entity)));
        let camera = commands
            .spawn((
                Name::new("Computer UI camera"),
                Camera2d,
                Camera {
                    order: -2,
                    clear_color: ClearColorConfig::Custom(Color::BLACK),
                    ..default()
                },
                RenderTarget::Image(image.clone().into()),
                RenderLayers::layer(1),
            ))
            .id();
        let desktop = ui::build(&mut commands, &assets, camera, size as f32);
        // Keep transform-independent UI and views attached for lifecycle cleanup with the lobby.
        commands.entity(desktop.root).insert(DespawnWith(entity));
        commands.entity(camera).insert(DespawnWith(entity));
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((
                MeshMaterial3d(materials.add(MonitorMaterial {
                    settings: settings.crt,
                    image: image.clone(),
                })),
                ComputerCanvas {
                    image,
                    pointer,
                    cursor: desktop.cursor,
                    position: Vec2::splat(size as f32 * 0.65),
                },
            ));
    }
}

// CODEX: a linked relationship owns cameras/UI without making them inherit screen transforms.
#[derive(Component)]
#[relationship(relationship_target = Dependents)]
struct DespawnWith(pub Entity);
// CODEX: Bevy handles cleanup when the monitor is removed, including hot-reloaded scene instances.
#[derive(Component)]
#[relationship_target(relationship = DespawnWith, linked_spawn)]
struct Dependents(Vec<Entity>);

// CODEX: the scoped bounds markers come from corn; runtime code only interprets their local rectangle.
fn setup_mice(
    mut commands: Commands,
    bounds: Query<(&Transform, &ChildOf), Added<MouseBounds>>,
    transforms: Query<&Transform>,
) {
    for (bound, parent) in &bounds {
        if let Ok(rest) = transforms.get(parent.parent()) {
            commands.entity(parent.parent()).insert(PhysicalMouse {
                rest: rest.translation,
                bounds: Rect::from_center_half_size(bound.translation.xz(), bound.scale.xz()),
            });
        }
    }
}

// CODEX: interaction requests focus; ease toward a corn-authored view, then restore on Escape (or E).
pub fn update_session(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<ComputerSession>,
    monitors: Query<&Monitor>,
    mut prompt: ResMut<crate::interaction::InteractionPrompt>,
    views: Query<(&GlobalTransform, &ChildOf), With<MonitorView>>,
    mut cameras: Query<&mut Transform, With<Player>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    no_mouse: Res<crate::controller::NoMouse>,
) {
    let Some(mut camera) = cameras.iter_mut().next() else {
        return;
    };
    // CODEX: sample before activation so the E completing `use` cannot immediately leave again.
    let leave_with_e = session.active.is_some() && keys.just_pressed(KeyCode::KeyE);
    if session.active.is_none() && !session.returning {
        if let Some(entity) = prompt.activate.take() {
            if let Ok(settings) = monitors.get(entity) {
                session.active = Some(entity);
                session.original = Some(*camera);
                session.from = *camera;
                session.elapsed = 0.0;
                session.duration = settings.transition_seconds;
                // CODEX: browser capture is owned by the DOM start/resume menu.
                if !cfg!(target_arch = "wasm32") && !no_mouse.on {
                    cursor.grab_mode = CursorGrabMode::Locked;
                    cursor.visible = false;
                }
            }
        }
    }
    if session.active.is_some()
        && (leave_with_e || keys.just_pressed(KeyCode::Escape) || !crate::browser::captured())
    {
        // CODEX: E preserves native capture too; Escape retains its release behavior.
        session.release_on_return = keys.just_pressed(KeyCode::Escape) || !crate::browser::captured();
        session.active = None;
        session.returning = true;
        session.from = *camera;
        session.elapsed = 0.0;
    }
    let (target, duration) = if let Some(entity) = session.active {
        let Some((target, _)) = views.iter().find(|(_, parent)| parent.parent() == entity) else {
            return;
        };
        (
            target.compute_transform(),
            monitors.get(entity).map_or(0.4, |m| m.transition_seconds),
        )
    } else if session.returning {
        (session.original.unwrap_or(*camera), session.duration)
    } else {
        return;
    };
    session.elapsed += time.delta_secs();
    let fraction = (session.elapsed / duration.max(0.001)).min(1.0);
    let ease = fraction * fraction * (3.0 - 2.0 * fraction);
    camera.translation = session.from.translation.lerp(target.translation, ease);
    camera.rotation = session.from.rotation.slerp(target.rotation, ease);
    if session.returning && fraction >= 1.0 {
        session.returning = false;
        // CODEX: resuming during this transition must not release the newly captured browser mouse.
        if !cfg!(target_arch = "wasm32") && session.release_on_return {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
    }
}

// CODEX: relative mouse motion drives both the on-screen cursor and the physical mouse model.
pub(crate) fn pointer_input(
    session: Res<ComputerSession>,
    motion: Res<AccumulatedMouseMotion>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut canvases: Query<(&Monitor, &mut ComputerCanvas)>,
    mut nodes: Query<&mut Node, With<ui::DesktopCursor>>,
    mut mice: Query<(&PhysicalMouse, &mut Transform)>,
    mut inputs: MessageWriter<PointerInput>,
    mut pointers: Query<(&PointerId, &mut PointerLocation, &mut PointerPressState)>,
    mut previous: Local<Option<Entity>>,
) {
    // CODEX: losing capture cancels picking before the menu consumes any pointer input.
    let Some(entity) = session.active.filter(|_| crate::browser::captured()) else {
        // CODEX: leaving while a button is held must clear picking/hover before the next session.
        if let Some(old) = previous.take() {
            if let Ok((_, canvas)) = canvases.get(old) {
                inputs.write(PointerInput::new(
                    canvas.pointer,
                    Location {
                        target: NormalizedRenderTarget::Image(canvas.image.clone().into()),
                        position: canvas.position,
                    },
                    PointerAction::Cancel,
                ));
                for (id, mut location, mut pressed) in &mut pointers {
                    if *id == canvas.pointer {
                        location.location = None;
                        *pressed = default();
                    }
                }
            }
        }
        return;
    };
    *previous = Some(entity);
    let Ok((settings, mut canvas)) = canvases.get_mut(entity) else {
        return;
    };
    let size = settings.texture_size.max(64) as f32;
    let old = canvas.position;
    canvas.position =
        (old + motion.delta * settings.cursor_speed).clamp(Vec2::ZERO, Vec2::splat(size - 1.0));
    if let Ok(mut node) = nodes.get_mut(canvas.cursor) {
        node.left = Val::Px(canvas.position.x);
        node.top = Val::Px(canvas.position.y);
    }
    for (physical, mut mouse) in &mut mice {
        let p = physical.bounds.min + canvas.position / size * physical.bounds.size();
        mouse.translation = physical.rest + Vec3::new(p.x, 0.0, p.y);
    }
    let location = Location {
        target: NormalizedRenderTarget::Image(canvas.image.clone().into()),
        position: canvas.position,
    };
    inputs.write(PointerInput::new(
        canvas.pointer,
        location.clone(),
        PointerAction::Move {
            delta: canvas.position - old,
        },
    ));
    for (button, pointer_button) in [
        (MouseButton::Left, PointerButton::Primary),
        (MouseButton::Right, PointerButton::Secondary),
    ] {
        if buttons.just_pressed(button) {
            inputs.write(PointerInput::new(
                canvas.pointer,
                location.clone(),
                PointerAction::Press(pointer_button),
            ));
        }
        if buttons.just_released(button) {
            inputs.write(PointerInput::new(
                canvas.pointer,
                location.clone(),
                PointerAction::Release(pointer_button),
            ));
        }
    }
}
