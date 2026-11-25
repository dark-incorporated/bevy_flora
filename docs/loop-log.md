# bevy_flora loop log

One entry per iteration of `.omp/commands/build-loop.md`, newest last. Append; never rewrite an
earlier entry.

## Iteration 0 — 2026-10-30 — loop setup

Written by the owner's session, not by a pass. It added `.omp/commands/build-loop.md` (from the
bevy_stigmergy loop command, rewritten for this crate) and this log; nothing under `src/`, `tests/`,
`Cargo.toml` or CI changed.

State at setup:

- `main` at `25e956b` (Merge PR #88, CI on big's self-hosted runners). `Cargo.toml` version 0.46.0.
- Latest published: **0.46.0** on crates.io (`cargo info bevy_flora`), tag `v0.46.0` dated
  2026-10-13 20:45 -0500, so the 6-hour release rule allows a release now.
- `fix/plant-lean-share` holds **0.46.1** (`f45fac5` plant lean as a share of the plant's height,
  `1e4c379` qa D-071/D-072 sway retune): version bump, `## [0.46.1] - 2026-10-15` CHANGELOG entry,
  `src/presets.rs`, `src/shading/common.wesl`, `src/shading/mod.rs`, `qa/visual_findings.md`. It was
  never merged or published. `git merge-tree --write-tree origin/main origin/fix/plant-lean-share`
  merges without conflict (checked at setup).
- `CHANGELOG.md` exists but has no `## [Unreleased]` heading (its top entry is `## [0.46.0]`): the
  first pass that ships adds one above the newest release.
- `cargo fmt --check` fails on the existing tree and CI does not run it, so it is not in the gate.
- CI (`.github/workflows/ci.yml`): build with declared features, `--features serde`, test,
  clippy, docs, examples, `cargo package --locked`; and `gpu-smoke`, the gallery with and without
  `--gpu` on lavapipe under `xvfb-run`, compared with `compare_gallery`. No wasm build.
- Gallery: 491 scenarios (README). `qa/visual_findings.md` open: D-052 (willow curtains, taste),
  D-067 (turned maple veins read pink close up, taste).
- No `benches/`; the forest benchmark is `examples/forest.rs --offscreen`, which prints one
  `forest <mode> ...` line with ms/frame, p95, median and cpu ms/frame.
- Host: big (Linux, Xvfb `DISPLAY=:87`, RTX 3090 shared through `gpu-tenant`). The README's frame
  times are from an Apple M4 Pro; big's numbers are not comparable to them, only to each other.

### Ranked backlog

Open issues first (bugs, then `consumer`, then the rest, oldest first). Neither open issue carries a
label; both come from Sanctuary.

1. **#89 Merge `fix/plant-lean-share` (0.46.1) into `main` and publish it.** Sanctuary pins
   `rev = "1e4c379"` on an unmerged branch, and crates.io (0.46.0) is older than what it runs.
   Ranked first: it is a consumer running on code that a branch deletion would lose, and it is
   ready. Merge with a merge commit (never rebase or squash: the issue's acceptance names
   `f45fac5` and `1e4c379`), run the full gate and the GPU smoke, re-record the README clips
   (`AGENTS.md`), then the Release block publishes 0.46.1. Close #89 with the crates.io URL.
   Acceptance: `git merge-base --is-ancestor 1e4c379 origin/main` succeeds and
   `cargo info bevy_flora@0.46.1` resolves.
2. **#87 Per-tree condition (healthy, sick, dying), settable and readable through `FloraIndex`.**
   Opened 2026-10-20. Its acceptance is five headless tests: set and read back through
   `FloraIndex`; a pooled tree keeps its condition after a pool recycle (or the condition is
   per-pool, documented, with a test); setting it on a felled tree does not panic; a `RecolorTree`
   and a condition change in one frame both apply without desyncing `SharedFlora`; existing tests
   pass. The issue leaves where the condition lives (entity component or index) to the crate:
   decide it, record the assumed decision on the issue. Keep the API general (no Sanctuary
   names); whether a condition also changes the look is a second slice.
3. **bevy_emerge #102 (F6): a stroke-bounded scatter.** The editor's scatter brush must plant
   trees only under the stroke, on the terrain. `Scatter::points_where(density)` can express a
   mask through a closure; measure whether a caller can get "only inside this polygon / along
   this capsule stroke, on `FloraGround`, deterministic per seed" without hand-rolling it, and
   add the general form if not. Acceptance: a test that a stroke's points all lie inside it and
   on the ground, and that the same seed gives the same points.
4. **bevy_emerge #102: same seed, same tree, across a `.tree.ron` round-trip.** A test that a
   `TreeParams` written with `to_ron` and read back with `from_ron` builds a tree whose vertex
   positions equal the original's bit for bit, for every preset (the editor saves `.tree.ron`,
   the game loads it). Acceptance: the test exists and passes; any field that does not
   round-trip is fixed.
5. **D-052: willow curtains.** The willow reads as a low mound with ~1 m of bare trunk above
   (seed 1). A generator feature (strands that keep hanging for a set length) rather than preset
   values. Acceptance: in `mature_willow`, the lowest leaf vertex sits below 30% of the tree's
   height and the crown silhouette is wider at its bottom third than at its middle, with
   `qa/visual_findings.md` D-052 moved to `## Fixed`.
6. **Baseline the forest bench on big.** No numbers from this host exist. Record three runs each
   of `forest --offscreen --deterministic --mode pool`, `--mode naive --trees 1000` and
   `--mode gpu-live` (ms/frame, p95, cpu ms/frame) in the first pass's log entry so later
   performance gates have a reference.

### For the owner

- GPU: the gallery, `capture_growth`, `forest` and the GPU smoke open a window; on big they need
  `DISPLAY=:87` and either the RTX 3090 (check `gpu-tenant status`) or software Vulkan. Every
  release re-records 11 README clips (`AGENTS.md`), which needs the display.
- `skill://bevy-flora-visual-qa` describes the owner's Mac clone (`/Users/ladvien/pixy_tree`) and
  319 scenarios; the gallery has 491 now. The command uses its checklist, not its paths.

## Iteration 4 — 2026-10-30 — 0.46.1 lands on main; the lean counts a tree's scale once (#90)

Host: this Mac (Apple M5, Metal, macOS 26.6.2), not big; `gpu-tenant` does not exist here. Clone
at `d07d870`, the baseline built from a detached worktree (`~/f4_base`). Passes 1, 2 and 3 left
no log entries: their work is on `origin/loop/iter-1`, `-2` and `-3` (pass 3 merged the first two
into itself, `e2e7c79`), and **`origin/main` was never pushed**, so #89 to #94 were all still open
and crates.io was still 0.46.0.

**State found.** Open issues: #89 (merge `fix/plant-lean-share`, 0.46.1), #90 (bug: the lean counts a
tree's `Transform` scale twice, blocked by pass 2 on a protected test), #91 (bug: crossed
`min_radial_segments` > `max_radial_segments` panicked), #92 (bug: `FloraIndex` queries with an
infinite bound never returned), #93 (bug: the forest bench's cpu figure on Linux), #94 (bug: the
release checklist re-recorded five clips of eleven), #87 (enhancement). No parked `wiggum/*`
branch. home-still: `system_status` ok (18 661 documents, 18 070 embedded), scribe healthy, distill
answered (the first call timed out at 30 s and succeeded when given 240 s).

**Pick.** Land the stranded passes and fix #90, then release 0.46.1. A bug outranks #87, and #90 was
a bug *in* the code #89 merges, so merging #89 alone would have put a known bug on `main`. Runner-up:
#87 (per-tree condition), untouched. Merged `origin/loop/iter-3` with a merge commit (it carries
`f45fac5` and `1e4c379`, which #89's acceptance names, and the finished commits of passes 2 and 3; none
is a `WIP(pass N, parked)` commit), then ran the gate on the result myself.

**What changed.**

- #90: `wind_bend`'s lean is `MAIN_BEND * sway * length(rel)`; `rel = world - origin` is already in
  world units, so the `scale_y` factor of 0.46.1 counted the scale twice. The function takes no scale
  and its six callers no longer pass one. The old test
  `the_lean_comes_from_the_vertexs_own_distance_from_the_root` finds the lean line by `scale_y` and a
  loop may not loosen it: **Assumed decision (owner may override)**: leave it alone and end the lean
  line with a comment that names `scale_y`, and add
  `wind_bend_takes_no_scale_and_the_lean_has_no_scale_factor` (comments stripped; fails on 0.46.1, run
  with the old shaders put back). Only a doc comment of a neighbouring test was corrected.
- Clippy on the stranded work: `FloraIndex::cells_in` (pass 3) failed `clippy::reversed_empty_ranges`
  under `-D warnings`; its empty ranges are built with `RangeInclusive::new(1, 0)`.
- All eleven README clips re-recorded; the README's GPU parity line says 492 scenarios (measured:
  mean 0.077, worst 2.109, 79 byte-identical, so 0.08, 2.11 and 79 stand); `ARCHITECTURE.md`,
  `WindResponse::sway`'s rustdoc, the README's wind paragraph, the 0.46.1 CHANGELOG entry (the Known
  issue became a Fix), `qa/visual_findings.md` (D-073 to Fixed, a `loop-04` line).

**Sources.** Botanical: Minamino and Tateno 2014 (`10.1371_journal.pone.0093535`), McMahon and
Kronauer's elastic similarity ("the deflection angle ... is determined by its relative position on
the branch, independent of branch length"); Eloy et al. 2017 (`10.1038_s41467-017-00995-6`).
Shipping game: Sousa, GPU Gems 3 chapter 16 (Crysis; web). Technical: Zioma, GPU Gems 3 chapter 6;
Pelzer, GPU Gems 1 chapter 7 (`GPU Gems- Programming Techniques...`). All quoted in
`docs/research/wind.md`. Corpus re-scan added three documents (`10.1111_cgf.13263`,
`10.14214_sf.a8529`, `sig2024_Scintilla_...`). Pending index: none new.

**Numbers** (`/tmp/f4/findings.md`, removed with the scratch).

- Gallery 491 -> 492 scenarios, 0 flags before and after, no BLANK_CAPTURE or CAPTURE_TIMEOUT, no
  PNG under 100 KB; `stats.tsv` columns 1-10 identical on the 491 shared scenarios; every image
  identical to main's below the overlay (mean of means 0.000; the gallery has no wind). New
  `extreme_crossed_sides`: three-sided oak, closed, leaves attached.
- Wind (`zz_probe`, deleted): a mature pine, steady +x wind of strength 1, crown top shift over height.
  Scale 0.3 / 1 / 3 / 7: 0.69% / 2.36% / 7.25% / 17.0% before, 2.36% at all four after (ratio of 3 to 1:
  3.07 -> 1.00), as a `ProceduralTree`, with `GpuGrowth`, pool levels 0-2 and the level 3 impostor.
- Forest bench, three runs, median of medians, ms/frame and cpu ms/frame, main -> this tree: pool
  10 000 4.71 / 12.72 -> 4.92 / 14.72 in sequential sittings, and interleaved four times each
  4.945 / 14.715 -> 4.955 / 14.675 (the same base binary moved from 12.4-13.5 to 14.7 cpu between
  sittings: the host, not the code); naive 1 000 35.49 / 19.60 -> 34.83 / 18.77; gpu-live 1 000, 200
  frames (the 10 000 default never finished here) 185.09 / 251.50 -> 188.48 / 250.95. Build probe: 20
  presets, mature and age 4, vertex and triangle counts identical, times within noise. The bench
  now polls (`UpdateMode::reactive(ZERO)`) offscreen on every OS, not just Linux: the interleaved
  runs show no cost on macOS.
- GPU parity, seven CI scenarios: mean of means 0.581, worst 1.205 (same as main to the digit); full
  gallery 0.077 / 2.109. Determinism: two runs of the seven, columns 1-10 and every PNG identical.
- Tests: 323 lib + 65 doc green; clippy `--all-targets --all-features -D warnings`, `cargo doc -D
  warnings`, `cargo package --locked` (verify build) clean.

