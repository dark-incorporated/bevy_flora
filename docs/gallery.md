# Gallery

The crate in pictures. Every clip and still here was rendered by an example in `examples/`; the italic
line under each is the command that made it and what it shows. Each section links to the guide page
that explains it, and `docs/media/README.md` gives every file its command, capture date and version.

## From the README

![A maple growing from a bare trunk into a full crown while the camera circles it](media/growth.gif)

*`cargo run --release --example capture_growth`: the maple preset from age 0 to maturity, in a light wind.*

![A mature maple in front of a bare sky: its trunk flaring into roots, its forking limbs, its scanned bark and its leaf cards cut to real maple outlines](media/still.png)

*The maple preset at maturity: `cargo run --release --example inspect -- --preset maple --mode tree --distance 15 --yaw 20 --eye 4`.*

## [Growing a tree over time](guide/growing.md)

### Trees from files: `ProceduralTreeAsset` and `.tree.ron`

![The maple preset as exported to a tree file, and the same file with three values changed: a fuller, lighter crown](media/tree_file.jpg)

*`cargo run --release --example tree_file --features ron` (add `file_watcher` to rebuild on save): left, `examples/assets/trees/maple.tree.ron` as exported; right, the same file with the branch density raised from 3 to 7, the leaf density from 4.5 to 7 and the leaf colour lightened.*

### Distant trees for less: `TreeLod`

![One oak at its three levels of detail, zoomed to the same size: the full tree, then fewer and larger leaves on the limbs and branches, then the limbs alone with a few large leaves](media/tree_lod.jpg)

*The same oak at level 0 (25 m), level 1 (33 m) and level 2 (91 m), each zoomed to the same size.*

## [Forests](guide/forests.md)

### Planting a forest: `Scatter`

![A scattered forest in autumn seen from above one edge: groves of scarlet maples, russet oaks, pale birches and green pines with bending borders, around a round clearing](media/scatter.jpg)

*`cargo run --release --example forest -- --trees 3000 --spacing 7 --scatter oak:3,birch:2,maple:2,pine:1 --grove 60 --clearing 25 --season 1,0 --eye 0,45,-200 --at 0,0,-20 --wind 0 --deterministic --screenshot scatter.png`*

![A pine forest from 60 m up, a stand of scarlet maples in its middle whose edge bends with the groves, pines mixing into its rim](media/stand.jpg)

*`cargo run --release --example forest -- --trees 3000 --scatter pine:1,maple:1 --grove 30 --stand 0,0,45 --season 0.8,0 --eye 0,60,-150 --at 0,0,0 --wind 0 --deterministic --screenshot stand.png`*

![Eye level in a birch grove with oaks beyond: shrubs stand between the trunks in thickets and gaps](media/undergrowth.jpg)

*`cargo run --release --example forest -- --trees 1500 --spacing 7 --scatter oak:3,birch:2,maple:2 --grove 30 --clearing 14 --undergrowth shrub:2.5 --eye -14,1.8,-30 --at 6,1,0 --fov 55 --sun 20,25,-30 --wind 0 --deterministic --screenshot undergrowth.png`: the example's undergrowth also thins out in patches about 30 m across.*

### On terrain: `FloraGround`

![Two oak trunks on a 25° slope, side by side: on the left, stood at the height under its centre, its flat base juts out over the downhill ground; on the right, stood at its footing, the trunk meets the slope all round](media/footing.jpg)

*`cargo run --release --example inspect -- --out target/footing --preset oak --mode tree --slope 25 --distance 4 --eye 0.6 --look 0.04` (right), with `--on-centre` (left).*

![A wooded valley of rolling hills from a clearing: oaks, maples, birches and pines in groves climb the slopes, shrubs between them](media/terrain.jpg)

*`cargo run --release --example forest -- --trees 3000 --spacing 7 --scatter oak:3,birch:2,pine:2,maple:1 --grove 50 --clearing 30 --terrain 14 --undergrowth shrub:3 --eye 0,2,0 --at -80,8,0 --fov 50 --wind 0.3 --deterministic --screenshot terrain.png`: `--terrain 14` draws hills about 14 m high and plants every tree and shrub at its footing, none on ground steeper than 30°.*

## [Seasons and weather](guide/seasons.md)

### Seasons

![A grove of cherry trees in full blossom on bare branches, pale pink crowns receding over green grass](media/blossom.jpg)

*`cargo run --release --example forest -- --trees 700 --spacing 5 --scatter cherry:1 --clearing 7 --season 0,1,0,1 --wind 0.3 --sun -20,25,20 --eye 2,4.5,2 --at -25,1.5,-18 --fov 50 --deterministic --screenshot blossom.png`, cropped.*

