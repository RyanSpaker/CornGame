//! CODEX (GPT-6): lobby-only loose-item collision filtering, gentle kicks, and impact playback.
use std::collections::{HashMap, HashSet};

use avian3d::prelude::*;
use bevy::{audio::Volume, ecs::system::SystemParam, prelude::*};

use crate::hold::Held;

// CODEX: reflected resource fields can be edited through the existing dev console.
#[derive(Clone, Copy, Debug, Default, PartialEq, Reflect)]
pub enum KickMode {
    #[default]
    Ignore,
    Kick,
}

// CODEX: speed is a mass-independent horizontal target in m/s, rather than an unbounded impulse.
#[derive(Resource, Reflect)]
#[reflect(Resource, Default)]
pub struct KickSettings {
    pub mode: KickMode,
    pub speed: f32,
    pub lift: f32,
    pub spin: f32,
    pub reach: f32,
    pub height: f32,
    pub cooldown: f32,
    pub volume: f32,
}

// CODEX: an ordinary walk nudges props about a shoe-length away, with very little upward motion.
impl Default for KickSettings {
    // CODEX: ignore loose props by default until collisions and kick sounds have been playtested.
    fn default() -> Self {
        Self {
            mode: KickMode::Ignore,
            speed: 2.0,
            lift: 0.25,
            spin: 1.8,
            reach: 0.08,
            height: 0.4,
            cooldown: 0.35,
            volume: 0.5,
        }
    }
}

// CODEX: author each prop's material sound on its body in corn, independent of its mesh hierarchy.
#[derive(Component, Reflect)]
#[reflect(Component, Default)]
pub struct KickSound {
    pub sound: String,
    pub gain: f32,
}

// CODEX: future dynamic props get a quiet generic thud until given their own profile.
impl Default for KickSound {
    // CODEX: the generated asset is mono, so Bevy can spatialize it at the contact point.
    fn default() -> Self {
        Self {
            sound: "sounds/kicks/thud.ogg".into(),
            gain: 1.0,
        }
    }
}

// CODEX: movement reports contacts; a separate system owns velocity writes and sound spawning.
#[derive(Message)]
struct KickContact {
    body: Entity,
    direction: Vec3,
    point: Vec3,
}

