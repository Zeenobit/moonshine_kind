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

// Represents a Human. Humans can eat Fruits.
#[derive(Component)]
struct Human;

// Extension trait to allow all instances of Human to eat an instance of a Fruit.
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
        .add_systems(Update, eat.run_if(should_eat))
        .add_systems(PostUpdate, human_eat);
    app.run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Human);
    commands.spawn_batch([Apple, Apple]);
    commands.spawn_batch([Orange, Orange, Orange]);
}

fn should_eat(input: Res<ButtonInput<KeyCode>>) -> bool {
    input.just_pressed(KeyCode::Space)
}

fn eat(human: Query<Instance<Human>>, fruits: Query<Instance<Fruit>>, mut commands: Commands) {
    let human = human.single();
    if let Some(fruit) = fruits.iter().next() {
        commands.instance(human).eat(fruit);
    } else {
        println!("No fruit to eat");
    }
}

fn human_eat(
    human: Query<(Instance<Human>, &Eat)>,
    apple: Query<Instance<Apple>>,
    orange: Query<Instance<Orange>>,
    mut commands: Commands,
) {
    for (human, Eat(fruit)) in human.iter() {
        if let Ok(apple) = apple.get(fruit.entity()) {
            println!("{human:?} ate a crunchy {apple:?}");
        } else if let Ok(orange) = orange.get(fruit.entity()) {
            println!("{human:?} ate a juicy {orange:?}");
        } else {
            println!("{human:?} ate a mysterious {fruit:?}");
        }
        commands.instance(*fruit).despawn();
        commands.instance(human).remove::<Eat>();
    }
}
