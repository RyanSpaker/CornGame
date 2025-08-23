use std::any::TypeId;

use bevy_editor_pls::default_windows::query::{DynamicRelation, DynamicRelationDumb, DynamicRelationMetadata};
use bevy::{animation::AnimationTarget, prelude::*};

// bevy
// XXX cheating because this isn't a relation
struct DynRelAnimationTarget;
impl DynamicRelationDumb for DynRelAnimationTarget {
    fn parent(&self,world: &mut World, entity:Entity) -> Option<Entity>  {
        world.get::<AnimationTarget>(entity).map(|t|t.player)
    }

    fn children(&self,world: &mut World, entity:Entity) -> Option<Vec<Entity> >  {
        let v : Vec<Entity> = world.query::<(Entity, &AnimationTarget)>().iter(world).filter(|t| entity == t.1.player).map(|t|t.0).collect();
        if v.is_empty(){
            None
        }else{
            Some(v)
        }
    }

    fn metadate(&self) -> DynamicRelationMetadata {
        DynamicRelationMetadata {
            relationship: TypeId::of::<AnimationTarget>(),
            relationship_target: TypeId::of::<AnimationPlayer>(),
        }
    }
}

// lightyear
struct DynRelAeronetLinkOf;
impl DynamicRelation for DynRelAeronetLinkOf {
    type Relationship = lightyear_aeronet::AeronetLinkOf;
}

struct DynRelLinkOf;
impl DynamicRelation for DynRelLinkOf {
    type Relationship = lightyear::prelude::LinkOf;
}

struct DynRelControlledBy;
impl DynamicRelation for DynRelControlledBy {
    type Relationship = lightyear::prelude::ControlledBy;
}

struct DynRelReplicateLike;
impl DynamicRelation for DynRelReplicateLike {
    type Relationship = lightyear::prelude::ReplicateLike;
}

// avian
struct DynRelColliderOf;
impl DynamicRelation for DynRelColliderOf {
    type Relationship = avian3d::prelude::ColliderOf;
}