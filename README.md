# 🍎 Moonshine Kind

[![crates.io](https://img.shields.io/crates/v/moonshine-kind)](https://crates.io/crates/moonshine-kind)
[![downloads](https://img.shields.io/crates/dr/moonshine-kind?label=downloads)](https://crates.io/crates/moonshine-kind)
[![docs.rs](https://docs.rs/moonshine-kind/badge.svg)](https://docs.rs/moonshine-kind)
[![license](https://img.shields.io/crates/l/moonshine-kind)](https://github.com/Zeenobit/moonshine_kind/blob/main/LICENSE)
[![stars](https://img.shields.io/github/stars/Zeenobit/moonshine_kind)](https://github.com/Zeenobit/moonshine_kind)

Simple type safety solution for [Bevy](https://github.com/bevyengine/bevy).

## Overview

An [`Entity`] is a generic way to reference entities within Bevy:

```rust
use bevy::prelude::*;

#[derive(Component)]
struct FruitBasket {
    fruits: Vec<Entity>
}
```
A problem with using entities in this way is the lack of information about the "kind" of the entity. This results in code that is error prone, hard to debug, and read.

This crate attempts to solve this problem by introducing a new [`Instance<T>`] type which behaves like an [`Entity`] but also contains information about the "kind" of the entity:

```rust
use bevy::prelude::*;
use moonshine_kind::prelude::*;

#[derive(Component)]
struct Fruit;

#[derive(Component)]
struct FruitBasket {
    fruits: Vec<Instance<Fruit>>
}
```

### Features

- Improved type safety and readability for Bevy code
- Ability to define custom entity kinds
- Ability to define commands for specific entity kinds
- Zero or minimal boilerplate

## Usage

### `Kind` and `Instance`

By definition, an [`Entity`] is of [`Kind`] `T` if it matches [`Query<(), <T as Kind>::Filter>`][`Query`].

Any [`Component`] automatically implements the [`Kind`] trait:
```rust,ignore
impl<T: Component> Kind for T {
    type Filter = With<T>;
}
```

This means you may use any [`Component`] as an argument to [`Instance<T>`]:
```rust
use bevy::prelude::*;
use moonshine_kind::prelude::*;

#[derive(Component)]
struct Apple;

fn count_apples(apples: Query<Instance<Apple>>) {
    println!("Apples: {}", apples.iter().count());
}
```

Alternatively, you may also define your own kind by implementing the [`Kind`] trait:
```rust
use bevy::prelude::*;
use moonshine_kind::prelude::*;

#[derive(Component)]
struct Apple;

#[derive(Component)]
struct Orange;

struct Fruit;

impl Kind for Fruit {
    type Filter = Or<(With<Apple>, With<Orange>)>;
}

fn count_fruits(fruits: Query<Instance<Fruit>>) {
    println!("Fruits: {}", fruits.iter().count());
}
```

### `InstanceRef` and `InstanceMut`

If a [`Kind`] is also a [`Component`], you may use [`InstanceRef<T>`] and [`InstanceMut<T>`] to access [`Instance<T>`] and the associated component data with a single query term:
```rust
use bevy::prelude::*;
use moonshine_kind::prelude::*;

#[derive(Component)]
struct Apple {
    freshness: f32
}

impl Apple {
    fn is_fresh(&self) -> bool {
        self.freshness >= 1.0
    }
}

fn fresh_apples(
    apples: Query<InstanceRef<Apple>>
) -> Vec<Instance<Apple>> {
    let mut fresh_apples = Vec::new();
    for apple in apples.iter() {
        if apple.is_fresh() {
            fresh_apples.push(apple.instance());
        }
    }
    fresh_apples
}
```
### `InstanceCommands`
You may also extend [`InstanceCommands<T>`] to define [`Commands`] specific to a [`Kind`].

[`InstanceCommands<T>`] behaves like [`EntityCommands`], and is accessible via `commands.instance(...)` (see [`GetInstanceCommands<T>`] for details):
```rust
use bevy::prelude::*;
use moonshine_kind::prelude::*;

struct Fruit;

impl Kind for Fruit {
    type Filter = (/* ... */);
}

#[derive(Component)]
struct Human;

trait Eat {
    fn eat(&mut self, fruit: Instance<Fruit>);
}

// Humans can eat:
impl Eat for InstanceCommands<'_, Human> {
    fn eat(&mut self, fruit: Instance<Fruit>) {
        // ...
    }
}

fn eat(
    human: Query<Instance<Human>>,
    fruits: Query<Instance<Fruit>>, mut commands: Commands
) {
    let human = human.single();
    if let Some(fruit) = fruits.iter().next() {
        commands.instance(human).eat(fruit);
    }
}
```

### `Instance<Any>`

When writing generic code, it may be desirable to have an instance that can be of [`Any`] kind:
```rust
use moonshine_kind::{prelude::*, Any};

struct Container<T: Kind = Any> {
    items: Vec<Instance<T>>
}
```
[`Instance<Any>`][`Instance<T>`] is functionally equivalent to [`Entity`].

### Casting

An [`Instance<T>`] is safely convertible to an [`Instance<U>`][`Instance<T>`] if [`CastInto<U>`][`CastInto`] is implemented for `T`.

This is done by using the `.cast_into()` method:

You may use the [`safe_cast`] macro to implement this trait for given kind pair:

```rust
use bevy::prelude::*;
use moonshine_kind::prelude::*;

#[derive(Component)]
struct Apple;

struct Fruit;

impl Kind for Fruit {
    type Filter = With<Apple>;
}

// An Apple is a Fruit because we said so:
safe_cast!(Apple => Fruit);

fn init_apple(apple: Instance<Apple>, commands: &mut Commands) {
    init_fruit(apple.cast_into(), commands);
    // ...
}

fn init_fruit(fruit: Instance<Fruit>, commands: &mut Commands) {
    // ...
}
```

## Examples

See [examples/fruits.rs](examples/fruits.rs) for a complete example.

## Limitations

### Instance Invalidation

This crate does not monitor instances for invalidation.

This means that if an entity is modified in such a way that it no longer matches some [`Kind`] `T` (such as removing [`Component`] `T`), any [`Instance<T>`] which references it would be invalid.

It is recommended to avoid using kind semantics for components that may be removed at runtime without despawning their associated entity.

However, if necessary, you may check instances for validity prior to usage:
```rust
use bevy::prelude::*;
use moonshine_kind::prelude::*;

struct Fruit;

impl Kind for Fruit {
    type Filter = (/* ... */);
}

fn prune_fruits(
    mut fruits: Vec<Instance<Fruit>>,
    query: &Query<(), <Fruit as Kind>::Filter>
) -> Vec<Instance<Fruit>> {
    fruits.retain(|fruit| {
        // Is the Fruit still a Fruit?
        query.get(fruit.entity()).is_ok()
    });
    fruits
}
```

## Support

Please [post an issue](https://github.com/Zeenobit/moonshine_kind/issues/new) for any bugs, questions, or suggestions.

You may also contact me on the official [Bevy Discord](https://discord.gg/bevy) server as **@Zeenobit**.

[`Entity`]:https://docs.rs/bevy/latest/bevy/ecs/entity/struct.Entity.html
[`Component`]:https://docs.rs/bevy/latest/bevy/ecs/component/trait.Component.html
[`Query`]:https://docs.rs/bevy/latest/bevy/ecs/system/struct.Query.html
[`Commands`]:https://docs.rs/bevy/latest/bevy/ecs/prelude/struct.Commands.html
[`EntityCommands`]:https://docs.rs/bevy/latest/bevy/ecs/system/struct.EntityCommands.html
[`Kind`]:https://docs.rs/moonshine-kind/0.1.4/moonshine_kind/trait.Kind.html
[`Instance<T>`]:https://docs.rs/moonshine-kind/latest/moonshine_kind/struct.Instance.html
[`InstanceRef<T>`]:https://docs.rs/moonshine-kind/latest/moonshine_kind/struct.InstanceRef.html
[`InstanceMut<T>`]:https://docs.rs/moonshine-kind/latest/moonshine_kind/struct.InstanceMut.html
[`InstanceCommands<T>`]:https://docs.rs/moonshine-kind/latest/moonshine_kind/struct.InstanceCommands.html
[`GetInstanceCommands<T>`]:https://docs.rs/moonshine-kind/latest/moonshine_kind/trait.GetInstanceCommands.html
[`Any`]:https://docs.rs/moonshine-kind/latest/moonshine_kind/struct.Any.html
[`CastInto`]:https://docs.rs/moonshine-kind/latest/moonshine_kind/trait.CastInto.html
[`safe_cast`]:https://docs.rs/moonshine-kind/latest/moonshine_kind/macro.safe_cast.html