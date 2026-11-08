# Trees a game touches

Hits, shakes, falling leaves, picking fruit, felling, fire, and bodies walking through undergrowth.

## Trees that react: `TreeShakes`

A game hits trees. `TreeShakes` is a resource like `FloraWind`: `shake(TreeShake { at, radius, strength, direction, duration }, &time)` swings every tree whose trunk stands within `radius` metres of `at` (world x and z), the nearest the most, first away from `at` (or along `direction`), then back and forth, dying away over `duration` seconds, while its leaves rustle. About a metre of radius singles out the tree that was hit; tens of metres reach a whole grove. Each tree swings at its own pace, taller trees slower, and the whole tree bends as it does in the wind, on every way a tree is drawn: `ProceduralTree`, `GpuGrowth`, pooled trees and their impostors. Up to eight shakes run at once (`TreeShakes::MAX`; a new one takes the place of the one that ends soonest, so a knock always shows). They live in the wind's shared buffer, so a shake writes a few hundred bytes and touches no material, and while none is under way the shaders compare one number a vertex. A shake is drawn, not simulated: colliders (`TreeShape`) stay where they are.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{TreeShake, TreeShakes};

// An axe on the tree standing at `trunk`, swung along `swing`: a metre of radius singles it out.
fn axe_hit(shakes: &mut TreeShakes, time: &Time, trunk: Vec3, swing: Vec3) {
    let hit = TreeShake { at: trunk.xz(), radius: 1.0, strength: 2.0, direction: swing.xz(), ..default() };
    shakes.shake(hit, time);
}

// A blast: every tree within 30 m swings away from it.
fn blast(shakes: &mut TreeShakes, time: &Time, at: Vec3) {
    shakes.shake(TreeShake { at: at.xz(), radius: 30.0, strength: 3.0, duration: 3.5, ..default() }, time);
}

#[derive(Component)]
struct Player;

// A character pushing through a thicket: a small knock where it is, a few times a second.
fn brush(mut shakes: ResMut<TreeShakes>, time: Res<Time>, player: Single<&Transform, With<Player>>, mut wait: Local<f32>) {
    *wait -= time.delta_secs();
    if *wait <= 0.0 {
        *wait = 0.3;
        let knock = TreeShake { at: player.translation.xz(), radius: 2.0, strength: 0.6, duration: 1.5, ..default() };
        shakes.shake(knock, &time);
    }
}
```

![The grown grove from a still camera: an axe hit swings the oak alone and knocks a few leaves off it, another swings the birch, then a blast swings every tree outward, leaves fall from them all, and they settle back](../media/shake.gif)

*`cargo run --release --example capture_growth -- --shake`: an axe on the oak, one on the birch, then a blast at the grove's middle, in a light wind; leaves fall from every tree hit.*

## Falling leaves: `DropLeaves`

Hit a tree and leaves come off it. Every new shake knocks `FallingLeaves::per_shake` leaves (12 by default) off each tree it reaches, times its strength and fewer toward its radius, and a felled tree sheds `per_landing` (80) as it lands. `DropLeaves { tree, count }` drops leaves from any tree whenever the game wants: a bird lands, an arrow hits, a gust. The leaves are copies of the tree's own cards: each hangs where it grew until its moment (a drop lets go over half a second), falls, picking up speed to between 0.9 and 1.5 m/s, tumbles about an axis of its own and swings from side to side, drifts with `FloraWind`, turns flat as it reaches the ground (`FloraGround`, or the height the tree stands at), lies there for about `FallingLeaves::rest` seconds (6) and shrinks away. They wear the tree's scan, tint and season (a red maple drops red leaves, and a deciduous tree drops none of those it has already shed), cast shadows, and a leaf of crossed planes or a star folds flat when it lands.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{DropLeaves, FallingLeaves, ProceduralTree};

// A bird lands in the oak.
fn bird_lands(mut drops: MessageWriter<DropLeaves>, oak: Single<Entity, With<ProceduralTree>>) {
    drops.write(DropLeaves { tree: *oak, count: 6 });
}

// More leaves to a hit, lying longer.
fn setup(mut leaves: ResMut<FallingLeaves>) {
    leaves.per_shake = 30.0;
    leaves.rest = 15.0;
}
```

