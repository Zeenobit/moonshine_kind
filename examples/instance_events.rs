use std::marker::PhantomData;

use bevy::prelude::*;
use moonshine_kind::prelude::*;

/// Represents an Apple. Apples are crunchy!
#[derive(Component)]
struct Apple;

/// Represents an Orange. Oranges are juicy!
#[derive(Component)]
struct Orange;

/// Triggered on a fruit when a hungry human gobbles it all up.
/// This event will only fire if the entity it was triggered on
/// contains the provided [`T`] component.
#[derive(Event)]
#[event(trigger=InstanceTrigger<Self, &'static ChildOf, T>)]
struct GobbleGobble<T: Component> {
    phantom: PhantomData<T>,
}
impl<T: Component> Default for GobbleGobble<T> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<T: Component> EventFromEntity for GobbleGobble<T> {}
impl<T: Component> IntoEventFromEntity<Self> for GobbleGobble<T> {
    type Event = Self;
    type Trigger = InstanceTrigger<Self, &'static ChildOf, T>;

    fn into_event_from_entity(self, entity: Entity) -> (Self::Event, Self::Trigger) {
        (self, InstanceTrigger::new(entity, true))
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands
        .spawn(Apple)
        .observe(|_ev: On<GobbleGobble<Apple>>| {
            println!("the grandparent apple was gobbled up");
        })
        .with_children(|parent| {
            parent
                .spawn(Orange)
                .observe(|_ev: On<GobbleGobble<Orange>>| {
                    // the orange will be skipped, it is not a match
                    println!("the parent orange was gobbled up");
                })
                .with_children(|parent| {
                    parent
                        .spawn(Apple)
                        .observe(|_ev: On<GobbleGobble<Apple>>| {
                            println!("the grandchild apple was gobbled up");
                        })
                        .trigger(GobbleGobble::<Apple>::default());
                });
        });
}
