//! CODEX — GPT-6.1: bounded-power radio mixing, regional reception and directional spatial playback.
use bevy::{
    audio::{AudioSinkPlayback, SpatialAudioSink, Volume},
    prelude::*,
};
use super::{Radio, RadioFalloff, RadioSignal};

// CODEX: Each looping source stays at the radio, even when a signal originates far away.
#[derive(Component)]
struct Voice {
    radio: Entity,
    kind: Source,
    gain: f32,
}
// CODEX: Region identity is stable if marker query ordering changes.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    Static,
    Station(usize),
    Region(Entity),
}
// CODEX: Corn can spawn before audio/markers finish loading; voices attach incrementally.
pub fn plugin(app: &mut App) {
    app.add_systems(Update, spawn_voices)
        .add_systems(PostUpdate, mix.after(TransformSystems::Propagate));
}

// CODEX: Every source is a mono looping AudioPlayer, initially silent to avoid a load-time burst.
fn spawn_voices(
    radios: Query<(Entity, &Radio)>,
    regions: Query<(Entity, &RadioSignal)>,
    voices: Query<&Voice>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    for (entity, radio) in &radios {
        let desired = std::iter::once((Source::Static, radio.static_sound.as_str()))
            .chain(
                radio
                    .stations
                    .iter()
                    .enumerate()
                    .map(|(i, s)| (Source::Station(i), s.sound.as_str())),
            )
            .chain(
                regions
                    .iter()
                    .map(|(e, s)| (Source::Region(e), s.sound.as_str())),
            );
        for (kind, path) in desired {
            if path.is_empty() || voices.iter().any(|v| v.radio == entity && v.kind == kind) {
                continue;
            }
            commands.spawn((
                Name::new("CODEX radio spatial voice"),
                ChildOf(entity),
                Transform::default(),
                Voice {
                    radio: entity,
                    kind,
                    gain: 0.0,
                },
                AudioPlayer::new(assets.load(path.to_owned())),
                PlaybackSettings::LOOP
                    .with_spatial(true)
                    .with_volume(Volume::Linear(0.0)),
            ));
        }
    }
}

// CODEX: Reception falls nonlinearly to zero across five percent of the whole dial on each side.
pub(super) fn reception(tuning: f32, frequency: f32, band: f32) -> f32 {
    (1.0 - (tuning - frequency).abs() / band.max(0.0001))
        .clamp(0.0, 1.0)
        .powf(1.5)
}

