use std::time::Duration;

use bevy::prelude::*;

use crate::systems::{audio::{Fade, WindNoise}, scripting::ScriptListener};

pub struct SoundtrackPlugin;

impl Plugin for SoundtrackPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Soundtrack>();
        app.register_type::<SoundTrackOnEnter>();
        // entering and leaving zones is scripted: assets/scripts/music.js (CLAUDE)
        app.add_systems(Startup, |mut commands: Commands| {
            commands.spawn(ScriptListener::new("OnAdd", "scripts/music.js", "zone").with_component("SoundTrackOnEnter"));
        });
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
/// a music zone: data for scripts/music.js (CLAUDE)
pub struct SoundTrackOnEnter {
    pub track: String,
    pub delay: Option<f32>,
    pub fade_on_leave: bool,
}

impl SoundTrackOnEnter {
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