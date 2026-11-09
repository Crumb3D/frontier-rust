# Learning Rust with Frontier

This port is structured so each phase teaches a Rust concept while you keep a playable game growing.

| Phase | What you build | Rust concepts |
|-------|----------------|---------------|
| 0 Scaffold | Cargo workspace, window | crates, `mod`, `Cargo.toml`, `Result` |
| 1 Sim core | Kepler, clock, frames, FAQ formulas | `struct`, `enum`, ownership, unit tests |
| 2 Soft gfx | Framebuffer, console, GRF, font | modules, borrowing buffers, crates |
| 3 Vertical slice | Title → dock → fly → dock | state machines, input, game loop |
| 4+ Full game | Market, jump, combat, audio | traits, error handling, larger modules |

## Mental model vs Python

- `dataclass` → `struct` with owned fields
- `None` / exceptions → `Option` / `Result`
- numpy arrays → `glam::Vec3` / `Mat3`
- pygame Surface pixels → `&mut [u8]` RGBA buffer
- class methods mutating `self` → `&mut self` methods

Keep the Python tree open as a behavior reference. Diff numbers in tests first, then visuals against `assets/` and `Frontier - PC ORIGINAL FOR REFERENCE/`.
