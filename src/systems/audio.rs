use std::{marker::PhantomData, time::Duration};

use avian3d::prelude::{Collision, CollisionEnded, CollisionStarted};
use bevy::{audio::Volume, prelude::*};
use wgpu::core::command;

use crate::{
    ecs::{
        corn::field::cf_image_carved::CornSensor,
        flycam::FlyCamMoveEvent,
        main_camera::MainCamera,
    },
    util::lerp,
};

use super::character::Player;

#[derive(Component, PartialEq)]
enum WindNoise {
    Wind,
    Rustle,
}

fn wind_volume(time: Res<Time>, settings: Query<(&mut AudioSink, &PlaybackSettings, &WindNoise)>) {
    for (s, initial, kind) in settings.iter() {
        let mut t = time.elapsed_secs();

        let mut min_vol = 0.12;
        let mut power = 2.0;
        if *kind == WindNoise::Rustle {
            // delay rustle
            t -= 2.0;
            min_vol = 0.0;
            power = 3.0;
        }

        // wind pattern: https://www.desmos.com/calculator/023vwitwiq
        let strength = (t / 3.0).cos() * (t / 5.2).cos() / 2.0 + 0.5; //matches wind.glsl
        let a = lerp(min_vol, 1.0, strength.powf(power));
        if a <= 0.0 {
            s.set_volume(0.0);
        } else {
            let db = 10.0 * a.log2();
            let adj = 10f32.powf(db / 20.0);
            s.set_volume(adj * initial.volume.get());
        }
    }
}

/// TODO https://github.com/vleue/bevy_easings
#[derive(Component)]
struct Footsteps {
    lerp: f32,
    lerp_speed: f32,

    /// speed [on_path, in_corn]
    s: [f32; 2],
    _s_lerp_seconds: f32,

    /// volume [on_path, in_corn]
    v: [f32; 2],
    _v_lerp_seconds: f32,
}

impl Default for Footsteps {
    fn default() -> Self {
        Self {
            lerp: 0.0,
            lerp_speed: 0.0,
            s: [1.0, 1.0],
            _s_lerp_seconds: 0.2,
            v: [1.0, 1.0],
            _v_lerp_seconds: 0.2,
        }
    }
}

fn play_footsteps(
    time: Res<Time>,
    move_events: EventReader<FlyCamMoveEvent>,
    camera: Query<(&CornSensor, &Transform), With<MainCamera>>,

    mut settings: Query<(&mut AudioSink, &PlaybackSettings, &mut Footsteps)>,
) {
    let moving = !move_events.is_empty();
    let Ok((sensor, t)) = camera.get_single() else {
        return;
    };
    let flying = t.translation.y > 2.0; //MOVEME

    for (s, initial, mut fs) in settings.iter_mut() {
        let speed = lerp(fs.s[0], fs.s[1], sensor.is_in_corn);
        let volume = match !moving || flying {
            true => 0.0,
            false => lerp(fs.v[0], fs.v[1], sensor.is_in_corn),
        };

        let diff = volume - fs.lerp;
        let delta = time.delta_secs() / 0.2;
        fs.lerp += f32::min(diff.abs(), delta) * diff.signum();

        let diff = speed - fs.lerp_speed;
        let delta = time.delta_secs() / 0.2;
        fs.lerp_speed += f32::min(diff.abs(), delta) * diff.signum();

        s.set_speed(fs.lerp_speed * initial.speed);
        s.set_volume(fs.lerp * initial.volume.get());
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        AudioPlayer::<AudioSource>(asset_server.load("sounds/wind.ogg")),
        PlaybackSettings {
            mode: bevy::audio::PlaybackMode::Loop,
            volume: Volume::new(0.8),
            ..Default::default()
        },
        WindNoise::Wind,
    ));

    commands.spawn((
        AudioPlayer::<AudioSource>(asset_server.load("sounds/wind_rustle.ogg")),
        PlaybackSettings {
            mode: bevy::audio::PlaybackMode::Loop,
            volume: Volume::new(0.2),
            ..Default::default()
        },
        WindNoise::Rustle,
    ));

    commands.spawn((
        AudioPlayer::<AudioSource>(asset_server.load("sounds/footstep_leaves.ogg")),
        PlaybackSettings {
            mode: bevy::audio::PlaybackMode::Loop,
            volume: Volume::new(0.4),
            ..Default::default()
        },
        Footsteps {
            s: [1.2, 0.7],
            v: [0.2, 1.0],
            ..Default::default()
        },
    ));

    // commands.spawn((
    //     AudioBundle {
    //         source: asset_server.load("sounds/corn_no_wind.ogg"),
    //         settings: PlaybackSettings {
    //             mode: bevy::audio::PlaybackMode::Loop,
    //             volume: Volume::new(0.4),
    //             ..Default::default()
    //         },
    //     },
    // ));

    // commands.spawn((
    //     AudioBundle {
    //         source: asset_server.load("sounds/wind_rustle.ogg"),
    //         settings: PlaybackSettings {
    //             mode: bevy::audio::PlaybackMode::Loop,
    //             volume: Volume::new(0.1),
    //             ..Default::default()
    //         },
    //     },
    //     Footsteps {
    //         v: [0.0,1.0],
    //         v_lerp_seconds: 1.0,
    //         ..Default::default()
    //     },
    // ));
}

