# Frontier: Elite II (Rust)

A Rust port of the 1993 space sim. The game is the `frontier` binary: a 640×400
software framebuffer (the DOS 320×200 screen at 2×), the station screens, and
the flight model.

The Python files still in the tree are the earlier attempt. They are not the
game being built. Behaviour is matched to the original DOS game and its manual,
not to that Python code.

## Run

```bash
cargo run
```

Starts on the title screen. Enter or 1 starts at Sirocco Station with ¢100.0.

## Tests

```bash
cargo test --workspace
```

## Layout

- `crates/frontier` is the window, the screens, and the renderer.
- `crates/frontier-sim` is the galaxy, market, shipyard, and flight model.
- `data/` is the shared JSON: ships, goods, the Sirocco day-one market, and notes on what was read off the original.
