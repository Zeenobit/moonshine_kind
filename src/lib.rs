use std::{
    any::TypeId,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use bevy_ecs::{
    archetype::{Archetype, ArchetypeComponentId},
    component::{ComponentId, Tick},
    entity::{EntityMapper, MapEntities},
    prelude::*,
    query::{Access, FilteredAccess, ReadOnlyWorldQuery, WorldQuery},
    storage::{Table, TableRow},
    system::EntityCommands,
    world::unsafe_world_cell::UnsafeWorldCell,
};
use bevy_reflect::Reflect;

pub mod prelude {
    pub use super::{
        GetInstanceCommands, Instance, InstanceCommands, InstanceMut, InstanceRef, Kind,
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
    type Filter: ReadOnlyWorldQuery;

    /// Returns `true` if this kind is safely convertible to `U`.
    ///
    /// By default, this kind is convertible to `U` if they are the same type or `U` is [`Any`].
    #[must_use]
    fn is_convertible_to<U: Kind>() -> bool {
        TypeId::of::<U>() == TypeId::of::<Self>() || TypeId::of::<U>() == TypeId::of::<Any>()
    }

    /// Returns the debug name of this kind.
    ///
    /// By default, this is the short type name (without path) of this kind.
    #[must_use]
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
    pub fn cast_into<U: Kind>(self) -> Result<Instance<U>, CastError<T, U>> {
        if T::is_convertible_to::<U>() {
            // SAFE: `T` must be convertible to `U`.
            Ok(unsafe { self.cast_into_unchecked() })
        } else {
            Err(CastError::new())
        }
    }

    /// Converts this instance into an instance of kind `U` without checking if `T` is convertible.
    ///
    /// # Safety
    /// Assumes this instance is also a valid `Instance<U>`.
    pub unsafe fn cast_into_unchecked<U: Kind>(self) -> Instance<U> {
        Instance::from_entity_unchecked(self.entity())
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

    type ReadOnly = Self;

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

    const IS_ARCHETYPAL: bool = <T::Filter as WorldQuery>::IS_ARCHETYPAL;

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

    fn update_archetype_component_access(
        _state: &Self::State,
        _archetype: &Archetype,
        _access: &mut Access<ArchetypeComponentId>,
    ) {
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

unsafe impl<T: Kind> ReadOnlyWorldQuery for Instance<T> {}

impl<T: Kind> MapEntities for Instance<T> {
    fn map_entities(&mut self, entity_mapper: &mut EntityMapper) {
        self.0 = entity_mapper.get_or_reserve(self.0);
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

pub struct CastError<T, U>(PhantomData<(T, U)>);

impl<T, U> CastError<T, U> {
    #[must_use]
    fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Kind, U: Kind> fmt::Debug for CastError<T, U> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CastError")
            .field(&T::debug_name())
            .field(&U::debug_name())
            .finish()
    }
}

#[derive(WorldQuery)]
pub struct InstanceRef<T: Kind + Component> {
    instance: Instance<T>,
    data: &'static T,
}

impl<T: Kind + Component> InstanceRefItem<'_, T> {
    #[must_use]
    pub fn entity(&self) -> Entity {
        self.instance.entity()
    }

    #[must_use]
    pub fn instance(&self) -> Instance<T> {
        self.instance
    }
}

impl<T: Kind + Component> From<InstanceRefItem<'_, T>> for Instance<T> {
    fn from(item: InstanceRefItem<T>) -> Self {
        item.instance()
    }
}

impl<T: Kind + Component> From<&InstanceRefItem<'_, T>> for Instance<T> {
    fn from(item: &InstanceRefItem<T>) -> Self {
        item.instance()
    }
}

impl<T: Kind + Component> Deref for InstanceRefItem<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T: Kind + Component> fmt::Debug for InstanceRefItem<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.instance())
    }
}

#[derive(WorldQuery)]
#[world_query(mutable)]
pub struct InstanceMut<T: Kind + Component> {
    instance: Instance<T>,
    data: &'static mut T,
}

impl<T: Kind + Component> InstanceMutReadOnlyItem<'_, T> {
    #[must_use]
    pub fn entity(&self) -> Entity {
        self.instance.entity()
    }

    #[must_use]
    pub fn instance(&self) -> Instance<T> {
        self.instance
    }
}

