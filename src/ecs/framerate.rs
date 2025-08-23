use bevy::{
    color::palettes::css, dev_tools::picking_debug::{DebugPickingMode, DebugPickingPlugin}, diagnostic::{Diagnostic, DiagnosticPath, DiagnosticsStore, FrameTimeDiagnosticsPlugin}, picking::hover::HoverMap, prelude::*
};
use super::cameras::MainCamera;

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct DiagPos;

pub fn update_position(
    mut query: Query<&mut TextSpan, With<DiagPos>>,
    camera: Query<(&Transform, &GlobalTransform), With<MainCamera>>
){
    if let Ok((t, _)) = camera.single(){
        for mut text in query.iter_mut(){
            text.0 = format!("{}", t.translation);
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct DiagPicking;

pub fn update_picking(
    mut query: Query<&mut TextSpan, With<DiagPicking>>,
    hovermap: Res<HoverMap>,
    name: Query<&Name>,
    parent: Query<&ChildOf>,
){  
    let mut t = String::new();
    for (pointer, hovermap) in hovermap.iter() {
        for (entity, hit) in hovermap.iter(){
            let mut path = String::new();

            let mut ls = parent.iter_ancestors(*entity).collect::<Vec<_>>();
            ls.reverse();
            ls.push(*entity);

            for e in ls{
                path += &e.to_string();
                if let Ok(name) = name.get(*entity) {
                    path += "#";
                    path += name.as_str();
                }
                path += "/";
            }
            t += &format!("{}", path);
        }
    }

    for mut text in query.iter_mut(){
        text.0 = t.to_string();
    }
}

pub struct FrameRatePlugin;
impl Plugin for FrameRatePlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app
            .add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_systems(Update, (
                update_diagnostics,
                update_position,
                update_picking,
            ));
        app.add_plugins(DebugPickingPlugin)        
        .insert_resource(DebugPickingMode::Disabled)
        // A system that cycles the debugging state when you press F3:
        .add_systems(
            PreUpdate,
            (|mut mode: ResMut<DebugPickingMode>| {
                *mode = match *mode {
                    DebugPickingMode::Disabled => DebugPickingMode::Normal,
                    DebugPickingMode::Normal => DebugPickingMode::Noisy,
                    DebugPickingMode::Noisy => DebugPickingMode::Disabled,
                }
            })
            .distributive_run_if(bevy::input::common_conditions::input_just_pressed(
                KeyCode::F4,
            )),
        );
    }
}

pub fn spawn_fps_text(mut commands: Commands){

    let mut node = Node::default();
    node.margin.top = Val::Px(10.0);

    commands.spawn((
        Name::from("FPS Display"),
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

        builder.spawn(TextSpan::new("\nPos: ")).with_children(|builder|{
            builder.spawn((TextSpan::default(), DiagPos));    
        }); 
        builder.spawn(TextSpan::new("\nHover: ")).with_children(|builder|{
            builder.spawn((TextSpan::default(), DiagPicking));    
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
                DiagnosticMode::Smoothed => match path.average(){
                    Some(f) => format!("{f:.2}"),
                    None => "n/a".into(),
                },
                DiagnosticMode::Function(foo) => foo(path),
            },
            None => "None".to_owned(),
        }.into();
    }
}