![On the left an orchard of round-headed apple trees receding over green grass, red apples among their leaves; on the right the crown of one apple tree up close, glossy red apples hanging among the leaves](media/fruit.jpg)

*`cargo run --release --example forest -- --mode pool --trees 400 --spacing 6.5 --scatter apple:1 --clearing 6 --wind 0.3 --sun -20,25,20 --eye 2,2.6,2 --at -20,1.8,-15 --fov 50 --deterministic --screenshot orchard.png`, cropped, and `cargo run --release --example inspect -- --out target/apple --preset apple --mode tree --distance 3.2 --yaw 300 --eye 1.6 --look 0.55 --fov 50`.*

![A winter forest: dark spruces and pines with snow on the upper sides of their branch tiers, bare oaks and pale birches, on white ground](media/snow.jpg)

*`cargo run --release --example forest -- --trees 3000 --spacing 7 --scatter spruce:3,pine:2,birch:2,oak:1 --grove 50 --clearing 25 --season 1,1,1 --eye 25,9,-75 --at 0,6,-20 --fov 50 --wind 0 --deterministic --screenshot snow.png`*

### Rain: `FloraWetness`

![The foot and lower crown of an oak, twice: on the left dry, its bark a warm mid brown; on the right soaked, its bark a darker, deeper brown with a soft sheen up the trunk and its leaves a little richer green](media/wet.jpg)

*`cargo run --release --example inspect -- --out target/wet --preset oak --mode tree --wet 1 --distance 6.5 --yaw 35 --eye 1.7 --look 0.3 --fov 55 --size 1200x1200` (and `--wet 0`), side by side.*

## [Trees a game touches](guide/interaction.md)

### Trees that react: `TreeShakes`

![The grown grove from a still camera: an axe hit swings the oak alone and knocks a few leaves off it, another swings the birch, then a blast swings every tree outward, leaves fall from them all, and they settle back](media/shake.gif)

*`cargo run --release --example capture_growth -- --shake`: an axe on the oak, one on the birch, then a blast at the grove's middle, in a light wind; leaves fall from every tree hit.*

### Falling leaves: `DropLeaves`

![An autumn maple dropping red leaves: they tumble through the air around its trunk at every angle, the lowest about to reach the grass](media/falling_leaves.jpg)

*`inspect --mode tree --preset maple --season 1,0.3 --drop 40,2.6`: forty leaves of an autumn maple, 2.6 seconds after they let go.*