// CODEX: one shared body lookup handles both root colliders and the lobby's fitted child colliders.
#[derive(SystemParam)]
pub struct LooseItems<'w, 's> {
    colliders: Query<'w, 's, (Entity, Option<&'static ColliderOf>, Has<Sensor>), With<Collider>>,
    bodies: Query<'w, 's, (&'static RigidBody, Has<Held>, Has<Sensor>)>,
    settings: Res<'w, KickSettings>,
    contacts: MessageWriter<'w, KickContact>,
}

// CODEX: exclude dynamics from walking/grounding but include them in the short kick sweep.
impl LooseItems<'_, '_> {
    // CODEX: exclusions must name collider entities; excluding only a prop root misses its children.
    pub fn filter(&self, player_body: Entity, include_dynamic: bool) -> SpatialQueryFilter {
        let excluded = std::iter::once(player_body).chain(self.colliders.iter().filter_map(
            |(entity, owner, sensor)| {
                let body = owner.map_or(entity, |owner| owner.body);
                let excluded = sensor
                    || self.bodies.get(body).is_ok_and(|(kind, held, sensor)| {
                        held || sensor || (!include_dynamic && *kind == RigidBody::Dynamic)
                    });
                excluded.then_some(entity)
            },
        ));
        SpatialQueryFilter::from_excluded_entities(excluded)
    }

    // CODEX: full shoe-height cylinder catches keys below the movement capsule; actual travel limits reach.
    pub fn sweep(
        &mut self,
        spatial: &SpatialQuery,
        player_body: Entity,
        from: Vec3,
        to: Vec3,
        radius: f32,
    ) {
        if self.settings.mode != KickMode::Kick {
            return;
        }
        let travel = (to - from).with_y(0.0);
        let Ok(direction) = Dir3::new(travel) else {
            return;
        };
        let height = self.settings.height.max(0.02);
        let shape = Collider::cylinder(radius, height);
        let origin = from + Vec3::Y * (height * 0.5 + 0.005);
        let filter = self.filter(player_body, true);
        let config =
            ShapeCastConfig::from_max_distance(travel.length() + self.settings.reach.max(0.0));
        let mut hits = spatial.shape_hits(
            &shape,
            origin,
            Quat::IDENTITY,
            direction,
            u32::MAX,
            &config,
            &filter,
        );
        hits.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        let mut visited = HashSet::new();
        for hit in hits {
            let Ok((_, owner, _)) = self.colliders.get(hit.entity) else {
                continue;
            };
            let body = owner.map_or(hit.entity, |owner| owner.body);
            // CODEX: a wall/door/step terminates the sweep, so reach cannot kick through geometry.
            if !self
                .bodies
                .get(body)
                .is_ok_and(|(kind, held, _)| *kind == RigidBody::Dynamic && !held)
            {
                break;
            }
            if visited.insert(body) {
                self.contacts.write(KickContact {
                    body,
                    direction: *direction,
                    point: hit.point1,
                });
            }
        }
    }
}

// CODEX: keep all tuning discoverable without adding controls to the gameplay HUD.
pub fn plugin(app: &mut App) {
    app.init_resource::<KickSettings>()
        .register_type::<KickSettings>()
        .register_type::<KickMode>()
        .register_type::<KickSound>()
        .add_message::<KickContact>()
        .add_systems(Update, apply.after(crate::controller::walk));
}

// CODEX: one kick per body/cooldown, even for many child meshes; sleeping bodies wake on contact.
fn apply(
    mut contacts: MessageReader<KickContact>,
    settings: Res<KickSettings>,
    time: Res<Time>,
    mut bodies: Query<
        (
            &RigidBody,
            &mut LinearVelocity,
            &mut AngularVelocity,
            Option<&KickSound>,
        ),
        Without<Held>,
    >,
    mut recent: Local<HashMap<Entity, f64>>,
    mut sounds: Local<HashMap<String, Handle<AudioSource>>>,
    assets: Option<Res<AssetServer>>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    recent.retain(|entity, last| {
        bodies.contains(*entity) && now - *last < settings.cooldown.max(0.0) as f64
    });
    for contact in contacts.read() {
        if settings.mode != KickMode::Kick || recent.contains_key(&contact.body) {
            continue;
        }
        let Ok((kind, mut velocity, mut angular, profile)) = bodies.get_mut(contact.body) else {
            continue;
        };
        if *kind != RigidBody::Dynamic {
            continue;
        }
        let delta = (settings.speed.max(0.0) - velocity.0.dot(contact.direction)).max(0.0);
        // CODEX: props already travelling away fast enough get neither a speed boost nor another sound.
        if delta < 0.05 {
            continue;
        }
        velocity.0 += contact.direction * delta;
        velocity.y = velocity.y.max(settings.lift.max(0.0));
        angular.0 += Vec3::Y.cross(contact.direction) * settings.spin.max(0.0);
        commands.entity(contact.body).remove::<Sleeping>();
        recent.insert(contact.body, now);

        let Some(assets) = assets.as_ref().filter(|_| settings.volume > 0.0) else {
            continue;
        };
        let fallback = KickSound::default();
        let profile = profile.unwrap_or(&fallback);
        if profile.sound.is_empty() {
            continue;
        }
        let sound = sounds
            .entry(profile.sound.clone())
            .or_insert_with(|| assets.load(profile.sound.clone()))
            .clone();
        // CODEX: short mono one-shots despawn themselves; subtle deterministic pitch changes avoid a looped feel.
        let variation = ((now * 137.0) as u32 % 7) as f32;
        commands.spawn((
            Name::new("CODEX item kick sound"),
            Transform::from_translation(contact.point),
            AudioPlayer::new(sound),
            PlaybackSettings::DESPAWN
                .with_spatial(true)
                .with_volume(Volume::Linear((settings.volume * profile.gain).max(0.0)))
                .with_speed(0.96 + variation * 0.013),
        ));
    }
}

// CODEX: use the real controller and Avian to verify ignoring, shoving, and wall occlusion.
#[cfg(test)]
#[path = "kick_tests.rs"]
mod tests;
