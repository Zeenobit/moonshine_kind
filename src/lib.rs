#![doc = include_str!("../README.md")]

use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use bevy_ecs::{
    archetype::Archetype,
    component::{ComponentId, Tick},
    entity::{EntityMapper, MapEntities},
    prelude::*,
    query::{FilteredAccess, QueryData, QueryFilter, ReadOnlyQueryData, WorldQuery},
    storage::{Table, TableRow},
    system::EntityCommands,
    world::unsafe_world_cell::UnsafeWorldCell,
};
use bevy_reflect::Reflect;

pub mod prelude {
    pub use crate::{
        safe_cast, GetInstanceCommands, Instance, InstanceCommands, InstanceMut, InstanceMutItem,
        InstanceRef, Kind, KindBundle, SpawnInstance, SpawnInstanceWorld, WithKind,
    };
}

/// A type which represents the kind of an [`Entity`].
///
/// An entity is of kind `T` if it matches `Query<Entity, <T as Kind>::Filter>`.
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
///         println!("{fruit:?}");
///     }
/// }
/// ```
pub trait Kind: 'static + Send + Sized + Sync {
    type Filter: QueryFilter;

    /// Returns the debug name of this kind.
    ///
    /// By default, this is the short type name (without path) of this kind.
    fn debug_name() -> String {
        bevy_utils::get_short_name(std::any::type_name::<Self>())
    }
}

impl<T: Component> Kind for T {
    type Filter = With<T>;
}

/// Represents the kind of any [`Entity`].
#[derive(Default, Clone, Copy)]
pub struct Any;

impl Kind for Any {
    type Filter = ();
}

/// Represents an [`Entity`] of kind `T`.
///
/// `Instance<Any>` is functionally equivalent to [`Entity`].
///
/// # Usage
/// An `Instance<T>` can be used to access entities in a "kind-safe" manner. This allows game logic to
/// reference entities in a safer and more readable way.
///
/// # Example
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
/// #[derive(Resource, Deref, DerefMut)]
/// struct FruitBasket(Vec<Instance<Fruit>>);
///
/// fn collect_fruits(mut basket: ResMut<FruitBasket>, fruits: Query<Instance<Fruit>>) {
///     for fruit in fruits.iter() {
///         println!("{fruit:?}");
///         basket.push(fruit);
///     }
/// }
/// ```
#[derive(Reflect)]
pub struct Instance<T: Kind>(Entity, #[reflect(ignore)] PhantomData<T>);

impl<T: Kind> Instance<T> {
    pub const PLACEHOLDER: Self = Self(Entity::PLACEHOLDER, PhantomData);

    /// Creates a new instance of kind `T` from some [`Entity`].
    ///
    /// # Safety
    /// Assumes `entity` is a valid instance of kind `T`.
    pub unsafe fn from_entity_unchecked(entity: Entity) -> Self {
        Self(entity, PhantomData)
    }

    /// Returns the [`Entity`] of this instance.
    pub fn entity(&self) -> Entity {
        self.0
    }

    /// Converts this instance into an instance of kind `U`.
    pub fn cast_into<U: Kind>(self) -> Instance<U>
    where
        T: CastInto<U>,
    {
        T::cast_into(self)
    }

    /// Converts this instance into an instance of kind `U` without checking if `T` is convertible.
    ///
    /// # Safety
    /// Assumes this instance is also a valid `Instance<U>`.
    pub unsafe fn cast_into_unchecked<U: Kind>(self) -> Instance<U> {
        Instance::from_entity_unchecked(self.entity())
    }
}

impl<T: Component> Instance<T> {
    pub fn from_entity(entity: EntityRef) -> Option<Self> {
        if entity.contains::<T>() {
            // SAFE: `entity` must be of kind `T`.
            Some(unsafe { Self::from_entity_unchecked(entity.id()) })
        } else {
            None
        }
    }
}

impl<T: Kind> Clone for Instance<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Kind> Copy for Instance<T> {}

impl<T: Kind> fmt::Debug for Instance<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(&T::debug_name()).field(&self.0).finish()
    }
}

impl<T: Kind> Hash for Instance<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T: Kind> PartialEq for Instance<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Kind> PartialEq<Entity> for Instance<T> {
    fn eq(&self, other: &Entity) -> bool {
        self.0 == *other
    }
}

impl<T: Kind> Eq for Instance<T> {}

impl<T: Kind> PartialOrd for Instance<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Kind> Ord for Instance<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

unsafe impl<T: Kind> WorldQuery for Instance<T> {
    type Item<'a> = Instance<T>;

    type Fetch<'a> = <T::Filter as WorldQuery>::Fetch<'a>;

    type State = <T::Filter as WorldQuery>::State;