Which leaves fall is drawn from where the tree stands and the time, never from its `Entity`: the same tree, hit at the same moment, drops the same leaves however many entities were spawned before it, so a replay, a lockstep game or a world streamed in another order sees the leaves it saw before. The season sheds the trees nearest the camera first, so the leaves it lets go do not depend on the order the trees were spawned in either.

![An autumn maple dropping red leaves: they tumble through the air around its trunk at every angle, the lowest about to reach the grass](../media/falling_leaves.jpg)

*`inspect --mode tree --preset maple --season 1,0.3 --drop 40,2.6`: forty leaves of an autumn maple, 2.6 seconds after they let go.*

In autumn the falling leaves are the tree's own. The shaders stop drawing a deciduous tree's leaf once `FloraSeason::leaf_fall` passes the leaf's moment, so raising it a little at a time (a season passing, as `FloraSeason::of_year` does) thins the crown leaf by leaf; on the trees within `FallingLeaves::autumn_distance` (50 m) of a camera each of those leaves now lets go from where it hung and falls like the others (every tenth of a second, those passed meanwhile, in one burst per look of leaves). A rise of more than 0.1 in one frame, such as setting `FloraSeason::WINTER`, is a season set, not one passing: those leaves are gone at once, as before, and spring brings leaves back without dropping any. Leaves fall at their real speed, so a season sped up to a few seconds leaves them in the air into winter: the seasons clip above, a year in eight seconds, sets `autumn_distance` to 0.

![A maple's autumn in three moments: its red crown streaming leaves, its branches bare while the last leaves fall around the trunk, and the bare tree with its leaves lying around its foot](../media/autumn_fall.jpg)

*`inspect --mode tree --preset maple --season 1,0 --autumn 6,0.5,0.75 --frames 14`, frames 3, 9 and 13: the leaf fall rises from 0 to 1 over six seconds; 2.75, 7.25 and 10.25 s in.*

Nothing moves on the CPU: a drop is one entity (`LeafBurst`) with one small mesh, drawn with a copy of its tree's leaf material that moves the cards in its vertex stage (`FloraExtension::falling`), and despawned when its last leaf is gone. Making one reads the tree's leaf mesh (a pool's variants once, then kept), and `FallingLeaves::max_leaves` (5 000) caps how many fall or lie at once. On an Apple M4 Pro (`forest --offscreen`): knocking trees every few frames with every shake slot busy (`--shake-animate`), so that thousands of leaves are always falling, cost 2% more frame time over 100 000 pooled oaks than before falling leaves and 4% over 1 000 individual ones; an autumn every six seconds, over and over (`--leaf-fall-animate 6`), costs 1 000 individual oaks 1% more than the same autumn standing still, and 100 000 pooled oaks 2 to 4%. With nothing falling no system runs. Trees drawn on the CPU and from pools drop leaves; `GpuGrowth` trees and trees drawn with plain `StandardMaterial`s do not. The see-through fade cuts falling leaves as it cuts the trees. A shake's or a drop's leaves are copies: the tree keeps its own, and a few falling copies are not missed from a crown of thousands.

## Picking fruit: `TreeFruit`

A fruiting tree (`FoliageParams::fruit`, the `Apple` preset) drawn as a `ProceduralTree` gets a `TreeFruit` once it is built: the fruit it grew, each with a key that holds as the tree grows, where it hangs in the tree's own space and its radius. `on_tree(&season)` lists the fruit the season hangs on it, `nearest(&season, tree, at, max)` finds the one nearest a hand, and `pick(key)` takes it off the tree: the shaders stop drawing it, on every level of detail, and it stays picked when the tree is rebuilt, until the next blossom brings the fruit back. A shake (`TreeShakes`) knocks loose a share of the fruit of the trees it reaches (`FallingFruit::per_shake`, 0.3 of it at strength 1, less the further the tree stands): each falls from where it hung as a `FallenFruit` entity (a sphere of its colour; its `Transform`'s scale is its radius), bounces and lies on the ground (`FloraGround`, or the height the tree stands at) for `FallingFruit::rest` seconds (60) for the game to pick up, and a `FruitFell` message tells of each. Pooled trees share their meshes, so their fruit hangs as the season says, and a tree grown on the GPU has no `TreeFruit`.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FallenFruit, FloraSeason, TreeFruit};

