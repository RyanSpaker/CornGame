pub mod console;

use bevy::prelude::*;
use kayak_ui::{
    prelude::{widgets::*, *},
    CameraUIKayak,
};
pub struct MenuPlugin;
impl Plugin for MenuPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins(KayakContextPlugin);
        app.add_plugins(KayakWidgets);
        app.add_systems(Startup, setup.run_if(run_once()));
    }
}

fn setup(
    mut commands: Commands,
    mut font_mapping: ResMut<FontMapping>,
    asset_server: Res<AssetServer>,
) {
    let camera_entity = commands
        .spawn(Camera2dBundle::default())
        .insert(Camera{
            order: 50, 
            clear_color: ClearColorConfig::None,
            ..Default::default()
        })
        .insert((CameraUIKayak, IsDefaultUiCamera))
        .id();

    font_mapping.set_default(asset_server.load("roboto.kttf"));

    let mut widget_context = KayakRootContext::new(camera_entity);
    widget_context.add_plugin(KayakWidgetsContextPlugin);
    let parent_id = None;
    rsx! {
        <KayakAppBundle>
            <TextWidgetBundle
                text={TextProps {
                    content: "corn".into(),
                    size: 200.0,
                    ..Default::default()
                }}
            />
        </KayakAppBundle>
    };

    commands.spawn((widget_context, EventDispatcher::default()));
}

fn setup2(
    mut commands: Commands,
    asset_server: Res<AssetServer>
){

    // commands
    //     .spawn(Camera2dBundle::default())
    //     .insert((Camera{
    //         order: 50, 
    //         clear_color: ClearColorConfig::None,
    //         ..Default::default()
    //     }, IsDefaultUiCamera));
    const PALETTE: [&str; 4] = ["27496D", "466B7A", "669DB3", "ADCBE3"];
    const HIDDEN_COLOR: Color = Color::rgb(1.0, 0.7, 0.7);

    let palette: [Color; 4] = PALETTE.map(|hex| Color::hex(hex).unwrap().into());

    let text_style = TextStyle {
        font: asset_server.load("roboto.ttf"),
        font_size: 24.0,
        ..default()
    };

    commands.spawn(NodeBundle {
        style: Style {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceEvenly,
            ..Default::default()
        },
        //background_color: BackgroundColor(Color::BLACK),
        ..Default::default()
    }).with_children(|parent| {
        parent.spawn(TextBundle {
            text: Text::from_section(
                "Use the panel on the right to change the Display and Visibility properties for the respective nodes of the panel on the left",                
                text_style.clone(),
            ).with_justify(JustifyText::Center),
            style: Style {
                margin: UiRect::bottom(Val::Px(10.)),
                ..Default::default()
            },
            ..Default::default()
        });

        parent
            .spawn(NodeBundle {
                style: Style {
                    width: Val::Percent(100.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .with_children(|parent| {
                let mut target_ids = vec![];
                parent.spawn(NodeBundle {
                    style: Style {
                        width: Val::Percent(50.),
                        height: Val::Px(520.),
                        justify_content: JustifyContent::Center,
                        ..Default::default()
                    },
                    ..Default::default()
                }).with_children(|parent| {
                    target_ids = spawn_left_panel(parent, &palette);
                });
            });

            parent.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Start,
                    justify_content: JustifyContent::Start,
                    column_gap: Val::Px(10.),
                    ..Default::default()
                },
                ..default() })
            .with_children(|builder| {
                let text_style = TextStyle {
                    font: asset_server.load("roboto.ttf"),
                    font_size: 20.0,
                    ..default()
                };

                builder.spawn(TextBundle {
                    text: Text::from_section(
                        "Display::None\nVisibility::Hidden\nVisibility::Inherited",
                        TextStyle { color: HIDDEN_COLOR, ..text_style.clone() }
                        ).with_justify(JustifyText::Center),
                    ..Default::default()
                    });
                    builder.spawn(TextBundle {
                        text: Text::from_section(
                            "-\n-\n-",
                            TextStyle { color: Color::rgb(0.0,0.0,0.0), ..text_style.clone() }
                            ).with_justify(JustifyText::Center),
                        ..Default::default()
                        });
                    builder.spawn(TextBundle::from_section(
                        "The UI Node and its descendants will not be visible and will not be allotted any space in the UI layout.\nThe UI Node will not be visible but will still occupy space in the UI layout.\nThe UI node will inherit the visibility property of its parent. If it has no parent it will be visible.",
                        text_style
                    ));
            });
    });
}

fn spawn_left_panel(builder: &mut ChildBuilder, palette: &[Color; 4]) -> Vec<Entity> {
    let mut target_ids = vec![];
    builder
        .spawn(NodeBundle {
            style: Style {
                padding: UiRect::all(Val::Px(10.)),
                ..Default::default()
            },
            background_color: BackgroundColor(Color::WHITE),
            ..Default::default()
        })
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    background_color: BackgroundColor(Color::BLACK),
                    ..Default::default()
                })
                .with_children(|parent| {
                    let id = parent
                        .spawn(NodeBundle {
                            style: Style {
                                align_items: AlignItems::FlexEnd,
                                justify_content: JustifyContent::FlexEnd,
                                ..Default::default()
                            },
                            background_color: BackgroundColor(palette[0]),
                            ..Default::default()
                        })
                        .with_children(|parent| {
                            parent.spawn(NodeBundle {
                                style: Style {
                                    width: Val::Px(100.),
                                    height: Val::Px(500.),
                                    ..Default::default()
                                },
                                ..Default::default()
                            });

                            let id = parent
                                .spawn(NodeBundle {
                                    style: Style {
                                        height: Val::Px(400.),
                                        align_items: AlignItems::FlexEnd,
                                        justify_content: JustifyContent::FlexEnd,
                                        ..Default::default()
                                    },
                                    background_color: BackgroundColor(palette[1]),
                                    ..Default::default()
                                })
                                .with_children(|parent| {
                                    parent.spawn(NodeBundle {
                                        style: Style {
                                            width: Val::Px(100.),
                                            height: Val::Px(400.),
                                            ..Default::default()
                                        },
                                        ..Default::default()
                                    });

                                    let id = parent
                                        .spawn(NodeBundle {
                                            style: Style {
                                                height: Val::Px(300.),
                                                align_items: AlignItems::FlexEnd,
                                                justify_content: JustifyContent::FlexEnd,
                                                ..Default::default()
                                            },
                                            background_color: BackgroundColor(palette[2]),
                                            ..Default::default()
                                        })
                                        .with_children(|parent| {
                                            parent.spawn(NodeBundle {
                                                style: Style {
                                                    width: Val::Px(100.),
                                                    height: Val::Px(300.),
                                                    ..Default::default()
                                                },
                                                ..Default::default()
                                            });

                                            let id = parent
                                                .spawn(NodeBundle {
                                                    style: Style {
                                                        width: Val::Px(200.),
                                                        height: Val::Px(200.),
                                                        ..Default::default()
                                                    },
                                                    background_color: BackgroundColor(palette[3]),
                                                    ..Default::default()
                                                })
                                                .id();
                                            target_ids.push(id);
                                        })
                                        .id();
                                    target_ids.push(id);
                                })
                                .id();
                            target_ids.push(id);
                        })
                        .id();
                    target_ids.push(id);
                });
        });
    target_ids
}
