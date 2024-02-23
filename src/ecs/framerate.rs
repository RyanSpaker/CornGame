use std::{collections::VecDeque, marker::PhantomData, ops::Range};

use bevy::{
    app::{Plugin, Update},
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    ecs::{
        component::Component,
        query::With,
        system::{Commands, Query, Res, ResMut, Resource},
    },
    prelude::default,
    reflect::Reflect,
    render::color::Color,
    text::{Text, TextSection, TextStyle},
    time::Time,
    transform::components::{GlobalTransform, Transform},
    ui::node_bundles::TextBundle,
};
use bevy_rapier3d::na::ComplexField;

use super::{corn::render::scan_prepass::LodCutoffs, main_camera::MainCamera};

#[derive(Component)]
pub struct DiagPos;

pub fn update_position(
    perf: Res<Performance>,
    mut query: Query<&mut Text, With<DiagPos>>,
    camera: Query<(&Transform, &GlobalTransform), With<MainCamera>>,
    lod: Res<LodCutoffs>,
) {
    if let Ok((t, gt)) = camera.get_single() {
        for mut text in query.iter_mut() {
            text.sections[8].value = format!("{}", t.translation);
            text.sections[10].value = format!("{}", gt.translation());
        }
    }

    let fps = &perf.fps;
    for mut text in query.iter_mut() {
        text.sections[1].value = format!("{:.2}", fps.mean);
        text.sections[3].value = format!("{:.2}", fps.get_min());
        text.sections[5].value = format!("{:.2}", fps.get_max());
        if fps.mean > 50.0 {
            text.sections[0].style.color = Color::GREEN;
        } else if fps.mean > 20.0 {
            text.sections[0].style.color = Color::ORANGE;
        } else {
            text.sections[0].style.color = Color::RED;
        }

        text.sections[11].value = format!(" {:.2}", perf.performance_pressure);
        text.sections[12].value = format!(" {:?}", lod.0);
    }
}

pub struct FrameRatePlugin;
impl Plugin for FrameRatePlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin)
            .register_type::<FPSData>()
            .register_type::<Performance>()
            .init_resource::<Performance>()
            .register_type::<LodTunable>()
            .init_resource::<LodTunable>()
            .add_systems(Update, Performance::update)
            .add_systems(Update, LodTunable::effect_lod_cuttoffs)
            .add_systems(Update, update_position);
    }
}

pub fn spawn_fps_text(mut commands: Commands) {
    commands.spawn((
        TextBundle::from_sections([
            TextSection::new(
                "FPS:",
                TextStyle {
                    font_size: 20.0,
                    color: Color::GOLD,
                    ..default()
                },
            ),
            TextSection::from_style(TextStyle {
                font_size: 20.0,
                color: Color::GOLD,
                ..default()
            }),
            TextSection::new(
                " [",
                TextStyle {
                    font_size: 15.0,
                    color: Color::WHITE,
                    ..default()
                },
            ),
            TextSection::from_style(TextStyle {
                font_size: 15.0,
                color: Color::ORANGE_RED,
                ..default()
            }),
            TextSection::new(
                "-",
                TextStyle {
                    font_size: 15.0,
                    color: Color::WHITE,
                    ..default()
                },
            ),
            TextSection::from_style(TextStyle {
                font_size: 15.0,
                color: Color::BLUE,
                ..default()
            }),
            TextSection::new(
                "]",
                TextStyle {
                    font_size: 15.0,
                    color: Color::WHITE,
                    ..default()
                },
            ),
            TextSection::new(
                " Local: ",
                TextStyle {
                    font_size: 20.0,
                    color: Color::GOLD,
                    ..default()
                },
            ),
            TextSection::new(
                "- ",
                TextStyle {
                    font_size: 15.0,
                    color: Color::WHITE,
                    ..default()
                },
            ),
            TextSection::new(
                " Global: ",
                TextStyle {
                    font_size: 20.0,
                    color: Color::GOLD,
                    ..default()
                },
            ),
            TextSection::new(
                "- ",
                TextStyle {
                    font_size: 15.0,
                    color: Color::WHITE,
                    ..default()
                },
            ),
            TextSection::from_style(TextStyle {
                font_size: 15.0,
                color: Color::BLUE,
                ..default()
            }),
            TextSection::from_style(TextStyle {
                font_size: 15.0,
                color: Color::BLUE,
                ..default()
            }),
            TextSection::from_style(TextStyle {
                font_size: 15.0,
                color: Color::BLUE,
                ..default()
            }),
        ]),
        DiagPos,
    ));
}

