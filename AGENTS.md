# AGENTS.md

Guidance for coding agents working on `bevy_flora`. Project layout, commands and conventions are in `CLAUDE.md`; the design is in `ARCHITECTURE.md`.

## Releasing

Every release, patch releases included, does all of these in the release PR.

1. **Docs.** Add a `## [x.y.z]` entry to `CHANGELOG.md` above the previous one, with its compare link at the bottom. Update the `README.md` compatibility table and install snippet (`0.x`), the gallery scenario count in `README.md` and any `///` docs for behaviour that changed. Update every frame time quoted in `README.md`, `src/lib.rs`, `src/pool.rs` and `ARCHITECTURE.md` to the numbers in the new CHANGELOG entry. Commit the refreshed `Cargo.lock` (CI runs `cargo package --locked`).
2. **Re-record ALL ELEVEN README clips, `docs/media/growth.gif`, `docs/media/grove.gif`, `docs/media/seasons.gif`, `docs/media/shake.gif`, `docs/media/fell.gif`, `docs/media/fire.gif`, `docs/media/push.gif`, `docs/media/chop.gif`, `docs/media/harvest.gif`, `docs/media/playground.gif` and `docs/media/isometric.gif`. Never skip this, even when the release looks like it cannot change them.** The clips must show the code being released: the growth clip was left showing the 0.1.0 square leaves through 0.2.1. From the repo root, frames first:
   ```sh
   rm -rf target/capture/growth target/capture/grove target/capture/seasons target/capture/shake target/capture/fell target/capture/fire target/capture/push target/capture/chop target/capture/harvest target/capture/playground target/capture/isometric; cargo run --release --example capture_growth -- --preset maple --out target/capture/growth; cargo run --release --example capture_growth -- --grove --out target/capture/grove; cargo run --release --example capture_growth -- --seasons --out target/capture/seasons; cargo run --release --example capture_growth -- --shake --out target/capture/shake; cargo run --release --example capture_growth -- --fell --out target/capture/fell; cargo run --release --example capture_growth -- --fire --out target/capture/fire; cargo run --release --example capture_growth -- --push --out target/capture/push; cargo run --release --example capture_growth -- --chop --out target/capture/chop; cargo run --release --example capture_growth -- --harvest --out target/capture/harvest; cargo run --release --example playground -- --demo; cargo run --release --example isometric -- --demo
   ```
   then encode them (`fire`, `push`, `chop` and `harvest` are "encoded like the grove"; `playground` and `isometric` have their own lines, copied from their examples' module docs):
   ```sh
   ffmpeg -y -framerate 24 -i target/capture/growth/frame_%04d.png -vf "fps=24,scale=480:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" docs/media/growth.gif
   ffmpeg -y -framerate 24 -i target/capture/grove/frame_%04d.png -vf "fps=20,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" docs/media/grove.gif
   ffmpeg -y -framerate 24 -i target/capture/seasons/frame_%04d.png -vf "fps=20,scale=560:-1:flags=lanczos,palettegen=max_colors=80:stats_mode=diff" -update 1 target/capture/seasons_clip.png
   ffmpeg -y -framerate 24 -start_number 136 -i target/capture/seasons/frame_%04d.png -vf "trim=end_frame=28,fps=20,scale=560:-1:flags=lanczos,crop=200:150:320:150,format=gbrp,geq=r='if(gt(r(X\,Y)-g(X\,Y)\,18)*gt(r(X\,Y)\,150)\,r(X\,Y)\,0)':g='if(gt(r(X\,Y)-g(X\,Y)\,18)*gt(r(X\,Y)\,150)\,g(X\,Y)\,0)':b='if(gt(r(X\,Y)-g(X\,Y)\,18)*gt(r(X\,Y)\,150)\,b(X\,Y)\,0)',format=rgb24,palettegen=max_colors=16:stats_mode=full" -update 1 target/capture/seasons_pink.png
   ffmpeg -y -i target/capture/seasons_clip.png -i target/capture/seasons_pink.png -filter_complex "[0]crop=16:5:0:0[a];[1]crop=16:1:0:0[b];[a][b]vstack,pad=16:16:0:0:black" -update 1 target/capture/seasons_palette.png
   ffmpeg -y -framerate 24 -i target/capture/seasons/frame_%04d.png -i target/capture/seasons_palette.png -filter_complex "[0]fps=20,scale=560:-1:flags=lanczos[x];[x][1]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" docs/media/seasons.gif
   ffmpeg -y -framerate 24 -i target/capture/shake/frame_%04d.png -vf "fps=20,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" docs/media/shake.gif
   ffmpeg -y -framerate 24 -i target/capture/fell/frame_%04d.png -vf "fps=20,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" docs/media/fell.gif
   ffmpeg -y -framerate 24 -i target/capture/fire/frame_%04d.png -vf "fps=20,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" docs/media/fire.gif
   ffmpeg -y -framerate 24 -i target/capture/push/frame_%04d.png -vf "fps=20,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" docs/media/push.gif
   ffmpeg -y -framerate 24 -i target/capture/chop/frame_%04d.png -vf "fps=20,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" docs/media/chop.gif
   ffmpeg -y -framerate 24 -i target/capture/harvest/frame_%04d.png -vf "fps=20,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle" docs/media/harvest.gif
   ffmpeg -y -framerate 24 -i target/capture/playground/frame_%04d.png -vf "fps=16,scale=560:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=64:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" docs/media/playground.gif
   ffmpeg -y -f lavfi -i color=c=0xC81C12:s=560x315 -frames:v 1 target/capture/isometric_red.png
   ffmpeg -y -framerate 24 -i target/capture/isometric/frame_%04d.png -i target/capture/isometric_red.png -filter_complex "[0]fps=16,scale=560:-1:flags=lanczos[a];[1]loop=loop=8:size=1[r];[a][r]concat=n=2:v=1,palettegen=max_colors=64:stats_mode=full" target/capture/isometric_palette.png
   ffmpeg -y -framerate 24 -i target/capture/isometric/frame_%04d.png -i target/capture/isometric_palette.png -lavfi "fps=16,scale=560:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" docs/media/isometric.gif
   ```
   Sizes, measured with `ffprobe` on the committed GIFs: `growth.gif` is 480x360, 24 fps, 192 frames (8 s), about 5 MB; `grove.gif`, `seasons.gif`, `shake.gif`, `fell.gif`, `fire.gif`, `push.gif`, `chop.gif` and `harvest.gif` are 560x315, 20 fps, 160 frames, 3 to 6 MB each; `playground.gif` and `isometric.gif` are 560x315, 16 fps, 128 frames, about 7 MB each. Keep those. The clips are deterministic (fixed clock), so on one machine a re-record of unchanged code comes out frame-identical; another GPU may differ by a level here and there. Check the results (`ffprobe` for the numbers, and look at the first, a middle and the last frame of each) and commit all eleven in the release PR. `docs/` is not in the crates.io package; the README embeds the clips and `CLAUDE.md` documents from a relative path (`docs/media/<name>.gif`), so they go live when the PR merges to `main` and crates.io serves them from the repository.
3. **Lint and test**, all clean: `cargo clippy --all-targets --all-features -- -D warnings`, `cargo nextest run --all-features`, `cargo test --all-features --doc`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`, `cargo build --examples`, `cargo publish --dry-run --allow-dirty`.
4. **Visual QA** when anything about the look changed: the `bevy-flora-visual-qa` skill (`qa/visual_findings.md`).
5. **Version.** Bump `version` in `Cargo.toml`; `Cargo.lock` refreshes on the next build; commit it in the release PR (CI's `cargo package --locked` fails otherwise). Before 1.0 a minor bump may break the API.
6. **PR and merge.** Open a PR to `main`, wait for CI, merge with a merge commit. **A loop pass skips
   this**: it publishes locally from the "Release" block of its own `.omp/commands/build-loop.md` and
   never waits on CI (owner, 2026-10-29).
7. **Publish from a clean checkout of the merged `main`**, not from a working tree with other edits:
   ```sh
   git fetch origin && git worktree add --detach ../bevy_flora-release origin/main
   cd ../bevy_flora-release && cargo publish
   ```
   Then tag the merge commit (`git tag -a vX.Y.Z -m "bevy_flora X.Y.Z" <merge-sha> && git push origin vX.Y.Z`), run `gh release create vX.Y.Z`, and `git worktree remove ../bevy_flora-release`.

`cargo` is already logged in to crates.io through `~/.cargo/credentials.toml`; no repo holds a token, so pass none and never print it.

## Docs and media

README and docs follow https://github.com/dark-incorporated/.github/blob/main/DOCUMENTATION.md; a feature is not done until its README line and, if visible, its GIF exist. A feature's detail goes in `docs/guide/<page>.md` (index: `docs/guide/README.md`), a picture in `docs/gallery.md` and in the guide page beside the text that explains it, and every media file is listed in `docs/media/README.md` with its command, date and version.
