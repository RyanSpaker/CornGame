use bevy::app::{Plugin, Update};

use self::{image_carved_corn_field::ImageCarvedHexagonalCornField, simple_corn_field::{SimpleRectangularCornField, SimpleHexagonalCornField}};

use super::data_pipeline::RenderableCornFieldPlugin;

pub mod simple_corn_field;
pub mod image_carved_corn_field;

pub struct CornFieldsPlugin{}
impl Plugin for CornFieldsPlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.register_type::<SimpleHexagonalCornField>()
            .register_type::<SimpleRectangularCornField>()
            .register_type::<ImageCarvedHexagonalCornField>()
            .add_systems(Update, ImageCarvedHexagonalCornField::update_image_state);
        app.add_plugins((
            RenderableCornFieldPlugin::<SimpleHexagonalCornField>::new(),
            RenderableCornFieldPlugin::<SimpleRectangularCornField>::new(),
            RenderableCornFieldPlugin::<ImageCarvedHexagonalCornField>::new()
        ));
    }
}