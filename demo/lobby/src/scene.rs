//! CODEX (GPT-6): small reusable animation/orbit adapters, replacing the game's scene infrastructure.
use bevy::prelude::*;

// CODEX: clip selection and repetition are authored in corn, using Bevy's native animation graph.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct AutoPlay {
    pub clip: String,
    pub clips: Vec<String>,
    pub repeat: bool,
}

// CODEX: preserve the authored apparent size while placing a sky object far from the cabin.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct Orbit {
    pub distance: f32,
    pub radians_per_second: f32,
}

// CODEX: install only what the lobby's exported metadata used.
pub fn plugin(app: &mut App) {
    app.register_type::<AutoPlay>()
        .register_type::<Orbit>()
        .register_type::<WebShadow>()
        .add_systems(Update, (play, orbit, web_shadows));
}

// CLAUDE (claude-sonnet-5-5): WebGL2 has one shadow-map layer, shared by every shadowed light. The overhead
// spot is the only light marked with this; on the web build every other light has shadows turned off.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct WebShadow;

// CLAUDE: lights get shadows from corn patches that can land after spawn, so check on every change.
#[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables, unused_mut))]
fn web_shadows(
    mut directional: Query<&mut DirectionalLight, Changed<DirectionalLight>>,
    mut spots: Query<(&mut SpotLight, Has<WebShadow>), Changed<SpotLight>>,
    mut points: Query<(&mut PointLight, Has<WebShadow>), Changed<PointLight>>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        for mut light in &mut directional {
            if light.shadow_maps_enabled { light.shadow_maps_enabled = false; }
        }
        for (mut light, keep) in &mut spots {
            if light.shadow_maps_enabled && !keep { light.shadow_maps_enabled = false; }
        }
        for (mut light, keep) in &mut points {
            if light.shadow_maps_enabled && !keep { light.shadow_maps_enabled = false; }
        }
    }
}


// CODEX: the cornfile attaches AutoPlay to the authored animation root referenced by glTF targets.
fn play(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    new: Query<(Entity, &AutoPlay), Added<AutoPlay>>,
) {
    for (entity, settings) in &new {
        // CODEX: match the original AnimationContext: one graph plays all clips for its hierarchy.
        let paths = settings
            .clips
            .iter()
            .cloned()
            .chain((!settings.clip.is_empty()).then(|| settings.clip.clone()));
        let (graph, nodes) = AnimationGraph::from_clips(paths.map(|path| assets.load(path)));
        let mut player = AnimationPlayer::default();
        for node in nodes {
            let playing = player.play(node);
            if settings.repeat {
                playing.repeat();
            }
        }
        commands
            .entity(entity)
            .insert((AnimationGraphHandle(graphs.add(graph)), player));
    }
}

// CODEX: startup size correction is applied once per spawned moon, then it revolves around the cabin.
fn orbit(time: Res<Time>, mut bodies: Query<(&Orbit, &mut Transform)>) {
    for (settings, mut transform) in &mut bodies {
        let distance = transform.translation.length();
        if distance <= f32::EPSILON {
            continue;
        }
        let factor = settings.distance / distance;
        if (factor - 1.0).abs() > 0.0001 {
            transform.translation *= factor;
            transform.scale *= factor;
        }
        let axis = transform.local_x();
        transform.rotate_around(
            Vec3::ZERO,
            Quat::from_axis_angle(*axis, settings.radians_per_second * time.delta_secs()),
        );
        *transform = transform.looking_at(Vec3::ZERO, Vec3::Y);
    }
}
