# Cameras

Seeing the player through the trees, and the levels of detail an isometric or top-down camera needs.

## Seeing the player: `SeeThroughTrees`

A third-person or top-down camera in a forest loses the player behind trunks and crowns. Put `SeeThroughTrees` on the camera and keep its `target` on the player: the parts of trees between the camera and the target dither out, in a cone around the line of sight `radius` wide at the target and a third of that at the camera (a cylinder under an orthographic camera, below), which closes over the last tenth of the way so nothing behind the player is cut. Only that camera sees through: shadow maps keep the whole trees (the cut tree's shadow still falls on the player), and so do other cameras. Every way a tree is drawn takes part, pooled levels and impostors included, and so do the camera's prepasses, so a `DepthPrepass` (for TAA, SSAO or depth of field) leaves no holes where a tree was cut.

```rust,no_run
use bevy::prelude::*;
use bevy_flora::SeeThroughTrees;

#[derive(Component)]
struct Player;

fn spawn_camera(mut commands: Commands) {
    commands.spawn((Camera3d::default(), SeeThroughTrees { radius: 1.8, ..default() }));
}

// Keep the clear view on the player's chest as it moves.
fn follow(mut camera: Single<&mut SeeThroughTrees>, player: Single<&Transform, With<Player>>) {
    camera.target = player.translation + Vec3::Y;
}
```

![A forest from behind and above a red stand-in for the player: on the left an oak hides the player; on the right the parts of the oak and its neighbours between the camera and the player are dithered out and the player stands in clear view, the oak's shadow still on the ground](../media/see_through.jpg)

*`cargo run --release --example forest -- --mode naive --trees 1500 --spacing 6 --scatter oak:2,birch:1,maple:1 --grove 30 --eye 0,7,-15 --at 0,1.5,0 --fov 50 --sun 20,30,-10 --wind 0 --deterministic --see-through 0,1,0,1.8 --screenshot see_through.png`, cropped; left without `--see-through`. The same images, byte for byte, with `--prepass depth` and `--prepass all`.*

While no camera has it, no shader carries the fade and nothing draws or costs differently (the 441 gallery scenarios the fade could change render as before; 1 000 individual oaks take 24.5 ms a frame and 100 000 pooled oaks 7.5 ms, as without it). While one has it, every flora pipeline carries the discard, which keeps the GPU from rejecting hidden fragments early, and a depth-only prepass gets a fragment stage: in `forest --offscreen` (Apple M4 Pro) 1 000 individual oaks take 27.0 ms instead of 24.5 (33.2 instead of 30.5 with a depth prepass), and 100 000 pooled oaks 7.5 ms either way. The edge of the clear view is a 4x4 ordered dither: TAA smooths it, and without TAA it reads as a fine screen door up close. One camera sees through at a time, and there is no fade with plain `StandardMaterial`s.

## Isometric and top-down games: orthographic cameras

An isometric camera is usually orthographic: a tree is as large on screen wherever it stands, and the game zooms by changing how much of the world the view holds (`ScalingMode::FixedVertical`), not by moving the camera. The crate follows such a camera in two ways.

**Levels of detail follow the zoom.** A distance says nothing about a tree's size on screen here, so while the game's main camera (the active `Camera3d` with the lowest `order`) is orthographic, the plugin scales every level-of-detail band of pooled trees and of `TreeLod` trees by its zoom: a tree where the view meets the ground (`FloraGround`, or y = 0) gets the level a 45° perspective camera gives it from where that camera would see as tall a view. Zoomed in, the trees are drawn in full; zoomed out over the whole forest, they are impostors; as the zoom changes, the bands are rewritten whenever it moves by more than 2%, and the trees cross over at slightly different zooms by their distance. `LodScale` multiplies the bands on top of that, and alone under a perspective camera: a quality setting (0.5 hands every tree over at half the distance). Distances are still measured from where the camera stands, so stand an orthographic camera well back, a few times as far as its widest view is tall: its picture does not change with the distance, and the trees at the top and the bottom of the screen are then at about the same distance and level.

**Impostors from the camera's pitch.** A pool's impostors are pictures taken 10° above the horizon, which suits a perspective camera; seen from 35° above they would show the trees from the side, flatter than the level 2 trees beside them. Build the pools with the camera's pitch, `LodSettings { impostor_elevation: 35.0, impostor_shadows: true, ..default() }` (up to 60°), and their far trees show their crowns from above as the near ones do, and stand on their shadows: with `impostor_shadows` each impostor casts its tree's silhouette, the quad turned toward the light, where the shadow cascades reach. An impostor is drawn at the front of its crown (each corner moved toward the camera along its own view ray, which leaves the picture on screen as it was), so the near edge of a crown seen from above is not hidden under the ground.

**The clear view is a cylinder** along the camera's view, since every ray is parallel, and four more fields of `SeeThroughTrees` shape it to match a fade the game draws on its own buildings: `keep`, the share of a tree kept at the centre (0.35 leaves a third of it dithered in, so what stands in front still reads); `inner`, where the edge starts to feather (0.7 of the radius by default); `in_front`, cutting only what stands more than that many metres before the target however far away the camera is (by default the cut closes over the last tenth of the way, which leaves 40 m in front of the player uncut for a camera 400 m away); and `floor`, a world height below which nothing is cut, so the trunk beside the player keeps its foot. They work under a perspective camera too.

```rust,no_run
use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy_flora::SeeThroughTrees;

#[derive(Component)]
struct Player;

/// 35° down from the south-east, 400 m back: an orthographic camera's picture does not change
/// with the distance.
const BACK: Vec3 = Vec3::new(0.579, 0.574, 0.579);

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical { viewport_height: 30.0 },
            ..OrthographicProjection::default_3d()
        }),
        SeeThroughTrees { radius: 4.0, keep: 0.35, inner: 0.55, in_front: Some(1.5), floor: Some(0.5), ..default() },
    ));
}

// Follow the player and keep the clear view on its chest; the levels of detail follow the zoom.
fn follow(camera: Single<(&mut Transform, &mut SeeThroughTrees)>, player: Single<&Transform, (With<Player>, Without<SeeThroughTrees>)>) {
    let (mut transform, mut fade) = camera.into_inner();
    fade.target = player.translation + Vec3::Y * 1.2;
    *transform = Transform::from_translation(fade.target + BACK * 400.0).looking_at(fade.target, Vec3::Y);
}
```

![An isometric view of a forest of pines, spruces, oaks and maples: a red walker crosses it along a path, the trees in front of it dithered to a third, then the view zooms out over the whole forest, a diamond of impostor trees, and back in](../media/isometric.gif)

*`cargo run --release --example isometric -- --demo`: about 1 300 pooled trees in groves under an orthographic camera at 35°, its view 22 m tall, out to 260 m and back while a walker crosses the forest; the line at the top says how the trees in the middle of the screen are drawn. Nothing in the example picks levels of detail; its pools' impostors are baked from 35° and cast shadows. Without `--demo` it is a small game: `W` `A` `S` `D` walk, the mouse wheel zooms, `T` turns the see-through off and on.*
