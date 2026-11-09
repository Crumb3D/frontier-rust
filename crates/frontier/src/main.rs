//! Frontier: Elite II in Rust. A 640x400 software framebuffer (the DOS 320x200
//! screen at 2x) shown in a scalable window.
//!
//! `frontier --shot <scene> <out.png>` renders one scene headless, for comparing
//! against the original's stills. Scenes: title, landed, comms, launchreq,
//! inventory, profile, roster, stock, buy, bulletin, video, federal, shipyard,
//! upgrades, repairs, ships, police, takeoff, climb, station, dock, land, space,
//! map, system, jump.

mod assets;
mod console;
mod fb;
mod font;
mod game;
mod music;
mod grf;
mod meshes;
mod palette;
mod render3d;
mod screens;

use game::{Game, H, W};
use pixels::{Pixels, SurfaceTexture};
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

struct App {
    game: Game,
    cv: fb::Canvas,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    last: Instant,
    mods: ModifiersState,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Frontier: Elite II")
            .with_inner_size(LogicalSize::new(W as f64 * 2.0, H as f64 * 2.0))
            .with_min_inner_size(LogicalSize::new(W as f64, H as f64));
        let window = Arc::new(el.create_window(attrs).expect("create window"));
        if let Some(icon) = window_icon() {
            window.set_window_icon(Some(icon));
        }
        let size = window.inner_size();
        let surface = SurfaceTexture::new(size.width, size.height, window.clone());
        let pixels = Pixels::new(W as u32, H as u32, surface).expect("create pixel buffer");
        self.window = Some(window);
        self.pixels = Some(pixels);
        self.last = Instant::now();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                if let Some(p) = &mut self.pixels {
                    if let Err(e) = p.resize_surface(size.width.max(1), size.height.max(1)) {
                        log::error!("resize: {e}");
                    }
                }
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed if !event.repeat => self.game.key_down(code, self.mods.shift_key()),
                        ElementState::Pressed => {}
                        ElementState::Released => self.game.key_up(code),
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(p) = &self.pixels {
                    let (x, y) = match p.window_pos_to_pixel((position.x as f32, position.y as f32)) {
                        Ok((x, y)) => (x as i32, y as i32),
                        Err((x, y)) => (x as i32, y as i32),
                    };
                    self.game.mouse_move(x, y);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 120.0,
                };
                self.game.wheel(lines);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => self.game.mouse_button(true, down),
                    MouseButton::Right => self.game.mouse_button(false, down),
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last).as_secs_f64().min(0.1);
                self.last = now;
                self.game.update(dt);
                self.game.draw(&mut self.cv);
                if let Some(p) = &mut self.pixels {
                    p.frame_mut().copy_from_slice(&self.cv.px);
                    if let Err(e) = p.render() {
                        log::error!("render: {e}");
                        el.exit();
                    }
                }
                if self.game.quit {
                    el.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn window_icon() -> Option<winit::window::Icon> {
    let img = image::open(frontier_sim::paths::assets_dir().join("icon.png")).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    winit::window::Icon::from_rgba(img.into_raw(), w, h).ok()
}

fn run_for(game: &mut Game, seconds: f64) {
    let dt = 1.0 / 30.0;
    for _ in 0..(seconds / dt).round() as usize {
        game.update(dt);
    }
}

fn hold(game: &mut Game, key: KeyCode, seconds: f64) {
    game.key_down(key, false);
    run_for(game, seconds);
    game.key_up(key);
}

fn shot(scene: &str, out: &str) -> Result<(), String> {
    let mut game = Game::new(assets::Assets::load()?)?;
    match scene {
        "title" => run_for(&mut game, 120.0),
        "landed" => {
            game.new_game();
            run_for(&mut game, 5.0);
        }
        "comms" => {
            game.new_game();
            game.key_down(KeyCode::F4, false);
        }
        "inventory" | "profile" | "roster" => {
            game.new_game();
            if scene == "roster" {
                game.shot_page("bulletin");
                game.shot_hit("ad", 11);
                game.shot_hit("reply", 0);
            }
            game.key_down(KeyCode::F3, false);
            if scene != "inventory" {
                game.key_down(KeyCode::F3, false);
            }
            if scene == "roster" {
                game.key_down(KeyCode::F3, false);
            }
            run_for(&mut game, 1.0);
        }
        // Station pages, e.g. `stock`, `upgrades`; `video` answers Betty Morris's
        // advert and asks how many there are.
        "stock" | "bulletin" | "shipyard" | "upgrades" | "repairs" | "ships" | "police" | "federal" => {
            game.new_game();
            game.shot_page(scene);
        }
        "buy" => {
            game.new_game();
            game.shot_page("stock");
            game.shot_hit("buy", 0);
        }
        "view" => {
            game.new_game();
            game.shot_page("ships");
            game.shot_hit("sell", 0);
        }
        "video" => {
            game.new_game();
            game.shot_page("bulletin");
            game.shot_hit("scroll", 6);
            game.shot_hit("ad", 3);
            game.shot_hit("reply", 1);
        }
        "launchreq" => {
            game.new_game();
            game.key_down(KeyCode::F4, false);
            game.key_down(KeyCode::Digit1, false);
        }
        "takeoff" | "climb" => {
            game.new_game();
            game.key_down(KeyCode::F4, false);
            game.key_down(KeyCode::Digit1, false);
            game.key_down(KeyCode::F1, false);
            game.key_down(KeyCode::F7, false);
            run_for(&mut game, 12.0);
            if scene == "climb" {
                hold(&mut game, KeyCode::KeyZ, 0.5);
                hold(&mut game, KeyCode::Enter, 2.5);
                run_for(&mut game, 40.0);
            }
        }
        "map" => {
            game.new_game();
            game.key_down(KeyCode::F2, false);
        }
        "system" => {
            game.new_game();
            game.key_down(KeyCode::F2, false);
            game.key_down(KeyCode::F2, false);
        }
        // Centre the next system east, then jump. The status line names where we arrive.
        "jump" => {
            game.new_game();
            game.key_down(KeyCode::F2, false);
            game.key_down(KeyCode::ArrowRight, false);
            game.key_down(KeyCode::KeyH, false);
        }
        "space" => {
            game.new_game();
            game.place_above_port(9000.0);
        }
        "station" => {
            game.new_game();
            game.place_near_station(4.0);
            run_for(&mut game, 1.0);
        }
        // Request docking permission over F4, then creep into the slot.
        "dock" => {
            game.new_game();
            game.place_near_station(3.0);
            game.key_down(KeyCode::F4, false);
            game.key_down(KeyCode::Digit1, false);
            game.key_down(KeyCode::F1, false);
            hold(&mut game, KeyCode::Enter, 2.0);
            run_for(&mut game, 90.0);
        }
        // Take off, ask for landing permission, then drop back with Engines Off.
        "land" => {
            game.new_game();
            game.key_down(KeyCode::F4, false);
            game.key_down(KeyCode::Digit1, false);
            game.key_down(KeyCode::F1, false);
            game.key_down(KeyCode::F7, false);
            run_for(&mut game, 2.5);
            game.key_down(KeyCode::F4, false);
            game.key_down(KeyCode::Digit1, false);
            game.key_down(KeyCode::F1, false);
            game.key_down(KeyCode::F7, false);
            run_for(&mut game, 20.0);
        }
        other => return Err(format!("unknown scene {other}")),
    }
    let mut cv = fb::Canvas::new(W, H);
    game.draw(&mut cv);
    cv.to_image().save(out).map_err(|e| format!("save {out}: {e}"))?;
    println!("{scene}: {} -> {out}", game.stardate.format());
    println!("  {}", game.status_line());
    if std::env::var_os("FRONTIER_SKY").is_some() {
        println!("  {}", game.sky_report());
    }
    Ok(())
}

fn main() {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 4 && args[1] == "--shot" {
        if let Err(e) = shot(&args[2], &args[3]) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }

    let assets = match assets::Assets::load() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Could not load assets: {e}");
            std::process::exit(1);
        }
    };
    let game = match Game::new(assets) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Could not load game data: {e}");
            std::process::exit(1);
        }
    };
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        game,
        cv: fb::Canvas::new(W, H),
        window: None,
        pixels: None,
        last: Instant::now(),
        mods: ModifiersState::empty(),
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("{e}");
    }
}
