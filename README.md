# 🍎 Moonshine Kind

A simple type safety solution for [Bevy](https://github.com/bevyengine/bevy) ECS.

## Overview

An [`Entity`](https://docs.rs/bevy/latest/bevy/ecs/entity/struct.Entity.html) is a generic way to reference entities within Bevy ECS:

```rust
#[derive(Component)]
struct FruitBasket {
    fruits: Vec<Entity>
}
```
A problem with using entities in this way is that there is no information about the "kind" of the entity. This can result in code that is error prone, hard to debug, and hard to read.

This crate attempts to solve the problem by introducing a new type `Instance<T>` which functions like an entity, but also contains information about the "kind" of the entity:

```rust
#[derive(Component)]
struct FruitBasket {
    fruits: Vec<Instance<Fruit>>
}
```

### Features

- Improved type safety and readability for entities
- Custom entity kinds
- Kind-specific commands and queries
- Zero or minimal boilerplate for defining entity kinds

## Usage

### `Kind` and `Instance`

By definition, an entity is of kind `T` if it matches the query `Query<(), <T as Kind>::Filter>`.

By default, all Bevy components automatically implement the `Kind` trait:
```rust
impl<T: Component> Kind for T {
    type Filter = With<T>;
}
```

This means you can use any component as an argument to `Instance<T>`. For example:
```rust
#[derive(Component)]
struct Apple;

#[derive(Component)]
struct Orange;

fn count_apples(apples: Query<Instance<Apple>>) {
    println!("Apples: {}", apples.iter().count());
}
```
Alternatively, you can define your own kinds by implementing the `Kind` trait:
```rust
struct Fruit;

impl Kind for Fruit {
    type Filter = Or<With<Apple>, With<Orange>>;
}

fn count_fruits(fruits: Query<Instance<Fruit>>) {
    println!("Fruits: {}", fruits.iter().count());
}
```

### `InstanceRef` and `InstanceMut`

If a kind is also a component (such as `Apple` or `Orange` in examples above), you may use `InstanceRef<T>` and `InstanceMut<T>` to access the instance and component data together:
```rust
impl Apple {
    fn is_fresh(&self) -> bool {
        ...
    }
}

fn fresh_apples(apples: Query<InstanceRef<Apple>>) -> Vec<Instance<Apple>> {
    let mut fresh_apples = Vec::new();
    for apple in apples.iter() {
        if apple.is_fresh() {
            fresh_apples.push(apple.instance());
        }
    }
    fresh_apples
}
```
### `Instance(Ref)Commands`
You may also extend `InstanceCommands<T>` and `InstanceRefCommands<T>` types to define kind-specific commands.
These behave similar to `Instance<T>` and `InstanceRef<T>`, and are accessible via `GetInstanceCommands` and `GetInstanceRefCommands` traits:
```rust
#[derive(Component)]
struct Human;

trait Eat {
    fn eat(&mut self, fruit: Instance<Fruit>);
}

impl Eat for InstanceCommands<'_, '_, '_, Human> {
    fn eat(&mut self, fruit: Instance<Fruit>) {
        ...
    }
}

fn eat(human: Query<Instance<Human>>, fruits: Query<Instance<Fruit>>, mut commands: Commands) {
    let human = human.single();
    if let Some(fruit) = fruits.iter().next() {
        commands.instance(human).eat(fruit);
        // Also valid:
        // commands.instance_ref(human).eat(fruit);
    }
}
```

⚠️ There is currently no support for `InstanceMutCommands<T>`.

### `Instance<Any>`

When writing generic code, it may be desirable to have an instance that can be of any kind:
```rust
use moonshine_kind::Any;

struct Container<T: Kind = Any> {
    items: Vec<Instance<T>>
}
```
Note that `Instance<Any>` is functionally equivalent to `Entity`.

## Examples

See [examples](examples) for more complete examples.

## Limitations

### Instance Invalidation

This crate does not monitor instances for invalidation. This means that if an entity is modified in such a way that it no longer matches a given kind `T` (such as removing component `T`), all instances which reference it would become invalid.

If necessary, you must manually check instances for validity prior to usage:
```rust
fn prune_fruits(
    In(fruits): In<Vec<Instance<Fruit>>>,
    query: Query<Instance<Fruit>>) -> Vec<Instance<Fruit>> {
    fruits.retain(|fruit| {
        // Is the Fruit still a Fruit?
        query.get(fruit.entity()).is_ok()
    })
}
```
