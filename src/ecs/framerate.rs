use std::collections::VecDeque;

use bevy::{
    app::{Plugin, Update}, 
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin}, 
    ecs::{component::Component, system::{Commands, Query, Res}}, 
    prelude::default, 
    reflect::Reflect, 
    render::color::Color, 
    text::{Text, TextSection, TextStyle}, 
    ui::node_bundles::TextBundle,
};

pub fn update_fps(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<(&mut FPSData, &mut Text)>
){
    for (mut item, mut text) in query.iter_mut(){
        item.insert(diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS).unwrap().value().unwrap_or(1.0));
        text.sections[1].value = format!("{:.2}", item.mean);
        text.sections[3].value = format!("{:.2}", item.get_min());
        text.sections[5].value = format!("{:.2}", item.get_max());
        if item.mean > 50.0 {text.sections[0].style.color = Color::GREEN;}
        else if item.mean > 20.0 {text.sections[0].style.color = Color::ORANGE;}
        else {text.sections[0].style.color = Color::RED;}

    }
}

pub struct FrameRatePlugin;
impl Plugin for FrameRatePlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app
            .register_type::<FPSData>()
            .add_plugins(FrameTimeDiagnosticsPlugin)
            .add_systems(Update, update_fps);
    }
}

pub fn spawn_fps_text(mut commands: Commands){
    commands.spawn((TextBundle::from_sections([
        TextSection::new("FPS:", TextStyle{font_size: 20.0, color: Color::GOLD, ..default()}),
        TextSection::from_style(TextStyle{font_size: 20.0, color: Color::GOLD, ..default()}),
        TextSection::new(" [", TextStyle{font_size: 15.0, color: Color::WHITE, ..default()}),
        TextSection::from_style(TextStyle{font_size: 15.0, color: Color::ORANGE_RED, ..default()}),
        TextSection::new("-", TextStyle{font_size: 15.0, color: Color::WHITE, ..default()}),
        TextSection::from_style(TextStyle{font_size: 15.0, color: Color::BLUE, ..default()}),
        TextSection::new("]", TextStyle{font_size: 15.0, color: Color::WHITE, ..default()}),
    ]), FPSData::default()));
}

#[derive(Clone, Debug, Reflect, Component)]
pub struct FPSData{
    node_queue: VecDeque<f64>,
    pub mean: f64,
    max_vals: VecDeque<usize>,
    min_vals: VecDeque<usize>,
    index_sub: usize,
    window_size: usize
}
impl Default for FPSData{
    fn default() -> Self {
        Self { 
            node_queue: VecDeque::from(vec![1.0; 100]), 
            mean: 1.0, 
            max_vals: VecDeque::from(vec![99]), 
            min_vals: VecDeque::from(vec![99]), 
            index_sub: 0, 
            window_size: 100 
        }
    }
}
impl FPSData{
    pub fn insert(&mut self, data: f64){
        let old = self.node_queue.pop_front().unwrap();
        self.node_queue.push_back(data);
        self.mean *= (data/old).powf(1.0/self.window_size as f64);
        if self.max_vals.front().unwrap() - self.index_sub == 0 {self.max_vals.pop_front();}
        if self.min_vals.front().unwrap() - self.index_sub == 0 {self.min_vals.pop_front();}
        self.index_sub += 1;
        while self.max_vals.back().is_some_and(|val| self.node_queue[*val - self.index_sub] < data) {self.max_vals.pop_back();}
        self.max_vals.push_back(self.window_size + self.index_sub - 1);
        while self.min_vals.back().is_some_and(|val| self.node_queue[*val - self.index_sub] > data) {self.min_vals.pop_back();}
        self.min_vals.push_back(self.window_size + self.index_sub - 1);
    }
    pub fn get_min(&self) -> f64{
        self.node_queue[self.min_vals.front().unwrap() - self.index_sub].clone()
    }
    pub fn get_max(&self) -> f64{
        self.node_queue[self.max_vals.front().unwrap() - self.index_sub].clone()
    }
}