// The player picks the apple nearest their hand, and gathers the fallen ones within reach.
fn pick(
    mut commands: Commands,
    hand: Single<&GlobalTransform, With<Camera3d>>,
    season: Res<FloraSeason>,
    mut trees: Query<(&GlobalTransform, &mut TreeFruit)>,
    fallen: Query<(Entity, &GlobalTransform), With<FallenFruit>>,
) {
    let at = hand.translation();
    for (tree, mut fruit) in &mut trees {
        if let Some((apple, _)) = fruit.nearest(&season, tree, at, 1.5) {
            fruit.pick(apple.key);
        }
    }
    for (apple, lying) in &fallen {
        if lying.translation().distance(at) < 1.5 {
            commands.entity(apple).despawn();
        }
    }
}
```

![Two apple trees on grass: the left one is shaken and its apples fall and scatter in the grass, then a red stand-in beyond the right one picks its apples one by one, a yellow line marking each, into a basket at its feet](../media/harvest.gif)

*`cargo run --release --example capture_growth -- --harvest`: `TreeShakes`, `FallenFruit` and `TreeFruit::nearest` and `pick`.*

## Felling: `FellTree`

Insert `FellTree { direction, cut_height, speed }` on a tree and it comes down. The plugin cuts its wood at `cut_height`: what stays becomes a `TreeStump` entity, and what falls keeps the tree's entity, both closed by a cut face that shows the end grain (pale sapwood, rings, the pith and a few checks). The tree tips over toward `direction` about the front edge of the cut, as a felled trunk turns on its hinge, slowly at first and then faster as a rod of its height would (the oak below lands about 2.7 seconds after the cut), until its trunk or one of its big limbs reaches the ground; it lands there with a small bounce, thin branches and leaves on the near side giving way into the ground. `TreeLanded` is sent the frame it hits, with the point it hit, for the crash, the dust and the camera shake, and the landing shakes the crowns of the trees it fell through and rustles its own leaves. `Felled` says how far it has tipped and when it lies still. A `PooledTree` becomes the `ProceduralTree` of its variant before it is cut (the same tree as the pool's finest level), and a `GpuGrowth` tree is moved to the CPU path first, so any tree in a forest can be felled. On a hillside, a `FloraGround` resource (see "On terrain" above) is the ground it is cut above and lands on.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FellTree, TreeLanded};

// The player's axe finished the job: the tree falls away from them.
fn fell(commands: &mut Commands, tree: Entity, player: Vec3, trunk: Vec3) {
    let away = (trunk - player).xz();
    commands.entity(tree).insert(FellTree { direction: away, ..default() });
}

// When it lands: a crash and dust where it hit; a minute later, swap it for logs.
fn crash(mut landed: MessageReader<TreeLanded>) {
    for landing in landed.read() {
        info!("tree {:?} hit the ground at {}", landing.tree, landing.at);
    }
}
```

Once it has landed the tree no longer sways (its leaves still flutter), it is not rebuilt if its `ProceduralTree` or `TreeAge` changes, and it stays until you despawn it or buck it (below); its `TreeShape`, if it has one, turns with it and is not cut. There is no physics: the fall is drawn, as a shake is.

![An oak among a few trees is notched by four blows of an axe, each shivering its crown, then cut; it leans, falls to the left with gathering speed, lands on its limbs with a small bounce and lies there by its stump](../media/fell.gif)

*`cargo run --release --example capture_growth -- --fell`*

### Notching: `TreeNotch`

