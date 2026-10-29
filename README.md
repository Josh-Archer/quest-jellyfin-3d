# quest-jellyfin-3d
Free immersive 3D Jellyfin player for Meta Quest 3+ (SBS/OU, 180/360, MV-HEVC).

Roadmap: [#199](https://github.com/Josh-Archer/quest-jellyfin-3d/issues) (originally DeskQuest#199).

## Layout
- `crates/jellyfin-player`: app crate.
- Reuses DeskQuest's `xr-core` and `xr-video` (from DeskQuest#198) as git dependencies pinned to
  `8a2b13d9afc1b61deefe480dc16d11cf879f3021`. Bump the `rev` in `Cargo.toml` to pick up changes.

## CI
Runs on the ARC scale set `home-gh-runner-quest-jellyfin-3d` (Josh-Archer/home PR #3956).
DeskQuest is private, so CI needs a read-only deploy key on DeskQuest with its private half stored here
as the `DESKQUEST_DEPLOY_KEY` secret.