    fn shrink<'wlong: 'wshort, 'wshort>(item: Self::Item<'wlong>) -> Self::Item<'wshort> {
        item
    }

    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        state: &Self::State,
        last_change_tick: Tick,
        change_tick: Tick,
    ) -> Self::Fetch<'w> {
        <T::Filter as WorldQuery>::init_fetch(world, state, last_change_tick, change_tick)
    }

    const IS_DENSE: bool = <T::Filter as WorldQuery>::IS_DENSE;

    unsafe fn set_archetype<'w>(
        _fetch: &mut Self::Fetch<'w>,
        _state: &Self::State,
        _archetype: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    unsafe fn set_table<'w>(fetch: &mut Self::Fetch<'w>, state: &Self::State, table: &'w Table) {
        <T::Filter as WorldQuery>::set_table(fetch, state, table)
    }

    unsafe fn fetch<'w>(
        _fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Self::Item<'w> {
        Instance::from_entity_unchecked(entity)
    }

    fn update_component_access(state: &Self::State, access: &mut FilteredAccess<ComponentId>) {
        <T::Filter as WorldQuery>::update_component_access(state, access)
    }

    fn get_state(world: &World) -> Option<Self::State> {
        <T::Filter as WorldQuery>::get_state(world)
    }

    fn init_state(world: &mut World) -> Self::State {
        <T::Filter as WorldQuery>::init_state(world)
    }

    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        <T::Filter as WorldQuery>::matches_component_set(state, set_contains_id)
    }
}

unsafe impl<T: Kind> ReadOnlyQueryData for Instance<T> {}

unsafe impl<T: Kind> QueryData for Instance<T> {
    type ReadOnly = Self;
}

impl<T: Kind> MapEntities for Instance<T> {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.0 = entity_mapper.map_entity(self.0);
    }
}

impl<T: Kind> From<Instance<T>> for Entity {
    fn from(instance: Instance<T>) -> Self {
        instance.entity()
    }
}

impl From<Entity> for Instance<Any> {
    fn from(entity: Entity) -> Self {
        Self(entity, PhantomData)
    }
}

pub trait CastInto<T: Kind>: Kind {
    fn cast_into(instance: Instance<Self>) -> Instance<T>;
}

impl<T: Kind> CastInto<Any> for T {
    fn cast_into(instance: Instance<Self>) -> Instance<Any> {
        // SAFE: `T` is convertible to `Any`.
        unsafe { Instance::from_entity_unchecked(instance.entity()) }
    }
}

#[macro_export]
macro_rules! safe_cast {
    ($T:ty => $U:ty) => {
        impl $crate::CastInto<$U> for $T {
            fn cast_into(instance: $crate::Instance<Self>) -> $crate::Instance<$U> {
                // SAFE: Because we said so!
                unsafe { instance.cast_into_unchecked() }
            }
        }
    };
}

pub struct InstanceRef<'a, T: Component> {
    instance: Instance<T>,
    data: &'a T,
}

unsafe impl<'a, T: Component> WorldQuery for InstanceRef<'a, T> {
    type Item<'w> = InstanceRef<'w, T>;

    type Fetch<'w> = <(Instance<T>, &'static T) as WorldQuery>::Fetch<'w>;

    type State = <(Instance<T>, &'static T) as WorldQuery>::State;

    fn shrink<'wlong: 'wshort, 'wshort>(item: Self::Item<'wlong>) -> Self::Item<'wshort> {
        InstanceRef {
            instance: item.instance,
            data: item.data,
        }
    }

    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        state: &Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        <(Instance<T>, &T) as WorldQuery>::init_fetch(world, state, last_run, this_run)
    }

    const IS_DENSE: bool = <(Instance<T>, &T) as WorldQuery>::IS_DENSE;

    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        archetype: &'w Archetype,
        table: &'w Table,
    ) {
        <(Instance<T>, &T) as WorldQuery>::set_archetype(fetch, state, archetype, table)
    }

    unsafe fn set_table<'w>(fetch: &mut Self::Fetch<'w>, state: &Self::State, table: &'w Table) {
        <(Instance<T>, &T) as WorldQuery>::set_table(fetch, state, table)
    }

    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w> {
        let (instance, data) = <(Instance<T>, &T) as WorldQuery>::fetch(fetch, entity, table_row);
        Self::Item { instance, data }
    }

    fn update_component_access(state: &Self::State, access: &mut FilteredAccess<ComponentId>) {
        <(Instance<T>, &T) as WorldQuery>::update_component_access(state, access)
    }

    fn init_state(world: &mut World) -> Self::State {
        <(Instance<T>, &T) as WorldQuery>::init_state(world)
    }

    fn get_state(world: &World) -> Option<Self::State> {
        <(Instance<T>, &T) as WorldQuery>::get_state(world)
    }

    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        <(Instance<T>, &T) as WorldQuery>::matches_component_set(state, set_contains_id)
    }
}

