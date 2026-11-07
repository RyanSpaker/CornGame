//! CODEX (GPT-6): original imported preview scene and camera-to-texture path; animation lives in glTF.
use super::*;

// CODEX: original view framing and the local stand-in for the network player count.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
pub struct PreviewSettings {
    pub players: usize,
    // CODEX (GPT-6): display consumers can own the count without depending on a networking type.
    pub editable: bool,
    pub ortho_height: f32,
    pub view_center: Vec3,
    pub camera_distance: f32,
    pub viewport_origin: UVec2,
    pub viewport_size: UVec2,
}

// CODEX: use the source camera framing rather than cropping and resizing the models.
impl Default for PreviewSettings {
    // CODEX: all four source spin clips last 1.25 seconds; the clip remains the timing authority.
    fn default() -> Self {
        Self {
            players: 1,
            editable: true,
            ortho_height: 2.0,
            view_center: Vec3::ZERO,
            camera_distance: 1.0,
            viewport_origin: UVec2::ZERO,
            viewport_size: UVec2::splat(512),
        }
    }
}

// CODEX: the same per-player visibility marker as the original menu; it never changes transforms.
// CLAUDE: reflected for scripts (dev.rs)
#[derive(Component, Reflect)]
#[reflect(Component)]
pub(super) struct PreviewModel {
    pub slot: usize,
}

// CODEX: setup marker preventing duplicate views and model initialization.
#[derive(Component)]
pub(super) struct PreviewReady;

// CODEX: install preview-only systems, separately from monitor interaction and UI construction.
pub(super) fn plugin(app: &mut App) {
    app.register_type::<PreviewSettings>().register_type::<PreviewModel>().add_systems(
        Update,
        (setup_previews, update_players)
            .chain()
            .after(setup)
            .before(update_session),
    );
}

// CODEX: preserve the exported arrangement in the original full-texture orthographic view.
fn setup_previews(
    mut commands: Commands,
    previews: Query<(Entity, &PreviewSettings), Without<PreviewReady>>,
    canvases: Query<(&ComputerCanvas, &Monitor)>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    let Some((canvas, monitor)) = canvases.iter().next() else {
        return;
    };
    for (root, settings) in &previews {
        commands.entity(root).insert(PreviewReady);
        let layer = RenderLayers::layer(2);
        for child in children.iter_descendants(root) {
            commands.entity(child).insert(layer.clone());
            let Ok(name) = names.get(child) else {
                continue;
            };
            let Some(slot) = name
                .as_str()
                .strip_prefix("guy")
                .and_then(|s| s.parse::<usize>().ok())
                .and_then(|s| s.checked_sub(1))
            else {
                continue;
            };
            commands.entity(child).insert(PreviewModel { slot });
        }
        let k = monitor.texture_size.max(64) as f32 / 512.0;
        commands.spawn((
            Name::new("Menu player preview camera"),
            Camera3d::default(),
            Camera {
                order: -1,
                clear_color: ClearColorConfig::None,
                viewport: Some(Viewport {
                    physical_position: (settings.viewport_origin.as_vec2() * k).as_uvec2(),
                    physical_size: (settings.viewport_size.as_vec2() * k).as_uvec2(),
                    ..default()
                }),
                ..default()
            },
            RenderTarget::Image(canvas.image.clone().into()),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::Fixed {
                    width: settings.ortho_height * settings.viewport_size.x as f32
                        / settings.viewport_size.y.max(1) as f32,
                    height: settings.ortho_height,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::from_translation(settings.view_center + Vec3::X * settings.camera_distance)
                .looking_at(settings.view_center, Vec3::Y),
            // CODEX: use the same default MSAA as the original UI camera so both share a target.
            layer.clone(),
            DespawnWith(root),
        ));
        commands.spawn((
            Name::new("Menu player fill light"),
            DirectionalLight {
                illuminance: 1000.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, 0.5, -0.8, 0.0)),
            layer,
            DespawnWith(root),
        ));
    }
}

// CODEX: match the original GuysPlugin: update count/visibility only, leaving animation to Bevy.
fn update_players(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: Query<&mut PreviewSettings>,
    mut models: Query<(&PreviewModel, &mut Visibility)>,
    mut counts: Query<&mut Text, With<ui::PlayerCount>>,
) {
    let Some(mut settings) = settings.iter_mut().next() else {
        return;
    };
    if settings.editable && keys.just_pressed(KeyCode::PageUp) {
        settings.players = (settings.players + 1).min(4);
    }
    if settings.editable && keys.just_pressed(KeyCode::PageDown) {
        settings.players = settings.players.saturating_sub(1);
    }
    for (model, mut visibility) in &mut models {
        *visibility = if model.slot < settings.players {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for mut text in &mut counts {
        text.0 = settings.players.to_string();
    }
}
