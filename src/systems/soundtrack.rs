use std::{marker::PhantomData, time::Duration};

use avian3d::prelude::Collider;
use bevy::{audio::Volume, prelude::*};

use crate::systems::{audio::{Fade, WindNoise}, character::Player};

pub struct SoundtrackPlugin;

impl Plugin for SoundtrackPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<DelayCounter>();
        app.register_type::<Soundtrack>();
        app.register_type::<SoundTrackOnEnter>();
        app.add_observer(SoundTrackOnEnter::enter);
        app.add_observer(SoundTrackOnEnter::exit);
        app.add_systems(Update, SoundTrackOnEnter::delay);
        app.add_systems(Update, SoundTrackOnEnter::attenuate);
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct Soundtrack;

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[require(avian3d::prelude::CollisionEventsEnabled)]
#[require(avian3d::prelude::Sensor)]
#[require(avian3d::prelude::Collider = avian3d::prelude::Collider::cuboid(2.0,2.0,2.0) )]
pub struct SoundTrackOnEnter {
    track: String,
    delay: Option<f32>,
    fade_on_leave: bool,
}

#[derive(Debug, Default, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct DelayCounter {
    counter: f32,
    // idea is that a single entity might need multiple of these.
    // _dummy: PhantomData<T>
}

impl SoundTrackOnEnter {
    /// system to play a track when entering a avian sensor
    fn exit(
        event: Trigger<avian3d::prelude::OnCollisionEnd>,
        mut commands: Commands,
        player: Single<Entity, With<Player>>,
        sensors: Query<(Entity, &Self)>,
        soundtrack: Query<Entity, With<Soundtrack>>,
    ) {
        let Ok((entity, conf)) = sensors.get(event.target()) else{
            return;
        };

        if event.body.unwrap_or(event.collider) != *player {
            return;
        }

        dbg!(&entity);
        commands.entity(entity).remove::<DelayCounter>();

        let Ok(soundtrack) = soundtrack.single() else {
            return;
        };

        // TODO what about multiple soundtracks, or if this collider is not responsible for the current soundtrack
        if conf.fade_on_leave {
            info!("Stop soundtrack");
            commands.entity(soundtrack).insert(Fade {
                duration: Duration::from_secs(2),
                target: 0.0,
            });
        }
    }

    fn enter(
        event: Trigger<avian3d::prelude::OnCollisionStart>,
        mut commands: Commands,
        player: Single<Entity, With<Player>>,
        mut sensors: Query<(Entity, &Self, Option<&DelayCounter>)>,
    ) {
        let Ok((entity, conf, delay)) = sensors.get_mut(event.target()) else{
            return;
        };

        if event.body.unwrap_or(event.collider) != *player {
            return;
        }

        dbg!(&entity);

        if delay.is_none() {
            commands.entity(entity).insert(DelayCounter::default());
        }
    }

    fn delay(
        mut sensors: Query<(Entity, &Self, &mut DelayCounter)>,
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        frame_time: Res<Time>,
        soundtrack: Query<Entity, With<Soundtrack>>,
    ){
        for (entity, conf, mut delay) in sensors.iter_mut(){
            delay.counter += frame_time.delta_secs();
            // TODO what if soundtrack did not get despawned

            if delay.counter >= conf.delay.unwrap_or_default() {
                for s in soundtrack.iter() {
                    warn!(entity=%s, "soundtrack should have despawned already");
                    commands.entity(s).despawn();
                }

                commands.spawn((
                    Soundtrack,
                    AudioPlayer::<AudioSource>(asset_server.load(&conf.track)),
                    PlaybackSettings {
                        mode: bevy::audio::PlaybackMode::Despawn,
                        volume: Volume::Linear(1.0),
                        ..Default::default()
                    },
                ));

                commands.entity(entity).remove::<DelayCounter>();
            }
        }
    }

    /// UNTESTED
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
            if Some(0.0) == fade.as_ref().map(|f| f.target){
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
                if fade.is_some_and(|f|f.target == settings.volume.to_linear() /* THIS IS DUMB FIXME */){
                    //already fading back to default
                    continue;
                }

                info!("cancelling attenuation");
                commands.entity(a).insert(Fade {
                    duration: Duration::from_secs_f32(4.0),
                    target: settings.volume.to_linear(),
                });
            }
            else if started{
                info!("attenuating");
                commands.entity(a).insert(Fade {
                    duration: Duration::from_secs_f32(4.0),
                    target: settings.volume.to_linear() * factor,
                });
            }
        }
    }
}

//NOTE: https://musicforprogramming.net/seventythree