use std::marker::PhantomData;

use bevy::{prelude::*, render::{sync_component::SyncComponentPlugin, sync_world::RenderEntity, Extract, RenderApp}};

fn extract_changed_components<C: Component+PartialEq+Clone>(
    mut commands: Commands,
    mut previous_len: Local<usize>,
    query: Extract<Query<(RenderEntity, Ref<C>)>>,
    render_data: Query<Has<C>>
) {
    let mut values = Vec::with_capacity(*previous_len);
    for (entity, query_item) in &query {
        // clone if changed or not in render world
        if !render_data.get(entity).unwrap_or(false) || query_item.is_changed(){
            values.push((entity, query_item.clone()));
        }
    }
    *previous_len = values.len();
    commands.try_insert_batch(values);
}
fn extract_changed_visible_components<C: Component+PartialEq+Clone>(
    mut commands: Commands,
    mut previous_len: Local<usize>,
    query: Extract<Query<(RenderEntity, &ViewVisibility, Ref<C>)>>,
    render_data: Query<Has<C>>
) {
    let mut values = Vec::with_capacity(*previous_len);
    for (entity, view_visibility, query_item) in &query {
        if !view_visibility.get() {continue;}
        if !render_data.get(entity).unwrap_or(false) || query_item.is_changed(){
            values.push((entity, query_item.clone()));
        }
    }
    *previous_len = values.len();
    commands.try_insert_batch(values);
}

pub struct ExtractChangedComponentPlugin<C: Component+PartialEq+Clone, F = ()> {
    only_extract_visible: bool,
    marker: PhantomData<fn() -> (C, F)>,
}
impl<C: Component+PartialEq+Clone, F> Default for ExtractChangedComponentPlugin<C, F> {
    fn default() -> Self {
        Self {
            only_extract_visible: false,
            marker: PhantomData,
        }
    }
}
impl<C: Component+PartialEq+Clone, F> ExtractChangedComponentPlugin<C, F> {
    pub fn extract_visible() -> Self {
        Self {
            only_extract_visible: true,
            marker: PhantomData,
        }
    }
}
impl<C: Component+PartialEq+Clone> Plugin for ExtractChangedComponentPlugin<C> {
    fn build(&self, app: &mut App) {
        app.add_plugins(SyncComponentPlugin::<C>::default());

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            if self.only_extract_visible {
                render_app.add_systems(ExtractSchedule, extract_changed_visible_components::<C>);
            } else {
                render_app.add_systems(ExtractSchedule, extract_changed_components::<C>);
            }
        }
    }
}

