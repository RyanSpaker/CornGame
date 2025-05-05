use std::{ops::AddAssign, time::Duration};
use bevy::{audio::Volume, ecs::{component::ComponentId, entity::EntityHashMap, world::DeferredWorld}, prelude::*};
use crate::{
    ecs::{cameras::MainCamera, corn::field::cf_image_carved::CornSensor, flycam::FlyCamMoveEvent},
    util::{math::lerp, observer_ext::{ObserveAsAppExt, ObserverParent}},
};

pub struct CornAudioPlugin;
impl Plugin for CornAudioPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_type::<WindNoise>()
            .register_type::<Footsteps>()
            .register_type::<AudioFactor>()
            .register_type::<Pause>()
            .register_type::<Fade>()
            .register_type::<FadeKeepOnEnd>()
            .register_type::<FadePauseOnEnd>()
            .register_type::<Ambient>()
            .register_type::<AudioObservers>()
            .configure_sets(Update, AudioSystems.run_if(AudioSystems::should_run))
            .add_systems(Update, (
                (WindNoise::adjust_wind, Footsteps::adjust_footsteps, Fade::update_fade),
                AudioFactor::calculate_volume
            ).chain().in_set(AudioSystems))
            .add_observer_as(Fade::fade_despawn_observer, AudioObservers);
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)] 
pub struct AudioObservers;
impl ObserverParent for AudioObservers{
    fn get_name(&self) -> Name {
        Name::from("Audio Observers")
    }
}

/// Tag component for audio sinks which automatically pauses the sink on add, and plays the sink on remove
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)] 
#[component(storage = "SparseSet", on_add = Pause::pause_sink, on_remove = Pause::play_sink)]
pub struct Pause;
impl Pause{
    fn pause_sink(world: DeferredWorld, entity: Entity, _: ComponentId){
        let Some(sink) = world.get::<AudioSink>(entity) else {return;};
        sink.pause();
    }
    fn play_sink(world: DeferredWorld, entity: Entity, _: ComponentId){
        let Some(sink) = world.get::<AudioSink>(entity) else {return;};
        sink.play();
    }
}

/// Tag component for audio sinks which play ambient noise
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)] 
pub struct Ambient;

/// SystemSet for all audio systems.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
struct AudioSystems;
impl AudioSystems{
    fn should_run(query: Query<&AudioSink>) -> bool{query.is_empty()}
}

