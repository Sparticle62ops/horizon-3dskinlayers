# 3D Skin Layers for Horizon

Behavioral clean-room port of [3D Skin Layers](https://modrinth.com/mod/3dskinlayers) by tr7zw for [Horizon](https://github.com/horizon-rust/horizon). Upstream is licensed under LicenseRef-tr7zw-Protective-License, so this port contains **no upstream code or assets**; it reimplements the observable behavior and is released under CC0-1.0. Ported by sparticle.

## What it does

Renders the skin's **second layer as an extruded 3D shell** instead of a flat billboard:

- **Pixel volume model**: the standard 64x64 skin overlay layout is parsed into per-part shells (head 8x8x8, body 8x12x4, arms and legs 4x12x4), with each face sampling its atlas region.
- **Greedy quad merging**: adjacent opaque overlay pixels merge into the fewest quads (a fully drawn face is one quad), keeping vertex count low.
- **Per-part toggles**: head, body, arms, legs can each be enabled or disabled, plus a master switch.
- **First-person policy**: the shell on the player's own head is hidden in first person by default, matching upstream, with an opt-in.
- **Shell animation**: a wobble/breathing phase advances per tick when animations are on.

## Degradations (backend/render seams)

1. **No client skin-layer render pass seam**: Horizon has no hook to replace a body part's mesh with extruded geometry at draw time, so the port ships the full geometry and policy logic (tested) while the draw call stays stock. Tracked on the Horizon tracker; the port upgrades to full 1:1 when the seam lands.
2. **No config screen seam**: the toggle set is exposed through the mod's config API rather than an in-game settings screen.

## Host seams requested

- `crates/horizon-client/src/game.rs`: per-part mesh override hook keyed on the skin texture, with the skin overlay passed to native mods.

## Tests

`cargo test`: 16 tests cover config gating, atlas layout, face extraction, run merging, and quad budget accounting.

## License

CC0-1.0 (this port). 3D Skin Layers is (c) tr7zw under its own license.
