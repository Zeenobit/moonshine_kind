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
        bevy_utils::get_short_name(std::any::type_name::<Self>())
    }
}

impl<T: Component> Kind for T {
    type Filter = With<T>;
}

/// Represents the kind of any [`Entity`].
///
/// See [`Instance<Any>`] for more information on usage.
#[derive(Default, Clone, Copy)]
pub struct Any;

impl Kind for Any {
    type Filter = ();
}

/// Represents an [`Entity`] of [`Kind`] `T`.
///
/// `Instance<Any>` is functionally equivalent to an entity.
///
/// # Usage
/// An `Instance<T>` can be used to access entities in a "kind-safe" manner to improve safety and readability.
///
/// This type is designed to behave exactly like an [`Entity`].
///
/// This means you may use it as a [`Query`] parameter, pass it to [`Commands`] to access [`InstanceCommands<T>`],
/// or store it as a type-safe reference to an [`Entity`].
///
/// Note that an `Instance<T>` has `'static` lifetime and does not contain any [`Component`] data.
/// It *only* contains type information.
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
///
/// # bevy_ecs::system::assert_is_system(collect_fruits);
/// ```
#[derive(Reflect)]
pub struct Instance<T: Kind>(Entity, #[reflect(ignore)] PhantomData<T>);

impl<T: Kind> Instance<T> {
    pub const PLACEHOLDER: Self = Self(Entity::PLACEHOLDER, PhantomData);

    /// Creates a new instance of kind `T` from some [`Entity`].
    ///
    /// # Usage
    /// This function is useful when you **know** an `Entity` is of a specific kind and you
    /// need an `Instance<T>` with no way to validate it.
    ///
    /// # Safety
    /// Assumes `entity` is a valid instance of kind `T`.
    ///
    /// # Example
    /// ```
    /// # use bevy::prelude::*;
    /// # use moonshine_kind::prelude::*;
    ///
    /// #[derive(Component)]
    /// struct Apple;
    ///
    /// fn init_apple(entity: Entity, commands: &mut Commands) -> Instance<Apple> {
    ///     commands.entity(entity).insert(Apple);
    ///     // SAFE: `entity` will be a valid instance of `Apple`.
    ///     unsafe { Instance::from_entity_unchecked(entity) }
    /// }
    /// ```
    pub unsafe fn from_entity_unchecked(entity: Entity) -> Self {
        Self(entity, PhantomData)
    }

    /// Returns the [`Entity`] of this instance.
    pub fn entity(&self) -> Entity {
        self.0
    }

    /// Converts this instance into an instance of another kind [`Kind`] `U`.
    ///
    /// # Usage
    /// A kind `T` is safety convertible to another kind `U` if `T` implements [`CastInto<U>`].
    ///
    /// # Example
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
    /// // `Apple` is a kind of `Fruit`.
    /// safe_cast!(Apple => Fruit);
    ///
    /// fn apple_as_fruit(apple: Instance<Apple>) -> Instance<Fruit> {
    ///     apple.cast_into() // This is safe because we said so!
    /// }
    /// ```
    pub fn cast_into<U: Kind>(self) -> Instance<U>
    where
        T: CastInto<U>,
    {
        T::cast_into(self)
    }

    /// Converts this instance into an instance of another kind [`Kind`] `U` without any validation.
    ///
    /// # Usage
    /// This function is useful when you **know** an `Instance<T>` is convertible to a specific type and you
    /// need an `Instance<U>` with no way to validate it.
    ///
    /// Always prefer to explicitly declare safe casts using [`safe_cast`] and use [`Instance::cast_into`] instead of this.
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

/// A trait which allows safe casting from one [`Kind`] to another.
///
/// # Usage
/// It is recommended to use the [`safe_cast`] macro to implement this trait.
///
/// # Example
///
/// The expression `safe_cast!(Apple => Fruit)` is equivalent to:
/// ```
/// # use bevy::prelude::*;
/// # use moonshine_kind::{CastInto, prelude::*};
/// # struct Fruit;
/// # impl Kind for Fruit {
/// #    type Filter = With<Apple>;
/// # }
/// # #[derive(Component)]
/// # struct Apple;
///
/// impl CastInto<Fruit> for Apple {
///     fn cast_into(instance: Instance<Apple>) -> Instance<Fruit> {
///         unsafe { instance.cast_into_unchecked() }
///     }
/// }
/// ```
pub trait CastInto<T: Kind>: Kind {
    fn cast_into(instance: Instance<Self>) -> Instance<T>;
}

impl<T: Kind> CastInto<Any> for T {
    fn cast_into(instance: Instance<Self>) -> Instance<Any> {
        // SAFE: `T` is convertible to `Any`.
        unsafe { Instance::from_entity_unchecked(instance.entity()) }
    }
}

/// A macro to safely implement [`CastInto`] for a pair of related [`Kind`]s.
///
/// See [`CastInto`] for more information.
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

/// A [`QueryData`] item which represents a reference to an [`Instance<T>`] and its associated [`Component`].
///
/// # Usage
/// If a [`Kind`] is also a component, it is often convenient to access the instance and component data together.
/// This type is designed to make these queries more ergonomic.
///
/// You may use this type as either a [`Query`] parameter, or access it from an [`EntityRef`].
///
/// # Example
/// ```
/// # use bevy::prelude::*;
/// # use moonshine_kind::prelude::*;
///
/// #[derive(Component)]
/// struct Apple {
///     freshness: f32,
/// }
///
/// impl Apple {
///     fn is_fresh(&self) -> bool {
///         self.freshness >= 0.5
///     }
/// }
///
/// // Query Access:
/// fn fresh_apples(query: Query<InstanceRef<Apple>>) -> Vec<Instance<Apple>> {
///     query.iter()
///         .filter_map(|apple| apple.is_fresh().then_some(apple.instance()))
///         .collect()
/// }
///
/// // Entity Access:
/// fn fresh_apples_world<'a>(world: &'a World) -> Vec<InstanceRef<'a, Apple>> {
///    world.iter_entities()
///         .filter_map(|entity| InstanceRef::from_entity(entity))
///         .collect()
/// }
///
/// # bevy_ecs::system::assert_is_system(fresh_apples);
/// ```
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
    /// Creates a new [`InstanceRef<T>`] from an [`EntityRef`] if it contains a given [`Component`] of type `T`.
    pub fn from_entity(entity: EntityRef<'a>) -> Option<Self> {
        Some(Self {
            data: entity.get()?,
            // SAFE: Kind is validated by `entity.get()` above.
            instance: unsafe { Instance::from_entity_unchecked(entity.id()) },
        })
    }

    /// Returns the associated [`Entity`].
    pub fn entity(&self) -> Entity {
        self.instance.entity()
    }

    /// Returns the associated [`Instance<T>`].
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

/// A [`QueryData`] item which represents a mutable reference to an [`Instance<T>`] and its associated [`Component`].
///
/// # Usage
/// This type behaves similar like [`InstanceRef<T>`] but allows mutable access to its associated [`Component`].
///
/// The main difference is that you cannot create an [`InstanceMut<T>`] from an [`EntityMut`].
/// See [`InstanceMutItem::from_entity`] for more details.
///
/// See [`InstanceRef<T>`] for more information and examples.
#[derive(QueryData)]
#[query_data(mutable)]
pub struct InstanceMut<T: Component> {
    instance: Instance<T>,
    data: &'static mut T,
}

impl<'a, T: Component> InstanceMutReadOnlyItem<'a, T> {
    /// Returns the associated [`Entity`].
    pub fn entity(&self) -> Entity {
        self.instance.entity()
    }

    /// Returns the associated [`Instance<T>`].
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
    /// Creates a new [`InstanceMutItem<T>`] from an [`EntityMut`] if it contains a given [`Component`] of type `T`.
    pub fn from_entity(world: &'a mut World, entity: Entity) -> Option<Self> {
        // TODO: Why can't I just pass `EntityWorldMut<'a>` here?
        world.get_mut(entity).map(|data| Self {
            data,
            // SAFE: Kind is validated by `entity.get()` above.
            instance: unsafe { Instance::from_entity_unchecked(entity) },
        })
    }

    /// Returns the associated [`Entity`].
    pub fn entity(&self) -> Entity {
        self.instance.entity()
    }

    /// Returns the associated [`Instance<T>`].
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
/// fn count_apples(query: Query<(), WithKind<Apple>>) -> usize {
///     query.iter().count()
/// }
///
/// # bevy_ecs::system::assert_is_system(count_apples);
/// ```
pub type WithKind<T> = <T as Kind>::Filter;

/// Extension trait to access [`InstanceCommands<T>`] from [`Commands`].
///
/// See [`InstanceCommands`] for more information.
pub trait GetInstanceCommands<T: Kind> {
    /// Returns the [`InstanceCommands<T>`] for an [`Instance<T>`].
    fn instance(&mut self, instance: Instance<T>) -> InstanceCommands<'_, T>;
}

impl<T: Kind> GetInstanceCommands<T> for Commands<'_, '_> {
    fn instance(&mut self, instance: Instance<T>) -> InstanceCommands<'_, T> {
        InstanceCommands(self.entity(instance.entity()), PhantomData)
    }
}

/// [`EntityCommands`] with kind semantics.
///
/// # Usage
/// On its own, this type is not very useful. Instead, it is designed to be extended using traits.
/// This allows you to design commands for a specific kind of an entity in a type-safe manner.
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
/// struct Eat;
///
/// trait EatApple {
///     fn eat(&mut self);
/// }
///
/// impl EatApple for InstanceCommands<'_, Apple> {
///     fn eat(&mut self) {
///         info!("Crunch!");
///         self.despawn();
///     }
/// }
///
/// fn eat_apples(apples: Query<Instance<Apple>, With<Eat>>, mut commands: Commands) {
///     for apple in apples.iter() {
///         commands.instance(apple).eat();
///     }
/// }
///
/// # bevy_ecs::system::assert_is_system(eat_apples);
pub struct InstanceCommands<'a, T: Kind>(EntityCommands<'a>, PhantomData<T>);

impl<'a, T: Kind> InstanceCommands<'a, T> {
    /// Creates a new [`InstanceCommands<T>`] from [`EntityCommands`] without any validation.
    ///
    /// # Safety
    /// Assumes `entity` is a valid instance of kind `T`.
    pub unsafe fn from_entity_unchecked(entity: EntityCommands<'a>) -> Self {
        Self(entity, PhantomData)
    }

    /// Returns the associated [`Instance<T>`].
    pub fn instance(&self) -> Instance<T> {
        // SAFE: `self.entity()` must be a valid instance of kind `T`.
        unsafe { Instance::from_entity_unchecked(self.entity()) }
    }

    /// Returns the associated [`Entity`].
    pub fn entity(&self) -> Entity {
        self.0.id()
    }

    /// Returns the associated [`EntityCommands`].
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

impl<T: Component> KindBundle for T {
    type Kind = T;
}

/// Extension trait to safely spawn an [`Instance<T>`] using [`Commands`] where `T` is also a [`KindBundle`].
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
    fn spawn_instance<T: KindBundle>(&mut self, _: T) -> InstanceCommands<'_, T::Kind>;
}

impl SpawnInstance for Commands<'_, '_> {
    fn spawn_instance<T: KindBundle>(&mut self, bundle: T) -> InstanceCommands<'_, T::Kind> {
        let entity = self.spawn(bundle).id();
        // SAFE: `entity` must be a valid instance of `T::Kind`.
        unsafe { InstanceCommands::from_entity_unchecked(self.entity(entity)) }
    }
}

/// Extension trait to safely spawn an [`Instance<T>`] using [`World`] where `T` is also a [`KindBundle`].
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