/// Component attached as a child of an audio sink. Every frame a system accumulates the factors to set the sink volume and speed
#[derive(Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)]
pub struct AudioFactor{
    /// Volume Multiplicative
    vm: f32,
    /// Volume Additive
    va: f32,
    /// Speed Multiplicative
    sm: f32,
    /// Speed Additive
    sa: f32
}
impl Default for AudioFactor{
    fn default() -> Self{Self{vm: 1.0, va: 0.0, sm: 1.0, sa: 0.0}}
}
impl AddAssign for AudioFactor{
    fn add_assign(&mut self, rhs: Self) {
        self.vm *= rhs.vm;
        self.va += rhs.va;
        self.sm *= rhs.sm;
        self.sa += rhs.sa;
    }
}
impl AudioFactor{
    /// For a set of volume factors, calculates the audio sink volume
    fn calculate_volume(
        sinks: Query<(Entity, &AudioSink, &PlaybackSettings)>,
        factors: Query<(&Parent, &Self)>
    ){
        let mut calculated_factors: EntityHashMap<Self> = EntityHashMap::default();
        for (parent, factor) in factors.iter(){
            let entity = parent.get();
            if !calculated_factors.contains_key(&entity) {calculated_factors.insert(entity, Self::default());}
            let Some(fact) = calculated_factors.get_mut(&entity) else {continue;};
            *fact += factor.clone();
        }
        for (entity, sink, settings) in sinks.iter(){
            let Some(Self{vm, va, sm, sa}) = calculated_factors.get(&entity) else {
                sink.set_volume(*settings.volume);
                sink.set_speed(settings.speed); 
                continue;
            };
            sink.set_volume((*settings.volume*vm+va).max(0.0));
            sink.set_speed((settings.speed*sm+sa).max(0.0));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub enum WindNoise {
    Wind,
    Rustle,
}
impl WindNoise{
    fn adjust_wind(time: Res<Time>, mut factors: Query<(&mut AudioFactor, &Self)>){
        for (mut factor, kind) in factors.iter_mut(){
            let mut t = time.elapsed_secs();

            let mut min_vol = 0.12;
            let mut power = 2.0;
            if *kind == Self::Rustle {
                // delay rustle
                t -= 2.0;
                min_vol = 0.0;
                power = 3.0;
            }

            // wind pattern: https://www.desmos.com/calculator/023vwitwiq
            let strength = (t / 3.0).cos() * (t / 5.2).cos() / 2.0 + 0.5; //matches wind.glsl
            let a = lerp(min_vol, 1.0, strength.powf(power));
            if a <= 0.0 {
                factor.vm = 0.0;
            } else {
                let db = 10.0 * a.log2();
                let adj = 10f32.powf(db / 20.0);
                factor.vm = adj;
            }
        }
    }
    pub fn spawn_wind_player(mut commands: Commands, asset_server: Res<AssetServer>){
        commands.spawn((
            AudioPlayer::<AudioSource>(asset_server.load("sounds/wind.ogg")),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Loop,
                volume: Volume::new(0.8),
                ..Default::default()
            },
            Name::from("Wind Audio Player")
        )).with_children(|parent| {
            parent.spawn((Name::from("Ambient Factor"), Ambient, AudioFactor::default()));
            parent.spawn((Name::from("Wind Volume Factor"), AudioFactor::default(), WindNoise::Wind));
        });
    }
    pub fn spawn_rustle_player(mut commands: Commands, asset_server: Res<AssetServer>){
        commands.spawn((
            AudioPlayer::<AudioSource>(asset_server.load("sounds/wind_rustle.ogg")),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Loop,
                volume: Volume::new(0.2),
                ..Default::default()
            },
            Name::from("Rustle Audio Player")
        )).with_children(|parent| {
            parent.spawn((Name::from("Ambient Factor"), Ambient, AudioFactor::default()));
            parent.spawn((Name::from("Rustle Volume Factor"), AudioFactor::default(), WindNoise::Rustle));
        });
    }
}

/// TODO https://github.com/vleue/bevy_easings
#[derive(Debug, Clone, PartialEq, Reflect, Component)]
pub struct Footsteps {
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
impl Footsteps{
    fn adjust_footsteps(
        time: Res<Time>,
        move_events: EventReader<FlyCamMoveEvent>,
        camera: Query<(&CornSensor, &Transform), With<MainCamera>>,
        mut factors: Query<(&mut AudioFactor, &mut Self)>
    ){
        let moving = !move_events.is_empty();
        let Ok((sensor, t)) = camera.get_single() else {
            return;
        };
        let flying = t.translation.y > 2.0; //MOVEME

        for (mut factor, mut fs) in factors.iter_mut(){
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

            factor.sm = fs.lerp_speed;
            factor.vm = fs.lerp;
        }
    }
    pub fn spawn_footsteps_player(mut commands: Commands, asset_server: Res<AssetServer>){
        commands.spawn((
            AudioPlayer::<AudioSource>(asset_server.load("sounds/footstep_leaves.ogg")),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Loop,
                volume: Volume::new(0.4),
                ..Default::default()
            },
            Name::from("Footsteps Audio Player")
        )).with_children(|parent| {
            parent.spawn((
                Name::from("Footsteps Audio Factor"), 
                AudioFactor::default(), 
                Footsteps {
                    s: [1.2, 0.7],
                    v: [0.2, 1.0],
                    ..Default::default()
                },
            ));
            parent.spawn((Name::from("Ambient Factor"), AudioFactor::default(), Ambient));
        });
    }
}

#[derive(Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)]
pub struct Fade{
    pub duration: Duration,
    pub target: f32
}
impl Fade{
    fn update_fade(
        time: Res<Time>,
        mut fades: Query<(Entity, &mut AudioFactor, &mut Fade)>,
        mut commands: Commands
    ){
        for (entity, mut factor, mut fade) in fades.iter_mut(){
            let step = (time.delta_secs()/fade.duration.as_secs_f32()).min(1.0);
            factor.vm = lerp(factor.vm, fade.target, step);
            fade.duration = fade.duration.saturating_sub(time.delta());
            if fade.duration.is_zero() {
                commands.entity(entity).remove::<Fade>();
            }
        }
    }
    fn fade_despawn_observer(
        trigger: Trigger<OnRemove, Fade>,
        query: Query<(&Parent, Option<&FadePauseOnEnd>, Option<&FadeKeepOnEnd>), With<Fade>>,
        mut commands: Commands
    ){
        let Ok((parent, pause, keep)) = query.get(trigger.entity()) else {return;};
        if pause.is_some(){
            commands.entity(parent.get()).insert(Pause);
        }
        if keep.is_none(){
            commands.entity(trigger.entity()).despawn_recursive();
        }
    }
}
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)]
pub struct FadePauseOnEnd;
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)]
pub struct FadeKeepOnEnd;
