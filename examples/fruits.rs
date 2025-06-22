use bevy::prelude::*;
use moonshine_kind::prelude::*;

// Represents an Apple. Apples are crunchy!
#[derive(Component)]
struct Apple;

// Represents an Orange. Oranges are juicy!
#[derive(Component)]
struct Orange;

// All Apples and Oranges are Fruits.
struct Fruit;

impl Kind for Fruit {
    type Filter = Or<(With<Apple>, With<Orange>)>;
}

// Define safe casts between related kinds explicitly.
// You only need to define these if you intend to cast between these kinds.
impl KindOf<Fruit> for Apple {}
impl KindOf<Fruit> for Orange {}

// Represents a Human. Humans can eat Fruits.
#[derive(Component)]
struct Human;

// Extension trait to allow all Humans to eat Fruits.
// Typically, this isn't required. It is mainly added here to demonstrate the usage of `InstanceCommands`.
trait EatFruit {
    fn eat(self, fruit: Instance<Fruit>) -> Self;
}

impl EatFruit for &mut InstanceCommands<'_, Human> {
    fn eat(self, fruit: Instance<Fruit>) -> Self {
        self.insert(Eat(fruit));
        self
    }
}

// A components which signals `human_eat` to actually eat the fruit.
#[derive(Component)]
struct Eat(Instance<Fruit>);

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, try_eat_fruit.run_if(should_eat))
        .add_systems(PostUpdate, human_eat);
    app.run();
}

// Spawn some stuff.
fn setup(mut commands: Commands) {
    commands.spawn(Human);
    commands.spawn_batch([Apple, Apple]);
    commands.spawn_batch([Orange, Orange, Orange]);
}

// Press Space to eat.
fn should_eat(input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::Space)
}

// Try to eat a fruit if there is one.
// Because of kind semantics, it is (somewhat) guaranteed that only humans can eat fruits.
fn try_eat_fruit(
    human: Single<Instance<Human>>,
    fruits: Query<Instance<Fruit>>,
    mut commands: Commands,
) {
    if let Some(fruit) = fruits.iter().next() {
        commands.instance(*human).eat(fruit);
    } else {
        println!("No fruit to eat");
    }
}

// Eat the fruit!
// This system demonstrates how to use queries to "downcast" a Fruit into an Apple or an Orange.
fn human_eat(
    human: Query<(Instance<Human>, &Eat)>,
    apple: Query<Instance<Apple>>,
    orange: Query<Instance<Orange>>,
    mut commands: Commands,
) {
    for (human, Eat(fruit)) in human.iter() {
        if let Ok(apple) = apple.get(fruit.entity()) {
            // Because `Apple` is a `KindOf<Fruit>`, all apples can be safely cast into fruits:
            human_likes_fruit(human, apple.cast_into());

            println!("{human:?} ate a crunchy {apple:?}.");
        } else if let Ok(orange) = orange.get(fruit.entity()) {
            println!("{human:?} ate a juicy {orange:?}.");
        } else {
            println!("{human:?} ate a mysterious {fruit:?}.");
        }

        commands.instance(*fruit).despawn();
        commands.instance(human).remove::<Eat>();
    }
}

// Use kind semantics to define safer and more readable code.
fn human_likes_fruit(human: Instance<Human>, fruit: Instance<Fruit>) {
    println!("{human:?} likes {fruit:?}!");
}
