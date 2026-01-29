//! utils to componentize behavior for replication and bundles

// https://github.com/tim-blackbird/bevy_observers/blob/main/src/lib.rs
use bevy::ecs::{component::HookContext, prelude::*, world::DeferredWorld};

/// A macro for setting [`Observer`]s on an entity from within a [`Bundle`]. It is similar to the [`children`] macro, but for observers.
///
/// ```rust
/// # use bevy::prelude::*;
/// # use bevy_bundled_observers::observers;
/// # #[derive(Event)] struct OnCollect;
///
/// fn coin() -> impl Bundle {
///     (
///         Name::new("Coin"),
///         observers![|_: Trigger<OnCollect>| {
///             info!("You collected a coin!");
///         }],
///     )
/// }
/// ```
#[macro_export]
macro_rules! observers {
    [$($observer:expr),*$(,)?] => {
       $crate::Observers(vec![$(bevy::ecs::observer::Observer::new($observer)),*])
    };
}

/// A component that sets observers on an entity when inserted. This is the underlying mechanism for the [`observers`] macro.
///
/// The component is immediately emptied and promptly removed after insertion.
///
/// The code example below shows what the [`observers`] macro expands to.
///
/// ```rust
/// # use bevy::prelude::*;
/// # use bevy_bundled_observers::Observers;
/// # #[derive(Event)] struct OnCollect;
///
/// fn coin() -> impl Bundle {
///     (
///         Name::new("Coin"),
///         Observers(vec![Observer::new(|_: Trigger<OnCollect>| {
///             info!("You collected a coin!");
///         })]),
///     )
/// }
/// ```
#[derive(Component)]
#[component(on_insert = on_insert)]
pub struct Observers(pub Vec<Observer>);

fn on_insert(mut world: DeferredWorld, context: HookContext) {
    let mut component: Mut<Observers> = world.get_mut(context.entity).unwrap();

    let observers = core::mem::take(&mut component.0)
        .into_iter()
        .map(move |observer| observer.with_entity(context.entity));

    let mut commands = world.commands();
    commands.spawn_batch(observers);
    commands.entity(context.entity).remove::<Observers>();
}