// CODEX: Smooth distance cutoff for the speaker's audible range; regional boundaries use ray intersections.
pub(super) fn region_strength(distance: f32, inner: f32, outer: f32) -> f32 {
    let t = ((distance - inner) / (outer - inner).max(0.001)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

// CODEX: Normalize energy, then take square roots for equal-power mixing; no volume pile-up in overlaps.
pub(super) fn weights(normal: &[f32], regions: &[f32]) -> (f32, Vec<f32>, Vec<f32>) {
    let regional = regions.iter().copied().fold(0.0, f32::max).clamp(0.0, 1.0);
    let normal_peak = normal.iter().copied().fold(0.0, f32::max).clamp(0.0, 1.0);
    let mut normal: Vec<_> = normal.iter().map(|q| q * (1.0 - regional * 0.85)).collect();
    let mut regions = regions.to_vec();
    // CODEX: A little hiss remains in a weak competing signal, allowing a real three-way mix at 0.2.
    let hiss = (1.0 - normal_peak.max(regional) + 0.12 * regional * (1.0 - regional)).max(0.0);
    let total = (hiss + normal.iter().sum::<f32>() + regions.iter().sum::<f32>()).max(0.0001);
    for q in normal.iter_mut().chain(regions.iter_mut()) {
        *q = (*q / total).sqrt();
    }
    ((hiss / total).sqrt(), normal, regions)
}

// CODEX: Panning uses Bevy spatial sinks; simple custom gain supplies source direction and range.
fn mix(
    radios: Query<(&Radio, &GlobalTransform)>,
    regions: Query<(Entity, &RadioSignal, &GlobalTransform)>,
    falloffs: Query<(&ChildOf, &GlobalTransform), With<RadioFalloff>>,
    listeners: Query<&GlobalTransform, With<SpatialListener>>,
    mut voices: Query<(
        &mut Voice,
        &mut PlaybackSettings,
        Option<&mut SpatialAudioSink>,
    )>,
    time: Res<Time>,
) {
    let Some(listener) = listeners.iter().next() else {
        return;
    };
    for (mut voice, mut settings, sink) in &mut voices {
        let Ok((radio, global)) = radios.get(voice.radio) else {
            continue;
        };
        let normal: Vec<_> = radio
            .stations
            .iter()
            .map(|s| reception(radio.tuning, s.frequency, radio.station_band))
            .collect();
        let signals: Vec<_> = regions
            .iter()
            .map(|(entity, signal, marker)| {
                let outer = falloffs
                    .iter()
                    .find(|(p, _)| p.parent() == entity)
                    .map_or(marker, |(_, g)| g);
                (
                    entity,
                    reception(radio.tuning, signal.frequency, radio.station_band)
                        * super::region::strength(global.translation(), marker, outer),
                    signal.gain,
                )
            })
            .collect();
        let strengths: Vec<_> = signals.iter().map(|(_, q, _)| *q).collect();
        let (hiss, normal, regional) = weights(&normal, &strengths);
        let source_gain = match voice.kind {
            Source::Static => hiss * radio.static_gain,
            Source::Station(i) => {
                normal.get(i).copied().unwrap_or(0.0)
                    * radio.stations.get(i).map_or(0.0, |s| s.gain)
            }
            Source::Region(entity) => signals
                .iter()
                .position(|(e, _, _)| *e == entity)
                .map_or(0.0, |i| regional[i] * signals[i].2),
        };
        let to_listener = listener.translation() - global.translation();
        let distance = to_listener.length();
        // CODEX: Blender's source -Y becomes +Z in Y-up glTF; preserve the authored facing direction.
        let forward = global.rotation()
            * (Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2) * radio.source_forward)
                .normalize_or_zero();
        let facing = (forward.dot(to_listener.normalize_or_zero()) + 1.0) * 0.5;
        let directional = radio.rear_gain + (1.0 - radio.rear_gain) * facing * facing;
        let attenuation = 1.0 / (1.0 + (distance / radio.distance_reference.max(0.1)).powi(2));
        let cutoff = region_strength(distance, radio.audio_range * 0.8, radio.audio_range);
        let target = if radio.on {
            source_gain * radio.volume * directional * attenuation * cutoff
        } else {
            0.0
        };
        let alpha = 1.0 - (-time.delta_secs() / radio.fade_seconds.max(0.001)).exp();
        voice.gain += (target - voice.gain) * alpha;
        settings.volume = Volume::Linear(voice.gain);
        if let Some(mut sink) = sink {
            sink.set_volume(settings.volume);
            if !radio.on && voice.gain < 0.0001 {
                sink.pause();
            } else {
                sink.play();
            }
        }
    }
}

// CODEX: Verify dial centers, dead bands, signal edges, and bounded three-way overlap without audio hardware.
#[cfg(test)]
mod tests {
    use super::*;
    // CODEX: Normal station centers are exact, with pure static between nonoverlapping bands.
    #[test]
    fn dial_centers_and_static_gaps() {
        for f in [0.1, 0.2, 0.3, 0.5, 0.8] {
            assert_eq!(reception(f, f, 0.05), 1.0);
        }
        assert_eq!(reception(0.4, 0.5, 0.05), 0.0);
        assert!(reception(0.52, 0.5, 0.05) > reception(0.54, 0.5, 0.05));
        assert_eq!(weights(&[0.0; 5], &[]).0, 1.0);
    }
    // CODEX: Overlapping secrets cannot make the mixed power exceed one.
    #[test]
    fn three_way_mix_is_bounded() {
        let (hiss, normal, secret) = weights(&[1.0], &[0.4, 0.2]);
        assert!(hiss > 0.0 && normal[0] > 0.0 && secret.iter().all(|v| *v > 0.0));
        let power = hiss * hiss
            + normal
                .iter()
                .chain(secret.iter())
                .map(|v| v * v)
                .sum::<f32>();
        assert!((power - 1.0).abs() < 0.0001);
    }
    // CODEX: Inner and outer boundaries are inclusive, continuous and monotonic.
    #[test]
    fn region_falloff_edges() {
        assert_eq!(region_strength(1.0, 2.0, 4.0), 1.0);
        assert_eq!(region_strength(2.0, 2.0, 4.0), 1.0);
        assert_eq!(region_strength(3.0, 2.0, 4.0), 0.5);
        assert_eq!(region_strength(4.0, 2.0, 4.0), 0.0);
    }
}