![A maple's autumn in three moments: its red crown streaming leaves, its branches bare while the last leaves fall around the trunk, and the bare tree with its leaves lying around its foot](media/autumn_fall.jpg)

*`inspect --mode tree --preset maple --season 1,0 --autumn 6,0.5,0.75 --frames 14`, frames 3, 9 and 13: the leaf fall rises from 0 to 1 over six seconds; 2.75, 7.25 and 10.25 s in.*

### Picking fruit: `TreeFruit`

![Two apple trees on grass: the left one is shaken and its apples fall and scatter in the grass, then a red stand-in beyond the right one picks its apples one by one, a yellow line marking each, into a basket at its feet](media/harvest.gif)

*`cargo run --release --example capture_growth -- --harvest`: `TreeShakes`, `FallenFruit` and `TreeFruit::nearest` and `pick`.*

### Felling: `FellTree`

![An oak among a few trees is notched by four blows of an axe, each shivering its crown, then cut; it leans, falls to the left with gathering speed, lands on its limbs with a small bounce and lies there by its stump](media/fell.gif)

*`cargo run --release --example capture_growth -- --fell`*

### Notching: `TreeNotch`

![Left: the foot of an oak with an axe's notch cut into its side, a pale level floor and a slanted face meeting at the back of the notch, the end grain's rings on both. Right: a pine felled at its notch beside its stump, the stump's top a full disc of end grain, the fallen trunk's butt showing the cut behind the notch with the pith and, below it, the slanted face of the notch](media/notch.jpg)

*`cargo run --release --example inspect -- --out target/notch --preset oak --mode tree --notch 90,0.3 --distance 3.1 --yaw 60 --eye 1.5 --look 0.042 --fov 50 --sun 20,25,30 --size 1200x1200` and `-- --out target/notch_fell --preset pine --mode tree --notch 0,0.3,0.6 --fell 0 --distance 3.1 --yaw 300 --eye 1.5 --look 0.03 --fov 50 --size 1200x1200`, side by side.*

![An oak bucked into three logs lying in a row from its stump, the pale end grain showing in the gaps between them](media/logs.jpg)

*`cargo run --release --example inspect -- --out target/logs --preset oak --mode tree --fell 0 --buck 1.5 --distance 11 --eye 3.2 --look 0 --yaw 25 --fov 40 --sun 25,22,-15 --size 1600x900`, cropped.*

### Fire: `FloraFire`

![The grove at dusk from a still camera: a fire lit on its right runs through it on the wind, each crown scorching and flaring orange as its leaves burn away, the trunks blackening with glowing seams, the ground going to ash under them and lit orange by the fire, until bare black trees stand on a burnt clearing and their embers cool](media/fire.gif)

*`cargo run --release --example capture_growth -- --fire`: the grove at dusk, a fire lit on its upwind edge. Like the rest of a clip it is sped up (`FireSpread` 3.5 m/s, each spot burnt through in 2.5 s), the clip's own ground scorches by `char_at`, and six point lights over the fire's hottest 4 m squares (`hottest`) light the ground and the trunks around it.*

![An oak burning in daylight, three moments: the char climbing the trunk from the ground in black blocks with orange glowing seams under a green crown; the trunk black and the crown scorching, leaves flaring orange as they burn away; the bare black tree, its seams still glowing](media/fire_burn.jpg)

*`inspect --mode tree --preset oak --burn 1,2 --frames 12 --distance 9 --size 1280x960`, frames 1, 3 and 6: 3, 7 and 13 s after the fire was lit at the oak's foot.*

### Walking through undergrowth: `FoliagePusher`

![A row of shrubs from a still camera: a red stand-in for a character walks through them from left to right, the leaves and stems bending out of its way around its body and closing up again behind it](media/push.gif)

*`cargo run --release --example capture_growth -- --push`: a `FoliagePusher` 0.45 m round walking at 1.4 m/s in a light wind.*

## [Cameras](guide/camera.md)

### Seeing the player: `SeeThroughTrees`

![A forest from behind and above a red stand-in for the player: on the left an oak hides the player; on the right the parts of the oak and its neighbours between the camera and the player are dithered out and the player stands in clear view, the oak's shadow still on the ground](media/see_through.jpg)

*`cargo run --release --example forest -- --mode naive --trees 1500 --spacing 6 --scatter oak:2,birch:1,maple:1 --grove 30 --eye 0,7,-15 --at 0,1.5,0 --fov 50 --sun 20,30,-10 --wind 0 --deterministic --see-through 0,1,0,1.8 --screenshot see_through.png`, cropped; left without `--see-through`. The same images, byte for byte, with `--prepass depth` and `--prepass all`.*

### Isometric and top-down games: orthographic cameras

![An isometric view of a forest of pines, spruces, oaks and maples: a red walker crosses it along a path, the trees in front of it dithered to a third, then the view zooms out over the whole forest, a diamond of impostor trees, and back in](media/isometric.gif)

*`cargo run --release --example isometric -- --demo`: about 1 300 pooled trees in groves under an orthographic camera at 35°, its view 22 m tall, out to 260 m and back while a walker crosses the forest; the line at the top says how the trees in the middle of the screen are drawn. Nothing in the example picks levels of detail; its pools' impostors are baked from 35° and cast shadows. Without `--demo` it is a small game: `W` `A` `S` `D` walk, the mouse wheel zooms, `T` turns the see-through off and on.*

## [Gameplay queries and saves](guide/gameplay.md)

### Gameplay: colliders and growth events

![An oak with its gameplay shape drawn over it: three tapering capsules up the trunk, capsules along the thick limbs, and a box around the crown](media/shape.jpg)

*`cargo run --release --example inspect -- --out target/shape --preset oak --mode tree --colliders --distance 13 --eye 2 --look 0.45 --yaw 30`*

### Finding trees: `FloraIndex`

![A row of four trees seen from the front: a red stand-in for a woodcutter walks to the birch on the left, a yellow ring at its foot, notches it, walks round it and fells it to the left; the ring jumps to the maple, then to the oak, each felled in turn, then to the pine](media/chop.gif)

*`cargo run --release --example capture_growth -- --chop`: the woodcutter asks `FloraIndex::nearest` for its next tree; a felled tree leaves the index at once, so the ring moves on while it falls.*

## [The examples](guide/examples.md)

### Everything at once: the playground

![First person among apple trees: walking up to an apple tree, shaking it so apples fall into the grass, picking apples off it, turning to an oak, striking it four times so it falls away, stepping back in the rain as it is bucked into logs beside its stump; a counter of apples and logs at the top](media/playground.gif)

*`cargo run --release --example playground -- --demo`; without `--demo` it is yours to play: `W` `A` `S` `D` walk, right mouse look, left click axe, `F` shake, `E` pick, `B` buck, `G` fire, `R` rain, `Y` a month on, `F5` save, `F9` load.*
