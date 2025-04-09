#![doc = include_str!("../README.md")]

use bevy_ecs::{prelude::*, query::QueryFilter};

pub mod prelude {
    pub use crate::{kind, Kind};
    pub use crate::{AsInstance, Instance, InstanceMut, InstanceRef};
    pub use crate::{GetInstanceCommands, InstanceCommands};
    pub use crate::{KindBundle, SpawnInstance, SpawnInstanceWorld};

    #[allow(deprecated)]
    pub use crate::OfKind;
}

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
    type Filter: QueryFilter;

    /// Returns the debug name of this kind.
    ///
    /// By default, this is the short type name (without path) of this kind.
    fn debug_name() -> String {
        moonshine_util::get_short_name(std::any::type_name::<Self>())
    }
}

impl<T: Component> Kind for T {
    type Filter = With<T>;
}

/// Represents the kind of any [`Entity`].
///
/// See [`Instance<Any>`] for more information on usage.
pub struct Any;

impl Kind for Any {
    type Filter = ();
}

mod instance;

pub use instance::*;

/// A trait which allows safe casting from one [`Kind`] to another.
///
/// # Usage
/// Prefer to use the [`kind`] macro to implement this trait.
pub trait CastInto<T: Kind>: Kind {
    fn cast_into(instance: Instance<Self>) -> Instance<T>;
}

impl<T: Kind> CastInto<T> for T {
    fn cast_into(instance: Instance<Self>) -> Instance<Self> {
        instance
    }
}

/// A macro to safely implement [`CastInto`] for a pair of related [`Kind`]s.
///
/// See [`CastInto`] for more information.
///
/// # Usage
/// ```
/// # use bevy::prelude::*;
/// # use moonshine_kind::prelude::*;
///
/// struct Fruit;
///
/// impl Kind for Fruit {
///    type Filter = With<Apple>;
/// }
///
/// #[derive(Component)]
/// struct Apple;
///
/// // We can guarantee all entities with an `Apple` component are of kind `Fruit`:
/// kind!(Apple is Fruit);
///
/// fn eat_apple(apple: Instance<Apple>) {
///    println!("Crunch!");
///    // SAFE: Because we said so.
///    eat_fruit(apple.cast_into());
/// }
///
/// fn eat_fruit(fruit: Instance<Fruit>) {
///    println!("Yum!");
/// }
/// ```
#[macro_export]
macro_rules! kind {
    ($T:ident is $U:ty) => {
        impl $crate::CastInto<$U> for $T {
            fn cast_into(instance: $crate::Instance<Self>) -> $crate::Instance<$U> {
                // SAFE: Because we said so!
                unsafe { instance.cast_into_unchecked() }
            }
        }
    };
}

/// A short alias for using a [`Kind`] as a [`QueryFilter`].
///
/// # Example
/// ```
/// # use bevy::prelude::*;
/// # use moonshine_kind::prelude::*;
///
/// #[derive(Component)]
/// struct Apple;
///
/// fn count_apples(query: Query<(), OfKind<Apple>>) -> usize {
///     query.iter().count()
/// }
///
/// # bevy_ecs::system::assert_is_system(count_apples);
/// ```
#[deprecated(since = "0.2.1", note = "use `T::Filter` instead")]
pub type OfKind<T> = <T as Kind>::Filter;

/// A [`Bundle`] which represents a [`Kind`].
///
/// # Usage
/// This trait is used to allow spawning an [`Instance<T>`] where `T` is [`<Self as KindBundle>::Kind`][`KindBundle::Kind`].
///
/// Any [`Component`] is automatically a kind bundle of its own kind.
///
/// See [`SpawnInstance`] for more information.
pub trait KindBundle: Bundle {
    /// The [`Kind`] represented by this [`Bundle`].
    type Kind: Kind;
}

impl<T: Kind + Bundle> KindBundle for T {
    type Kind = T;
}

/// Extension trait to safely spawn an [`Instance<T>`] using [`Commands`] where `T` associated with a [`KindBundle`].
pub trait SpawnInstance {
    /// Spawns a new [`Instance<T>`] using its associated [`KindBundle`].
    ///
    /// # Example
    /// ```
    /// # use bevy::prelude::*;
    /// # use moonshine_kind::prelude::*;
    ///
    /// #[derive(Component)]
    /// struct Apple;
    ///
    /// fn spawn_apple(mut commands: Commands) {
    ///     let apple: Instance<Apple> = commands.spawn_instance(Apple).instance();
    ///     println!("Spawned {apple:?}!");
    /// }
    ///
    /// # bevy_ecs::system::assert_is_system(spawn_apple);
    #[deprecated(since = "0.2.1")]
    fn spawn_instance<T: KindBundle>(&mut self, _: T) -> InstanceCommands<'_, T::Kind>;
}

impl SpawnInstance for Commands<'_, '_> {
    fn spawn_instance<T: KindBundle>(&mut self, bundle: T) -> InstanceCommands<'_, T::Kind> {
        let entity = self.spawn(bundle).id();
        // SAFE: `entity` must be a valid instance of `T::Kind`.
        unsafe { InstanceCommands::from_entity_unchecked(self.entity(entity)) }
    }
}

/// Extension trait to safely spawn an [`Instance<T>`] using [`World`] where `T` associated with a [`KindBundle`].
pub trait SpawnInstanceWorld {
    /// Spawns a new [`Instance<T>`] using its associated [`KindBundle`].
    ///
    /// # Example
    /// ```
    /// # use bevy::prelude::*;
    /// # use moonshine_kind::prelude::*;
    ///
    /// #[derive(Component)]
    /// struct Apple;
    ///
    /// fn spawn_apple(world: &mut World) {
    ///     let apple: Instance<Apple> = world.spawn_instance(Apple).instance();
    ///     println!("Spawned {apple:?}!");
    /// }
    #[deprecated(since = "0.2.1")]
    fn spawn_instance<T: KindBundle>(&mut self, _: T) -> InstanceMutItem<'_, T::Kind>
    where
        T::Kind: Component;
}

impl SpawnInstanceWorld for World {
    fn spawn_instance<T: KindBundle>(&mut self, bundle: T) -> InstanceMutItem<'_, T::Kind>
    where
        T::Kind: Component,
    {
        let entity = self.spawn(bundle).id();
        // SAFE: `entity` must be a valid instance of kind `T`.
        InstanceMutItem::from_entity(self, entity).unwrap()
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

        kind!(Foo is Bar);

        let any = Instance::<Any>::PLACEHOLDER;
        let foo = Instance::<Foo>::PLACEHOLDER;
        let bar = foo.cast_into::<Bar>();
        assert!(foo.cast_into_any() == any);
        assert!(bar.cast_into_any() == any);
        // assert!(any.cast_into::<Foo>() == foo); // <-- Must not compile!
        // assert!(bar.cast_into::<Foo>() == foo); // <-- Must not compile!
        assert!(bar.entity() == foo.entity());
    }
}
