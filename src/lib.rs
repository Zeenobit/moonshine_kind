#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

/// Prelude module to import all necessary traits and types for [`Kind`] semantics.
pub mod prelude {
    pub use crate::{CastInto, Kind};
    pub use crate::{
        ComponentInstance, InsertInstance, InsertInstanceWorld, SpawnInstance, SpawnInstanceWorld,
    };
    pub use crate::{ContainsInstance, Instance, InstanceMut, InstanceRef};
    pub use crate::{GetInstanceCommands, InstanceCommands};
    pub use crate::{GetTriggerTargetInstance, TriggerInstance};
}

mod instance;

use bevy_ecs::world::DeferredWorld;
use bevy_reflect::TypePath;
pub use instance::*;

use bevy_ecs::component::Mutable;
use bevy_ecs::{prelude::*, query::QueryFilter};

/// A type which represents the kind of an [`Entity`].
///
/// An entity is of kind `T` if it matches [`Query<Entity, <T as Kind>::Filter>`][`Query`].
///
/// By default, an entity with a [`Component`] of type `T` is also of kind `T`.
///
/// # Examples
/// ```
/// # use bevy::prelude::*;
/// # use moonshine_kind::prelude::*;
///
/// #[derive(Component)]
/// struct Apple;
///
/// #[derive(Component)]
/// struct Orange;
///
/// struct Fruit;
///
/// impl Kind for Fruit {
///     type Filter = Or<(With<Apple>, With<Orange>)>;
/// }
///
/// fn fruits(query: Query<Instance<Fruit>>) {
///     for fruit in query.iter() {
///         println!("{fruit:?} is a fruit!");
///     }
/// }
///
/// # bevy_ecs::system::assert_is_system(fruits);
/// ```
pub trait Kind: 'static + Send + Sized + Sync {
    /// The [`QueryFilter`] which defines this kind.
    type Filter: QueryFilter;

    /// Returns the debug name of this kind.
    ///
    /// By default, this is the short type name (without path) of this kind.
    /// This is mainly used for [`Debug`](std::fmt::Debug) and [`Display`](std::fmt::Display) implementations.
    fn debug_name() -> String {
        disqualified::ShortName::of::<Self>().to_string()
    }
}

impl<T: Component> Kind for T {
    type Filter = With<T>;
}

/// Represents the kind of any [`Entity`].
///
/// See [`Instance<Any>`] for more information on usage.
#[derive(TypePath)]
pub struct Any;

impl Kind for Any {
    type Filter = ();
}

/// A trait which allows safe casting from one [`Kind`] to another.
pub trait CastInto<T: Kind>: Kind {
    #[doc(hidden)]
    unsafe fn cast(instance: Instance<Self>) -> Instance<T> {
        // SAFE: Because we said so.
        // TODO: Can we use required components to enforce this?
        Instance::from_entity_unchecked(instance.entity())
    }
}

impl<T: Kind> CastInto<T> for T {
    unsafe fn cast(instance: Instance<Self>) -> Instance<T> {
        Instance::from_entity_unchecked(instance.entity())
    }
}

/// Extension trait used to spawn instances via [`Commands`].
pub trait SpawnInstance {
    /// Spawns a new [`Entity`] which contains the given instance of `T` and returns an [`InstanceCommands<T>`] for it.
    fn spawn_instance<T: Component>(&mut self, instance: T) -> InstanceCommands<'_, T>;
}

impl SpawnInstance for Commands<'_, '_> {
    fn spawn_instance<T: Component>(&mut self, instance: T) -> InstanceCommands<'_, T> {
        let entity = self.spawn(instance).id();
        // SAFE: `entity` is spawned as a valid instance of kind `T`.
        unsafe { InstanceCommands::from_entity_unchecked(self.entity(entity)) }
    }
}

/// Extension trait used to spawn instances via [`World`].
pub trait SpawnInstanceWorld {
    /// Spawns a new [`Entity`] which contains the given instance of `T` and returns an [`InstanceRef<T>`] for it.
    fn spawn_instance<T: Component>(&mut self, instance: T) -> InstanceRef<T>;

    /// Spawns a new [`Entity`] which contains the given instance of `T` and returns an [`InstanceMut<T>`] for it.
    fn spawn_instance_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        instance: T,
    ) -> InstanceMut<T>;
}

impl SpawnInstanceWorld for World {
    fn spawn_instance<T: Component>(&mut self, instance: T) -> InstanceRef<T> {
        let mut entity = self.spawn_empty();
        entity.insert(instance);
        // SAFE: `entity` is spawned as a valid instance of kind `T`.
        unsafe { InstanceRef::from_entity_unchecked(entity.into_readonly()) }
    }

    fn spawn_instance_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        instance: T,
    ) -> InstanceMut<T> {
        let mut entity = self.spawn_empty();
        entity.insert(instance);
        // SAFE: `entity` is spawned as a valid instance of kind `T`.
        unsafe { InstanceMut::from_entity_unchecked(entity.into_mutable()) }
    }
}

/// Extension trait used to insert instances via [`EntityCommands`].
pub trait InsertInstance {
    /// Inserts the given instance of `T` into the entity and returns an [`InstanceCommands<T>`] for it.
    fn insert_instance<T: Component>(&mut self, instance: T) -> InstanceCommands<'_, T>;
}