Before the saw, the axe. Insert `TreeNotch { direction, height, depth }` on a standing tree (a `ProceduralTree`, or a `PooledTree`, which becomes the tree of its variant as for felling) and the plugin carves an axe's notch out of its trunk: a wedge on the side `direction` points to, its floor level `height` above the base (0.5 m by default; with a `FloraGround`, above the uphill side, as the saw's cut), its face rising from the floor at 45°, reaching `depth` of the way through the trunk (0.25 by default, at most 0.9). Both faces show the end grain, its rings centred on the trunk's pith. Change `depth` with each blow and the notch is carved again; remove the component and the trunk is whole. Fell a notched tree and the saw goes in at the notch's floor whatever `cut_height` says: the stump keeps the floor and the falling trunk takes the face, as a real felling notch splits. A notched tree stops growing, a change of its `ProceduralTree` or `TreeAge` rebuilds it notch and all, and with `TreeLod` only its nearest level is notched. Pair each blow with a `TreeShake` and the crown shivers and drops a few leaves, as in the clip above.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{FellTree, TreeNotch};

// One blow of the player's axe on the side facing them: the notch deepens; once it is a third of
// the way through, the tree falls toward the notch.
fn chop(commands: &mut Commands, notches: &mut Query<&mut TreeNotch>, tree: Entity, toward_player: Vec2) {
    let Ok(mut notch) = notches.get_mut(tree) else {
        commands.entity(tree).insert(TreeNotch { direction: toward_player, depth: 0.1, ..default() });
        return;
    };
    if notch.depth >= 0.3 {
        commands.entity(tree).insert(FellTree { direction: notch.direction, ..default() });
    } else {
        notch.depth += 0.1;
    }
}
```

![Left: the foot of an oak with an axe's notch cut into its side, a pale level floor and a slanted face meeting at the back of the notch, the end grain's rings on both. Right: a pine felled at its notch beside its stump, the stump's top a full disc of end grain, the fallen trunk's butt showing the cut behind the notch with the pith and, below it, the slanted face of the notch](../media/notch.jpg)

*`cargo run --release --example inspect -- --out target/notch --preset oak --mode tree --notch 90,0.3 --distance 3.1 --yaw 60 --eye 1.5 --look 0.042 --fov 50 --sun 20,25,30 --size 1200x1200` and `-- --out target/notch_fell --preset pine --mode tree --notch 0,0.3,0.6 --fell 0 --distance 3.1 --yaw 300 --eye 1.5 --look 0.03 --fov 50 --size 1200x1200`, side by side.*

Insert `BuckTree { log_length, min_radius }` on a felled tree, with its `FellTree` or later, and once it lies still the plugin bucks it: its trunk is cut into logs `log_length` long (2 m by default), which lie on the ground in a row from the stump, the way the tree fell, on the `FloraGround` if there is one, each with its bark and end grain at both ends; the tree's entity and its crown are gone. The logs stop where the trunk gets thinner than `min_radius` (8 cm). Each is a `TreeLog` entity: `index` counts up the trunk from the butt, and `capsule` is a `WoodCapsule` in its own space, for its collider. `TreeBucked { tree, stump, logs }` is sent the same frame.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::{BuckTree, FellTree, TreeLog};

// Fell the tree away from the player, and buck it into 2 m logs once it lies still.
fn chop_down(commands: &mut Commands, tree: Entity, away: Vec2) {
    commands.entity(tree).insert((FellTree { direction: away, ..default() }, BuckTree::default()));
}

// A collider for every new log, for the player to pick up.
fn log_colliders(logs: Query<&TreeLog, Added<TreeLog>>) {
    for log in &logs {
        let (capsule, place) = log.capsule.to_primitive(); // in the log's own space
        info!("log {}: {:.2} m across, its middle at {}", log.index, 2.0 * capsule.radius, place.translation);
    }
}
```

![An oak bucked into three logs lying in a row from its stump, the pale end grain showing in the gaps between them](../media/logs.jpg)

*`cargo run --release --example inspect -- --out target/logs --preset oak --mode tree --fell 0 --buck 1.5 --distance 11 --eye 3.2 --look 0 --yaw 25 --fov 40 --sun 25,22,-15 --size 1600x900`, cropped.*

## Fire: `FloraFire`