impl<T: Kind + Component> From<InstanceMutReadOnlyItem<'_, T>> for Instance<T> {
    fn from(item: InstanceMutReadOnlyItem<T>) -> Self {
        item.instance()
    }
}

impl<T: Kind + Component> From<&InstanceMutReadOnlyItem<'_, T>> for Instance<T> {
    fn from(item: &InstanceMutReadOnlyItem<T>) -> Self {
        item.instance()
    }
}

impl<T: Kind + Component> Deref for InstanceMutReadOnlyItem<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T: Kind + Component> fmt::Debug for InstanceMutReadOnlyItem<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.instance())
    }
}

impl<T: Kind + Component> InstanceMutItem<'_, T> {
    #[must_use]
    pub fn entity(&self) -> Entity {
        self.instance.entity()
    }

    #[must_use]
    pub fn instance(&self) -> Instance<T> {
        self.instance
    }
}

impl<T: Kind + Component> From<InstanceMutItem<'_, T>> for Instance<T> {
    fn from(item: InstanceMutItem<T>) -> Self {
        item.instance
    }
}

impl<T: Kind + Component> From<&InstanceMutItem<'_, T>> for Instance<T> {
    fn from(item: &InstanceMutItem<T>) -> Self {
        item.instance
    }
}

impl<T: Kind + Component> Deref for InstanceMutItem<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data.as_ref()
    }
}

impl<T: Kind + Component> DerefMut for InstanceMutItem<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data.as_mut()
    }
}

impl<T: Kind + Component> fmt::Debug for InstanceMutItem<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.instance())
    }
}

pub trait GetInstanceCommands<'w, 's, T: Kind> {
    fn instance(&mut self, _: impl Into<Instance<T>>) -> InstanceCommands<'w, 's, '_, T>;
}

impl<'w, 's, T: Kind> GetInstanceCommands<'w, 's, T> for Commands<'w, 's> {
    fn instance(&mut self, instance: impl Into<Instance<T>>) -> InstanceCommands<'w, 's, '_, T> {
        let instance: Instance<T> = instance.into();
        InstanceCommands(self.entity(instance.entity()), PhantomData)
    }
}

pub struct InstanceCommands<'w, 's, 'a, T: Kind>(EntityCommands<'w, 's, 'a>, PhantomData<T>);

impl<'w, 's, 'a, T: Kind> InstanceCommands<'w, 's, 'a, T> {
    /// # Safety
    /// Assumes `entity` is a valid instance of kind `T`.
    #[must_use]
    pub unsafe fn from_entity_unchecked(entity: EntityCommands<'w, 's, 'a>) -> Self {
        Self(entity, PhantomData)
    }

    #[must_use]
    pub fn instance(&self) -> Instance<T> {
        // SAFE: `self.entity()` must be a valid instance of kind `T`.
        unsafe { Instance::from_entity_unchecked(self.entity()) }
    }

    #[must_use]
    pub fn entity(&self) -> Entity {
        self.0.id()
    }

    #[must_use]
    pub fn as_entity(&mut self) -> &mut EntityCommands<'w, 's, 'a> {
        &mut self.0
    }
}

impl<'w, 's, 'a, T: Kind> Deref for InstanceCommands<'w, 's, 'a, T> {
    type Target = EntityCommands<'w, 's, 'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'w, 's, 'a, T: Kind> DerefMut for InstanceCommands<'w, 's, 'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub trait SpawnInstance<'w, 's> {
    fn spawn_instance<T: Kind + Component>(
        &mut self,
        instance: T,
    ) -> InstanceCommands<'w, 's, '_, T>;
}

impl<'w, 's> SpawnInstance<'w, 's> for Commands<'w, 's> {
    fn spawn_instance<T: Kind + Component>(
        &mut self,
        instance: T,
    ) -> InstanceCommands<'w, 's, '_, T> {
        let entity = self.spawn(instance).id();
        // SAFE: `entity` must be a valid instance of kind `T`.
        unsafe { InstanceCommands::from_entity_unchecked(self.entity(entity)) }
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
        assert!(Any::is_convertible_to::<Any>());
        assert!(Foo::is_convertible_to::<Foo>());
        assert!(Foo::is_convertible_to::<Any>());
        assert!(!Foo::is_convertible_to::<Bar>());
        assert!(!Any::is_convertible_to::<Foo>());
    }
}