pub struct MyAudioPlugin;
impl Plugin for MyAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (wind_volume, play_footsteps))
            .add_systems(Startup, setup);
        app.register_type::<SoundTrackOnEnter>();
        app.register_type::<Soundtrack>();
        app.add_systems(Update, (fade));
        app.add_systems(Update, SoundTrackOnEnter::observer);
        app.add_systems(PostUpdate,SoundTrackOnEnter::attenuate);//PostUpdate so it can reliably get RemovedComponent<Soundtrack>
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct Soundtrack;

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[require(DelayCounter)]
pub struct SoundTrackOnEnter {
    track: String,
    delay: Option<f32>,
    fade_on_leave: bool,
}

#[derive(Debug, Default, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct DelayCounter<T=()>{
    counter: f32,
    // idea is that a single entity might need multiple of these.
    _dummy: PhantomData<T>
}

impl SoundTrackOnEnter {
    /// system to play a track when entering a avian sensor
    fn observer(
        mut commands: Commands,
        frame_time: Res<Time>,
        asset_server: Res<AssetServer>,
        player: Query<(Entity, &Player)>,
        mut sensors: Query<(Entity, &Self, &mut DelayCounter)>,
        mut collisions: EventReader<Collision>,
        mut collision_end: EventReader<CollisionEnded>,
        mut soundtrack: Query<(Entity, &Soundtrack)>,
    ) {
        for c in collision_end.read() {
            for s in sensors.iter(){
                if s.0 == c.0 || s.0 == c.1 {
                    info!("Stop soundtrack");
                    if let Ok((entity, _)) = soundtrack.get_single() {
                        dbg!();
                        commands.entity(entity).insert(Fade(Duration::from_secs(2), Some(0.0)));
                    }
                    return;
                }
            }
        }

        let Ok((player, _)) = player.get_single() else { return };
        for c in collisions.read() {
            let e = [c.0.entity1, c.0.entity2, c.0.body_entity1.unwrap_or(c.0.entity1), c.0.body_entity2.unwrap_or(c.0.entity1)];
            if c.0.is_sensor && e.contains(&player) {
                if let Some(mut s) = sensors.iter_mut().find(|s| e.contains(&s.0)) {
                    if c.0.collision_stopped() {
                        dbg!(); // TODO, why doesn't this work
                    }

                    s.2.counter += frame_time.delta_secs();
                    if soundtrack.is_empty() && s.2.counter >= s.1.delay.unwrap_or_default(){
                        dbg!();
                        commands.spawn((
                            Soundtrack,
                            AudioPlayer::<AudioSource>(asset_server.load(&s.1.track)),
                            PlaybackSettings {
                                mode: bevy::audio::PlaybackMode::Despawn,
                                volume: Volume::new(0.2),
                                ..Default::default()
                            },
                        ));
                        return;
                    }
                }
            }
        }
    }

    fn attenuate(
        mut commands: Commands, 
        soundtrack: Query<(Entity, Ref<Soundtrack>, Option<Ref<Fade>>)>,
        ended: RemovedComponents<Soundtrack>,
        ambient: Query<(Entity, &PlaybackSettings, Option<&Fade>), With<WindNoise>>
    ){
        let factor = 0.3;

        let mut playing = false;
        let mut started = false;
        let mut stopped = ! ended.is_empty();
        for (_entity, s, fade) in soundtrack.iter(){
            // treat soundtrack with active fade out as if it isn't playing
            if Some(0.0) == fade.as_ref().and_then(|f|f.1){
                if fade.unwrap().is_changed() {
                    stopped = true;
                }
            } else {
                if s.is_added(){
                    started = true;
                }
                playing = true;
            }
        }

        for (a, settings, fade) in ambient.iter(){
            if stopped && !playing{
                if fade.is_some_and(|f|f.1.is_none()){
                    //already fading back to default
                    continue;
                }

                info!("cancelling attenuation");
                commands.entity(a).insert(Fade(Duration::from_secs_f32(4.0), None));
            }
            else if started{
                info!("attenuating");
                commands.entity(a).insert(Fade(Duration::from_secs_f32(4.0), Some(settings.volume.get() * factor)));
            }
        }
    }
}

//NOTE: https://musicforprogramming.net/seventythree
// This component will be attached to an entity to fade the audio in
#[derive(Component)]
struct Fade(Duration, Option<f32>);

// Fades in the audio of entities that has the FadeIn component. Removes the FadeIn component once
// full volume is reached.
fn fade(
    mut commands: Commands,
    mut audio_sink: Query<(&mut AudioSink, Entity, &mut Fade, Option<&PlaybackSettings>)>,
    time: Res<Time>,
) {
    for (audio, entity, mut fade, settings) in audio_sink.iter_mut() {
        let target = fade.1.or(settings.map(|f|f.volume.get())).unwrap_or(1.0);

        let inc =(target - audio.volume()) * time.delta_secs() / fade.0.as_secs_f32();
        fade.0 = fade.0.saturating_sub(time.delta());
        if ! fade.0.is_zero() {
            audio.set_volume(audio.volume() + inc);
        } else {
            if target == 0.0 {
                if audio.volume() == 0.0 {
                    info!("fade out complete, despawning");
                    commands.entity(entity).despawn_recursive();
                }
            }else{
                info!("fade complete {}", target);
                commands.entity(entity).remove::<Fade>(); 
            }
            audio.set_volume(target);
        }
    }
}