Trees burn. Insert a `FloraFire` over the game's world, a square grid of cells that each hold how charred they are and how hot they burn, and light it where the game says (`ignite(at, radius)`): a lightning strike, a fire arrow, a campfire left burning. The plugin spreads it from cell to cell through the trees at `FireSpread::speed` (1 m/s by default) whatever the size of the cells, faster downwind (`FloraWind`), as far as there is something to burn: the cells within `reach` (3 m) of a trunk, so a clearing, a road or a firebreak stops it. Each tree it reaches scorches its leaves brown and burns them away one by one, each flaring as it goes (in the shadow maps too), and chars its wood from the ground up to black charcoal split into blocks whose seams glow while it burns and cool after. Every way a tree is drawn takes part: individual trees, `TreeLod` levels, pools and their impostors, `GpuGrowth` trees, and the leaves a burnt tree no longer has do not fall from it.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::FloraFire;

#[derive(Component)]
struct Player;

// A 512 m square around the origin, 128 cells a side (4 m each). Insert it while the level
// loads: the flora materials are prepared again once, with the fire in their shaders.
fn setup(mut commands: Commands) {
    commands.insert_resource(FloraFire::new(Vec2::ZERO, 512.0));
}

fn lightning(mut fire: ResMut<FloraFire>) {
    fire.ignite(Vec3::new(40.0, 0.0, -12.0), 3.0);
}

// The grid is the game's too: read it where the shaders do.
fn burn_the_player(fire: Res<FloraFire>, players: Query<&Transform, With<Player>>) {
    for player in &players {
        if fire.heat_at(player.translation) > 0.5 {
            info!("the player stands in the fire");
        }
    }
}