**Critique and verdicts** (`/tmp/f4/critique.md`): the permanent guard for #90 reads shader source (a
GPU test is not allowed), so a different way of scaling twice would not trip it; the behaviour is proven
by the probe on every path. #90 GREAT. Landing 0.46.1 GREAT after the clippy fix. The clips: growth,
grove against main's binary on this machine differ at leaf edges (the 0.46.1 sway retune), nothing
else; first, middle and last frame of all eleven looked at. Passes 1-3's gate claims were not true
(clippy failed); their regression tests (#91, #92) pass.

**Gate gaps, said plainly.** The disk was at 97% before the pass and 100% during it (the shared
`CARGO_TARGET_DIR` holds 79 GB of debug builds). `cargo build --examples` links 2 to 4 GB of debug
binaries and filled it, so the gate ran `cargo test --all-features --lib --tests` plus `--doc`, and
`cargo check --examples`; clippy `--all-targets --all-features` compiles every example. A shared
`CARGO_TARGET_DIR` across two worktrees also made cargo call an example binary fresh by mtime after
a source change: `touch src/lib.rs` before trusting a rebuilt example. `cargo-semver-checks` is not
installed and a baseline rustdoc build would not fit; the version is 0.46.1 by reading the diff
(`git diff v0.46.0..HEAD -- src | grep '^[+-].*pub '`: no public item changed). `Cargo.lock` carries
`clipboard-win 5.4.1`, which crates.io marks yanked (bevy's transitive dependency).

**Commits** (`loop/iter-4`): `ca5bcb3` Research: wind, `da71cb2` Research: corpus, `88ddba7` the fix
(Fixes #90), `a132924` docs, `1544352` clippy, `e846e11` clips, `e6dd828` qa.

**Issues.** #90, #92, #93, #94 carry `Fixes` in their commits and close when `main` is pushed; #93 is
Linux-only code that this host cannot run, so it is landed, not verified; #89 and #91 close with the
release. #87 skipped (enhancement, not reached).

### Ranked backlog after iteration 4

1. **#87 per-tree condition** (healthy, sick, dying), through `FloraIndex`, five headless tests in the
   issue; decide entity component or index, record the assumed decision.
2. **bevy_emerge #102 (F6) stroke-bounded scatter**: points only under a brush stroke, on `FloraGround`,
   deterministic per seed.
3. **bevy_emerge #102: `.tree.ron` round trip**: same seed, same vertices bit for bit for every preset
   through `to_ron` / `from_ron`.
4. **D-074 / D-075**: a lone gallery scenario draws no leaves; the gallery's renders are not
   byte-identical run to run on big. Both undermine the harness; D-075 did not show on this Mac.
5. **D-052 willow curtains**, then **owner clean-up of #90's locator** (drop `scale_y` from the old
   test's locator and the comment together) and a CPU-side behavioural mirror of `wind_bend` held to
   the shader as `growth_common.wesl` is held to `frame.rs`.

**Release and clean-up (iteration 4).** Merge `63439ab` on `main` (pushed, tree identical to
`loop/iter-4`, so the gate stood), `cargo publish` **bevy_flora 0.46.1**, tag `v0.46.1`, GitHub
release: https://crates.io/crates/bevy_flora/0.46.1 (`cargo info bevy_flora@0.46.1` resolves).
#89, #90, #91, #92, #93 and #94 are closed (#90 carries the `assumed-decision` label); #87 is open.
Removed, mine: the worktree `~/f4_base` and `~/f4_base_bin`, the release-profile example binaries and
incremental caches and the debug example binaries and incremental caches in `CARGO_TARGET_DIR`
(`/Users/ladvien/target/flora-loop`), `/tmp/f4` and its findings and critique files. Kept: that
target directory's dependency builds, release ones included (about 12 GB added by this pass: the
release profile did not exist there before, and the debug default-feature variant is new). The disk
was 16 GB free when the pass began and is 3.7 GB now (`df -h /`, 100% by Apple's rounding): the next
pass on this Mac should free space first (`cargo build --examples` alone needs 2 to 4 GB). Leftovers
of earlier passes: none found (`git worktree list` shows only the clone; no `f*` directories in `~`
or `/tmp`); the branches `loop/iter-1`, `-2`, `-3` and `-4` remain on origin and are not deleted.

## Iteration 5 — 2026-10-31 — TreeCondition: a tree a game can call sick or dying (#87)

Host: this Mac (Apple M5, Metal, macOS 26.6.2), not big. Disk 13 GB free at the start, 6 GB at the end
(the shared `CARGO_TARGET_DIR` is 73 GB). HomeStill: `system_status` ok (18 668 documents, 488 794
embedded chunks), `scribe_health` ready (11 VLM slots); `distill_search` answered a GPU-busy 500 for two of
twelve queries (bge-m3 wanted 5 000 MB, 4 794 free) and the retries returned. Corpus re-scan of
`catalog_recent` (200): one phenology paper added (`10.5194_bg-2015-619`); the other 199 were unrelated.

**State found.** One open issue, #87 (enhancement, opened 2026-10-20, skipped last pass). No bug, no
`consumer`-labelled issue, no parked branch (`git branch --list 'wiggum/parked-*'` empty; the
`WIP(pass 1, checkpoint)` commits are driver snapshots). bevy_emerge's F6 (#102) is closed: its own loop
landed the stroke-masked scatter, single-tree placement, the `.tree.ron` editor and `forest_view` against
this crate (so backlog items 2 and 3 of iteration 0 were met on its side). v0.46.1 was tagged 49 minutes
before this pass ended, so the 6-hour release rule allows no release: the change rides in the next one.

**Pick.** #87, by the rule that an open enhancement outranks a candidate the pass invents. Runner-up:
the look of a condition (below), which the issue itself leaves as a separate matter. Ranked candidates:
(1) #87 state, (2) #87 look (colour, defoliation), (3) `.tree.ron` round trip bit for bit for every
preset, (4) D-074 / D-075 harness bugs, (5) D-052 willow curtains.

**What changed.**

- `TreeCondition { Healthy, Sick, Dying }` (`src/condition.rs`): a component on the tree's entity,
  ordered, reflected and registered. A tree without one is healthy, so a 100 000-tree pooled forest pays
  nothing for the trees that are well. Set by inserting, cleared by inserting `Healthy` or removing it.
- `IndexedTree::condition` and `FloraIndex::condition_of(entity) -> Option<TreeCondition>`. `index_trees`
  reads the component whenever it inserts a tree and patches the entry in place from
  `RemovedComponents` then `Changed<TreeCondition>` (removals first: a remove and an insert in one frame
  end at the insert; a tree not indexed has no entry to patch).
- Per instance, not per pool: survives a variant change, a pool reload and a `PooledTree` taken off and
  put back; a condition set before the pool exists shows when it does; a felled tree leaves the index
  (`None`) and keeps the component.
- **Assumed decision (owner may override)**, recorded on #87 with the `assumed-decision` label: a
  component on the entity, not a `SetTreeCondition` message or an index-owned map (a message that only
  inserts a component is a second way to do one thing; Bevy's change detection is the notification), and
  `Option<TreeCondition>` from `condition_of` (the issue's example returned a bare value). No `Dead`
  class: a dead tree is the game's to fell. State only: the crate draws a sick tree as a healthy one.
- **Breaking:** `IndexedTree` has a new public field, so the next release is 0.47.0. `index.rs`'s test
  helper `tree()` gained `condition: Healthy` (an assumed owner exception: a new field's default in a
  literal; no assertion changed; stated in the commit).
- Docs: rustdoc, a crate-root doctest (`# Sick and dying trees`), the README's "Finding trees" section
  (its snippet gained `blight` and compiles as a doctest), `ARCHITECTURE.md`, `CLAUDE.md`, an
  `## [Unreleased]` CHANGELOG entry (the first; none existed), a `loop-05` line in `qa/visual_findings.md`.

**Sources.** Botanical: `10.3390_f8040137` ("Crown defoliation is the most widely used parameter for the
assessment of tree vitality"), `10.1007_s10342-005-0085-3` (transparency classes; "next-year mortality
rate increased exponentially with transparency"), `10.3389_fpls.2026.1809811` (stand grades by crown
dieback, ~10 / 30 / 50 / 80%), `10.1038_s41467-020-18996-3` (mode of death). Shipping game: Timberborn's
alive / dying / dead plants as one mod's notes on the decompiled game record them
(<https://github.com/timbermods/dam-good-maps/pull/742>, a web page, second hand), Gregory ch. 16 (a game
object is its attributes and behaviours). Technical: `10.1145_3748522.3779910` (components added to and
removed from entities), Bevy 0.20 `Changed` / `RemovedComponents` doc comments. All quoted in
`docs/research/tree-condition.md`. Pending index: Dobbertin and Brang 2001, Timberborn first-party.

**Numbers** (`/tmp/f5/findings.md`, removed with the scratch).

- Gallery narrowed to the seven CI scenarios (the change reaches no mesh or draw call; the full 492 not
  run, said plainly): 7 PNG = 7 rows, none under 100 KB, 0 flags, no BLANK_CAPTURE / CAPTURE_TIMEOUT;
  against main's binary `stats.tsv` columns 1-10 identical, 0 of 7 PNG differ (`compare_gallery` mean
  of means 0.000); two new CPU runs byte for byte alike; CPU vs `--gpu`: mean of means 0.581, worst 1.205,
  exit 0, the same to the digit as main's and as iteration 4's. Warnings: only the asset-watcher WARN
  (binaries run from `/tmp`, no `assets` dir; textures are compiled in) and the benign winit `Destroyed`.
- Forest bench, interleaved base / new, three runs each, median of medians ms/frame (cpu ms/frame): pool
  10 000 4.90 (14.58) -> 4.91 (14.70); naive 1 000 34.87 (18.57) -> 34.86 (18.62); gpu-live 1 000, 200
  frames 185.11 (247.65) -> 186.67 (250.05). All within 1.0%. No building code touched, so no build probe.
- Smoke at forest scale (a throwaway example, deleted): 10 000 pooled oaks spawn and index in 22.6 ms,
  an idle update is 1.1 ms, 1 993 conditions inserted through `within` take 0.8 ms and one update
  applies them in 0.5 ms; the index reads back 1 993 dying and none outside the radius; a scan for the
  ill is 0.03 ms; a felled one: `condition_of` `None`, component kept, index 9 999.
- Tests: 328 lib (323 + 5 new) and 66 doc green; clippy `--all-targets --all-features -D warnings`,
  `cargo doc -D warnings`, `cargo build`, `--features serde`, `--release`, `cargo check --examples`,
  `cargo package --locked` (verify build) clean. Mutation check of the new tests: no-`Changed` path,
  no-removal path, procedural insert ignoring the component and pooled insert ignoring it each fail a
  test (the third survived the first draft; the move-and-rebuild step was added for it).

**Critique and verdict** (`/tmp/f5/critique.md`): the feature is state only, so a sick tree is drawn as a
healthy one and a game shows it with `PooledLook` / `RecolorTree` beside the component (two components to
keep in step); the recolour test cannot fail until the look slice exists and says so; `IndexedTree`'s new
field breaks hand-built literals. #87's five criteria: GREAT. The look: not built, NOT YET, first in the
backlog.

**Gate gaps, said plainly.** `cargo build --examples` (default features) was `cargo check --examples`
because of the disk (clippy `--all-targets --all-features` compiled every example); the full 492-scenario
gallery and the eleven README clips were not re-run (nothing that draws changed, no release this pass).
`cargo-semver-checks` is not installed: 0.47.0 follows from the new public field.

**Commits** (`loop/iter-5`): `8dd1ed1` Research: corpus, `323ba42` Research: tree-condition, `f1ce6c9`
the implementation (Fixes #87), `5e81b2f` docs.

### Ranked backlog after iteration 5

1. **The look of a condition** (#87 second slice): `MeshTag` per instance on a pool's mesh entities
   (batches survive), read in the leaf and wood fragment stages and the leaf vertex stage; foliage colour
   green to ochre and a share of leaves hidden by the per-leaf hash `FloraFire` uses (~30-50% sick, ~80%
   dying); impostors and `GpuGrowth` follow; gallery scenarios for each class. Design in
   `docs/research/tree-condition.md`.
2. **`.tree.ron` round trip**: `to_ron` / `from_ron` builds vertex-identical trees for every preset (the
   editor writes the file, the game loads it); emerge's own tests only check the seed and one field.
3. **Release 0.47.0** (breaking: `IndexedTree::condition`) once the tag is 6 hours old: re-record the
   eleven clips, `cargo semver-checks` if installed.
4. **D-074 / D-075**: a lone gallery scenario draws no leaves; renders not byte-identical run to run on big.
5. **D-052 willow curtains**; owner clean-up of #90's locator test; a CPU mirror of `wind_bend`.

**Clean-up (iteration 5).** Merge `434f411` on `main` (pushed, tree identical to `loop/iter-5`, so the
gate stood). No release: v0.46.1 was 49 minutes old, the 6-hour rule; 0.47.0 (breaking: the new
`IndexedTree` field) carries it. #87 closed by the push (`Fixes #87`), with a comment on delivery; it
carries `assumed-decision`. Baseline kept as copies of main's three release binaries under
`/tmp/f5/base` (no second worktree or target directory: `git worktree list` shows only the clone).
Removed, mine: `/tmp/f5` (720 MB: binaries, captures, findings, critique), the release example
binaries and incremental caches of `gallery`, `forest`, `compare_gallery` and `zz_probe`, the debug
incremental caches and `bevy_flora` rlibs and test binary this pass built, and
`package/bevy_flora-0.46.1*` in `CARGO_TARGET_DIR`; `df -h /` 6.1 GB free before, 12 GB after. Kept: the
target directory's dependency builds. Leftovers of earlier passes: none found (no `f*_base` in `~` or
`/tmp`); the branches `loop/iter-1` to `-5` remain on origin, not deleted.

## Iteration 6 — 2026-10-31 — TreeCondition is drawn: a sick crown thins and mottles, a dying one is bare and dry (#87)

Host: this Mac (Apple M5, Metal, macOS 26.6.2), not big. Disk 12 GB free at the start, about 4 GB before
the clean-up (the shared `CARGO_TARGET_DIR` is 78 GB). HomeStill: `system_status` ok (18 668 documents,
488 794 embedded chunks), `scribe_health` ok; `distill_search` and `hs distill search` answered a GPU-busy
500 to every query (bge-m3 wanted 5 000 MB, 2 500 to 2 800 free), so the sources were read with
`markdown_read` and `paper_search`. Corpus re-scan of `catalog_recent` (200): one relevant paper
(`10.3832_ifor0538-003`, crown defoliation as an indicator; in the catalog, its conversion did not return
within the call) and one general text (Flake, "The Computational Beauty of Nature"), as `Research: corpus`.

**State found.** No open issue on `bevy_flora` (the last five, #90-#94, are closed), no `consumer` label, no
parked branch (`git branch --list 'wiggum/parked-*'` empty), bevy_emerge's family issue F6 (#102) closed and
no open issue there naming this crate. v0.46.1 was 52 minutes old, so the 6-hour rule allows no release this
pass; 0.47.0 (breaking: `IndexedTree::condition`, now also `FloraUniform::withered`) carries it.

**Pick.** The look of a condition, the first item of iteration 5's backlog: #87 asks for a sick tree that "the
player can read ... as a symptom rather than as scenery", and iteration 5 shipped only the state. Ranked:
(1) the look; (2) release 0.47.0 (blocked until 04:42 local); (3) D-074 / D-075, the two harness bugs: **not
reproducible here**, `gallery --filter extreme_mesh` and `--filter extreme_crossed_sides` alone drew their
leaves and HUD at a warmup of 0, 5 and 20 s on Metal, so they cannot be worked on this host (reading
`gallery.rs`, the four cameras stand at the origin, inside the tree, until `Phase::Apply`, so the leaf
pipelines may not be requested until then; a suspicion for the next pass on big, not tested); (4) D-052
willow curtains; (5) a gradual decline. Dropped from the backlog: the `.tree.ron` round trip, which
`tree_asset::tests::every_preset_survives_a_tree_file` already holds (`from_ron(to_ron(p)) == p` for every
preset, and equal parameters build the same tree).

**What changed.**

- A tree is drawn in its `TreeCondition`. The class travels in bits 8-9 of the `MeshTag` of every mesh the
  tree is drawn with, above a pooled look's glide tag (the low byte): `sync_condition_tags` (PostUpdate,
  only trees that have the component) writes it, `spawn_pooled_trees` and `sync_pooled_looks` carry it with
  the look's tag, so equal mesh and material still batch and no material is made for a condition (a test
  holds the material count).
- The leaf vertex stages (main pass, prepass and shadow maps, GPU growth) hide `TreeCondition::defoliation`
  of the leaves (sick 0.35, dying 0.8; a per-leaf draw from the leaf's phase, not the season's moment) and
  move the colour multiplier the season gives each leaf toward a dry ochre-brown by
  `TreeCondition::withering` (0.5, 1.0, each leaf 0.4 either side) through `FloraUniform::withered`, the
  ratio of the dry colour to the species' leaf colour. Flowers and fruit (alpha below 0.5) and wood are not
  touched, so what `TreeFruit` counts is what is drawn; a hull crown keeps its facets. The impostor does it
  per leaf-sized patch of baked texels. A mix toward the dry colour, not a product with the season's ratio
  (the first build multiplied, and a sick maple's autumn leaves came out pink-white).
- Falling leaves: `condition::Decline` mirrors the draw in `falling.rs` (a leaf the shaders hide is not
  chosen to fall) and the burst carries each leaf's withering in the fourth lane of `ATTRIBUTE_FALL_FACE`
  for `falling.wesl`, so they fall as dry as those on the tree.
- The shaders read the tag only while the glide header's lane y or x says some tree has a condition or a look
  glides (`tags_read`, `glide::sync_condition_flag`).
- `gallery`: 30 `condition_*` scenarios (522 in all; two join CI's GPU-parity filters, additive);
  `inspect --condition sick|dying`; `forest --sick <share> --dying <share>`.
- Breaking: `FloraUniform::withered` is a new public field (with `IndexedTree::condition`, next release
  0.47.0).
- **Assumed decision (owner may override)**, to go on #87 (it carries `assumed-decision`): the shares 0.35 and
  0.8 (the survey's slightly-to-moderately and severely declining grades), the withering 0.5 and 1.0 and the dry
  colour srgb (0.47, 0.31, 0.11) are design defaults; the alternatives were the species' autumn colour for the
  dry leaf (a dying maple would look like autumn) and a continuous severity (the issue asks for three
  classes). Recorded as D-076.

**Sources.** Botanical: `10.3390_f8040137` ("It is a visual indicator of the relative amount of foliage in the
crown of the assessed tree"; "symptoms displayed on leaves and branches"), `10.1007_s10342-005-0085-3`
("defoliation by insects becomes first visible in crown transparency ... foliage reduction becomes only
visible months later"), `10.3389_fpls.2026.1809811` (the grades). Shipping game: Timberborn's dry models (a
mod's notes on the decompiled game, second hand: "green to ochre", "A grown plant loses no leaves"). Technical:
Bevy 0.20 `MeshTag` ("the tag is free for you to use for whatever purpose you wish"), `get_tag`; this
crate's `glide.rs`. All quoted in `docs/research/tree-condition.md`. Pending index: the iForest paper, a
first-party source for how shipped games draw a dying tree.

**Numbers** (`/tmp/f6/findings.md`, removed with the scratch).

- Gallery, 522 scenarios on CPU and `--gpu`: 522 PNG = 522 rows, none under 100 KB, no flag, no
  BLANK_CAPTURE / CAPTURE_TIMEOUT, no `ERROR` or `panicked`. Against main's binary the 492 earlier
  scenarios: `stats.tsv` columns 1-10 identical, and every image identical outside the overlay (0 differing
  pixels in 492 of 492; the overlay's "(n/N)" counts the list, so the overlay band is masked).
- GPU parity: all 522 mean of means 0.073, worst 2.109 (`smoothyoung_shrub_4`, as in iteration 4); the 30
  condition scenarios 0.023 and worst 0.511 (`condition_oak_dying_6`); the nine CI scenarios 0.521 and
  1.205 (the seven before: 0.581 and 1.205).
- Forest bench, interleaved base / new, medians of the runs, ms/frame (cpu ms/frame): pool 10 000 4.93 (14.69)
  -> 4.97 (14.68) +0.9%; naive 1 000 34.81 (18.75) -> 34.95 (18.93) +0.4%; gpu-live 1 000, 200 frames 181.7
  (248.4) -> 178.9 (244.5) -1.5%. The Mac has a fast mode (pool median 4.5-4.6, cpu 13.9) that hits both
  binaries and made two earlier windows unusable; a forest with 25% sick and 12% dying trees, 4.75 ms against
  4.97 healthy (the lost leaves are not drawn). The first build cost +1.8% on the pool (the per-instance tag
  fetch); with a binary that has the condition code compiled out the same window read +1.0%, and the
  `tags_read` flag left +0.9%, so the fetch was not all of it. No build code touched, so no build probe.
- Looked at: `/tmp/f6/after/sheet_*.png` (every condition scenario, front view), `inspect` at levels 0, 1 and 2
  and the impostor at 70 m and 140 m, a mixed forest screenshot, the falling leaves of a dying oak.
- Tests: 334 lib (328 + 6 new) and 66 doc green; clippy `--all-targets --all-features -D warnings`,
  `cargo doc -D warnings`, `cargo build`, `--features serde`, `--release`. Three mutations each fail a test:
  removal not clearing the tags, `sync_pooled_looks` writing the look's tag without the condition, a drop
  that ignores the leaves a tree has lost.

**Critique and verdict** (`/tmp/f6/critique.md`): the state, the mesh look on every preset (all levels of
detail, shadow maps, GPU growth, falling leaves, composed with the season): GREAT. The impostor: GREAT at 70 m,
and at 140 m a dying crown is faint patches against the sky (four fifths gone), NOT YET only in that its colour
washes toward the sky. Open flaws: a class change pops (leaves vanish and recolour in one frame, the lost
leaves do not fall), three classes and no severity, wood and bark untouched, one dry colour for every species,
a casting impostor's shadow is the healthy silhouette. Two defects of the first build (the autumn product and
the impostor's salmon fringe) were found by looking, not by a test, and fixed before shipping.

**Gate gaps, said plainly.** `cargo build --examples` (default features) was clippy's `--all-targets
--all-features` (it compiles every example) because of the disk; no README clip was re-recorded (healthy
trees draw as before: the 492 images above; the release re-records all eleven). The docs ride in the
implementation commit (the harness staged them with it). `cargo-semver-checks` is not installed.
`cargo package --locked` verifies (131 files) and warns that `clipboard-win 5.4.1` in `Cargo.lock` is
yanked on crates.io (a transitive dependency of Bevy; not touched here).

**Commits** (`loop/iter-6`): `2bdd2b8` Research: tree-condition look, `14d1523` Research: corpus, `5958c9b`
the implementation and its docs.

### Ranked backlog after iteration 6

1. **Release 0.47.0** (breaking: `IndexedTree::condition`, `FloraUniform::withered`) once v0.46.1 is 6 hours
   old (04:42 local, Oct 31): re-record the eleven clips, `cargo semver-checks` if installed; a twelfth clip
   of a blighted stand would show the new look (`AGENTS.md` lists eleven, so that is an owner call).
2. **A gradual decline**: a class change pops. On a change toward ill the leaves the new class loses should
   fall (`DropLeaves` from the cards the old class kept and the new one hides) and the colour should glide,
   so "a copse that dies in one season" is a season, not a frame.
3. **A dying tree's wood and shadow**: bark greyed and cracked for `Dying`, and a casting impostor's
   silhouette thinned by the class.
4. **D-074 / D-075** on big: try the gallery's cameras placed at the first scenario's views during the warmup.
5. **D-052 willow curtains**; owner clean-up of #90's locator test; a CPU mirror of `wind_bend`.

**Clean-up (iteration 6).** Merge `63eea41` on `main` (pushed, tree identical to `loop/iter-6`, so the gate
stood). No release: v0.46.1 was 52 minutes old at the start and under 3 hours old at the end, the 6-hour rule;
0.47.0 carries this and iteration 5's work. #87 (closed in iteration 5, label `assumed-decision`) has a comment
on delivery and on the assumed decision. Baseline: copies of main's release binaries under `/tmp/f6/base` (no
second worktree or target directory: `git worktree list` shows only the clone). A `file_watcher` build reloads
an embedded shader when its file changes, so a baseline binary run while the shaders in the clone are edited is
not a baseline: no `.wesl` was edited during the windows quoted above. Removed, mine: `/tmp/f6` (1.9 GB:
binaries, galleries, captures, bench output, findings, critique), and what this pass built in
`CARGO_TARGET_DIR`: the release example binaries and the `bevy_flora` rlibs and test binaries of both profiles
(files modified in the last four hours), the release and debug incremental caches of that window, and
`package/bevy_flora-0.46.1*`; `df -h /` 12 GB free at the start, 3.4 GB before, 15 GB after. Kept: the target
directory's dependency builds (71 GB). Leftovers of earlier passes: none found (no `f*_base` in `~` or `/tmp`);
the branches `loop/iter-1` to `-6` remain on origin, not deleted.

## Iteration 7 — 2026-10-31 — Weeping strands: the willow's branches hang to the ground (D-052)

Host: this Mac (Apple M5, Metal, macOS 26.6.2), not big. Disk 15 GB free at the start, 5.6 GB at the end (the
shared `CARGO_TARGET_DIR` is 76 GB); `bevy_emerge#195` says the loop hosts are out of memory, and this pass ran
one build or one render at a time. HomeStill: `system_status` ok (18 669 documents, 488 829 embedded chunks;
Qdrant upserts time out and bge-m3 finds the GPU busy at times), `scribe_health` ok, `distill_search` answered
(thin for weeping habit; `paper_search` and the web gave the sources). Corpus re-scan of `catalog_recent` (200):
nothing a note needs that `corpus.md` did not list (Scintilla is already there; the rest of the batch is SIGGRAPH
rendering and medical text), so no `Research: corpus` commit.

**State found.** No open issue on `bevy_flora`, no `consumer` label, no parked branch, bevy_emerge's open family
issues (#96 F0, #103 F7) name nothing this crate lacks; its `forest_scatter` hand-rolls a polyline distance test
(`under_stroke`, 10 lines) around `Scatter::points_where`, which works and ships (not taken). v0.46.1 was 2 h 25 m
old, so the 6-hour rule allows no release this pass; 0.47.0 (breaking, see below) carries iterations 5, 6 and 7.

**Pick.** D-052, the willow, the one tree in the set that fails the brief's first test: "the silhouette reads as
the species it is named for". `mature_willow` and `inspect --preset willow` read as arches of bare wood on a
flat-capped pole with a thin scatter of leaves; eight preset variants had been tried and all gave the mound, so
it needed a generator feature. Ranked: (1) D-052 as a generator feature; (2) release 0.47.0 (blocked until 04:42);
(3) a gradual decline (#87's class change pops); (4) `Scatter::along` a polyline for bevy_emerge (they have it);
(5) D-074 / D-075 on big. The runner-up was the gradual decline: a game can live with a pop, and cannot ship that
willow.

**What changed** (`docs/research/weeping-strands.md`, committed first as `33ba0d7`).

- `BranchParams::strands` (`StrandParams`: `density`, `start`, `length`, `spread`, `clearance`) and
  `NodeKind::Strand`. After the skeleton is seated, every branch gets `density * length * (1 - start)` straight
  ropes hung from its outer part, plumb within `spread`, `length` times the trunk's height long or down to
  `clearance` above the floor, a twig's radius, two rings of wood. They draw from streams of their own and come
  after every other node: a tree without them is the tree it was and one with them keeps every other branch
  where it was (both tested). A strand is born when its branch reaches it and carries its leaves along its whole
  length; its leaf `i` keeps its place on the grown strand at every age, which is what `GpuGrowth` shows (the
  first build re-spread them as every branch does and the GPU differed by 2.3 of 255 on `age_willow_8`, 1.4 after).
  Levels of detail, felling, fire, wind and falling leaves needed nothing: a strand is a node.
- The willow uses them (`qa/visual_findings.md` D-077): strands 1.6 per metre from a tenth of each branch,
  0.7 of the trunk's height; limbs from 40% of the trunk (was 50%), `angle` 46 (50), `angle_curve` -0.1 (-0.4),
  `trunk.taper` 0.22 (0.4), leaves 6.5 per metre (10) and 0.34 m (0.26). A mature willow is 119k vertices (wood
  30k, leaves 93k) where it was 55k, builds in 4.4 ms where it took 2.2, level 1 holds 24k leaf vertices (9.5k)
  and level 2 9k (3.6k). Every other preset is unchanged.
- **Assumed decision (owner may override)**, in the commit body and D-077: the willow's look values. Options:
  strands alone (left bare arcs on a pole), the strands with the limb retune above (taken), or more branches
  (D-052's eight failed variants). The retune is a design default; the owner's reply wins.
- Found on the way, fixed: `TreeShape`'s crown box did not hold the leaves on a bent branch's arch (it spanned a
  branch's start and end; `the_crown_holds_every_leaf_and_the_box_holds_the_crown_at_every_age` caught it on the
  new willow at age 5), and under `pipe_radius_enabled` floor avoidance let drooping wood dip up to
  `pipe_radius_min` below the floor (the first build's `pipe_willow` `BELOW_FLOOR`); the pipe model also floored
  every strand to a 2 cm rod, so a strand keeps its own radius there. The pipe fix moves `pipe_shrub`,
  `pipe_fern` and their young scenarios by a centimetre or two (opt-in mode only).
- Breaking: `BranchParams::strands` (new field), `NodeKind::Strand` (new variant): 0.47.0.

**Sources.** Botanical: "Gravity sensing and responses in the coordination of the shoot gravitropic setpoint
angle" (doi `10.1111/nph.18474`, read at PMC9828789: "Stems grow vertically upward, whereas lateral organs ...
tend to grow at a specific angle according to a gravity vector known as the gravitropic setpoint angle (GSA)";
the WEEP gene "results in pendulous branch growth in peach trees"), Missouri Botanical Garden on *Salix
babylonica* ("a stout trunk topped by a graceful broad-rounded crown of branches that sweep downward to the
ground ... as wide" as tall), the URI identification sheet ("hanging or drooping for long distances, almost
rope-like"). Shipping game: the SpeedTree Modeler manual's branch generator (Gravity "equivalent to a downward
pointing direction force", the force's profile curve along the branch, Length "+ % of parent", Prune > Ground).
Technical: `book_real_time_rendering_4th_akenine` (alpha-textured leaf cards against overdraw), this crate's
`lod.rs`. Pending index: Weber and Penn 1995 (paywalled), Moulia et al. 2022, the WEEP paper, EDIS ENH-734.

**Numbers** (`/tmp/f7`, removed with the scratch).

- Gallery, 526 scenarios on CPU and `--gpu`: 526 PNG = 526 rows, none under 100 KB, no flag, no BLANK_CAPTURE
  or CAPTURE_TIMEOUT, no `ERROR` or `panicked` (one `WARN` under `--gpu`: the known hull-crown note). Against
  main's binary the 522 earlier scenarios: `stats.tsv` columns 1-10 identical for 489; the 33 that moved are 29
  willow ones and the four pipe-model shrub and fern ones; all 488 non-willow scenarios with an unmoved mesh are
  the same image below the HUD (the willow's `grow_willow_1`, a bare young trunk, differs by its taper). Two runs
  of one filter and the full run agree on columns 1-10; CPU and `--gpu` stats agree.
- GPU parity: all 526 mean of means 0.080, worst 2.109 (`smoothyoung_shrub_4`, as before); the seven CI
  scenarios 0.581 and 1.205 before, **0.694 and 1.452 after**: `age_willow_6` 0.664 to 1.452. Not "no worse", and
  said so: with strands off the retuned willow reads 1.29, so most of it is the fuller, larger-leafed crown showing
  the existing difference in how a growing limb's leaves are spread (CPU re-spreads them over the aged length, the
  GPU shows the first ones of the grown branch); the strands add 0.13. `compare_gallery` exits 0 on both sets.
- Forest bench, interleaved base / new, medians of 4, 3 and 3 runs, ms/frame (cpu): pool 10 000 4.97 (14.51) ->
  4.90 (14.45) -1.5%; naive 1 000 35.01 (18.92) -> 34.96 (18.92) -0.1%; gpu-live 1 000, 200 frames 182.46
  (248.1) -> 179.01 (244.9) -1.9%. One pool run of the new binary was in the Mac's fast mode (4.56).
- Build probe (`build_tree`, best of 5): the willow 2.22 -> 4.44 ms mature, 0.43 -> 0.75 ms at age 4; `build_lods`
  3.7 -> 7.4 ms; every other preset's vertex and triangle counts unchanged and its time within noise.
- Looked at: `/tmp/f7/cur/{a..h}` (eight tuning rounds: front and side at 15 m, a 9 m game view, the gallery's four
  views), the age and seed series, seasons, conditions, painterly, toon, the four oaks with strands, pipe willow.
- Tests: 341 lib (334 + 7 new) and 67 doc green; clippy `--all-targets --all-features -D warnings`, `cargo doc
  -D warnings`, `cargo check`, `--features serde`, `--examples`, `cargo package --locked` (131 files, the
  yanked `clipboard-win` note as before). Mutation: re-spreading the strand leaves over the current count fails
  `a_strands_leaves_keep_their_places_as_it_grows`; the unfixed crown box fails the shape test.

**Critique and verdict** (`/tmp/f7/critique.md`): the generator feature (`StrandParams`: deterministic, closed,
off the trunk and the floor, grows with its branch, CPU and GPU agree on its leaves, no other tree moves): GREAT.
The willow's look: **NOT YET**, much better than before and not a tree to ship at 9 to 15 m. Flaws, measured
against the note's "what great looks like": the strands' wood reads as thin pale wires among sparse leaf cards (a
string curtain; the leaf cards are 6 cm wide and 6 a metre, the rope is bare between them); the crown's top is bare
arcs on a slim flat-capped trunk; from 30 m level 1 drops all wood under 1.5 cm, so the curtain is gone and the
tree reads as before; it costs 2.2 times the vertices. Eight tuning rounds (strand density, leaf size and count,
limb angle, branch start, trunk taper; a rising top limb made a plume above the trunk and was dropped) and none
reached GREAT with straight, constant-colour ropes of this vertex budget, so D-052 stays Open with its remaining
flaws and options, and the improved willow ships rather than reverting to the mound. The rule's "revert to the last
GREAT state" has no state to revert to: the old willow was never GREAT.

**Gate gaps, said plainly.** `cargo build`, `cargo build --release` and `cargo build --examples` (default
features) were `cargo check` and `cargo check --examples`, and `cargo package --locked`'s own verification build,
because each would rebuild Bevy in a new feature set (3 to 4 GB) on a disk with 5 GB free; `cargo test
--all-features` ran as `--lib --tests` and `--doc` for the same reason (clippy `--all-targets` compiles every
example). No README clip was re-recorded: no healthy non-willow tree changed, and the release re-records all
eleven (the `grove.gif` and `growth.gif` clips that show the willow will change then). No sibling-repo commit.

**Commits** (`loop/iter-7`): `33ba0d7` Research: weeping strands, `a8ee746` the implementation, its docs and
the QA log.

### Ranked backlog after iteration 7

1. **Release 0.47.0** (breaking: `IndexedTree::condition`, `FloraUniform::withered`, `BranchParams::strands`,
   `NodeKind::Strand`) once v0.46.1 is 6 hours old (04:42 local, Oct 31): re-record the eleven clips (the willow
   in `grove.gif` and `growth.gif` changes), `cargo semver-checks` if installed.
2. **Finish the willow (D-052)**: leaf ribbons (a wider or longer leaf on a strand, so the leaves cover the rope),
   strands kept as lines at level 1, limbs whose leaves reach their tips (`add_distributed_leaves` bunches them at
   the base), a greener strand colour (a per-node wood colour), a forked trunk instead of a flat-capped pole; and
   the cost: coarse levels keep every 3rd and 8th leaf, so a willow's far levels are 2.5 times an old willow's.
3. **Age-stable leaf places for every `AllBranches` node** (CPU = GPU for growing limbs): the strand's rule
   (`leaf i keeps its place`) for all of them. It is the cause of the willow's parity (1.29 without its strands)
   and of the `age_*` residue in every `AllBranches` preset; it moves those presets' young trees, so it needs a
   gallery pass of its own.
4. **A gradual decline** (iteration 6's #2): a class change pops; leaves the new class loses should fall.
5. **D-074 / D-075** on big; a dying tree's wood and shadow; D-067; a CPU mirror of `wind_bend`.

**Clean-up (iteration 7).** Merge `aef29ad` on `main` (pushed, tree identical to `loop/iter-7`, so the gate
stood). No release: v0.46.1 was 3 h 43 m old at the push, the 6-hour rule; 0.47.0 carries iterations 5 to 7. No
issue referenced or closed (none was open). Baseline kept as copies of main's release binaries under `/tmp/f7/base`
(no second worktree or target directory: `git worktree list` shows only the clone). Removed, mine: `/tmp/f7`
(3.1 GB: binaries, galleries, tuning renders, bench output, findings, critique, a PMC page), the throwaway
`examples/zz_probe.rs` (never committed), and in `CARGO_TARGET_DIR` every file this pass wrote (modified after
01:00 today): the release example binaries and `bevy_flora` rlibs, the debug test binaries and rlibs and the
default-feature check metadata of Bevy, the release and debug incremental caches, and `package/bevy_flora-0.46.1*`;
`df -h /` 15 GB free at the start, 5.7 GB before, 15 GB after. Kept: the target directory's dependency builds (all
older than this pass). Leftovers of earlier passes: none found (no `f*_base` in `~` or `/tmp`); the branches
`loop/iter-1` to `-7` remain on origin, not deleted.

## Iteration 8 — 2026-10-31 — Leaves keep their places as a branch grows: CPU growth = GPU growth (D-078)

Host: this Mac (Apple M5, Metal, macOS 26.6.2), not big. Disk 15 GB free at the start, 4.9 GB before the clean-up
(the shared `CARGO_TARGET_DIR` is 79 GB); one build or one render at a time. HomeStill: `system_status` ok (18 669
documents, 488 829 embedded chunks), `scribe_health` ok, `distill_search` and `markdown_read` answered (the
shoot-apex papers and de Reffye et al. 1988 were in the corpus; SpeedTree's growth manual came from the web).
Corpus re-scan of `catalog_recent` (200): nothing on trees, plants, leaves, wind, fire, impostors or levels of
detail that `corpus.md` lacked (one SIGGRAPH winding-number paper), so no `Research: corpus` commit.

**State found.** No open issue on `bevy_flora` (the list was read; `gh` answered), no `consumer` label, no parked
branch (`git branch --list 'wiggum/parked-*'` empty), a clean `main` at `a91e2dc`. bevy_emerge's family issue #102
(F6) is closed for its slices 2 to 6 and owes a clip only (its `forest_view` draws a scattered tree as a
`PooledTree` and a placed one as a `ProceduralTree` with `TreeAge`, the crate's own components): nothing
this crate lacks. v0.46.1 was 3 h 41 m old at the start, so the 6-hour rule gave no release before 04:42.

**Pick.** Iteration 7's backlog item 3, the one in the brief's first category (a GPU-grown tree that does not
match the CPU one, and growth that jumps): under `AllBranches` (11 of the 20 presets) a branch spread the leaves it
had over its length, so each time it gained one every leaf of it moved. Measured with a throwaway probe over every
preset (ages 0 to 20 in steps of 0.1, branches neither new nor grown): a leaf hopped a mean 1 to 2 cm and a p95 of
9 to 18 cm (oak 15, maple 18, up to 36) per step, and a leaf of the CPU tree stood a mean 8 to 14 cm, p95 35 to
58 cm and up to 1.3 m from where `GpuGrowth` puts it (oak cards are 33 cm). Ranked: (1) this, correctness and
both growth paths; (2) release 0.47.0 (blocked until 04:42; it follows the pass); (3) a gradual decline for
`TreeCondition` (a class change pops); (4) finish the willow (D-052, eight tuning rounds already failed); (5)
D-074/D-075 on big. The runner-up, the gradual decline, is a feature a game can fake with two class changes; this
is a fault a game cannot fix from outside.

**What changed** (`docs/research/leaf-places.md`, committed first as `f4ce982`; implementation `43fbb3d`).

- A grown `AllBranches` branch has `N` leaf slots (`distributed_leaf_count` of `Node::full_length`, a new public
  field: the exact grown length, because `length / progress` is it only to a rounding error and a count that
  lands on a whole number fell by one at some ages, which moved every leaf of a branch for that age: found by the
  probe on a pine's leader at 6.3 years, before the field), each slot at `0.3 + 0.7 t^2` of the branch with
  `t = (j + u_j) / N` and draws of its own (size, direction, turn, tint, hue, drawn in slot order, `draw_per_leaf`).
  A branch `n` leaves long shows the `n` slots that appear first, `foliage::leaf_ranks`: the tip's, then the gaps'
  (van der Corput order counted from the tip). A grown branch shows every slot in slot order, so a mature tree
  is byte for byte what it was (all 462 unchanged gallery images, every mature scenario among them; levels of
  detail, pools and impostors are built from the mature list and do not move).
- `GpuGrowth` shows the same leaves with no change to its shaders or its node buffer: `gpu/mesh.rs` tags each
  leaf's vertices with its rank as the ordinal and `state.rs` already wrote `leaf_count = n`. Its doc loses the
  "`AllBranches` leaves keep their mature positions; the CPU path respaces them" difference and gains the one
  that remains (a leaf that faces out from its branch's tip, `LeafOrientation::RadialOutward`, faces out from the
  mature tip on the GPU and from the tip it has now on the CPU).
- **Assumed decision (owner may override)**, in the commit body and `docs/research/leaf-places.md`: the order in
  which leaves appear. Options: base-first, acropetal (the botanical order, what `GpuGrowth` showed; my first
  build, which left thin bare limb ends past the last leaf on young oaks and apples: `s1.png`), tip-first then
  the gaps (taken: young crowns keep the look they had, and mature trees and coarse levels are unchanged), or
  absolute "in place" timing (SpeedTree's: a leaf starts when the branch has grown to where it stands; the
  shader cannot find a point on a bent axis at a fraction it was not measured at). The rank order is a design
  default; the owner's reply wins.
- **Assumed owner exception**, in the commit body and the CHANGELOG: `a_strands_leaves_keep_their_places_as_it_grows`
  asserted that a young strand shows the first leaves of the grown strand; the rule it pins is unchanged and now
  holds for every `AllBranches` branch, but the leaves that show first are the tip's, so it became
  `every_branchs_leaves_keep_their_places_as_it_grows` (the same places, draws and counts, on every preset and
  every age from 30% to 95% of maturity, and at least ten young strands). `grown_leaves_and_flowers_match_the_cpu_meshers`
  no longer skips the `AllBranches` presets (widened, with `RadialOutward` leaves made to face along their branch).
- Breaking: `Node::full_length` (a new public field): 0.47.0.

**Sources.** Botanical: Mirabet, Besnard, Vernoux and Boudaoud, "Noise and robustness in phyllotaxis" (stem
`10.1371_journal.pcbi.1002389`: "Lateral organs (leaves and flowers) are successively initiated as primordia at
the shoot apex yielding a regular arrangement on the stem known as phyllotaxis"); de Reffye, Edelin, Francon,
Jaeger and Puech, "Plant models faithful to botanical structure and development" (stem `10.1145_378456.378505`:
"the leaves and fruits are inserted in the right places on the branches (according to the age of the tree)").
Shipping game: the SpeedTree Modeler manual, "Growth" and "Leaf mesh generator properties" ("In place: The leaf
will start growing exactly when the parent is long enough to reach its starting point in the fully grown
model", "you create the model in the fully grown state and use these properties to control how it gets there",
and Smoothing "to prevent wild leaf motion during growth"). Technical: `book_real_time_rendering_4th_akenine`
(alpha LODs "experienced as much more continuous than the discrete geometry LOD method, and so avoids
popping"), this crate's `gpu/frame.rs` and `growth.rs`. Pending index: Prusinkiewicz and Lindenmayer (not in the
corpus as text), Hallé, Oldeman and Tomlinson (paywalled); SpeedTree's real-time modelers have no growth.

**Numbers** (`/tmp/f8`, removed with the scratch).

- Gallery, 526 scenarios on CPU and `--gpu`: 526 PNG = 526 rows each, none under 100 KB, no flag, no
  BLANK_CAPTURE or CAPTURE_TIMEOUT, no `ERROR` or `panicked` (one `WARN` under `--gpu`: the known hull-crown note).
  Against main's binary: 462 images byte for byte, 64 changed (the young trees of the 11 presets, plus their
  `hull_*` crowns); `stats.tsv` columns 2-8 identical but for the leaf count of `age_apple_8` and
  `pipeyoung_apple_5` (a leaf's fruit is its slot's), the bounding box of 37 young scenarios; CPU and `--gpu`
  columns 1-10 identical for all 526. Two runs of `--filter age_ --filter grow_`: identical columns and all 117 PNGs
  byte for byte, and the same as the full run.
- GPU parity (`compare_gallery`), all 526: mean of means **0.030** (0.080 before), worst **1.186** (`grow_willow_7`;
  2.109, `smoothyoung_shrub_4`, before), 83 PNGs byte-identical (79 of 492 in the README); the seven CI scenarios
  **0.221 and 0.920** against 0.694 and 1.452 measured here with main's binary. The probe's leaf places, CPU against
  GPU, every `AllBranches` preset and age: 0 (was p95 35 to 58 cm, max 1.3 m); a leaf's re-spread hop per step: 0
  (was p95 9 to 18 cm).
- Forest bench, interleaved base / new, medians of three, ms/frame (cpu): pool 10 000 4.96 (14.55) -> 4.98 (14.60)
  +0.4% (+0.3%); naive 1 000 35.05 (19.18) -> 34.94 (18.85) -0.3% (-1.7%); gpu-live 1 000, 200 frames 177.47 (243.35)
  -> 178.82 (243.30) +0.8% (0.0%).
- Build probe (`build_tree`, best of 9, `/tmp/f8/after/build_new.txt` and `before/build_base.txt`): mature and age 4
  within +0 to 3.5% for 19 of 20 presets (oak +3.5%, willow 4.89 -> 5.01 ms) and `dead_tree` 0.020 -> 0.021 ms, a
  microsecond; vertex and triangle counts identical for every preset at both ages.
- Looked at: `/tmp/f8/cmp/{s1..s5,r1..r7}.png` (before | after, front, side and close-up quarters of grow_oak_6,
  age_apple_6, age_maple_6, age_spruce_4, age_willow_6, age_shrub_4, grow_willow_9, extreme_lagthick_6,
  extreme_fastgrow_2, hull_fir_2, hull_spruce_2; hull_spruce_5 was read before the final rule and not after).
- Tests: 343 lib (341 before: one replaced, three added, one widened) and 67 doc green; clippy
  `--all-targets --all-features -D warnings`, `cargo doc -D warnings`, `cargo check`, `--features serde`,
  `--examples`, `--examples --all-features`, `cargo package --locked` (131 files). Mutations that fail the new
  tests: the re-spread (oak branch 15 at 0.4 of maturity, leaf 0 at 0.322 of it, 0.302 grown), the leaf's index as
  its GPU ordinal (oak age 4, node 14), and `length / progress` (progress 0.8004: slot 1 at 0.370, 0.320 grown).

**Critique and verdict** (`/tmp/f8/critique.md`): the leaf places on a growing branch: **GREAT** (CPU against
GPU 0, no hop, parity 0.030 and 1.19, mature trees bit-identical, build and forest cost within 1% and 3.5%).
Flaws found, none stopping a ship: a branch with one leaf carries it at its tip, so a young shrub shows a single
leaf standing above its crown (`age_shrub_4`, the largest change outside the hull scenarios, mean 4.5 of 255); a
leaf that appears still appears at the node's current leaf size, not from nothing (one pop per count increment,
as before: this change ended the re-spread of the others, not the appearance of the new one; a fractional
`leaf_count` in the vertex stage would ease it in: backlog 1); the `RadialOutward` facing difference above; leaves
move outward with their wood while sub-branches hold their attach points (continuous, the shader's limit); the
README clips are re-recorded by the release.

**Gate gaps, said plainly.** `cargo build`, `cargo build --release` and `cargo build --examples` (default
features) were `cargo check` and `cargo check --examples`, and `cargo package --locked`'s own verification build
(it compiles `bevy_flora` alone), because each would rebuild Bevy in a new feature set (3 to 4 GB) on a disk with
5 GB free; `cargo test --all-features` ran as `--lib --tests` and `--doc` for the same reason (clippy
`--all-targets` compiles every example). The release example binaries (`--all-features`) were built and run.

**Commits** (`loop/iter-8`): `f4ce982` Research: leaf places, `43fbb3d` the implementation and its tests, the
docs commit (CHANGELOG, ARCHITECTURE, README parity numbers, the note, D-078).

### Ranked backlog after iteration 8

1. ~~Release 0.47.0~~ done at the end of this pass (below). Next release: the first of the items here that
   changes the package, once v0.47.0 is 6 hours old (10:43 local, Oct 31); the CHANGELOG has no `[Unreleased]`
   section yet, start one.
2. **A leaf eases in**: a fractional `leaf_count` (`n + the share of the next leaf`) in the vertex stage and in
   `leaf_points_for_node`, so a leaf that appears grows from nothing over the interval until the next, instead of
   appearing at its node's full leaf size (the one pop left in a growing crown).
3. **A gradual decline** (iteration 6's #2): a class change pops; leaves the new class loses should fall.
4. **Finish the willow (D-052)**: leaf ribbons, strands kept as lines at level 1, limbs whose leaves reach their
   tips, a greener strand colour, a forked trunk instead of a flat-capped pole.
5. **`RadialOutward` facing on a growing branch** (the CPU uses the tip it has now, the GPU the mature tip: an
   orientation difference of a few degrees), D-074 / D-075 on big, a dying tree's wood and shadow, D-067, a CPU
   mirror of `wind_bend`.

**Release 0.47.0** (published at 04:43 local, v0.46.1 being 6 h 0 m 30 s old by `git log -1 --format=%ct`).
Contents: iterations 5 to 8 (`TreeCondition` and its look, the willow's strands, the leaf places). Version by the
breaking items the CHANGELOG lists (`IndexedTree::condition`, `FloraUniform::withered`, `BranchParams::strands`,
`NodeKind::Strand`, `Node::full_length`: a minor bump before 1.0); `cargo semver-checks` is not installed here and
its rustdoc builds of two Bevy-sized trees do not fit a disk that was down to 2.4 GB free at the clips' peak, so
the bump is by the documented list, not by the tool (gap said plainly). `release: v0.47.0` (`e029efc`): Cargo.toml
and Cargo.lock, the README's compatibility row and install snippet (0.47), the CHANGELOG entry and its compare
link, and all eleven README clips re-recorded with the release binaries (`/tmp/f8/record.sh`, the commands of
`AGENTS.md`; `ffprobe`: growth 480x360 24 fps 192 frames 4.96 MB, grove, seasons, shake, fell, fire, push, chop
and harvest 560x315 20 fps 160 frames 2.9 to 6.0 MB, playground and isometric 560x315 16 fps 128 frames 7.0 MB).
Seven changed (growth by a few leaves of the young maple; grove, seasons, shake, fell and fire show the new willow;
isometric by 78 bytes), four are byte for byte the 0.46.1 files (push, chop, harvest, playground); first, middle
and last frames of the changed ones against the old files at `/tmp/f8/clips/cmp_*.png`: the captions stand. The
frame times the README, `src/lib.rs`, `src/pool.rs` and `ARCHITECTURE.md` quote (Apple M4 Pro, 1080p) are not
re-measured: this Mac is an M5, and the forest bench here moved by under 1% (above), so they stand.
`cargo publish --dry-run`, `cargo publish` (crates.io 0.47.0, created 09:43 UTC:
<https://crates.io/crates/bevy_flora/0.47.0>), tag `v0.47.0` pushed, `gh release create v0.47.0` with the CHANGELOG
entry. No issue referenced or closed (none was open).

**Clean-up (iteration 8).** Merge `6112532` on `main`, release commit `e029efc` (pushed, tree of `loop/iter-8` plus
the release's files, so the gate stood; `cargo package --locked` and `publish --dry-run` both passed on it).
Baseline kept as copies of main's release binaries under `/tmp/f8/base` (no second worktree or target directory:
`git worktree list` shows only the clone). Removed, mine: `/tmp/f8` (3 GB at its peak: binaries, galleries, clip
frames and old-against-new montages, bench output, findings, critique, a venv with Pillow), the repo's
`target/capture` (1.9 GB of clip frames), the throwaway `tests/zz_probe.rs` (never committed), and in
`CARGO_TARGET_DIR` every file this pass wrote (modified after 02:15 today, 17 227 files, 1.9 GB): the debug test
binaries and rlibs, the default-feature check metadata of Bevy, the release and debug incremental caches, the
`doc` output and `package/bevy_flora-0.46.1*` and `-0.47.0*`; `df -h /` 15 GB free at the start, 2.4 GB at the
lowest (the clips' frames), 19 GB after. Kept: the target directory's dependency builds (71 GB, all older than this
pass). Leftovers of earlier passes: none of this loop's (no `f*_base` in `~` or `/tmp`); `~/i62base`,
`~/bevy_emerge-baseline*.txt` belong to another project and were not touched; the branches `loop/iter-1` to `-8`
remain on origin, not deleted.

## Iteration 9 — 2026-10-31 — A leaf that appears grows from nothing: the last pop in a growing crown (D-079)

Host: this Mac (Apple M5, Metal, macOS 26.6.2), not big. Disk 19 GB free at the start, 11 GB at the lowest (the
shared `CARGO_TARGET_DIR` is 79 GB); one build or one render at a time. HomeStill: `hs status` ok (18 670
documents, 488 845 chunks; scribe and distill up, 788 queued), `distill_search` and the CLI answered. Corpus
re-scan of `catalog_recent` (200): the SIGGRAPH 2015-2025 "Advances in Real-Time Rendering" notes were indexed
the day before and hold one vegetation slide (a season colour LUT and a with-and-without-leaves alpha channel,
`s2025-advances-materials-3`), so one `Research: corpus` commit (`5636e6d`); the rest was psychology and
medicine.

**State found.** No open issue on `bevy_flora` (`gh` answered; the 30 newest are all closed), no `consumer`
label, no parked branch (`git branch --list 'wiggum/parked-*'` empty), a clean `main` at `ab5a349`. bevy_emerge's
family issue #102 (F6) is closed after slice 6 (`forest_view`: a scattered tree is a `PooledTree`, a placed one a
`ProceduralTree` with `TreeAge`, the crate's own components) and owes a clip only. Read in `bevy_emerge`'s
`ops_tree.rs` and `forest_view.rs`: two hand-rolled lookups of a preset by name
(`TreePreset::ALL.iter().find(|t| t.name() == name)`), and a dotted-path field editor over `serde_json`: neither
is a defect, the preset lookup is a candidate (backlog 4). Sanctuary's open notes about trees are game-side
(stacking logs, a woodstock). v0.47.0 is 1 h old at the start (tag `04:23`), so the 6-hour rule gives no
release before 10:23.

**Pick.** Iteration 8's backlog item 2, in the brief's "growth never jumps": under `AllBranches` (11 of the 20
presets) the leaf a branch gains appears at its node's whole leaf size. Measured with a throwaway probe
(`tests/zz_probe.rs`, deleted; each leaf keyed by node id and tint draw, `TreeAge` 0 to maturity in 0.05-year
steps, the README clip's pace): a leaf was on average 0.27 to 0.43 of its mature size at first sight, one in
four above half of it (spruce 43%), the largest at full size; an oak pops 1.5 leaves a frame, a willow 16.
Ranked: (1) this (a visible fault in the crate's headline, growth, in both growth paths, self-contained and
bit-identical at maturity); (2) a gradual decline for `TreeCondition` (a class change pops; a game can fake it
with two class changes); (3) finish the willow (D-052); (4) `TreePreset::from_name` for the editor; (5)
D-074/D-075 on big. No bug and no consumer issue outranked it.

**What changed** (`docs/research/leaf-ease-in.md`, committed first as `bf1b583`; implementation `21c831f`, `2e56bdf`).

- A branch carries a real number of leaves, `distributed_leaf_amount` (`density * length * height_mod`, at
  least 1; `distributed_leaf_count` is its truncation, unchanged). The leaf of rank `r` has flushed
  `clamp(amount - r, 0, 1)` and is drawn at `smoothstep` of it times its size (`leaf_flush_scale`): born at
  nothing when its turn comes, whole a leaf-spacing of branch growth later. The leaves before it are whole, so
  a grown branch (every flush 1, exactly) is byte for byte what it was; the rank order, places and draws of
  iteration 8 are untouched.
- `GpuGrowth` carries the real `amount` in `leaf_count` (`gpu/state.rs`; it was truncated) and scales the leaf in
  `piece_scale` (`growth_common.wesl`) by `smoothstep(clamp(leaf_count - ordinal, 0, 1))`; `gpu/frame.rs` mirrors
  it through the same `leaf_flush_scale`. No new buffer, no tag change.
- Tests: `a_leaf_that_appears_grows_from_nothing` (all 11 presets, 0.01-year steps: at first sight under a tenth
  of its size, no step over 0.3; with the pop restored by a mutation it fails: an oak leaf appears at 0.365),
  `a_leaf_is_born_at_nothing_and_reaches_its_size_over_the_next_leafs_growth` (the curve), and
  `all_branches_leaves_appear_as_the_branch_lengthens` now also asserts leaves easing in on the GPU side (0
  with the pop restored). Existing leaf tests pass unchanged (`every_branchs_leaves_keep_their_places...`,
  `a_count_that_lands_on_a_whole_number...`, `grown_leaves_and_flowers_match_the_cpu_meshers`: no protected
  check was touched). 345 lib and 67 doc tests.
- Docs: CHANGELOG `[Unreleased]`, ARCHITECTURE's leaf paragraph, `AllBranches`' rustdoc, the README's GPU
  parity line and growth bullet, D-079 and the `loop-09` line of `qa/visual_findings.md`.
- No public API change (everything is `pub(crate)` or a shader), so the next release is a patch: 0.47.1.

**Sources.** Botanical: "Tree Growth Modelling Constrained by Growth Equations" (`10.1111_cgf.13263`: the
logistic growth law, "growth with variational speeds equal to a nonlinear speed curve"), "A phenological
timetable of oak growth under experimental drought and air warming" (`10.1371_journal.pone.0089724`), the
leaf-development methods of `10.1093_treephys_tpy041`. Shipping game: the SpeedTree Modeler manual, Leaf mesh
generator, Growth ("Size: This curve controls how the size increases over the course of the leaf's growth
time", "ease in and out of the growth animation by using an S curve"), `s2025-advances-materials-3`. Technical:
`book_real_time_rendering_4th_akenine` (alpha LODs avoid popping), this crate's `gpu/frame.rs`. Pending index:
a leaf-area-against-days curve for broadleaf trees (the corpus has timetables and the logistic law, not the
curve); SpeedTree's real-time modelers have no growth.

**Numbers** (`/tmp/f9`, removed with the scratch).

- Gallery, 526 scenarios, CPU and `--gpu`: 526 PNG = 526 rows each, none under 100 KB, no flag in any, no
  BLANK_CAPTURE or CAPTURE_TIMEOUT, no `ERROR` or `panicked` (CPU: the two asset-watcher `WARN`s of a binary
  run outside the repo; `--gpu`: those and the known hull-crown note). Against main's binary: 462 images
  byte for byte, 64 changed (young trees of the 11 presets, `hull_*` among them), mean of means 0.012 of 255,
  the changed ones about 0.1, worst `hull_spruce_2` 1.61, `hull_spruce_5` 0.92, `hull_fir_5` 0.27, the rest
  0.25 and under. `stats.tsv` columns 1-10: no scenario at maturity moved; 60 young scenarios gained leaf
  vertices and triangles (median +13%, the Japanese maple at age 4 200 to 320 vertices: five leaves to
  eight), 4 bounding boxes moved, trunk columns identical; CPU and `--gpu` columns 1-10 identical for all
  526. A second filtered run of the seven CI scenarios: PNGs byte for byte the first's and the same columns.
  The final binary (after `2e56bdf`) gives `age_` and `grow_` (117 scenarios) identical to the measured run.
- GPU parity (`compare_gallery`), all 526: mean of means **0.029** (0.030 before), worst **1.162**
  (`grow_willow_7`; 1.186), 83 PNGs byte-identical (83); the seven CI scenarios **0.218 and 0.922** (0.221 and
  0.920). GPU before against after 0.021 mean on those seven, CPU 0.028: both paths moved alike.
- Forest bench, interleaved base / new (b n n b b n n b), medians of four, ms/frame (cpu): pool 10 000 4.99
  (14.69) -> 4.97 (14.70) -0.3% (+0.1%); naive 1 000 34.98 (18.78) -> 34.98 (18.82) 0.0% (+0.2%); gpu-live
  1 000, 200 frames 176.94 (241.88) -> 176.27 (241.90) -0.4% (0.0%).
- Build probe (`build_tree`, best of 9, interleaved three times, `/tmp/f9/{before,after}/build_*.txt`):
  mature within 1% for 20 presets, vertex and triangle counts identical; age 4 +2% to +7% on the leafiest
  (spruce 0.400 to 0.424 ms with +11% leaf vertices, shrub 0.688 to 0.731, juniper 0.326 to 0.341, maple 0.323
  to 0.329, fern +3%), identical for the rest. **A logged trade** over the 5% line (shrub, spruce, and the
  juniper at 4.9%): a young tree has one more part-grown leaf per branch, and those are real vertices.
- Leaf appearance probe, before -> after, 0.05 year steps: mean share at first sight 0.27 to 0.43 -> 0.00 to
  0.05; first seen above half 9% to 43% of the leaves -> none; the largest 1.00 -> 0.07 on the oak (0.47
  willow strands, 0.37 fern, whose nodes grow fast); the largest step in one leaf 1.00 -> 0.25 to 0.33 on
  oak, maple, spruce, pine, cherry, apple, juniper (0.80 willow, 0.52 shrub, 0.59 fern). At 0.01 year: at
  first sight 0.03 and under everywhere, the largest step 0.18 (the willow's strands).
- Looked at: `/tmp/f9/cmp/*` (before | after, front and close-up of grow_oak_6, age_apple_6, age_shrub_4,
  age_willow_4, age_japanese_maple_4, age_spruce_2, age_maple_6, hull_spruce_2, ...) and `/tmp/f9/ease/m_*.png`
  (a young maple at ages 4.00 to 4.21 in 0.03 steps, `inspect --mode tree`, before over after): no hole, gap,
  floating or embedded leaf, shading seam or silhouette change; crowns a touch fuller; a new leaf on the long
  limb grows over the successive ages in the after row.
- Gate: `cargo test --all-features --lib` (345) and `--doc` (67), `cargo clippy --all-targets --all-features -D
  warnings`, `cargo doc -D warnings`, `cargo check`, `--features serde`, `--examples`, `cargo package --locked`
  (131 files), the release examples with `--all-features` built and run. Gate gaps, said plainly: `cargo
  build`, `cargo build --release` and `cargo build --examples` with default features were `cargo check` (each
  would build Bevy in a new feature set, 3 to 4 GB, on an 11 GB disk), and `cargo test --all-features` ran as
  `--lib` and `--doc` (it would link every debug example binary).

**Critique and verdict** (`/tmp/f9/critique.md`): a leaf that appears grows from nothing: **GREAT** (at 0.01
year no leaf is above 3% of its size at first sight or moves more than 0.18 a step, CPU = GPU in size and
place, mature trees bit-identical, forest bench within 0.5%). Flaws found, none stopping a ship: the ease is
one leaf-spacing of branch growth, so at the README clip's pace (0.05 year a frame) an oak or maple leaf takes
3 to 4 frames and a willow strand's or a shrub's 1 to 2 (those nodes also grow their whole leaf set fast; a
game's pace is slower, and a longer window of 2 to 3 spacings was rejected: it would thicken every young crown
by 15 to 30%); a young tree carries one more part-grown leaf per branch (the trade above); the stylised hull
crown is built from where the leaves are, whatever their size, so it still pops when a leaf is born and moves
a little earlier (backlog 3); the WGSL smoothstep is checked by `compare_gallery` (CPU against GPU and GPU
before against after) and not by a Rust test (the mirror in `frame.rs` shares the CPU's function, so
`grown_leaves_and_flowers_match_the_cpu_meshers` proves the mirror); a fruit on a part-grown leaf is
part-grown (its radius follows the leaf, `harvest.rs` reads the bounds), so a CPU-grown tree briefly has a
tiny pickable apple; the README clips are not re-recorded (no release before 10:23), `growth.gif` still shows
the old pops until the release and its caption stays true.

**Commits** (`loop/iter-9`): `5636e6d` Research: corpus, `bf1b583` Research: leaf ease-in, `21c831f` the
implementation and its tests, `bc134dc` the docs, `2e56bdf` the leaf count keeps its `i32` saturation.

### Ranked backlog after iteration 9

1. **Release 0.47.1** (a patch: no public API changed) once v0.47.0 is 6 hours old (10:23 local, Oct 31):
   re-record the eleven clips (`growth.gif` changes: a young maple's leaves ease in; the others by a leaf per
   branch at young ages), `cargo semver-checks` if it fits the disk; `[Unreleased]` is written.
2. **A gradual decline** (iteration 6's #2): a `TreeCondition` class change pops; leaves the new class loses
   should fall and the colour should glide.
3. **The hull crown by leaf size**: `crown_hull` takes positions only, so a `CrownStyle::Hull` tree growing
   on the CPU still pops its hull when a leaf is born (and moves a little earlier now); weight leaves by
   size (a weighted percentile) so the hull grows continuously.
4. **`TreePreset::from_name` / `FromStr`** for editors: bevy_emerge's `ops_tree.rs` and `forest_view.rs` both
   hand-roll `TreePreset::ALL.iter().find(|t| t.name() == name)`; a `.tree.ron` field path editor
   (`set_path` over `serde_json`) is its own code too.
5. **Finish the willow (D-052)**: leaf ribbons, strands kept as lines at level 1, limbs whose leaves reach
   their tips, a greener strand colour, a forked trunk; the fast-growing nodes' pace at a clip's rate (strands
   grow 2 m a year, 0.8 of a leaf's size in one frame); D-074 / D-075 on big; `RadialOutward` facing on a
   growing branch; a CPU mirror of `wind_bend`.

**Clean-up (iteration 9).** Merge `23e7f66` on `main` (pushed; tree identical to `loop/iter-9`, so the gate stood,
and re-run on the merged tree: 345 lib tests, clippy). No release: v0.47.0 (tag `04:23`) was under 2 hours old at
the push, the 6-hour rule; the next release is 0.47.1 from 10:23 local on, and carries this pass (the
CHANGELOG's `[Unreleased]` is written and the README claims it makes are true of the code, not of the clips: the
eleven clips are re-recorded by that release). No issue referenced or closed (none was open). Baseline kept as
copies of main's release binaries under `/tmp/f9/base` (no second worktree or target directory: `git worktree
list` shows only the clone). Removed, mine: `/tmp/f9` (3.4 GB: binaries, three galleries, parity renders, montages,
bench output, findings, critique, probe copies), the throwaway `tests/zz_probe.rs` (never committed, moved to the
scratch before the commit and removed with it), and in `CARGO_TARGET_DIR` every file this pass wrote (modified
after 04:40 today, 2.4 GB: the release and debug incremental caches of `bevy_flora` and its examples, the debug
test binaries, the release example binaries, `package/bevy_flora-0.47.0*`); `df -h /` 19 GB free at the start,
11 GB at the lowest, 19 GB after. Kept: the target directory's dependency builds (older than this pass).
Leftovers of earlier passes: none found (no `f*_base` in `~` or `/tmp`, `git worktree list` shows only the clone);
the branches `loop/iter-1` to `-9` remain on origin, not deleted.

## Iteration 10 — 2026-10-31 — A tree falls ill over seconds, not in one frame: `ConditionGlide` (D-080)

Host: this Mac (Apple M5, Metal), not big. Disk 19 GB free at the start, 9 GB at the lowest (the shared
`CARGO_TARGET_DIR` is 80 GB); one build or one render at a time. HomeStill: `hs status` ok (18 670 documents,
488 845 chunks, 780 queued; both scribes and distill up), `distill_search` and `catalog_recent` answered.
Corpus re-scan of `catalog_recent` (200): the three documents it holds that touch the crate (`10.3832_ifor0538-003`,
`sig2024_Interactive_Invigoration_...`, `10.5194_bg-2015-619`) were already in `corpus.md`; the rest was
psychology and medicine, so no re-scan commit. The note's three botanical sources were found with
`distill_search` and added in one `Research: corpus` commit (`c29df23`; its first text named the wrong authors
of `10.1051_forest:2006042`, corrected in the docs commit: Bréda, Huc, Granier and Dreyer, 2006).

**State found.** No open issue on `bevy_flora` (`gh` answered, the list empty), no `consumer` label, no parked
branch (`git branch --list 'wiggum/parked-*'` empty), a clean `main` at `8e6e423`. bevy_emerge's family issue #102
(F6) is closed; its `ops_tree.rs` and `forest_view.rs` still hand-roll `TreePreset::ALL.iter().find(|t| t.name() ==
name)` (backlog 2). Sanctuary uses presets as constants only. v0.47.0 is 1 h 30 m old at the start (tag `04:23`), so the
6-hour rule gives no release before 10:23 and none this pass.

**Pick.** The backlog's "a gradual decline" (iteration 6's #2, kept through iterations 7 to 9): a `TreeCondition`
change was drawn the frame after it was set, a sick oak losing 35% of its leaves and a dying one 80% at once, up to
a thousand leaves and the whole colour in one frame; a game can fake it with two class changes and not smoothly,
and #87's own ask was "a copse that dies in one season and not the next". Ranked: (1) this (the largest pop left in
a state change of a shipped feature, in the brief's "growth never jumps"; it needs per-tree draw state a game cannot
supply); (2) `TreePreset::from_name` / `FromStr` for editors (API friction by the brief's order, ten lines; taken
next); (3) the hull crown by leaf size; (4) finish the willow (D-052); (5) D-074 / D-075, which do not reproduce
here (full galleries and filtered runs agree byte for byte per binary on this Mac, 40 scenarios twice each).
No bug and no consumer issue outranked it.

**What changed** (`docs/research/condition-glide.md`, committed first as `2e8c232`; implementation `7181947`).

- `ConditionGlide { seconds }`, a resource, 0 by default (a change is drawn at once, as every caller has had it).
  Above 0 a change of `TreeCondition` is drawn as a decline: a level 0 / 0.5 / 1 (healthy / sick / dying) rises
  along a smoothstep, the lost share (0, 0.35, 0.8) and the withering (0, 0.5, 1) are functions of it through the
  classes' own values, so a settled tree is drawn exactly as before. A change half way starts at the level on
  screen (continuous); `Healthy` or a removed component recovers it, the leaves coming back; a tree that appears
  with its condition (its `Children` were added the frame the condition changed, or later) shows it at once;
  `FloraIndex` and `condition_of` answer the class at once. The time is the app's `Time`.
- The level rides in the mesh tag above the class (bit 10 "gliding", bits 11-18 the level in 254 steps); the
  shaders read `condition_level` / `level_defoliation` / `level_withering` (`common.wesl`, `impostor.wesl`) in
  place of the class functions, in every leaf stage including the GPU-growth twins. `ConditionShown` rides on every
  tree with a condition, `ConditionGliding` (sparse-set storage) on a tree for the length of its glide, so only
  gliding trees are visited each frame; `tag_for` and the falling leaves' `decline_of` take the level, so a pooled
  tree's new look keeps its glide and a leaf that falls is as dry as those on the tree.
- Each tree steps its level at its own moments (its level in steps plus a per-entity fraction of a step, rounded
  down, never against the way it goes): found by measurement, below.
- `inspect --decline <seconds>[,<after>[,<every>]]` photographs a glide (the tree starts healthy, the condition is
  set once it is ready; the clock is fixed, so runs repeat).
- Assumed decision (owner may override): (1) default `seconds` 0 over 4 s or a per-tree component, because a default
  above 0 changes the tag every existing condition test asserts and every caller's picture (a protected check may only
  grow), and the resource is one line; (2) a glide only for a tree already standing, so a save loaded with a thousand
  dying trees does not spend a minute dying; (3) the smoothstep and 254 steps are design defaults (the beech papers
  give a sequence and yearly grades, not a curve).
- Tests: 8 new in `condition.rs` and 1 in `falling.rs` (listed in the CHANGELOG), the existing condition tests pass
  unchanged (no protected check touched; the mutation check of the in-phase test fails with all 120 trees stepping in
  one frame). 354 lib and 69 doc tests (the README snippet and the type's example are two of them).
- Docs: CHANGELOG `[Unreleased]`, the README's condition section and its `inspect` row, ARCHITECTURE's condition
  glide bullet, `lib.rs`'s crate docs, D-080 and the `loop-10` line of `qa/visual_findings.md`.
- No public API removed or changed, one type added: the next release is a patch, 0.47.1.

**Sources.** Botanical: "From the comfort zone to crown dieback: Sequence of physiological stress thresholds in
mature European beech trees" (`10.1016_j.scitotenv.2020.141792`: "important responses ... to progressive drought,
such as reduced transpiration rate, early defoliation, native embolism and crown dieback"; "mean crown dieback in
the year after the drought exceeded 20%"), "European beech dieback after premature leaf senescence during the 2018
drought in northern Switzerland" (`10.1111_plb.13467`: "mortality increased monotonously over time"), Bréda et al.
2006 (`10.1051_forest:2006042`: "premature leaf shedding during August. The fallen leaves were still green").
Shipping game: Unreal Engine 5.7's Procedural Vegetation Editor documentation ("adjust the Offset parameter to make
the plant overall healthier"), Whatley's Far Cry vegetation as Real-Time Rendering quotes it (stem
`book_real_time_rendering_4th_akenine`, p. 879: a dissolve "experienced as much more continuous than the discrete
geometry LOD method, and so avoids popping"), Timberborn's dry/dead models (second hand, as iteration 5). Technical:
Luebke et al. (`book_level_of_detail_3d_graphics_luebke`, pp. 140-141: "a noticeable and distracting popping during
the transition between LODs"), Bevy's `MeshTag`, this crate's `glide.rs`. Pending index: Whatley's chapter itself; a
per-week curve of one tree's defoliation; a shipped game that animates tree health at runtime.

**Numbers** (`/tmp/f10`, removed with the scratch).

- Gallery, 526 scenarios, CPU and `--gpu`: 526 PNG = 526 rows each, none under 100 KB, no flag in any (`-` x 526,
  before and after), no BLANK_CAPTURE or CAPTURE_TIMEOUT, no `ERROR` or `panicked` (CPU: the two asset-watcher
  `WARN`s; `--gpu`: those and the known hull-crown note). Against main's binary: 522 images byte for byte, on the
  CPU and under `--gpu`; 4 (`painterly_oak`, `painterly_maple`, `toon_oak`, `toon_maple`) differ in ONE pixel of
  1 228 800 each (`magick compare -metric AE` 1; (306,688) for the oaks, (349,696) for the maples). Not noise: two
  runs of the 40 `painterly_`/`toon_` scenarios agree byte for byte for each binary (base1 = base2, new1 = new2);
  painterly in the filtered run is equal base to new, so the pixel moved in the full run only and in the toon pair
  in both; I read it as instruction order after the `common.wesl` edit (not proved). The full run's ffmpeg
  `signalstats` first made these look like 1.2 to 8.1 of 255; `magick compare` says one pixel. `stats.tsv` columns
  1-10: identical for all 526, CPU against `--gpu` and against main's.
- GPU parity (`compare_gallery`), all 526: mean of means **0.029**, worst **1.162** (iteration 9: 0.029, 1.162); the
  seven CI scenarios **0.218 and 0.922** (0.218 and 0.922); with the 30 `condition_*` scenarios added, 37 images
  0.044 / 0.922 for the main binary and the new one alike. A GPU-grown oak (`GpuGrowth`, a throwaway `inspect`
  insert, removed) against the CPU oak at the nine levels of an 8 s decline: mean 4e-5 of 255, one pixel.
- Forest bench, interleaved base / new (b n n b b n n b), medians of four, ms/frame (cpu): pool 10 000 4.97 (13.84)
  -> 5.00 (13.84) +0.4% (0.0%); naive 1 000 34.88 (18.71) -> 34.89 (18.77) +0.1% (+0.3%); gpu-live 1 000, 200 frames
  178.51 (243.40) -> 180.11 (245.55) +0.9% (+0.9%). A second, shorter interleave of the final binary ran while the
  host was slow (naive 154 ms against 35, other loops on the machine): base and final agreed within 1% and it is not
  quoted. No tree-building file was touched, so no build probe.
- **What a glide costs** (the one finding that shaped the code): 10 000 pooled oaks, all declining at once from the
  start of the measured window (a throwaway `FOREST_DECLINE` hack in `forest`, removed), against a run without,
  interleaved. Without per-tree fractions of a step, every tree wrote its tags in the same frames: 6 s glide 5.0 ->
  6.4-6.5 ms, p95 5.4 -> 10.8-11.1, cpu 14.7 -> 18.8. With them: 6 s 5.0 -> 6.5 (p95 10.5, cpu 18.9), 30 s 5.0 -> 5.6
  (p95 6.1, cpu 16.8); the in-phase spikes of a longer glide are gone (a test fails without the fractions) but the
  total is the number of tags that change. Headless, the systems alone, 10 000 trees: 0.377 ms healthy, 0.395 settled
  sick, 0.50-0.52 while all glide over 30 s (worst frame 0.73; 1.4 when they all arrive in one frame), the frame all
  10 000 begin a glide 1.95 ms (the at-once path 4.9). A stand of a few hundred trees: under 0.05 ms (linear
  extrapolation, not measured).
- Looked at: `/tmp/f10/glide/oak2_strip.png` (nine frames of an 8 s decline, `--decline 8,0,1`): a green crown
  thinning and drying with no frame that jumps; frame 0 is the healthy oak and frame 8 the `--condition dying` oak
  byte for byte; frame 4 (level 0.5) is 0.46 of 255 from `--condition sick`, healthy and dying are 4.0 and 5.2 from
  it. `/tmp/f10/glide/imp_strip.png` (`--mode pool --level 3`, the impostor): its patches thin and dry the same way
  to the same end. Not looked at: a mid-glide frame next to the autumn (the two compose by the shader's own
  products); no clip or still was added to the README (the eleven clips are re-recorded by the release).
- Gate: `cargo test --all-features --lib` (354) and `--doc` (69), `cargo clippy --all-targets --all-features -D
  warnings` (two `type_complexity` fixed with the crate's `#[allow]`), `cargo doc -D warnings`, `cargo check`,
  `--features serde`, `cargo package --locked` (131 files, verification build passed), the release examples with
  `--all-features` built and run. Gate gaps, said plainly: `cargo build`, `cargo build --release` and `cargo build
  --examples` with default features were `cargo check`, and `cargo test --all-features` ran as `--lib` and `--doc`,
  each of the others being a 3 to 4 GB Bevy build on a disk down to 9 GB.

**Critique and verdict** (`/tmp/f10/critique.md`): condition glide: **GREAT** for what it promises (monotone,
arrives as the class's own tag bit for bit, continuous across a change, a tree appearing with its condition shows it at
once, the index answers at once, settled forests cost what they cost, the strips read as one tree thinning). Flaws
found, none stopping a ship: (1) off by default, so a game still gets the pop until it sets the resource (assumed
decision above; one number to change); (2) a leaf vanishes whole between steps (6-10 of 3 000 on an oak) and does
not fall or shrink (a game adds `DropLeaves`); (3) the cost while many trees glide scales with the tags that change:
all 10 000 at once cost +1.5 ms over 6 s, +0.6 over 30 s; (4) wood meshes are tagged though their vertex stage never
reads the level, so about half the writes are wasted (nothing marks leaf meshes); (5) the "already standing" rule is
a heuristic on `Children`'s added tick (tested for a save loaded with its conditions, not for a rebuild in the
frame of a change); (6) no gallery scenario shows a level between classes (a still has no clock), so the levels in
between rest on `inspect --decline`, the CPU and impostor strips and the throwaway GPU run; (7) the four
one-pixel differences, unexplained beyond instruction order; (8) the clips are not re-recorded (no release yet).

**Commits** (`loop/iter-10`): `2e8c232` Research: condition glide, `7181947` the implementation and its tests,
`c29df23` Research: corpus, `10408d1` and `df894f1` the docs and numbers.

### Ranked backlog after iteration 10

1. **Release 0.47.1** (a patch: one public type added, nothing removed) once v0.47.0 is 6 hours old (10:23 local, Oct
   31): it carries iterations 9 and 10; re-record the eleven clips (`growth.gif` changes by iteration 9's leaf
   ease), `cargo semver-checks` if it fits the disk; `[Unreleased]` is written.
2. **`TreePreset::from_name` / `FromStr`** for editors: bevy_emerge's `ops_tree.rs` and `forest_view.rs` hand-roll
   `TreePreset::ALL.iter().find(|t| t.name() == name)`; the `.tree.ron` dotted-path editor (`set_path` over
   `serde_json`) is its own code too.
3. **A glide with no per-tree cost**: a leaf marker so only leaf meshes are tagged (about half the writes), or a
   shared slot in the buffer for trees that began together (no per-tree write at all, a layout change in eight
   shaders' constants); and the leaves a glide loses falling (`LeafBurst`) instead of vanishing.
4. **The hull crown by leaf size**: `crown_hull` takes positions only, so a `CrownStyle::Hull` tree growing on the
   CPU still pops its hull when a leaf is born.
5. **Finish the willow (D-052)**: leaf ribbons, strands kept as lines at level 1, limbs whose leaves reach their tips,
   a greener strand colour, a forked trunk; D-074 / D-075 on big; `RadialOutward` facing on a growing branch; a CPU
   mirror of `wind_bend`.

**Clean-up (iteration 10).** Merge `24b7463` on `main` (pushed; tree identical to `loop/iter-10`, so the gate stood,
and re-run on the merged tree: 354 lib tests, clippy `--all-targets --all-features -D warnings`). No release:
v0.47.0 (tag `04:23`) was 3 h 19 m old at the push, the 6-hour rule; the next release is 0.47.1 from 10:23 local on,
and carries iterations 9 and 10 (the CHANGELOG's `[Unreleased]` is written; the README's claims about the glide are
true of the code, and no clip shows it). No issue referenced or closed (none was open). Baseline kept as copies of
main's release binaries under `/tmp/f10/base` (no second worktree or target directory: `git worktree list` shows only
the clone). Removed, mine: `/tmp/f10` (4.7 GB: binaries, the before, after, final and `--gpu` galleries, parity and
re-run renders, glide strips, bench output, findings, critique, the throwaway probe, `forest` and `inspect` copies),
the throwaway `tests/zz_probe.rs` (copied to the scratch and deleted before the first commit), the `FOREST_DECLINE`
edit of `examples/forest.rs` and the `INSPECT_GPU` edit of `examples/inspect.rs` (both restored from copies before
their commits and never committed), and in `CARGO_TARGET_DIR` every file this pass wrote (modified after 05:50 today,
12 240 files, 4.0 GB: the release and debug incremental caches of `bevy_flora` and its examples, the debug test
binaries, the release example binaries, `doc`, `package/bevy_flora-0.47.0*`); `df -h /` 19 GB free at the start, 9 GB
at the lowest (the full `--gpu` galleries, the packaged build and the debug test binaries), 20 GB after. Kept: the
target directory's dependency builds (71 GB, older than this pass). Leftovers of earlier passes: none found (no
`f*_base` in `~` or `/tmp`, `git worktree list` shows only the clone); the branches `loop/iter-1` to `-10` remain on
origin, not deleted.

## Iteration 11 — 2026-10-31 — A preset parses from its name and a tree says when it is grown (`TreePreset::from_name`, `TreeParams::mature_at`)

Pass on `loop/iter-11` from `main` `66c650a`, Apple M5 (Metal; no `gpu-tenant` on this Mac). Home-still at the start:
`system_status` healthy (distill `rc.389` on CUDA and idle, two scribe instances healthy, 18 670 documents, 18 073
embedded, 761 conversions queued), `catalog_recent` read (200 rows; its tree-relevant ones were already in
`corpus.md`). `distill_search` worked and found two catalog documents the note uses that `corpus.md` did not list
(de Reffye 1988, Makowski 2019); they were added.

**Triage.** `gh issue list` on this repository: 0 open (the last closed, #87, #89-#94 on Oct 31). bevy_emerge's F6
(`#102`) is closed (slices 1 to 6 on its `main`); its open family issues are F0 and F7. No `wiggum/parked-*` branch
and no `parked` commit. `git log origin/main..HEAD` empty. So no bug and no issue to take; section 4's first
fallback applies ("what the crate's consumers hand-roll"). Release not taken: v0.47.0 (04:23) was 3 h 51 m old at the
merge, the 6-hour rule; 0.47.1 is for the next pass after 10:23.

**What changed and why it ranked first.** Read the consumers: bevy_emerge's `ops_tree.rs:89` and `forest_view.rs:68`
each write `TreePreset::ALL.iter().find(|t| t.name() == name)` (the first adds an error message joining all 20
names), this crate's own examples write it six times (`capture_growth`, `forest` x3, `inspect`, `tree_file`), and
Sanctuary's `prototype/trees.rs:179` builds a whole mature tree (`build_tree(&params, f32::INFINITY).mature_at`)
once per kind to read one number. A downstream crate cannot add `FromStr` to `TreePreset` (the orphan rule), so
the lookup could only ever be re-written. Added, no behaviour changed:

- `TreePreset::from_name(&str) -> Option<TreePreset>`, `impl FromStr` (error `UnknownPreset`: `input()` and a
  `Display` that lists all 20 names, exported from the crate root) and `impl Display` (prints `name()`): exact,
  snake_case, the inverse of `name()`; `"Oak"`, `"oak "`, `"dragon-tree"` and the serde spelling `"DragonTree"` are
  refused. The six example call sites use it (`tree_file --export` still lower-cases first).
- `TreeParams::mature_at() -> f32` (named as `TreeMeshes::mature_at`, `TreeRender::mature_at`,
  `Skeleton::mature_at`; `mature_age` would have sat beside `GrowthParams::maturity_age`, a different number, now
  documented as such): the age a tree stops changing, `generate_skeleton(self).mature_at(&self.growth)`; `capture_growth`
  uses it in place of `build_tree(..).mature_at` (the same number, so the clips are not touched). `TreeAge`'s and
  `GrowthParams::maturity_age`'s docs point at it; README (Quickstart, a compiled snippet), ARCHITECTURE and CHANGELOG
  `[Unreleased]` say it.

Runner-up: a glide with no per-tree cost (backlog 3 of iteration 10), real but invisible to a game that leaves
`ConditionGlide` at 0. Why this one: three repositories carry the same lines today, the change is additive (a
patch release), and with no open issue a consumer's hand-rolled code is what section 4 says to take first.
Not built, on purpose: a date-to-`FloraSeason` helper (bevy_emerge's `forest_season.rs` writes "year counts from 1
March, or 1 September in the south"): the calendar is the caller's, `FloraSeason::of_year` is already the hook.

**Sources.** Botanical/model: de Reffye et al. 1988, "Plant Models Faithful to Botanical Structure and Development"
(`10.1145_378456.378505`: "the unit of discretized time is the time taken by the growth of a growth unit, this
length of time being supposed constant for the axes a given order of a given plant"), Makowski et al. 2019,
"Synthetic silviculture" (`10.1145_3306346.3323039`: "We use a flowering age parameter to indicate when in the
simulation a plant type reaches maturity"). Shipping game: Epic's Procedural Vegetation Editor documentation
(UE 5.8, web: the Grower's Cycles "is not equivalent to years", "Save Grower Preset ... a good way to expand a
preset library"). Technical: Rust API Guidelines C-COMMON-TRAITS and `std::str::FromStr` (web). Note:
`docs/research/preset-names.md`; both stems added to `corpus.md`. Pending index: the three web pages; no source
measures years-to-maturity of a species against a preset's `maturity_age`.

**Numbers** (`/tmp/f11`, removed with the scratch).

- Tests: `cargo test --all-features`: 357 lib (354 + `a_preset_is_found_by_its_name_and_by_nothing_else`,
  `the_error_says_what_was_asked_for_and_what_there_is`, `mature_at_is_the_age_a_tree_stops_changing`) and 72 doc
  tests (69 + `from_name`'s, `mature_at`'s and the README snippet). `mature_at_is_the_age_a_tree_stops_changing`:
  every preset x seeds 1, 42, 7777: the number `build_tree` reports, wood and leaves at that age within 1e-6 m of the
  full-grown (rounding; the one gap I saw was 1.5e-8 m), the wood at half of it different. Clippy
  `--all-targets --all-features -D warnings` clean, `cargo doc -D warnings` clean (one `redundant_explicit_links`
  fixed), `cargo check` default, `--features serde`, `--examples` clean, `cargo package --locked` (131 files).
- Cost of `mature_at()` (throwaway `examples/zz_probe.rs`, release, median of 30, deleted): all 20 presets 2.4 ms
  against 18.7 ms for `build_tree(INF)`, 7.8x; oak 187 us vs 885 us, willow 677 us vs 4 940 us, maple 249 vs 1 142,
  poplar 39 vs 1 112 (28x), fern 2 vs 120 (48x), baobab 57 vs 210 (3.6x, the least).
- Forest bench, 3 rounds each, medians, ms/frame (cpu), base -> new: pool 10 000 4.91 (12.62) -> 5.01 (13.82)
  +2.0% (+9.5%); naive 1 000 35.57 (19.65) -> 35.41 (19.30) -0.4% (-1.8%); gpu-live 1 000, 200 frames 195.74
  (259.45) -> 178.56 (246.10) (noise, -8.8% / -5.1%). The pool cpu figure is the known bimodal one (base alone ran
  14.07, 12.33, 12.62 in a row); interleaved b n n b b n n b, medians of four: 5.035 (14.32) -> 5.045 (14.73)
  +0.2% (+2.9%), the base runs themselves spanning 13.88-14.80. No hot path was touched (the binaries differ in
  argument parsing and added methods).
- Gallery, the 7 scenarios of CI's GPU job, base and new, CPU and `--gpu`: 7 PNG = 7 rows each, none under 100 KB,
  flags `-` x 7, no BLANK_CAPTURE or CAPTURE_TIMEOUT, only the known `WARN`s (asset watcher path of the copied
  binary, winit `Destroyed`); every PNG byte for byte the main binary's (CPU and GPU) and run 2 equals run 1;
  `stats.tsv` columns 1-10 identical in all five runs; `compare_gallery` 0.218 mean of means, worst 0.922, as base.
  The full 526 was not run: no tree-building code changed.
- Smoke: `forest --preset dragon_tree`, `--scatter oak:2,birch:1 --undergrowth fern:2.5` run; `--preset Dragon_Tree`,
  `--scatter oak:2,sequoia:1` and `inspect --preset sequoia` print usage.
- Gate gaps, said plainly: `cargo build` and `cargo build --release` with default features were `cargo check` (a
  3-4 GB Bevy variant on a 14 GB disk); `cargo test --all-features` ran in full (examples are not test targets, so
  iteration 10's note "ran as --lib and --doc" was how it runs, not a gap).

**Critique and verdict** (`/tmp/f11/critique.md`): **GREAT** for what it promises. Flaws found, none stopping a
ship: exact-only parsing (a caller lower-cases or reads the error's list); two spellings of a preset (`name()` and the
serde form) kept for saved files; `mature_at()` builds a skeleton per call (documented: ask once per kind); at the
very age it returns the wood is a float's rounding short; neither consumer has adopted it (their pins are
`25e956b` and `1e4c379`, older than this commit); the gallery was the 7 CI scenarios, not 526; it is not a visible
improvement and no clip changed.

**Commits** (`loop/iter-11`): `e7c6ce1` Research: preset names, `86405c5` Research: corpus, `ebc68c3` the
implementation, its tests and the examples, `c7e5727` docs, `7d3f41d` the private `UnknownPreset` field, its crate-root
export and the cost sentence. Sibling repositories: none touched.

### Ranked backlog after iteration 11

1. **Release 0.47.1** (a patch: types and methods added, nothing removed) from 10:23 local on: it carries
   iterations 9, 10 and 11; re-record the eleven clips (`growth.gif` changes by iteration 9's leaf ease),
   `cargo semver-checks` if it fits the disk; `[Unreleased]` is written.
2. **A glide with no per-tree cost**: a leaf marker so only leaf meshes are tagged (about half the writes), or a
   shared slot in the buffer for trees that began together; and the leaves a glide loses falling (`LeafBurst`)
   instead of vanishing.
3. **The hull crown by leaf size**: `crown_hull` takes positions only, so a `CrownStyle::Hull` tree growing on the
   CPU still pops its hull when a leaf is born.
4. **Finish the willow (D-052)**: leaf ribbons, strands kept as lines at level 1, limbs whose leaves reach their
   tips, a greener strand colour, a forked trunk; D-074 / D-075 on big; `RadialOutward` facing on a growing branch;
   a CPU mirror of `wind_bend`.
5. **What an editor still hand-rolls around `TreeParams`**: bevy_emerge's `.tree.ron` dotted-path edit (`set_path`
   over `serde_json`) and a panel's need for each field's kind, unit and range; look at it once bevy_emerge has
   moved its pin to this commit and says what it still writes. Also Sanctuary's rain-and-fire rule (`douse_forest`
   writes `fire.spread.speed` from a wetness each frame): `FireSpread` is writable and the policy is the game's, so
   only a hook if two consumers want the same rule.

**Clean-up (iteration 11).** Merge `dbf338c` on `main` (pushed; tree identical to `loop/iter-11`, re-gated on the
merged tree: 357 lib and 72 doc tests, clippy `--all-targets --all-features -D warnings`). No release: v0.47.0
(tag 04:23) was 3 h 51 m old at the merge, the 6-hour rule; the next release is 0.47.1, from 10:23 local on, and
carries iterations 9, 10 and 11 (`[Unreleased]` is written). No issue referenced or closed (none was open).
Baseline kept as copies of main's release binaries under `/tmp/f11/base` (no second worktree or target directory:
`git worktree list` shows only the clone). Removed, mine: `/tmp/f11` (1.0 GB: base and new binaries, galleries, bench
output, the throwaway `zz_probe.rs` copy, findings, critique), the throwaway `examples/zz_probe.rs` (deleted before
its first commit), and in `CARGO_TARGET_DIR` every file this pass wrote (modified after 07:40 today, 17 997 files,
5.0 GB: release and debug incremental caches of `bevy_flora` and its examples, the release example binaries, `doc`,
`package/`); `df -h /` 20 GB free at the start, 13 GB at the lowest (the packaged build and the debug test
binaries), 22 GB after. Kept: the target directory's dependency builds (71 GB, older than this pass). Leftovers of
earlier passes: none found (no `f*_base` in `~` or `/tmp`); the branches `loop/iter-1` to `-11` remain on origin, not
deleted. Process slip, not repeated: one edit to `qa/visual_findings.md` (a column of its last line) was made with a
short `python3` script instead of `edit`.

## Iteration 13 — 2026-11-25 — A `.tree.yaml` is a tree: the `yaml` feature, `TreeParamsYamlLoader`, `to_yaml`, `from_yaml` (#95)

Pass on `loop/iter-13` from `main` `42ec6a4`, Apple M5 (Metal; no `gpu-tenant` on this Mac). **No log entry exists for
iteration 12** (the hostile-parameters pass, commits `213486a` to `8637dee`, and `b2b0160`, the nextest note): its notes
are `docs/research/hostile-parameters.md` and the commits. This entry is 13 because `loop/iter-12` is taken.
Home-still at the start: `system_status` healthy (19 543 documents, 19 387 embedded), `scribe_health` ok
(`0.0.1-rc.393`, RTX 3090), `distill_search` worked; `catalog_recent` (200 rows, 2026-11-17 to 2026-11-20) held no tree,
plant, foliage, L-system, phenology, fire, impostor, wind or felling document that `corpus.md` lacked. One stem the note
uses (`10.1109_38.736469`, Lintermann and Deussen 1999) was added with the two web sources (`Research: corpus`).

**Triage.** `gh issue list` on `bevy-coven/bevy_flora`: one open issue, **#95 "A YAML loader for TreeParams
(.tree.yaml)"**, labels `enhancement`, `consumer`, `consumer:bevy_dark_factory`, opened 2026-11-11: no bug is open, so a
`consumer` issue is the pass's item (section 3). `wiggum/parked-pass-9-20261102-054127` is the 160-line `zz_probe.rs` that
iteration 12 already adopted: stale, ignored. `git log` since iteration 11: iteration 12's commits only. Read the
requester (not built, not run): `bevy_dark_factory`'s `ops_tree.rs` and `forest_view.rs` read a `TreeParams` through
`blueprint_model::yaml` (`serde_yaml_ng::to_string` / `Deserializer::from_str`, `serde_yaml_ng = "0.10"`), pin this crate
at `25e956b` with `features = ["serde"]`, and write `trees/<name>.tree.yaml` atomically.

**What changed.** Behind a new feature `yaml` (`serde` + `serde_yaml_ng 0.10`, independent of `ron`): `TreeParamsYamlLoader`
(an `AssetLoader` for `tree.yaml`, registered by the plugin beside the RON one), `TreeParams::to_yaml()` (exactly
`serde_yaml_ng::to_string`, so the text equals the editor's, a crown or trunk style with fields a `!Hull` tag) and
`TreeParams::from_yaml(&str)`; `TreeFileError` is now the error of both formats and exported under either feature; one
`read_text` serves both loaders; `examples/assets/trees/maple.tree.yaml`; `tree_file` takes a `.tree.yaml` and
`--export <preset> --yaml`; CI builds `--features yaml` and `--features ron` alone. Nothing that draws a tree changed.
Runner-up: none filed; the backlog's "glide with no per-tree cost" (iteration 11, item 2) waits.

**Assumed decisions** (owner may override; recorded on #95): the feature is `yaml` and the loader `TreeParamsYamlLoader`
(renaming `TreeParamsLoader` would break `ron` users); one extension, `.tree.yaml`; **an empty file is an error, not a tree
of all defaults** (found by the first test run: `serde_yaml_ng` reads `""` as the defaults because every struct has
`serde(default)`; a watcher can read a file between a non-atomic editor's truncate and write, and the tree on screen must
not flash to the defaults; `{}` is the all-defaults tree); the example keeps `required-features = ["ron"]` (Cargo has no OR)
and reads a YAML tree when `yaml` is on too.

**Sources.** Model: Lintermann and Deussen 1999, "Interactive Modeling of Plants" (`10.1109_38.736469`: "The user edits a
plant type's parameters and runs the simulation to produce the desired geometry", "models can easily be reused and changed").
Shipping game: Unity's "Format of text serialized files" (assets are a subset of YAML, floats written losslessly) and Epic's
Procedural Vegetation Editor ("Save Grower Preset ... written to a data asset that can be loaded"), both web. Technical:
`serde_yaml_ng` 0.10.0's crate page and source (`de.rs`, `ser.rs`), Bevy's `AssetLoader`. Note: `docs/research/tree-yaml.md`.
Measured to settle a conflict between sources: the crate page says "only support the YAML Specification 1.1", the editor's
`Cargo.toml` says 1.2; for a typed `bool` it accepts `true`/`false`/`True`/`False` and refuses `yes`, `no`, `on`, `1`.
Pending index: the three web pages.

**Numbers** (`/tmp/f13`, removed with the scratch).

- Tests: `cargo nextest run --all-features` 378 passed (357 + the 9 new `tree_asset` tests and the 12 `hostile-parameters`
  ones of iteration 12 that the earlier count predates; the new ones are listed in the CHANGELOG), `cargo test --all-features
  --doc` clean, `cargo clippy --all-targets --all-features -- -D warnings` clean, `cargo doc --no-deps --all-features`
  with `-D warnings` clean, `cargo build`, `--features serde`, `--features yaml`, `--features ron` and `--release` clean,
  `cargo build --examples` with and without `--all-features` clean, `cargo package --locked` (135 files) clean. The first
  test run found the empty-file behaviour above; the research note records every behaviour it measured (alias bomb and
  10 000-level nesting are `invalid type: sequence, expected struct TrunkParams at line 1 column 8`, not the parser's own
  limits; singleton-map enums refused; two documents refused).
- End to end: `tree_file --tree trees/maple.tree.yaml --screenshot` loads through the real asset server and draws the maple
  (`/tmp/f13/after/tree_yaml.png`, read: HUD "loaded, built 1 time(s)"); against `maple.tree.ron` below the HUD the
  difference is 0.07 of 255 on average, against 0.69 between two runs of the RON file (the example's clock and wind are not
  deterministic), so the two loaders draw one tree; a file with `hieght` logs the field, the fields that exist and the line.
- Gallery, the 7 scenarios of CI's GPU job (no tree-building file changed, so not the 526), base and new, CPU and `--gpu`,
  Apple M5: 7 PNG = 7 rows in each of 5 runs, none under 100 KB, flags `-` x 7, no `BLANK_CAPTURE`/`CAPTURE_TIMEOUT`, only
  the known `WARN`s (no `assets` folder, winit `Destroyed`); every PNG byte for byte the main binary's (CPU and GPU), run 2
  equals run 1, `stats.tsv` columns 1-10 identical in all five; `compare_gallery` 0.218 mean of means, worst 0.922, as base.
- Forest bench, this host at load 14 to 46 (other loops build on it), so a 5% gate is not readable: the same binary spanned
  6.2 to 11.4 ms median in six runs. Pool 10 000, ABBA interleaved, 6 + 6, medians of six, base -> new: mean 8.12 -> 8.22
  ms/frame (+1.2%), median 8.07 -> 7.93 (-1.7%), p95 10.71 -> 11.95, cpu 20.55 -> 20.48 (-0.3%); the quieter last four runs:
  base 6.75, 6.20 against new 6.59, 6.37. gpu-live 1 000, one pair: base 437 (median 386) against new 308 (262) ms/frame, the
  spread of one binary at this load. naive 1 000 was not run in this pass (5 min a run and unreadable at this load); no code
  of those paths changed: the diff to `src/` is `tree_asset.rs` (yaml-gated) and the `lib.rs` exports.
- Clips: all eleven re-recorded for 0.47.1 (`AGENTS.md` recipe, `/tmp/f13/clips.sh`, `encode.sh`): 8 are byte for byte the
  committed files (`seasons`, `shake`, `fell`, `fire`, `push`, `chop`, `harvest`, `playground`), `growth` (4 956 083 ->
  4 963 997 B) and `grove` (6 040 984 -> 6 062 838) changed because growing trees changed with iteration 9's leaf ease, and
  `isometric` by 78 bytes of 7.0 MB (not investigated). `ffprobe`: sizes and frame counts as before (480x360 24 fps 192;
  560x315 20 fps 160; 560x315 16 fps 128). First, middle and last frame of the three changed ones read.

**Critique and verdict** (`/tmp/f13/critique.md`): **GREAT** for what it promises (the loader, the two functions, a
feature that does not pull RON, the editor's text read as written). Flaws, none stopping a ship: `to_yaml` has an `expect`
like `to_ron` (cannot fail for this type; round-trips of every preset and of `NaN`/`inf` are tested); an empty file is
refused but a file cut at a line is not (block YAML stays valid), so non-atomic writers can still flash a part-tree (the
Cauldron writes atomically; the guide says write-then-rename); `unsafe-libyaml` joins the tree under `yaml` (opt-in, the
editor's choice); the example needs `ron`; the shipped maple YAML is the older RON export, not `TreePreset::Maple.params()`;
no visual change, so no clip shows it; the forest bench is noise-limited; hot reload through the real `file_watcher` was not
run with a window, only through `AssetServer::reload` (the watcher calls it).

**Commits** (`loop/iter-13`): `54d56a7` Research: corpus, `e9cd554` Research: .tree.yaml, `3968d75` the implementation,
`164c9ac` docs. Sibling repositories: none touched; #95's requester (`bevy_dark_factory#365`) moves its pin to a `main`
commit that contains this.

### Ranked backlog after iteration 13

1. **A glide with no per-tree cost** (iteration 11's item 2): a leaf marker so only leaf meshes are tagged, or a shared
   slot in the buffer for trees that began together; and the leaves a glide loses falling (`LeafBurst`).
2. **The hull crown by leaf size**: `crown_hull` takes positions only, so a `CrownStyle::Hull` tree growing on the CPU
   still pops its hull when a leaf is born.
3. **Finish the willow (D-052)**: leaf ribbons, strands kept as lines at level 1, limbs whose leaves reach their tips, a
   greener strand colour, a forked trunk; D-074 / D-075 on big; `RadialOutward` facing on a growing branch.
4. **What an editor still hand-rolls around `TreeParams`** now that it writes `.tree.yaml`: per-field kind, unit and range
   for a panel (the ranges are in `limits.rs`) and a dotted-path edit; wait for the requester to file it. Also Sanctuary's
   rain-and-fire rule (`douse_forest`): only a hook if two consumers want the same rule.
5. **A YAML-only `tree_file`** (it requires `ron` today) and a shipped `maple.tree.yaml` that is `TreePreset::Maple.params()`.
