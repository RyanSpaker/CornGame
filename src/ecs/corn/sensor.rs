use std::{collections::HashMap, time::Duration};

use bevy::{ecs::{entity, system::SystemParam}, image::ImageSampler, prelude::*};

use crate::ecs::corn::{sensor, stored::image::ImageCarvedHexagonalShader};

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

    pub accumulated: Duration,
}

#[derive(Clone, Debug, Reflect)]
pub struct CornFieldLocation {
    // position in cornfield in UV coordinates
    pub pos: Vec2,

    // value of corn texture
    pub val: Color,
}

// TODO spawn randomly in CornField

/// get information needed for querying corn location
#[derive(SystemParam)]
pub struct CornQuery<'w, 's> {
    cornfields: Query<'w, 's, (Entity, &'static ImageCarvedHexagonalShader, &'static GlobalTransform), Without<CornSensor>>,
    images: Res<'w, Assets<Image>>,
}

/// TODO
pub struct CornSensorData {
    pub is_in_corn: bool,

    // TODO get nearest corn pixel location
    // TODO: create a image with nearest corn pixel precomputed for lookup
    // pub nearest: Vec3,
    pub cornfield: Option<Entity>,
    pub value: f32,

    pub gradient: Option<Vec2>,
}

pub struct GradientTestSettings {
    pub sample_offset: f32,
    pub num_samples: usize,
}

impl<'w, 's> CornQuery<'w, 's> {
    // EAS: I am working on splitting this out because character controllers needs to ask about whether we *will* be in corn, not just whether we are currently in corn.
    pub fn test(&self, position: Vec3, gradient: Option<GradientTestSettings>) -> CornSensorData {
        let gt = GlobalTransform::from_translation(position);

        let mut ret = CornSensorData {
            is_in_corn: false,
            value: 0.0,
            cornfield: None,
            gradient: None,
        };

        for (entity, corn, corn_gt) in self.cornfields.iter() {
            //TODO make corn fields use transform for gods sake!
            let corn_gt : GlobalTransform = Transform::from_translation(corn.center).into();

            // sensor position in corn space
            let in_corn_space = gt.reparented_to(&corn_gt).translation;

            if !in_corn_space.abs().xz().cmplt(corn.half_extents).all() {
                // we are outside this cornfield
                continue;
            }

            let Some(image) = self.images.get(corn.image.id()) else {
                // treat missing image as no corn
                continue;
            };

            if ret.cornfield.is_none() {
                // set cornfield if we are in one. 
                // XXX If in multiple cornfields, but not in any corn, this will be inconsistent.
                ret.cornfield = Some(entity);
            }

            let u = 0.5 * in_corn_space.x / corn.half_extents.x + 0.5;
            let v = 0.5 * in_corn_space.z / corn.half_extents.y + 0.5;
            let pos = Vec2::new(u, v);

            let value = sample_bilinear(image, u, v).unwrap().red;

            if let Some(grad_settings) = &gradient {
                // sample gradient
                let mut grad = Vec2::ZERO;
                let offset = grad_settings.sample_offset; // TODO assume square pixels
                let samples = grad_settings.num_samples as f32;

                for i in 0..grad_settings.num_samples {
                    let angle = (i as f32 / samples) * std::f32::consts::TAU;

                    // TODO is there a more efficient way to do this (avoiding gt recompute)
                    let sample_pos = GlobalTransform::from_translation(
                        position + Vec3::new(angle.cos(), 0.0, angle.sin()) * offset,
                    ).reparented_to(&corn_gt).translation;

                    let sample_uv_pos = Vec2::new(
                        0.5 * sample_pos.x / corn.half_extents.x + 0.5,
                        0.5 * sample_pos.z / corn.half_extents.y + 0.5,
                    );

                    if let Some(sample_value) = sample_bilinear(image, sample_uv_pos.x, sample_uv_pos.y) {
                        let sample_value = sample_value.red;
                        grad += Vec2::new(angle.cos(), angle.sin()) * (sample_value - value);
                    }
                }

                grad /= samples * offset;

                // add gradients so edges between cornfields cancel out
                ret.gradient = Some(match ret.gradient {
                    Some(existing) => existing + grad,
                    None => grad,
                });
            }

            ret.value = ret.value.max(value);
            if value > 0.1 { // TODO is this the right channel?
                ret.is_in_corn = true;
                ret.cornfield = Some(entity);
            }
        }

        ret
    }
}

fn sample_bilinear(image: &Image, u: f32, v: f32) -> Option<LinearRgba> {
    // sample bilinearly from image at uv coordinates
    let x = u * (image.width() as f32 - 0.5);
    let y = v * (image.height() as f32 - 0.5);

    let x0 = x.floor() as u32;
    let x1 = (x0 + 1).min(image.width() - 1);
    let y0 = y.floor() as u32;
    let y1 = (y0 + 1).min(image.height() - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let c00 = image.get_color_at(x0, y0).ok()?.to_linear();
    let c10 = image.get_color_at(x1, y0).ok()?.to_linear();
    let c01 = image.get_color_at(x0, y1).ok()?.to_linear();
    let c11 = image.get_color_at(x1, y1).ok()?.to_linear(); // TODO conversion to linear should be unnecessary
    let c0 = c00 * (1.0 - fx) + c10 * fx;
    let c1 = c01 * (1.0 - fx) + c11 * fx;
    Some(c0 * (1.0 - fy) + c1 * fy)
}   

fn nearest_pixel(uv: Vec2, image: &Image, white:bool ) -> Option<Vec2> {
    let start_x = (uv.x * image.width() as f32).round() as u32;
    let start_y = (uv.y * image.height() as f32).round() as u32;

    //Todo
    None
}

impl CornSensor {
    // TODO support other corn types
    fn update(
        mut query: Query<(&mut Self, &GlobalTransform)>,
        cornfields: Query<(Entity, &ImageCarvedHexagonalShader, &GlobalTransform), Without<Self>>,
        images: Res<Assets<Image>>,
        time: Res<Time>,
    ) {
        for (mut sensor, gt) in query.iter_mut() {
            let v = sensor.value;
            sensor.accumulated += time.delta().mul_f32(v);
            sensor.is_in_corn = false;
            sensor.value = 0.0;
            for (entity, corn, corn_gt) in cornfields.iter() {
                // TODO make corn fields use transform
                let corn_gt : GlobalTransform = Transform::from_translation(corn.center).into();

                // sensor position in corn space
                let in_corn_space = gt.reparented_to(&corn_gt).translation;

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

                        let val2 = sample_bilinear(image, u, v).unwrap().red;
                        if (val2 - val.to_linear().red).abs() >= 0.01 {
                            trace!("Sample mismatch: {} vs {}", val2, val.to_linear().red);
                        }

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