impl InsertInstance for EntityCommands<'_> {
    fn insert_instance<T: Component>(&mut self, instance: T) -> InstanceCommands<'_, T> {
        self.insert(instance);
        // SAFE: `entity` is spawned as a valid instance of kind `T`.
        unsafe { InstanceCommands::from_entity_unchecked(self.reborrow()) }
    }
}

/// Extension trait used to insert instances via [`EntityWorldMut`].
pub trait InsertInstanceWorld {
    /// Inserts the given instance of `T` into the entity and returns an [`InstanceRef<T>`] for it.
    fn insert_instance<T: Component>(&mut self, instance: T) -> InstanceRef<T>;

    /// Inserts the given instance of `T` into the entity and returns an [`InstanceMut<T>`] for it.
    ///
    /// This requires `T` to be [`Mutable`].
    fn insert_instance_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        instance: T,
    ) -> InstanceMut<T>;
}

impl InsertInstanceWorld for EntityWorldMut<'_> {
    fn insert_instance<T: Component>(&mut self, instance: T) -> InstanceRef<T> {
        self.insert(instance);
        // SAFE: `entity` is spawned as a valid instance of kind `T`.
        InstanceRef::from_entity(self.as_readonly()).unwrap()
    }

    fn insert_instance_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        instance: T,
    ) -> InstanceMut<T> {
        self.insert(instance);
        // SAFE: `entity` is spawned as a valid instance of kind `T`.
        InstanceMut::from_entity(self.as_mutable()).unwrap()
    }
}

/// Extension trait used to get [`Component`] data from an [`Instance<T>`] via [`World`].
pub trait ComponentInstance {
    /// Returns a reference to the given instance.
    fn instance<T: Component>(&self, instance: Instance<T>) -> Option<InstanceRef<T>>;

    /// Returns a reference to the given instance, if it is of [`Kind`] `T`.
    fn get_instance<T: Component>(&self, entity: Entity) -> Option<InstanceRef<T>> {
        // SAFE: Inner function will ensure entity is a valid instance of kind `T`
        self.instance(unsafe { Instance::from_entity_unchecked(entity) })
    }

    /// Returns a mutable reference to the given instance.
    ///
    /// This requires `T` to be [`Mutable`].
    fn instance_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        instance: Instance<T>,
    ) -> Option<InstanceMut<T>>;

    /// Returns a mutable reference to the given instance, if it is of [`Kind`] `T`.
    ///
    /// This requires `T` to be [`Mutable`].
    fn get_instance_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        entity: Entity,
    ) -> Option<InstanceMut<T>> {
        // SAFE: Inner function will ensure entity is a valid instance of kind `T`
        self.instance_mut(unsafe { Instance::from_entity_unchecked(entity) })
    }
}

impl ComponentInstance for World {
    fn instance<T: Component>(&self, instance: Instance<T>) -> Option<InstanceRef<T>> {
        InstanceRef::from_entity(self.entity(instance.entity()))
    }

    fn instance_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        instance: Instance<T>,
    ) -> Option<InstanceMut<T>> {
        InstanceMut::from_entity(self.entity_mut(instance.entity()).into_mutable())
    }
}

impl ComponentInstance for DeferredWorld<'_> {
    fn instance<T: Component>(&self, instance: Instance<T>) -> Option<InstanceRef<T>> {
        InstanceRef::from_entity(self.entity(instance.entity()))
    }

    fn instance_mut<T: Component<Mutability = Mutable>>(
        &mut self,
        instance: Instance<T>,
    ) -> Option<InstanceMut<T>> {
        InstanceMut::from_entity(self.entity_mut(instance.entity()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    fn count<T: Kind>(query: Query<Instance<T>>) -> usize {
        query.iter().count()
    }

    #[test]
    fn kind_with() {
        #[derive(Component)]
        struct Foo;

        let mut world = World::new();
        world.spawn(Foo);
        assert_eq!(world.run_system_once(count::<Foo>).unwrap(), 1);
    }

    #[test]
    fn kind_without() {
        #[derive(Component)]
        struct Foo;

        struct NotFoo;

        impl Kind for NotFoo {
            type Filter = Without<Foo>;
        }

        let mut world = World::new();
        world.spawn(Foo);
        assert_eq!(world.run_system_once(count::<NotFoo>).unwrap(), 0);
    }

    #[test]
    fn kind_multi() {
        #[derive(Component)]
        struct Foo;

        #[derive(Component)]
        struct Bar;

        let mut world = World::new();
        world.spawn((Foo, Bar));
        assert_eq!(world.run_system_once(count::<Foo>).unwrap(), 1);
        assert_eq!(world.run_system_once(count::<Bar>).unwrap(), 1);
    }

    #[test]
    fn kind_cast() {
        #[derive(Component)]
        struct Foo;

        #[derive(Component)]
        struct Bar;

        impl CastInto<Bar> for Foo {}

        let any = Instance::<Any>::PLACEHOLDER;
        let foo = Instance::<Foo>::PLACEHOLDER;
        let bar = foo.cast_into::<Bar>();
        assert!(foo.cast_into_any() == any);
        assert!(bar.cast_into_any() == any);
        // assert!(foo.cast_into() == any); // TODO: Can we make this compile?
        // assert!(any.cast_into::<Foo>() == foo); // <-- Must not compile!
        // assert!(bar.cast_into::<Foo>() == foo); // <-- Must not compile!
        assert!(bar.entity() == foo.entity());
    }
}