unsafe impl<'a, T: Component> QueryData for InstanceRef<'a, T> {
    type ReadOnly = Self;
}

unsafe impl<'a, T: Component> ReadOnlyQueryData for InstanceRef<'a, T> {}

impl<'a, T: Component> InstanceRef<'a, T> {
    pub fn from_entity(entity: EntityRef<'a>) -> Option<Self> {
        Some(Self {
            data: entity.get()?,
            // SAFE: Kind is validated by `entity.get()` above.
            instance: unsafe { Instance::from_entity_unchecked(entity.id()) },
        })
    }

    pub fn entity(&self) -> Entity {
        self.instance.entity()
    }

    pub fn instance(&self) -> Instance<T> {
        self.instance
    }
}

impl<T: Component> Clone for InstanceRef<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Component> Copy for InstanceRef<'_, T> {}

impl<T: Component> From<InstanceRef<'_, T>> for Instance<T> {
    fn from(item: InstanceRef<T>) -> Self {
        item.instance()
    }
}

impl<T: Component> From<&InstanceRef<'_, T>> for Instance<T> {
    fn from(item: &InstanceRef<T>) -> Self {
        item.instance()
    }
}

impl<T: Component> PartialEq for InstanceRef<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.instance == other.instance
    }
}

impl<T: Component> Eq for InstanceRef<'_, T> {}

impl<T: Component> Deref for InstanceRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T: Component> AsRef<Instance<T>> for InstanceRef<'_, T> {
    fn as_ref(&self) -> &Instance<T> {
        &self.instance
    }
}

impl<T: Component> AsRef<T> for InstanceRef<'_, T> {
    fn as_ref(&self) -> &T {
        self.data
    }
}

impl<T: Component> fmt::Debug for InstanceRef<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.instance())
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct InstanceMut<T: Component> {
    instance: Instance<T>,
    data: &'static mut T,
}

impl<'a, T: Component> InstanceMutReadOnlyItem<'a, T> {
    pub fn entity(&self) -> Entity {
        self.instance.entity()
    }

    pub fn instance(&self) -> Instance<T> {
        self.instance
    }
}

impl<T: Component> From<InstanceMutReadOnlyItem<'_, T>> for Instance<T> {
    fn from(item: InstanceMutReadOnlyItem<T>) -> Self {
        item.instance()
    }
}

impl<T: Component> From<&InstanceMutReadOnlyItem<'_, T>> for Instance<T> {
    fn from(item: &InstanceMutReadOnlyItem<T>) -> Self {
        item.instance()
    }
}

impl<T: Component> PartialEq for InstanceMutReadOnlyItem<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.instance == other.instance
    }
}

impl<T: Component> Eq for InstanceMutReadOnlyItem<'_, T> {}

impl<T: Component> Deref for InstanceMutReadOnlyItem<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T: Component> fmt::Debug for InstanceMutReadOnlyItem<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.instance())
    }
}

impl<'a, T: Component> InstanceMutItem<'a, T> {
    pub fn from_entity(world: &'a mut World, entity: Entity) -> Option<Self> {
        // TODO: Why can't I just pass `EntityWorldMut<'a>` here?
        world.get_mut(entity).map(|data| Self {
            data,
            // SAFE: Kind is validated by `entity.get()` above.
            instance: unsafe { Instance::from_entity_unchecked(entity) },
        })
    }

    pub fn entity(&self) -> Entity {
        self.instance.entity()
    }

    pub fn instance(&self) -> Instance<T> {
        self.instance
    }
}

impl<T: Component> From<InstanceMutItem<'_, T>> for Instance<T> {
    fn from(item: InstanceMutItem<T>) -> Self {
        item.instance
    }
}

impl<T: Component> From<&InstanceMutItem<'_, T>> for Instance<T> {
    fn from(item: &InstanceMutItem<T>) -> Self {
        item.instance
    }
}

impl<T: Component> PartialEq for InstanceMutItem<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.instance == other.instance
    }
}
impl<T: Component> Eq for InstanceMutItem<'_, T> {}

impl<T: Component> Deref for InstanceMutItem<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data.as_ref()
    }
}

impl<T: Component> DerefMut for InstanceMutItem<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data.as_mut()
    }
}

impl<T: Component> AsRef<Instance<T>> for InstanceMutItem<'_, T> {
    fn as_ref(&self) -> &Instance<T> {
        &self.instance
    }
}

impl<T: Component> AsRef<T> for InstanceMutItem<'_, T> {
    fn as_ref(&self) -> &T {
        self.data.as_ref()
    }
}

impl<T: Component> AsMut<T> for InstanceMutItem<'_, T> {
    fn as_mut(&mut self) -> &mut T {
        self.data.as_mut()
    }
}

impl<T: Component> fmt::Debug for InstanceMutItem<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.instance())
    }
}

