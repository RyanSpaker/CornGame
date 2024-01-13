use bevy::app::{Plugin, Update};

use self::image_carved_corn_field::ImageCarvedHexagonalCornField;

pub mod simple_corn_field;
pub mod image_carved_corn_field;

pub struct CornFieldsPlugin{}
impl Plugin for CornFieldsPlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.register_type::<simple_corn_field::SimpleHexagonalCornField>()
            .register_type::<simple_corn_field::SimpleRectangularCornField>()
            .register_type::<ImageCarvedHexagonalCornField>()
            .add_systems(Update, ImageCarvedHexagonalCornField::update_image_state);
    }
}