use std::collections::VecDeque;
use bevy::{color::palettes::css::*, diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin}, prelude::*};
use super::main_camera::MainCamera;

pub fn update_fps(
    diagnostics: Res<DiagnosticsStore>,
    mut data: Query<(&mut FPSData, &Children)>,
    mut sections: Query<(&mut TextSpan, &mut TextColor)>
){
    for (mut data, children) in data.iter_mut(){
        data.insert(diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS).unwrap().value().unwrap_or(1.0));
        if let Ok((mut text, _)) = sections.get_mut(children[1]){
            text.0 = format!("{:.2}", data.mean);
        }
        if let Ok((mut text, _)) = sections.get_mut(children[3]){
            text.0 = format!("{:.2}", data.get_min());
        }
        if let Ok((mut text, _)) = sections.get_mut(children[5]){
            text.0 = format!("{:.2}", data.get_max());
        }
        if let Ok((_, mut color)) = sections.get_mut(children[0]){
            match data.mean{
                50.0.. => {color.0 = GREEN.into();}
                20.0..50.0 => {color.0 = ORANGE.into();}
                _ => {color.0 = RED.into();}
            }
        }
        
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct DiagPos;

pub fn update_position(
    query: Query<&Children, With<DiagPos>>,
    mut text_query: Query<&mut Text>,
    camera: Query<(&Transform, &GlobalTransform), With<MainCamera>>
){
    if let Ok((t, gt)) = camera.get_single(){
        for children in query.iter(){
            if let Ok(mut text) = text_query.get_mut(children[8]){
                text.0 = format!("{}", t.translation);
            }
            if let Ok(mut text) = text_query.get_mut(children[10]){
                text.0 = format!("{}", gt.translation());
            }
        }
    }
}

pub struct FrameRatePlugin;
impl Plugin for FrameRatePlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app
            .register_type::<FPSData>()
            .register_type::<DiagPos>()
            .add_plugins(FrameTimeDiagnosticsPlugin)
            .add_systems(Update, update_fps)
            .add_systems(Update, update_position);
    }
}

pub fn spawn_fps_text(mut commands: Commands){
    commands.spawn((TextLayout::default(), FPSData::default(), DiagPos)).with_children(|parent| {
        parent.spawn((TextSpan("FPS:".into()), TextFont{font_size: 20.0, ..Default::default()}, TextColor(GOLD.into())));
        parent.spawn((TextSpan("".into()), TextFont{font_size: 20.0, ..Default::default()}, TextColor(GOLD.into())));
        parent.spawn((TextSpan(" [".into()), TextFont{font_size: 15.0, ..Default::default()}, TextColor(WHITE.into())));
        parent.spawn((TextSpan("".into()), TextFont{font_size: 15.0, ..Default::default()}, TextColor(ORANGE_RED.into())));
        parent.spawn((TextSpan("-".into()), TextFont{font_size: 15.0, ..Default::default()}, TextColor(WHITE.into())));
        parent.spawn((TextSpan("".into()), TextFont{font_size: 15.0, ..Default::default()}, TextColor(BLUE.into())));
        parent.spawn((TextSpan("]".into()), TextFont{font_size: 15.0, ..Default::default()}, TextColor(WHITE.into())));
        parent.spawn((TextSpan(" Local: ".into()), TextFont{font_size: 20.0, ..Default::default()}, TextColor(GOLD.into())));
        parent.spawn((TextSpan("- ".into()), TextFont{font_size: 15.0, ..Default::default()}, TextColor(WHITE.into())));
        parent.spawn((TextSpan(" Global: ".into()), TextFont{font_size: 20.0, ..Default::default()}, TextColor(GOLD.into())));
        parent.spawn((TextSpan("- ".into()), TextFont{font_size: 15.0, ..Default::default()}, TextColor(WHITE.into())));
    });
}

#[derive(Debug, Clone, PartialEq, Reflect, Component)]
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
            max_vals: VecDeque::from([99]), 
            min_vals: VecDeque::from([99]), 
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