pub type WithKind<T> = <T as Kind>::Filter;

pub trait GetInstanceCommands<T: Kind> {
    fn instance(&mut self, _: impl Into<Instance<T>>) -> InstanceCommands<'_, T>;
}

impl<T: Kind> GetInstanceCommands<T> for Commands<'_, '_> {
    fn instance(&mut self, instance: impl Into<Instance<T>>) -> InstanceCommands<'_, T> {
        let instance: Instance<T> = instance.into();
        InstanceCommands(self.entity(instance.entity()), PhantomData)
    }
}

pub struct InstanceCommands<'a, T: Kind>(EntityCommands<'a>, PhantomData<T>);

impl<'a, T: Kind> InstanceCommands<'a, T> {
    /// # Safety
    /// Assumes `entity` is a valid instance of kind `T`.
    pub unsafe fn from_entity_unchecked(entity: EntityCommands<'a>) -> Self {
        Self(entity, PhantomData)
    }

    pub fn instance(&self) -> Instance<T> {
        // SAFE: `self.entity()` must be a valid instance of kind `T`.
        unsafe { Instance::from_entity_unchecked(self.entity()) }
    }

    pub fn entity(&self) -> Entity {
        self.0.id()
    }

    pub fn as_entity(&mut self) -> &mut EntityCommands<'a> {
        &mut self.0
    }
}

impl<'a, T: Kind> From<InstanceCommands<'a, T>> for Instance<T> {
    fn from(commands: InstanceCommands<'a, T>) -> Self {
        commands.instance()
    }
}

impl<'a, T: Kind> From<&InstanceCommands<'a, T>> for Instance<T> {
    fn from(commands: &InstanceCommands<'a, T>) -> Self {
        commands.instance()
    }
}

impl<'a, T: Kind> Deref for InstanceCommands<'a, T> {
    type Target = EntityCommands<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, T: Kind> DerefMut for InstanceCommands<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub trait KindBundle: Bundle {
    type Kind: Kind;
}

impl<T: Component> KindBundle for T {
    type Kind = T;
}

pub trait SpawnInstance {
    fn spawn_instance<T: KindBundle>(&mut self, _: T) -> InstanceCommands<'_, T::Kind>;
}

impl SpawnInstance for Commands<'_, '_> {
    fn spawn_instance<T: KindBundle>(&mut self, bundle: T) -> InstanceCommands<'_, T::Kind> {
        let entity = self.spawn(bundle).id();
        // SAFE: `entity` must be a valid instance of `T::Kind`.
        unsafe { InstanceCommands::from_entity_unchecked(self.entity(entity)) }
    }
}

pub trait SpawnInstanceWorld {
    fn spawn_instance<T: Component>(&mut self, instance: T) -> InstanceMutItem<'_, T>;
}

impl SpawnInstanceWorld for World {
    fn spawn_instance<T: Component>(&mut self, instance: T) -> InstanceMutItem<'_, T> {
        let entity = self.spawn(instance).id();
        // SAFE: `entity` must be a valid instance of kind `T`.
        InstanceMutItem::from_entity(self, entity).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    #[derive(Component)]
    struct Foo;

    #[derive(Component)]
    struct Bar;

    fn count<T: Kind>(query: Query<Instance<T>>) -> usize {
        query.iter().count()
    }

    #[test]
    fn kind_with() {
        let mut world = World::new();
        world.spawn(Foo);
        assert_eq!(world.run_system_once(count::<Foo>), 1);
    }

    #[test]
    fn kind_without() {
        struct NotFoo;

        impl Kind for NotFoo {
            type Filter = Without<Foo>;
        }

        let mut world = World::new();
        world.spawn(Foo);
        assert_eq!(world.run_system_once(count::<NotFoo>), 0);
    }

    #[test]
    fn kind_multi() {
        let mut world = World::new();
        world.spawn((Foo, Bar));
        assert_eq!(world.run_system_once(count::<Foo>), 1);
        assert_eq!(world.run_system_once(count::<Bar>), 1);
    }

    #[test]
    fn kind_cast() {
        impl CastInto<Bar> for Foo {
            fn cast_into(instance: Instance<Self>) -> Instance<Bar> {
                // SAFE: `Foo` is convertible to `FooBase`.
                unsafe { Instance::from_entity_unchecked(instance.entity()) }
            }
        }

        let any = Instance::<Any>::PLACEHOLDER;
        let foo = Instance::<Foo>::PLACEHOLDER;
        let bar = foo.cast_into::<Bar>();
        assert!(foo.cast_into::<Any>() == any);
        assert!(bar.cast_into::<Any>() == any);
        // assert!(any.cast_into::<Foo>() == foo); // <-- Must not compile!
        // assert!(bar.cast_into::<Foo>() == foo); // <-- Must not compile!
        assert!(bar.entity() == foo.entity());
    }
}
