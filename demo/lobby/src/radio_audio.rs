//! CODEX — GPT-6: independent radio acquisition/clarity, regional reception and directional playback.
use bevy::{
    audio::{AudioSinkPlayback, SpatialAudioSink, Volume},
    prelude::*,
};
use super::{Radio, RadioFalloff, RadioSignal};

// CODEX (GPT-6): One-way session latch; optional audio waits until a tuning hint is actually shown.
#[derive(Resource, Default)]
pub(super) struct RadioAudioPreload {
    pub unlocked: bool,
}

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
    app.init_resource::<RadioAudioPreload>()
        .add_systems(Update, spawn_voices)
        .add_systems(PostUpdate, mix.after(TransformSystems::Propagate));
}

// CODEX: Every source is a mono looping AudioPlayer, initially silent to avoid a load-time burst.
fn spawn_voices(
    radios: Query<(Entity, &Radio)>,
    regions: Query<(Entity, &RadioSignal)>,
    voices: Query<&Voice>,
    assets: Res<AssetServer>,
    preload: Res<RadioAudioPreload>,
    mut commands: Commands,
) {
    for (entity, radio) in &radios {
        // CODEX (GPT-6): Before the hint, request only the broadcast nearest the initial dial setting.
        let default_station = radio
            .stations
            .iter()
            .enumerate()
            .filter(|(_, station)| !station.sound.is_empty())
            .min_by(|(_, a), (_, b)| {
                (a.frequency - radio.tuning)
                    .abs()
                    .total_cmp(&(b.frequency - radio.tuning).abs())
            })
            .map(|(index, _)| Source::Station(index));
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
            if !preload.unlocked && Some(kind) != default_station {
                continue;
            }
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

// CODEX (GPT-6): Linear proximity is combined with spatial strength before acquisition and clarity.
pub(super) fn reception(tuning: f32, frequency: f32, band: f32) -> f32 {
    (1.0 - (tuning - frequency).abs() / band.max(0.0001)).clamp(0.0, 1.0)
}

// CODEX (GPT-6): Reflected threshold pairs stay safe while edited, including reversed/equal endpoints.
fn response(proximity: f32, thresholds: Vec2) -> f32 {
    let low = thresholds.x.min(thresholds.y);
    let high = thresholds.x.max(thresholds.y);
    let t = ((proximity - low) / (high - low).max(0.0001)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// CODEX: Smooth distance cutoff for the speaker's audible range; regional boundaries use ray intersections.
pub(super) fn region_strength(distance: f32, inner: f32, outer: f32) -> f32 {
    let t = ((distance - inner) / (outer - inner).max(0.001)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

// CODEX (GPT-6): Acquisition keeps voices loud while clarity independently admits hiss; no power budget.
pub(super) fn weights(radio: &Radio, normal: &[f32], regions: &[f32]) -> (f32, Vec<f32>, Vec<f32>) {
    let regional = regions.iter().copied().fold(0.0, f32::max).clamp(0.0, 1.0);
    let clarity = normal
        .iter()
        .chain(regions.iter())
        .map(|q| response(*q, radio.clarity_thresholds))
        .fold(0.0, f32::max);
    let normal = normal
        .iter()
        .map(|q| response(*q, radio.acquisition_thresholds) * (1.0 - regional * 0.85))
        .collect();
    let regions = regions
        .iter()
        .map(|q| response(*q, radio.acquisition_thresholds))
        .collect();
    // CODEX: A little hiss remains in a weak competing signal, allowing a real three-way mix at 0.2.
    let hiss = 1.0 - clarity + 0.12 * regional * (1.0 - regional);
    (hiss, normal, regions)
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
        let (hiss, normal, regional) = weights(radio, &normal, &strengths);
        let source_gain = match voice.kind {
            Source::Static => hiss * radio.static_gain,
            Source::Station(i) => {
                normal.get(i).copied().unwrap_or(0.0)
                    * radio.stations.get(i).map_or(0.0, |s| s.gain)
                    * radio.broadcast_gain
            }
            Source::Region(entity) => signals
                .iter()
                .position(|(e, _, _)| *e == entity)
                .map_or(0.0, |i| regional[i] * signals[i].2 * radio.broadcast_gain),
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

// CODEX (GPT-6): Verify forgiving tuning, independent hiss, and regional overlaps without audio hardware.
#[cfg(test)]
mod tests {
    use super::*;
    // CODEX (GPT-6): Exercise actual asset-handle creation and tooltip visibility, without an audio device.
    #[test]
    fn optional_audio_waits_for_first_visible_tuning_hint() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: format!("{}/assets", env!("CARGO_MANIFEST_DIR")),
                ..default()
            },
        ))
        .init_asset::<AudioSource>()
        .init_asset_loader::<bevy::audio::AudioLoader>()
        .init_resource::<RadioAudioPreload>()
        .init_resource::<super::super::RadioTuning>()
        .init_resource::<crate::interaction::InteractionPrompt>()
        .init_resource::<crate::computer::ComputerSession>()
        .init_resource::<crate::post_it::PostItSession>()
        .add_systems(Update, (spawn_voices, super::super::hints).chain());
        app.world_mut().spawn((
            super::super::RadioHint(1),
            Text::new(""),
            Visibility::Hidden,
        ));
        let radio = app
            .world_mut()
            .spawn(Radio {
                stations: vec![
                    super::super::RadioStation {
                        frequency: 0.2,
                        sound: "sounds/radio/station_2.ogg".into(),
                        gain: 1.0,
                    },
                    super::super::RadioStation {
                        frequency: 0.5,
                        sound: "sounds/radio/station_4.ogg".into(),
                        gain: 1.0,
                    },
                ],
                ..default()
            })
            .id();
        app.world_mut().spawn(RadioSignal {
            frequency: 0.9,
            sound: "sounds/radio/near.ogg".into(),
            gain: 1.0,
        });
        app.update();
        let mut voices = app.world_mut().query::<&Voice>();
        assert_eq!(voices.iter(app.world()).count(), 1);
        assert!(matches!(
            voices.single(app.world()).unwrap().kind,
            Source::Station(1)
        ));
        assert!(!app.world().resource::<RadioAudioPreload>().unlocked);

        // CODEX: A target under the computer UI does not count as displaying the tuning hint.
        app.world_mut()
            .resource_mut::<crate::interaction::InteractionPrompt>()
            .target = Some(radio);
        app.world_mut()
            .resource_mut::<crate::computer::ComputerSession>()
            .returning = true;
        app.update();
        assert!(!app.world().resource::<RadioAudioPreload>().unlocked);
        assert_eq!(voices.iter(app.world()).count(), 1);
        app.world_mut()
            .resource_mut::<crate::computer::ComputerSession>()
            .returning = false;
        app.update();
        assert!(app.world().resource::<RadioAudioPreload>().unlocked);
        app.update();
        assert_eq!(
            voices.iter(app.world()).count(),
            4,
            "all station, static and regional voices now load"
        );

        // CODEX: Looking away never resets the latch or spawns duplicate looping voices.
        app.world_mut()
            .resource_mut::<crate::interaction::InteractionPrompt>()
            .target = None;
        app.update();
        assert!(app.world().resource::<RadioAudioPreload>().unlocked);
        assert_eq!(voices.iter(app.world()).count(), 4);
    }

    // CODEX: Normal station centers are exact, with pure static between nonoverlapping bands.
    #[test]
    fn dial_centers_and_static_gaps() {
        for f in [0.1, 0.2, 0.3, 0.5, 0.8] {
            assert_eq!(reception(f, f, 0.05), 1.0);
        }
        assert_eq!(reception(0.4, 0.5, 0.05), 0.0);
        assert!(reception(0.52, 0.5, 0.05) > reception(0.54, 0.5, 0.05));
        assert_eq!(weights(&Radio::default(), &[0.0; 5], &[]).0, 1.0);
    }
    // CODEX (GPT-6): Hiss increases before acquisition decreases, with a genuinely flat clean center.
    #[test]
    fn clean_plateau_and_early_full_gain() {
        let radio = Radio::default();
        for offset in [-0.012, -0.005, 0.0, 0.005, 0.012] {
            let q = reception(0.5 + offset, 0.5, radio.station_band);
            let (hiss, station, _) = weights(&radio, &[q], &[]);
            assert_eq!(station, [1.0]);
            assert_eq!(hiss, 0.0);
        }
        let (hiss, station, _) = weights(&radio, &[reception(0.525, 0.5, 0.05)], &[]);
        assert!((hiss - 0.5).abs() < 1e-5);
        assert_eq!(station, [1.0], "hiss does not spend the station's gain");
        let (hiss, station, _) = weights(&radio, &[reception(0.535, 0.5, 0.05)], &[]);
        assert!(hiss > 0.9 && station[0] > 0.0 && station[0] < 1.0);
    }

    // CODEX (GPT-6): Sweep the actual dial to detect discontinuities or asymmetric response.
    #[test]
    fn tuning_is_symmetric_continuous_and_monotonic() {
        let radio = Radio::default();
        let mut previous = (0.0, 1.0);
        for step in 0..=500 {
            let offset = step as f32 * 0.0001;
            let left = weights(&radio, &[reception(0.5 - offset, 0.5, 0.05)], &[]);
            let right = weights(&radio, &[reception(0.5 + offset, 0.5, 0.05)], &[]);
            assert!((left.0 - right.0).abs() < 1e-5);
            assert!((left.1[0] - right.1[0]).abs() < 1e-5);
            assert!(left.0 + 1e-6 >= previous.0 && left.1[0] <= previous.1 + 1e-6);
            assert!((left.0 - previous.0).abs() < 0.02);
            assert!((left.1[0] - previous.1).abs() < 0.02);
            previous = (left.0, left.1[0]);
        }
        assert_eq!(previous, (1.0, 0.0));
    }

    // CODEX (GPT-6): Layered secrets retain independent amplitudes and the intentional interference hiss.
    #[test]
    fn three_way_mix_preserves_layers_without_normalization() {
        let radio = Radio::default();
        let (hiss, normal, secret) = weights(&radio, &[1.0], &[0.5, 0.2]);
        assert!(hiss > 0.0 && normal[0] > 0.0 && secret.iter().all(|v| *v > 0.0));
        assert!((hiss - 0.03).abs() < 1e-6);
        assert!((normal[0] - 0.575).abs() < 1e-6);
        assert_eq!(secret[0], 1.0);
        let single = weights(&radio, &[1.0], &[0.5]);
        assert_eq!(hiss, single.0);
        assert_eq!(normal, single.1);
        assert_eq!(
            secret[0], single.2[0],
            "adding a weaker layer cannot duck existing voices"
        );
    }

    // CODEX (GPT-6): Spatial strength enters the curves, giving loud-but-noisy regional edge reception.
    #[test]
    fn regional_edges_use_the_same_acquisition_and_clarity() {
        let radio = Radio::default();
        let inner = GlobalTransform::from(Transform::from_scale(Vec3::splat(2.0)));
        let outer = GlobalTransform::from(Transform::from_scale(Vec3::splat(4.0)));
        for (distance, expected_gain, expected_hiss) in [
            (2.0, 1.0, 0.0),
            (3.0, 1.0, 0.53),
            (4.0, 0.0, 1.0),
            (5.0, 0.0, 1.0),
        ] {
            let q = reception(0.9, 0.9, radio.station_band)
                * super::super::region::strength(Vec3::X * distance, &inner, &outer);
            let (hiss, _, regions) = weights(&radio, &[], &[q]);
            assert!((regions[0] - expected_gain).abs() < 1e-5);
            assert!((hiss - expected_hiss).abs() < 1e-5);
        }
        let q = reception(0.925, 0.9, radio.station_band) * 0.5;
        let (_, _, regions) = weights(&radio, &[], &[q]);
        assert!(
            regions[0] < 0.5,
            "dial and spatial weaknesses combine before acquisition"
        );
    }

    // CODEX (GPT-6): Console tuning changes the response without changing speaker volume or bandwidth.
    #[test]
    fn reflected_thresholds_control_gain_and_clarity_independently() {
        let mut radio = Radio::default();
        let baseline = weights(&radio, &[0.5], &[]);
        radio.clarity_thresholds = Vec2::new(0.1, 0.4);
        let cleaner = weights(&radio, &[0.5], &[]);
        assert_eq!(cleaner.0, 0.0);
        assert_eq!(cleaner.1, baseline.1);
        radio.acquisition_thresholds = Vec2::new(0.4, 0.8);
        let weaker = weights(&radio, &[0.5], &[]);
        assert_eq!(weaker.0, cleaner.0);
        assert!(weaker.1[0] < cleaner.1[0]);
        assert_eq!(response(0.5, Vec2::new(0.8, 0.4)), weaker.1[0]);
        assert!(response(0.5, Vec2::splat(0.5)).is_finite());
        assert!(reception(0.5, 0.5, 0.0).is_finite());
    }

    // CODEX (GPT-6): Exercise the runtime mixer, source gains and smoothing without a physical audio sink.
    #[test]
    fn mixer_separates_broadcast_and_static_levels_and_preserves_power_fade() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs_f32(0.1),
            ))
            .add_systems(Update, mix);
        let radio = app
            .world_mut()
            .spawn((
                Radio {
                    tuning: 0.9,
                    broadcast_gain: 0.4,
                    static_gain: 0.7,
                    stations: vec![super::super::RadioStation {
                        frequency: 0.925,
                        gain: 0.6,
                        ..default()
                    }],
                    ..default()
                },
                GlobalTransform::from_translation(Vec3::X * 3.0),
            ))
            .id();
        app.world_mut().spawn((
            SpatialListener::default(),
            GlobalTransform::from_translation(Vec3::new(3.0, 0.0, 1.0)),
        ));
        let region = app
            .world_mut()
            .spawn((
                RadioSignal {
                    frequency: 0.9,
                    gain: 0.8,
                    ..default()
                },
                GlobalTransform::from(Transform::from_scale(Vec3::splat(2.0))),
            ))
            .id();
        app.world_mut().spawn((
            RadioFalloff,
            ChildOf(region),
            GlobalTransform::from(Transform::from_scale(Vec3::splat(4.0))),
        ));
        let hiss = app
            .world_mut()
            .spawn((
                Voice {
                    radio,
                    kind: Source::Static,
                    gain: 0.0,
                },
                PlaybackSettings::LOOP,
            ))
            .id();
        let station = app
            .world_mut()
            .spawn((
                Voice {
                    radio,
                    kind: Source::Region(region),
                    gain: 0.0,
                },
                PlaybackSettings::LOOP,
            ))
            .id();
        // CODEX (GPT-6): Both broadcast kinds share the group knob, while per-source gains still apply.
        let normal = app
            .world_mut()
            .spawn((
                Voice {
                    radio,
                    kind: Source::Station(0),
                    gain: 0.0,
                },
                PlaybackSettings::LOOP,
            ))
            .id();
        for _ in 0..20 {
            app.update();
        }
        let speaker_gain = Radio::default().volume / (1.0 + (1.0_f32 / 3.0).powi(2));
        let hiss_gain = app.world().get::<Voice>(hiss).unwrap().gain;
        let station_gain = app.world().get::<Voice>(station).unwrap().gain;
        let normal_gain = app.world().get::<Voice>(normal).unwrap().gain;
        assert!((station_gain - speaker_gain * 0.8 * 0.4).abs() < 1e-5);
        assert!((normal_gain - speaker_gain * 0.6 * 0.575 * 0.4).abs() < 1e-5);
        assert!((hiss_gain - speaker_gain * 0.7 * 0.53).abs() < 1e-5);
        // CODEX (GPT-6): Live broadcast changes affect both broadcasts and leave static unchanged.
        app.world_mut()
            .get_mut::<Radio>(radio)
            .unwrap()
            .broadcast_gain = 0.2;
        for _ in 0..20 {
            app.update();
        }
        assert!(
            (app.world().get::<Voice>(station).unwrap().gain - station_gain * 0.5).abs() < 1e-5
        );
        assert!((app.world().get::<Voice>(normal).unwrap().gain - normal_gain * 0.5).abs() < 1e-5);
        assert!((app.world().get::<Voice>(hiss).unwrap().gain - hiss_gain).abs() < 1e-5);
        // CODEX (GPT-6): Live hiss changes leave both broadcast amplitudes unchanged.
        app.world_mut().get_mut::<Radio>(radio).unwrap().static_gain = 0.35;
        for _ in 0..20 {
            app.update();
        }
        assert!(
            (app.world().get::<Voice>(station).unwrap().gain - station_gain * 0.5).abs() < 1e-5
        );
        assert!((app.world().get::<Voice>(normal).unwrap().gain - normal_gain * 0.5).abs() < 1e-5);
        assert!((app.world().get::<Voice>(hiss).unwrap().gain - hiss_gain * 0.5).abs() < 1e-5);
        app.world_mut().get_mut::<Radio>(radio).unwrap().on = false;
        app.update();
        let fading = app.world().get::<Voice>(station).unwrap().gain;
        assert!(fading > 0.0 && fading < station_gain);
        for _ in 0..20 {
            app.update();
        }
        assert!(app.world().get::<Voice>(station).unwrap().gain < 1e-5);
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
