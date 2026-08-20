# 06 — Subsurface + Viewporter Support

## Objective

Enable programs that compose via subsurfaces and popups (GTK/Qt menus, tooltips,
toolkits) and viewport scaling. This is the biggest app-compat win and a real
renderer refactor: the renderer must walk the surface's subsurface tree and
composite each subsurface at its stacked position, not blit a single
per-window buffer.

## Files

| File | Change |
|------|--------|
| `crates/server/src/state.rs` | Add `SubcompositorState` + `ViewporterState` globals; `delegate_subcompositor!` / `delegate_viewporter!`; track subsurface state per surface. |
| `crates/server/Cargo.toml` | Ensure smithay features for subsurface/viewporter are enabled (they are part of the default delegate set — confirm). |
| `crates/server/src/rendering/mod.rs` | Replace the single-buffer blit with a subsurface-tree walk: for a window's root `WlSurface`, enumerate subsurfaces (sync/async, stacked), import each committed buffer, render at its position with clipping. |
| `crates/server/src/window.rs` | Render path keyed on the toplevel's `WlSurface`, not a stored buffer. |
| `crates/server/src/state.rs` (`commit`) | Update `SurfaceInfo` to capture the full surface tree, or query it live at render time via `with_states`. |

## Steps

1. Add the globals in `State::new` (state.rs:303 area): `SubcompositorState::new::<State>(&dh)` and `ViewporterState::new::<State>(&dh)`; add the fields to `State`; add `delegate_subcompositor!(State)` and `delegate_viewporter!(State)` at the bottom. Confirm the handler traits have default impls (no extra methods needed for a headless compositor).
2. Switch the per-window render entrypoint from "stored buffer" to "root surface": `render_window(window_id)` resolves the toplevel `WlSurface` and renders the tree.
3. Implement the tree walk in `rendering/mod.rs`: for a root surface, use smithay's subsurface enumeration (`with_states(surface, |st| st.cached_state.get::<SurfaceAttributes>())` + the subsurface list / `SubsurfaceCachedState`) to collect `(buffer, x, y, z)` for the root + each mapped subsurface; import each as a texture and `render_texture_at` in stacking order. Clip to the window rect.
4. Handle `wp_viewporter` / `wp_single_pixel_buffer` / `viewporter` crop+scale: read the viewport src/dst from surface cached state and apply it to the texture draw.
5. Preserve the existing `render_surface` (single-buffer) path as a fallback for windows with no subsurfaces, or unify on the tree walk (walk of depth 1 = the old behavior).
6. Update existing render/streaming tests to still pass (they use a single root commit, which the walk handles as a degenerate tree).

## Verification

- `cargo test -p wayland-remote-server` green; the render snapshot test still produces the expected pattern.
- New test: a client that creates a root surface + a subsurface with a different color, commits both; assert the rendered frame shows both regions at the correct stacked positions.
- Live on gary-agents: a GTK or Qt popup app (or a weston client that uses subsurfaces) renders its popup/tooltip. Confirm `weston-clickdot` still renders after the refactor.
- `lat check` green; add a `lat.md/` architecture note on the subsurface render walk.