// Where it burns now, for the game's lights and its AI: the six hottest 4 m squares.
fn watch_the_fire(fire: Res<FloraFire>) {
    for spot in fire.hottest(4.0, 6) {
        info!("burning at {} ({:.0}% of its square)", spot.at, spot.share * 100.0);
    }
}
```

`char_at` and `heat_at` read the grid between the cell centres, as the shaders do: to hurt what stands in the fire, make a burnt tree worthless, or scorch the game's own ground (the clip below colours its ground by `char_at`). A tree the fire burns through (the char where its trunk stands passes `BURNT`, 0.95) is reported once with a `TreeBurnt` message and marked `Burnt`: insert `Burnt` on the burnt trees of a save, with the fire's restored grid, and loading reports none of them. `extinguish` puts the fire out where rain or water falls, keeping the char, and `scorch` lays an old burn down with no fire, for a level that starts burnt. `FireSpread` (`FloraFire::with_spread`) sets how fast the fire moves, how long a spot burns and its embers glow, how far a tree's fire reaches and whether the ground itself burns; `FloraFire::with_cells` sets the size of the cells (smaller cells burn along finer lines). The same fire lit the same way on the same clock burns the same way (`FloraFire::seed` makes its front ragged).

`burning()` hands out every cell burning now (heat above 0, cooling embers included) with its centre, for the game's lights and its AI: keep the living out of it, feed a map of what the watch can see. `hottest(square, n)` gathers them into squares `square` world units wide and hands out the `n` hottest as `FireSpot`s: the heat-weighted middle of the square's burning cells, their heat summed, and `share`, that heat over the cells a square holds (0..1, whatever the size of the cells): a few lights where it burns hottest, each as bright as its share. Both look only at the cells inside `touched_bounds()`, the rectangle outside which nothing has burnt, so a small fire on a large grid is cheap to read, and the fire is marked changed only in a frame it steps (ten times a second while anything burns) or the game changed it, so a system run on `resource_changed::<FloraFire>` reads it just when there is something new.

A save needs only what burnt: `snapshot()` is a `FireSnapshot`, the grid's side and the cells holding any char or heat (a burnt corner of a 512² grid is a few hundred entries, not 262 144; it serializes with the `serde` feature), and `restore_snapshot` puts a fire of as many cells back as it was, a cell still burning burning on. `restore(&cells)` does the same from every cell.

![The grove at dusk from a still camera: a fire lit on its right runs through it on the wind, each crown scorching and flaring orange as its leaves burn away, the trunks blackening with glowing seams, the ground going to ash under them and lit orange by the fire, until bare black trees stand on a burnt clearing and their embers cool](../media/fire.gif)

*`cargo run --release --example capture_growth -- --fire`: the grove at dusk, a fire lit on its upwind edge. Like the rest of a clip it is sped up (`FireSpread` 3.5 m/s, each spot burnt through in 2.5 s), the clip's own ground scorches by `char_at`, and six point lights over the fire's hottest 4 m squares (`hottest`) light the ground and the trunks around it.*

![An oak burning in daylight, three moments: the char climbing the trunk from the ground in black blocks with orange glowing seams under a green crown; the trunk black and the crown scorching, leaves flaring orange as they burn away; the bare black tree, its seams still glowing](../media/fire_burn.jpg)

*`inspect --mode tree --preset oak --burn 1,2 --frames 12 --distance 9 --size 1280x960`, frames 1, 3 and 6: 3, 7 and 13 s after the fire was lit at the oak's foot.*

While no `FloraFire` exists no shader carries any of it, and frames cost what they did before (1 000 individual oaks 24.5 ms, 100 000 pooled oaks 7.5-7.6 ms, as in 0.25.0; Apple M4 Pro, `forest --offscreen`). Inserting one prepares the flora materials again once, with the fire in their shaders; with nothing burning a shader stops at the grid's header (1 000 oaks 24.6-24.7 ms, `forest --fire armed`), and outside the burnt rectangle a tree reads two vectors and no cell. A forest on fire costs 1 000 individual oaks 2% (25.0-25.1 ms, every tree within 60 m of the middle lit at once) and 100 000 pooled oaks nothing measurable (7.6 ms, within 300 m). The grid rides in the buffer the wind is in, written ten times a second while anything burns or glows (8 bytes a cell: 128 KB at 128 cells a side), so a wind that changes every frame uploads it too; each step of the fire looks only at the cells inside the burnt rectangle. Leaf shading and wind need storage buffers, and so does fire: on WebGL2 trees do not burn.

## Walking through undergrowth: `FoliagePusher`

Shrubs and low branches that a character walks through as if they were not there break the illusion. Put `FoliagePusher` on the player, the NPCs or a cart (`radius`, 0.45 m by default; `height`, 1.8 m, centred on its translation; `strength`, 1) and the leaves and thin stems it walks through give way: what is nearer its axis than its radius goes out of its way, bending a little down as a pushed stem does, and closes up behind it over half a second as it walks on. Trunks and limbs thicker than 5 cm stay where they are, and a plant stays rooted at its foot. The shaders do it, in the main pass and the shadows alike, on every way a plant is drawn up close (`ProceduralTree`, `TreeLod`, pooled levels, `GpuGrowth`); the plugin writes the `MAX_PUSHERS` (4) bodies nearest the camera into the shared buffer when they move.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::FoliagePusher;

#[derive(Component)]
struct Player;

fn spawn_player(mut commands: Commands) {
    // The translation is the middle of the body: 1.8 m tall, 0.45 m round, standing on the ground.
    commands.spawn((Player, Transform::from_xyz(0.0, 0.9, 0.0), FoliagePusher::default()));
}
```

![A row of shrubs from a still camera: a red stand-in for a character walks through them from left to right, the leaves and stems bending out of its way around its body and closing up again behind it](../media/push.gif)

*`cargo run --release --example capture_growth -- --push`: a `FoliagePusher` 0.45 m round walking at 1.4 m/s in a light wind.*

While no entity has a `FoliagePusher` no shader carries the push, and frames cost what they did before. Inserting the first prepares the flora materials again once, with the push in their vertex stages; a vertex outside the circle around everything the pushers reach reads one vector and goes on. With one body standing in the forest (`forest --push`, Apple M4 Pro) 1 000 individual oaks take 24.9 ms a frame against 24.5 without it (2%), and 100 000 pooled oaks 7.6 ms against 7.5. A still body writes nothing; a moving one rewrites the shared buffer every frame (under a kilobyte, and the fire's grid with it while there is a fire). The push is in the vertex stages, so a leaf card bends with it and a card across the body's axis stretches in its core, which the body hides; shake a plant the body runs into (`TreeShakes`) to make it rustle as well.
