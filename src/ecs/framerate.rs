use std::collections::VecDeque;

use bevy::{
    app::{Plugin, PostUpdate, Update}, diagnostic::{Diagnostic, DiagnosticPath}, ecs::{component::Component, query::With, system::{Commands, Query, Res}}, prelude::{default, Text}, reflect::Reflect, transform::components::{GlobalTransform, Transform}
};

use bevy::prelude::*;
use bevy::color::palettes::css;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};

use super::main_camera::MainCamera;

#[derive(Component)]
pub struct DiagPos;

pub fn update_position(
    mut query: Query<&mut TextSpan, With<DiagPos>>,
    camera: Query<(&Transform, &GlobalTransform), With<MainCamera>>
){
    if let Ok((t, gt)) = camera.get_single(){
        for mut text in query.iter_mut(){
            text.0 = format!("{}", t.translation);
        }
    }
}

pub struct FrameRatePlugin;
impl Plugin for FrameRatePlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app
            .add_plugins(FrameTimeDiagnosticsPlugin)
            .add_systems(Update, (
                update_diagnostics,
                update_position,
            ));
    }
}

pub fn spawn_fps_text(mut commands: Commands){

    let mut node = Node::default();
    node.margin.top = Val::Px(10.0);

    commands
            .spawn((
                Text::new("FPS: "),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(css::GOLD.into()), 
                node
            ))
            .with_children(|builder|{
                let fps = (TextSpan::default(), TextFromDiagnostic(FrameTimeDiagnosticsPlugin::FPS), DiagnosticMode::Function(|d|{
                    format!("{:.2}", d.smoothed().unwrap_or_default())
                }));
                let fps_range = (TextSpan::default(), TextFromDiagnostic(FrameTimeDiagnosticsPlugin::FPS), DiagnosticMode::Function(|d|{
                    // TODO for the love of god, this should not be this complicated and it shouldn't take me 5 minutes to write fucking max()

                    let mut sorted : Vec<f64> = d.measurements().map(|d|d.value).collect();
                    sorted.sort_by(f64::total_cmp);

                    if ! sorted.is_empty(){
                        let max = sorted.last().unwrap();
                        let min = sorted.first().unwrap();
                    
                        let mean = d.average().unwrap();
                        let stddev = sorted.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / sorted.len() as f64;
                        format!("[+/- {:.0}] {:.0} - {:.0}", stddev, min, max)
                    }else{
                        Default::default()
                    }
                }
                ));


                builder.spawn(fps);
                builder.spawn(fps_range);

                builder.spawn((Text::new("\nPos: "))).with_children(|builder|{
                    builder.spawn((TextSpan::default(), DiagPos));    
                }); 
            });
}

#[derive(Debug, Component)]
enum DiagnosticMode {
    Smoothed,
    Function(fn(&Diagnostic) -> String),
}

#[derive(Debug, Component)]
struct TextFromDiagnostic(DiagnosticPath);

fn update_diagnostics(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<(&TextFromDiagnostic, Option<&DiagnosticMode>, &mut TextSpan)>
){
    for (path,mode, mut text) in &mut query {
        let mode = mode.unwrap_or(&DiagnosticMode::Smoothed);
        *text = match diagnostics.get(&path.0) {
            Some(path) => match mode {
                DiagnosticMode::Smoothed => match path.smoothed(){
                    Some(f) => f.to_string(),
                    None => "n/a".into(),
                },
                DiagnosticMode::Function(foo) => foo(path),
            },
            None => "None".to_owned(),
        }.into();
    }
}