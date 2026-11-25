# bevy_flora: the guide

Longer material moved out of the README. Read in this order if you are new, or jump to the page for
what you are building.

| Page | What is in it |
|---|---|
| [Getting started](getting-started.md) | The crate's features, how to add it, what the plugin does on its own, presets by name, and `build_tree` without the plugin at all. |
| [Growing a tree over time](growing.md) | `TreeAge` and `Growing`, `TreeParams::mature_at`, trees from `.tree.ron` files, `GpuGrowth`, `BuildInBackground` and `TreeLod`. |
| [Forests](forests.md) | `TreePool` and `PooledTree`, `BackgroundPools`, `PoolColors`, `Scatter` and `PointGrid`, `FloraGround`, `FloraUnits` and `PooledDeadfall`. |
| [Bark, leaves, wind and looks](appearance.md) | `FloraWind` and gusts, `BarkParams`, `LeafScan` and grown veins, `RenderStyle`, and the low-poly `CrownStyle::Hull` and `TrunkStyle::Prism`. |
| [Seasons and weather](seasons.md) | `FloraSeason` (autumn, leaf fall, snow, blossom), its fruit, and `FloraWetness` for rain. |
| [Trees a game touches](interaction.md) | `TreeShakes`, `DropLeaves`, `TreeFruit`, `TreeNotch` and `FellTree`, `BuckTree`, `FloraFire` and `FoliagePusher`. |
| [Cameras](camera.md) | `SeeThroughTrees`, and the levels of detail and impostors an orthographic camera needs. |
| [Gameplay queries and saves](gameplay.md) | `TreeShape` and its capsules, `FloraIndex`, `TreeCondition`, and what a save of a forest holds. |
| [The examples](examples.md) | Every example in the crate, what it shows and how to run it, and the `playground` sandbox. |
| [Everything the crate does](features.md) | The full feature list, one line each, with the type or function that does it. |
| [How it works](how-it-works.md) | The pipeline from `TreeParams` to a Bevy `Mesh`. |

Every picture in these pages is also in the [gallery](../gallery.md), with the command that made it.
The crate's own API docs are on [docs.rs](https://docs.rs/bevy_flora); the design and its invariants
are in [ARCHITECTURE.md](../../ARCHITECTURE.md).

## The two ideas worth knowing first

- **A tree is a value.** `TreeParams` is a plain struct with a `seed` in it, so the same value makes
  the same tree on every run and at every age, and the game can edit one copy without touching another.
  Nothing is re-rolled by aging: a branch's birth time and the moment each leaf appears are derived from
  where the branch sits.
- **Nothing per tree per frame.** Wind, seasons, rain, snow, fire, shakes, the see-through fade and the
  push of a body through foliage are all done in the vertex and fragment stages, from one small shared
  buffer the plugin writes. A forest's frame cost is what it draws, not how many trees it holds.
