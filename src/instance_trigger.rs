use bevy_ecs::{
    event::{Trigger, trigger_entity_internal},
    observer::{CachedObservers, TriggerContext},
    prelude::*,
    traversal::Traversal,
    world::DeferredWorld,
};
use std::{fmt, marker::PhantomData};

/// A custom trigger for events targeting an [`Instance`], differing from the default
/// [`EntityEvent`] / [`PropagateEntityTrigger`] pair in two ways:
///
///
pub struct InstanceTrigger<E: Event, T: Traversal<E>, K: Component> {
    /// The original [`Entity`] the [`Event`] was _first_ triggered for.
    pub original_event_target: Entity,
    /// [`Entity`] the [`Event`] is _currently_ triggered for.
    pub event_target: Entity,

    /// Whether or not to continue propagating using the `T` [`Traversal`]. If this is false,
    /// The [`Traversal`] will stop on the current entity.
    pub propagate: bool,

    _marker: PhantomData<(E, T, K)>,
}

impl<E: Event, T: Traversal<E>, K: Component> InstanceTrigger<E, T, K> {
    /// Create a new [`InstanceTrigger`] with the specified component.
    pub fn new(event_target: Entity, propagate: bool) -> Self {
        Self {
            original_event_target: event_target,
            event_target,
            propagate,
            _marker: Default::default(),
        }
    }
}

impl<E: Event, T: Traversal<E>, K: Component> fmt::Debug for InstanceTrigger<E, T, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstanceTrigger")
            .field("original_event_target", &self.original_event_target)
            .field("propagate", &self.propagate)
            .field("kind", &std::any::type_name::<K>())
            .field("_marker", &self._marker)
            .finish()
    }
}

// SAFETY:
// - `E`'s [`Event::Trigger`] is constrained to [`InstanceTrigger<E>`]
unsafe impl<E: for<'a> Event<Trigger<'a> = Self>, T: Traversal<E>, K: Component> Trigger<E>
    for InstanceTrigger<E, T, K>
{
    unsafe fn trigger(
        &mut self,
        mut world: DeferredWorld,
        observers: &CachedObservers,
        trigger_context: &TriggerContext,
        event: &mut E,
    ) {
        if !world.entity(self.event_target).contains::<K>() {
            // here you can insert custom error handling as required
            panic!(
                "the triggered entity is not of kind {}",
                std::any::type_name::<K>()
            )
        }
        // let kind_query = world.query()
        // SAFETY:
        // - `observers` come from `world` and match the event type `E`, enforced by the call to `trigger`
        // - the passed in event pointer comes from `event`, which is an `Event`
        // - `trigger` is a matching trigger type, as it comes from `self`, which is the Trigger for `E`
        // - `trigger_context`'s event_key matches `E`, enforced by the call to `trigger`

        unsafe {
            let target = self.event_target;
            trigger_entity_internal(
                world.reborrow(),
                observers,
                event.into(),
                self.into(),
                target,
                trigger_context,
            );
        }

        loop {
            if !self.propagate {
                return;
            }
            if let Ok(entity) = world.get_entity(self.event_target)
                && let Some(item) = entity.get_components::<T>()
                && let Some(traverse_to) = T::traverse(item, event)
            {
                self.event_target = traverse_to;
            } else {
                break;
            }
            if !world.entity(self.event_target).contains::<K>() {
                println!("skipped ancestor, does not match");
                // here i'm deciding to 'jump over' ancestors without K but you
                // could also break or panic
                continue;
            }

            // SAFETY:
            // - `observers` come from `world` and match the event type `E`, enforced by the call to `trigger`
            // - the passed in event pointer comes from `event`, which is an `Event`
            // - `trigger` is a matching trigger type, as it comes from `self`, which is the Trigger for `E`
            // - `trigger_context`'s event_key matches `E`, enforced by the call to `trigger`
            unsafe {
                let target = self.event_target;
                trigger_entity_internal(
                    world.reborrow(),
                    observers,
                    event.into(),
                    self.into(),
                    target,
                    trigger_context,
                );
            }
        }
    }
}
