use std::{fs, ops::Range};

use bevy::{audio::Volume, prelude::*};

use crate::{
    ecs::{
        corn::field::cf_image_carved::CornSensor, flycam::FlyCamMoveEvent, main_camera::MainCamera,
    },
    util::lerp,
};

#[derive(Component, PartialEq)]
enum WindNoise {
    Wind,
    Rustle,
}

fn wind_volume(time: Res<Time>, settings: Query<(&mut AudioSink, &PlaybackSettings, &WindNoise)>) {
    for (s, initial, kind) in settings.iter() {
        let mut t = time.elapsed_seconds();

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
    s: [f32;2],
    s_lerp_seconds: f32,

    /// volume [on_path, in_corn]
    v: [f32;2],
    v_lerp_seconds: f32,
}

impl Default for Footsteps {
    fn default() -> Self {
        Self {
            lerp: 0.0,
            lerp_speed: 0.0,
            s: [1.0,1.0],
            s_lerp_seconds: 0.2,
            v: [1.0,1.0],
            v_lerp_seconds: 0.2,
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
        let delta = time.delta_seconds() / 0.2;
        fs.lerp += f32::min(diff.abs(), delta) * diff.signum();

        let diff = speed - fs.lerp_speed;
        let delta = time.delta_seconds() / 0.2;
        fs.lerp_speed += f32::min(diff.abs(), delta) * diff.signum();

        s.set_speed(fs.lerp_speed * initial.speed);
        s.set_volume(fs.lerp * initial.volume.get());
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        AudioBundle {
            source: asset_server.load("sounds/wind.ogg"),
            settings: PlaybackSettings{
                mode: bevy::audio::PlaybackMode::Loop,
                ..Default::default()
            }
        },
        WindNoise::Wind,
    ));

    commands.spawn((
        AudioBundle {
            source: asset_server.load("sounds/wind_rustle.ogg"),
            settings: PlaybackSettings{
                mode: bevy::audio::PlaybackMode::Loop,
                volume: Volume::new(0.1),
                ..Default::default()
            }
        },
        WindNoise::Rustle,
    ));

    commands.spawn((
        AudioBundle {
            source: asset_server.load("sounds/footstep_leaves.ogg"),
            settings: PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Loop,
                volume: Volume::new(0.3),
                ..Default::default()
            },
        },
        Footsteps {
            s: [1.2,0.7],
            v: [0.2,1.0],
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
    }
}
