//! Sets up a special event that is sent every frame in the render app when components change.  
//! Since extracted components are inserted every frame, change detection doesnt work.  
//! This module emits events in ExtractSchedule to inform the renderapp of component changes

use std::marker::PhantomData;
use bevy::{prelude::*, render::{Extract, RenderApp}};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Event)]
pub struct ComponentChanged<C: Component>(pub Entity, PhantomData<C>);
impl<C: Component> ComponentChanged<C>{
    pub fn emit_events(
        query: Extract<Query<Entity, Changed<C>>>,
        mut event_writer: EventWriter<Self>
    ){
        event_writer.write_batch(query.iter().map(|entity| Self(entity, PhantomData::default())));
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Event)]
pub struct ResourceChanged<R: Resource>(PhantomData<R>);
impl<R: Resource> ResourceChanged<R>{
    pub fn emit_events(
        resource: Extract<Res<R>>,
        mut event_writer: EventWriter<Self>
    ){
        if resource.is_changed() {
            event_writer.write(Self(PhantomData::default()));
        }
    }
}

pub trait RenderAppChangesExt{
    fn register_render_app_component_change_detection<C: Component>(&mut self) -> &mut Self;
    fn register_render_app_resource_change_detection<R: Resource>(&mut self) -> &mut Self;
}
impl RenderAppChangesExt for App{
    fn register_render_app_component_change_detection<C: Component>(&mut self) -> &mut Self {
        self.add_event::<ComponentChanged<C>>()
        .sub_app_mut(RenderApp)
            .add_event::<ComponentChanged<C>>()
            .add_systems(ExtractSchedule, ComponentChanged::<C>::emit_events);
        self
    }
    fn register_render_app_resource_change_detection<R: Resource>(&mut self) -> &mut Self {
        self.add_event::<ResourceChanged<R>>()
        .sub_app_mut(RenderApp)
            .add_event::<ResourceChanged<R>>()
            .add_systems(ExtractSchedule, ResourceChanged::<R>::emit_events);
        self
    }
}
