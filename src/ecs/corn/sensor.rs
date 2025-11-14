use std::collections::HashMap;

use bevy::{image::ImageSampler, prelude::*};

use crate::ecs::corn::stored::image::ImageCarvedHexagonalShader;

pub struct CornSensorPlugin;
impl Plugin for CornSensorPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CornSensor>();
        app.add_systems(Update, CornSensor::update);
    }
}

#[derive(Clone, Component, Default, Debug, Reflect)]
#[reflect(Component)]
pub struct CornSensor {
    pub is_in_corn: bool,
    pub value: f32,
    pub cornfields: HashMap<Entity, CornFieldLocation>,
}

#[derive(Clone, Debug, Reflect)]
pub struct CornFieldLocation {
    // position in cornfield in UV coordinates
    pub pos: Vec2,

    // value of corn texture
    pub val: Color,
}

// TODO spawn randomly in CornField

impl CornSensor {
    // TODO support other corn types
    fn update(
        mut query: Query<(&mut Self, &GlobalTransform)>,
        cornfields: Query<(Entity, &ImageCarvedHexagonalShader, &GlobalTransform), Without<Self>>,
        images: Res<Assets<Image>>,
    ) {
        for (mut sensor, gt) in query.iter_mut() {
            sensor.is_in_corn = false;
            sensor.value = 0.0;
            for (entity, corn, corn_gt) in cornfields.iter() {
                // sensor position in corn space
                let in_corn_space = gt.reparented_to(corn_gt).translation;

                if in_corn_space.abs().xz().cmplt(corn.half_extents).all() {
                    if let Some(image) = images.get(corn.image.id()) {
                        let u = 0.5 * in_corn_space.x / corn.half_extents.x + 0.5;
                        let v = 0.5 * in_corn_space.z / corn.half_extents.y + 0.5;
                        let pos = Vec2::new(u, v);

                        // let uu = ((u * image.width() as f32).floor() as u32).clamp(0, image.width());
                        // let vu = ((v * image.height() as f32).floor() as u32).clamp(0, image.height());

                        // let val = image.get_color_at(uu,vu);
                        // let mut val = match val {
                        //     Ok(v) => v,
                        //     Err(e) => {
                        //         error!("{}", e);
                        //         sensor.cornfields.remove(&entity);
                        //         continue;
                        //     }
                        // };
                        
                        // TODO pr bevy native image sample
                        let im = image.clone().try_into_dynamic().unwrap(); // XXX doing this every frame
                        let Some(pixel) = image::imageops::sample_bilinear(&im, u, v) else {continue};
                        let val = Color::Srgba(Srgba::from_u8_array(pixel.0));

                        let value = val.to_linear().red;
                        sensor.value = sensor.value.max(value);
                        sensor.is_in_corn |= value != 0.0; // TODO is this the right channel?

                        sensor
                            .cornfields
                            .insert(entity, CornFieldLocation { pos, val });
                    }
                } else {
                    sensor.cornfields.remove(&entity);
                }
            }
        }
    }
}
