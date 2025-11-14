//! a grid of crickets, using positional (spatial) audio. Experimental.
use bevy::pbr::wireframe::Wireframe;
use bevy::render::view::RenderLayers;
use bevy::{pbr::wireframe::WireframeColor, prelude::*};
use bevy::audio::Volume;
use bevy_asset_loader::loading_state::config::ConfigureLoadingState;
use bevy_asset_loader::loading_state::{LoadingState, LoadingStateAppExt};
use bevy_asset_loader::prelude::AssetCollection;
use bevy_editor_pls::default_windows::cameras::EDITOR_RENDER_LAYER;

use crate::Cmds;
use crate::systems::audio::{AudioFactor, Pause};
use crate::util::register_system_named::RegisterSystemNamed;

/// Assets loaded by bevy_asset_loader. Uses a key instead of a path so the
/// 
/// This is useless for two reasons
/// 1. panics if cricket key does not exist (what if it isn't loaded yet?)
/// 2. resource does not get created until load. which is the opposite of what we want. 
/// bad crate
// #[derive(AssetCollection, Resource)]
// pub struct CricketAssets {
// 	#[asset(key = "cricket")]
// 	pub cricket: Handle<AudioSource>,
// }

#[derive(Resource, Reflect, Clone)]
#[reflect(Resource)]
pub struct CricketSettings {
	pub enabled: bool,
	pub rows: u32,
	pub cols: u32,
	/// spacing.x is horizontal (x), spacing.y is vertical (z) step between rows
	pub spacing: Vec2,
	/// per-emitter jitter in world units
	pub jitter: f32,
	/// master volume multiplier (linear)
	pub volume: f32,
	/// play/pause toggle
	pub play: bool,
}
impl Default for CricketSettings {
	fn default() -> Self {
		Self {
			enabled: true,
			rows: 6,
			cols: 10,
			spacing: Vec2::new(1.0, 0.8660254), // hex spacing default (sqrt(3)/2)
			jitter: 0.05,
			volume: 0.6,
			play: true,
		}
	}
}

/// Marker component for parent audio emitter entities
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Cricket;

/// Stores grid indices so we can reposition when spacing changes
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct CricketIndex {
	pub row: u32,
	pub col: u32,
}

/// Marker component for the child AudioFactor used to control per-cricket volume
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct CricketFactor;

pub struct CricketsPlugin;
impl Plugin for CricketsPlugin {
	fn build(&self, app: &mut App) {
		app.register_type::<CricketSettings>()
			.register_type::<Cricket>()
			.register_type::<CricketIndex>()
			.register_type::<CricketFactor>();

        app			.add_systems(Update, (
				sync_cricket_volumes,
				reposition_crickets,
				apply_play_state,
			).run_if(resource_changed::<CricketSettings>));
        
        // app.init_resource::<AssetCollection>();
        // #[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
        // enum MyStates {
        //     #[default]
        //     AssetLoading,
        //     Next,
        // }

        // I don't like this.
        // app.init_state::<MyStates>()
        //     .add_loading_state(
        //     LoadingState::new(MyStates::AssetLoading)
        //         .continue_to_state(MyStates::Next)
        //         .load_collection::<CricketAssets>(),
        // );

        app.insert_resource(CricketSettings{
            enabled: true,
            rows: 5,
            cols: 5,
            spacing: Vec2::splat(10.0),
            jitter: 0.1,
            volume: 0.5,
            play: true,
        });

        // this method of supporting cli args also sucks
        // app.add_systems(crate::Cmds, spawn_cricket_field);
        app.register_system_named(spawn_cricket_field);
	}
}

/// Spawn a hexagonal grid of spatial audio players (one per cricket).
pub fn spawn_cricket_field(
	mut commands: Commands,
	// assets: Res<CricketAssets>,
	settings: Res<CricketSettings>,
    server: Res<AssetServer>,
){
    dbg!();
	// center the grid around origin
	let rows = settings.rows as i32;
	let cols = settings.cols as i32;

	for r in 0..rows {
		for c in 0..cols {
            dbg!();
			let row = r as f32;
			let col = c as f32;

			// hexagonal staggering: offset every other row by 0.5 * spacing.x
			let x = (col + (row % 2.0) * 0.5) * settings.spacing.x;
			let z = row * settings.spacing.y;

			// center
			let x = x - (cols as f32 - 1.0) * 0.5 * settings.spacing.x;
			let z = z - (rows as f32 - 1.0) * 0.5 * settings.spacing.y;

			// small random-ish jitter (deterministic by seed could be used, but
			// we keep it simple here)
			let jitter_x = ( (r * 73856093) ^ (c * 19349663) ) as i32 as f32 * 0.00001;
			let jitter_z = ( (r * 83492791) ^ (c * 19349663) ) as i32 as f32 * 0.000013;
			let jitter_x = jitter_x * settings.jitter;
			let jitter_z = jitter_z * settings.jitter;

			let pos = Vec3::new(x + jitter_x, 0.0, z + jitter_z);

			commands.spawn((
				Name::new(format!("Cricket {}/{}", r, c)),
				Cricket,
				CricketIndex { row: r as u32, col: c as u32 },
				Transform::from_translation(pos),
				GlobalTransform::default(),

				// spatial audio player
				AudioPlayer::<AudioSource>(server.load("sounds/crickets.ogg")),
				PlaybackSettings {
					mode: bevy::audio::PlaybackMode::Loop,
					spatial: true,
					volume: Volume::Linear(settings.volume),
                    
					..Default::default()
				},

                // editor view
                Mesh3d(server.add(Mesh::from(Sphere::new(0.3)))),
                Wireframe,
                WireframeColor{color: Color::Srgba(Srgba::GREEN)},
                RenderLayers::layer(EDITOR_RENDER_LAYER)
			));
		}
	}
}

/// Sync master volume (and any other realtime settings) into per-cricket factors.
fn sync_cricket_volumes(
	settings: Res<CricketSettings>,
	mut sinks: Query<&mut AudioSink, With<Cricket>>,
){
	// Always ensure playbacksettings reflect master volume.
	for mut s in sinks.iter_mut() {
		s.set_volume(Volume::Linear(settings.volume));
	}
}

/// Reposition crickets when spacing/rows/cols change.
fn reposition_crickets(
	settings: Res<CricketSettings>,
	mut query: Query<(&CricketIndex, &mut Transform), With<Cricket>>,
){
	if !settings.is_changed() { return; }

	let rows = settings.rows as f32;
	let cols = settings.cols as f32;

	for (idx, mut t) in query.iter_mut() {
		let row = idx.row as f32;
		let col = idx.col as f32;
		let mut x = (col + (row % 2.0) * 0.5) * settings.spacing.x;
		let mut z = row * settings.spacing.y;

		x = x - (cols - 1.0) * 0.5 * settings.spacing.x;
		z = z - (rows - 1.0) * 0.5 * settings.spacing.y;

		t.translation.x = x;
		t.translation.z = z;
	}
}

/// Apply play/pause state by (removing/adding) the `Pause` tag used in the
/// project's audio system. Adding `Pause` pauses the sink (on_add) and
/// removing it resumes playback (on_remove).
fn apply_play_state(
	settings: Res<CricketSettings>,
	query: Query<Entity, With<Cricket>>,
	mut commands: Commands,
	has_pause: Query<&Pause>,
){
	if !settings.is_changed() { return; }

	for e in query.iter() {
		if settings.play {
			if has_pause.get(e).is_ok() {
				commands.entity(e).remove::<Pause>();
			}
		} else {
			if has_pause.get(e).is_err() {
				commands.entity(e).insert(Pause);
			}
		}
	}
}