#[derive(Debug, Clone, Reflect)]
pub struct FPSData {
    node_queue: VecDeque<f64>,
    pub mean: f64,
    max_vals: VecDeque<usize>,
    min_vals: VecDeque<usize>,
    index_sub: usize,
    window_size: usize,
}
impl Default for FPSData {
    fn default() -> Self {
        Self {
            node_queue: VecDeque::from(vec![1.0; 100]),
            mean: 1.0,
            max_vals: VecDeque::from(vec![99]),
            min_vals: VecDeque::from(vec![99]),
            index_sub: 0,
            window_size: 100,
        }
    }
}
impl FPSData {
    pub fn insert(&mut self, data: f64) {
        let old = self.node_queue.pop_front().unwrap();
        self.node_queue.push_back(data);
        self.mean *= (data / old).powf(1.0 / self.window_size as f64);
        if self.max_vals.front().unwrap() - self.index_sub == 0 {
            self.max_vals.pop_front();
        }
        if self.min_vals.front().unwrap() - self.index_sub == 0 {
            self.min_vals.pop_front();
        }
        self.index_sub += 1;
        while self
            .max_vals
            .back()
            .is_some_and(|val| self.node_queue[*val - self.index_sub] < data)
        {
            self.max_vals.pop_back();
        }
        self.max_vals
            .push_back(self.window_size + self.index_sub - 1);
        while self
            .min_vals
            .back()
            .is_some_and(|val| self.node_queue[*val - self.index_sub] > data)
        {
            self.min_vals.pop_back();
        }
        self.min_vals
            .push_back(self.window_size + self.index_sub - 1);
    }
    pub fn get_min(&self) -> f64 {
        self.node_queue[self.min_vals.front().unwrap() - self.index_sub].clone()
    }
    pub fn get_max(&self) -> f64 {
        self.node_queue[self.max_vals.front().unwrap() - self.index_sub].clone()
    }
}

/// IDEA: create a resource with FPS, Target FPS, and "Performance Pressure"
/// The idea is that Performance pressure increases the longer that fps has been below target.
/// As performance pressure approaches 1, systems which react to it will try harder and harder to reduce resource usage, as it gets closer to -1 systems will try to improve graphics
/// At perfomance pressure of 0 (the target fps is reached) systems will not change settings.
/// Data could be saved to restore on game reload.
#[derive(Debug, Clone, Reflect, Resource)]
pub struct Performance {
    fps: FPSData,
    target: f32,

    performance_pressure: f32,
}

impl Default for Performance {
    fn default() -> Self {
        Self {
            fps: Default::default(),
            target: 55.0,
            performance_pressure: 0.0,
        }
    }
}

impl Performance {
    pub fn update(mut this: ResMut<Self>, diagnostics: Res<DiagnosticsStore>, time: Res<Time>) {
        this.fps.insert(
            diagnostics
                .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
                .unwrap()
                .value()
                .unwrap_or(1.0),
        );

        let diff = this.target - (this.fps.mean as f32);

        //this.performance_pressure += diff.signum() * diff.powf(2.0) / 1000.0;
        // at 30fps it should take 1 second to reach .2
        this.performance_pressure += (diff / 30.0) * 0.2 / time.delta_seconds();

        this.performance_pressure = match diff > 0.0 {
            true => this.performance_pressure.clamp(0.0, 1.0), // perf degraded !
            false => this.performance_pressure.clamp(-1.0, 0.0),
        };
    }
}
#[derive(Debug, Clone, Reflect, Resource)]
pub struct LodTunable {
    lod0: Range<f32>,
    lod_max: f32,
    cooldown: f32,

    last_adjustment: f32,
}

impl Default for LodTunable {
    fn default() -> Self {
        Self {
            lod0: 1.0..20.0,
            lod_max: 200.0,
            cooldown: 1.0,

            last_adjustment: 0.0,
        }
    }
}

impl LodTunable {
    pub fn effect_lod_cuttoffs(
        perf: Res<Performance>,
        mut lod_cuttoffs: ResMut<LodCutoffs>,
        mut this: ResMut<Self>,
        time: Res<Time>,
    ) {
        if lod_cuttoffs.0.is_empty() {
            return;
        };

        if perf.performance_pressure.abs() > 0.1
            && time.elapsed_seconds() - this.last_adjustment > this.cooldown
        {
            let max = match perf.performance_pressure > 0.0 {
                true => this.lod0.start,
                false => this.lod0.end,
            };
            let diff = max - lod_cuttoffs.0[0];
            let new = lod_cuttoffs.0[0] + diff / 10.0;

            lod_cuttoffs.0[0] = new.floor();
            assert_eq!(new, new.clamp(this.lod0.start, this.lod0.end));

            for i in 1..lod_cuttoffs.0.len() {
                lod_cuttoffs.0[i] = (lod_cuttoffs.0[i - 1] * 1.5 + lod_cuttoffs.0[0] / 2.0).clamp(1.0, this.lod_max).floor();
            }

            this.last_adjustment = time.elapsed_seconds();
        }
    }
}
