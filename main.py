"""Frontier: Elite II — Python recreation main loop."""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import numpy as np
import pygame

from audio.music import MusicPlayer
from audio.sfx import SfxPlayer
from render.chart_cam import ChartCamera
from render.cinematics import draw_docking, draw_hyperspace
from render.console_bar import hit_console
from render.fonts import load_frontier
from render.hud import console_height, draw_compass, draw_hud, draw_scanner
from render.maps import draw_galactic_chart, draw_orrery
from render.orrery3d import OrreryCamera, body_world_km
from render.meshes import mesh_for_ship, slot_station, sphere
from render.soft import draw_mesh, draw_sphere_approx, draw_sun
from render.surface import draw_surface_approach
from render.intro import IntroFilm
from render.title import draw_start
from sim.ai import AIController, spawn_traffic
from sim.clock import Stardate, TimeCompression
from sim.combat import CombatWorld, Combatant
from sim.flight import FlightModel
from sim.frame import FrameState
from sim.galaxy import Galaxy, build_frame_system
from sim.market import Market
from sim.mission import MissionBoard
from sim.nav import (
    COL_MOON,
    COL_PLANET,
    COL_STAR,
    COL_STATION,
    COL_STARPORT,
    ap_speed_for_distance,
    draw_direction_markers,
    draw_targeting_tunnel,
    edge_clamp,
    project_body,
    steer_toward,
)
from sim.shipyard import (
    FittedShip,
    Shipyard,
    apply_equipment_flags,
    default_eagle,
    fitted_to_stats,
)
from ui.screens import DockedUI

SAVE_PATH = Path(__file__).resolve().parent / "savegame.json"
SETTINGS_PATH = Path(__file__).resolve().parent / "settings.json"
# Amiga FE2 was 320x200 — we render 640x400 and integer-scale for the same chunky look
WIDTH, HEIGHT = 640, 400


class Game:
    def __init__(self) -> None:
        pygame.mixer.pre_init(44100, -16, 2, 1024)
        pygame.init()
        pygame.display.set_caption("Frontier: Elite II")
        # SCALED = nearest-neighbour upscale to desktop, FE2 CRT feel
        self.screen = pygame.display.set_mode((WIDTH, HEIGHT), pygame.SCALED | pygame.RESIZABLE)
        self.clock = pygame.time.Clock()
        # Authentic Frontier font (assets/Frontier.ttf)
        self.font_huge = load_frontier(36)
        self.font = load_frontier(14)
        self.small = load_frontier(11)
        self.title_t = 0.0
        self.music = MusicPlayer()
        self.sfx = SfxPlayer()
        self.time_paused = False
        self._music_mode: str | None = None
        self.console_hits = None
        self.settings_sel = 0
        self._pause_return = "docked"
        self._pause_click_at = 0
        self.map_tools = False
        self.map_target = False
        self.detail_level = 2
        self.music_on = [True] * 8
        self.opt = {
            "sfx": True, "engine": True, "station": True, "wind": True, "beeps": True,
            "music": True, "music_hs": True, "music_dock": True, "music_battle": False, "music_cont": False,
            "dust": True, "stars": True, "lock_ext": True, "rev_lr": True, "rev_ud": False,
            "elite_ctrl": False, "tunnels": False, "ship_tunnels": False, "filenames": True,
        }
        self._opt_hits: list[tuple[str, str | int, pygame.Rect]] = []
        self._load_settings()

        self.galaxy = Galaxy()
        self.market = Market()
        self.shipyard = Shipyard()
        self.missions = MissionBoard(self.galaxy, self.market)
        self.stardate = Stardate(days=0.0)
        self.compression = TimeCompression()

        self.system_id = "ross154"
        self.system = self.galaxy.get(self.system_id)
        self.frames = build_frame_system(self.system)

        self.fitted = default_eagle()
        self.credits = 100
        self.legal = True
        self.fines = 0
        self.kills = 0
        self.mb4_sites = []
        self.launch_clearance = False
        self.clearance_expires_at = 0.0
        self.UNAUTHORIZED_LAUNCH_FINE = 400
        self._launch_grace_until = 0.0  # wall-clock: ignore auto-land after takeoff

        # Start at Sirocco Starport on Merlin (FE2 intro) — moon of Aster
        self.docked = True
        self.dock_location = "ross154_sirocco"
        self.mode = "title"  # title | docked | flight | orrery | chart | paused
        self.god_mode = False
        self.infinite_fuel = False
        self.free_purchases = False
        self._free_purchases_armed = False
        self.cheat_jump_ly: float | None = None
        self.cheat_sel = 0
        self._cheat_return = "flight"

        stats = apply_equipment_flags(
            fitted_to_stats(self.fitted, self.shipyard.ships, self.shipyard.equipment),
            self.fitted,
            self.shipyard.equipment,
        )
        self.flight = FlightModel(stats)
        self.player_state = FrameState(
            parent_id=self.dock_location,
            position=np.array([0.0, 0.2, 0.0], dtype=np.float64),
        )

        self.combat = CombatWorld()
        self._rebuild_player_combatant()
        self.ai_controllers: list[AIController] = []
        self.board_missions = self.missions.generate(self.system, int(self.stardate.days))
        self.ui = DockedUI()
        self.messages: list[str] = [
            "Sirocco Starport, Merlin — moon of Aster, Ross 154.",
            "1 Jan 3200",
        ]
        self.selected_system: str | None = None
        self.selected_body: str | None = None
        self.chart = ChartCamera()
        self.orrery = OrreryCamera()
        self.chart_rot_y = 0.0  # kept for save compat; prefer self.chart
        self.chart_rot_x = 0.3
        self.target_id: str | None = None
        self.ap_body_id: str | None = None  # local system destination for autopilot
        self.ap_enabled: bool = False
        self.laser_cool = 0.0
        self.mouse_look = True
        pygame.event.set_grab(False)
        pygame.mouse.set_visible(True)

        self.station_mesh = slot_station(2.0)
        self.planet_mesh = sphere(radius=1.0)
        self.title_menu = 0
        self.title_phase = "intro"  # intro film, then the position list
        self.intro = IntroFilm()
        self._start_idle = 0.0
        self.start_sel = 0
        self._title_rects: list = []
        self._start_rects: list = []
        self._begin_rect = None
        self.fade = 0.0
        self._fade_phase: str | None = None
        self._fade_action = None
        self._quit_requested = False
        self._starts = (
            ("ross154", "ross154_sirocco", "Ross 154"),
            ("sol", "sol_olympus", "Sol"),
            ("lave", None, "Lave"),
        )
        self.landing_gear = True
        self.show_id_text = True
        self.engines_off = False
        self.info_page = 2
        self.system_page = "bodies"
        self.center_mode = "scanner"
        self.hs_clouds: list[dict] = []
        self.mapper_active = False
        self.camera_line = ""
        self.show_hud = True
        self.fullscreen = False
        self.view_mode = "front"  # front | rear | external
        self.ext_yaw = 0.6
        self.ext_pitch = 0.35
        self.ext_zoom = 1.15
        self._esc_armed = 0.0
        # Cinematics: ("hyperspace"|"dock", elapsed, duration, payload_dict)
        self.cinematic: tuple[str, float, float, dict] | None = None
        self.dock_clearance_id: str | None = None

    def _first_dock(self, system_id: str, prefer: str | None = None) -> str:
        sys = self.galaxy.get(system_id)
        if prefer:
            if sys.starport_by_id(prefer):
                return prefer
            for st in sys.stations:
                if st.id == prefer:
                    return prefer
        for p in sys.planets:
            for sp in p.starports:
                return sp.id
        if sys.stations:
            return sys.stations[0].id
        return system_id

    def start_new_game(self, system_id: str, dock_id: str | None = None) -> None:
        """Begin a new career at a start position."""
        self.stardate = Stardate(days=0.0)
        self.compression = TimeCompression()
        self.system_id = system_id
        self.system = self.galaxy.get(system_id)
        self.frames = build_frame_system(self.system)
        self.fitted = default_eagle()
        self.credits = 100
        self.legal = True
        self.fines = 0
        self.kills = 0
        self.mb4_sites = []
        self.launch_clearance = False
        self.clearance_expires_at = 0.0
        self.docked = True
        self.dock_location = dock_id or self._first_dock(system_id)
        stats = apply_equipment_flags(
            fitted_to_stats(self.fitted, self.shipyard.ships, self.shipyard.equipment),
            self.fitted,
            self.shipyard.equipment,
        )
        self.flight = FlightModel(stats)
        self.player_state = FrameState(
            parent_id=self.dock_location,
            position=np.array([0.0, 0.2, 0.0], dtype=np.float64),
        )
        self.combat = CombatWorld()
        self._rebuild_player_combatant()
        self.ai_controllers = []
        self.board_missions = self.missions.generate(self.system, int(self.stardate.days))
        self.ui = DockedUI()
        loc = self.frames.bodies.get(self.dock_location)
        loc_name = loc.name if loc else self.dock_location
        indoors = self._berth_is_indoors()
        self.messages = [
            f"{loc_name}, {self.system.name}.",
            "F4 Comms -> Launch Request, then F7 to take off." if indoors else "Landed. F1 is the view. F4 Comms for services.",
        ]
        self.selected_system = None
        self.target_id = None
        self.ap_body_id = None
        self.ap_enabled = False
        self._open_berth()
        pygame.event.set_grab(False)
        pygame.mouse.set_visible(True)
        self.msg(f"Welcome to {loc_name}.")
        self.chart.look_at(self.system.position_ly)

    def _berth_is_indoors(self, location_id: str | None = None) -> bool:
        """Orbital stations and sealed ports open comms. An open surface pad does not."""
        bid = location_id or self.dock_location
        body = self.frames.bodies.get(bid) if bid else None
        if body is None:
            return False
        if body.is_station:
            return True
        if body.is_starport and body.parent_id:
            parent = self.frames.bodies.get(body.parent_id)
            if parent is not None and parent.atmosphere_km <= 0:
                return True
        return False

    def _open_berth(self) -> None:
        self.docked = True
        self.view_mode = "front"
        if self._berth_is_indoors():
            self.mode = "docked"
            self.ui.flight_links = None
            self.ui.state.tab = "comms"
            self.ui.state.selected = 0
            self.ui.state.scroll = 0
            self.ui._default_prompt()
        else:
            self.mode = "flight"

    def _sync_music(self) -> None:
        if self.docked and self.mode not in ("title", "start"):
            if self._music_mode != "berthed":
                self._music_mode = "berthed"
                self.music.stop()
            indoors = self._berth_is_indoors()
            if not self.opt.get("sfx", True):
                self.sfx.ambience(None)
            elif indoors and self.opt.get("station", True):
                self.sfx.ambience("station")
            elif not indoors and (self.opt.get("engine", True) or self.opt.get("wind", True)):
                self.sfx.ambience("ground")
            else:
                self.sfx.ambience(None)
            return
        self.sfx.ambience(None)
        if self._music_mode != self.mode:
            self._music_mode = self.mode
            if not self.opt.get("music", True):
                self.music.stop()
                return
            allowed = {
                key for key, on in zip(
                    ("docked", "intro", "baba", "bare", "intro2", "flight", "space", "combat"),
                    self.music_on,
                ) if on
            }
            self.music.set_mode(self.mode, allowed, continuous=bool(self.opt.get("music_cont")))

    def _logical_pos(self, pos: tuple[int, int] | None = None) -> tuple[int, int]:
        """Best-effort mouse position in logical 640x400 surface space."""
        return self._click_pos_variants(pos)[0]

    def _click_pos_variants(self, pos: tuple[int, int] | None = None) -> list[tuple[int, int]]:
        """Candidate click positions in logical surface space.

        pygame.SCALED may leave window size >> surface size; event.pos is often
        already logical, but some backends still report window pixels. Only keep
        candidates that actually lie on the 640x400 surface (never clamp window
        coords into a corner — that false-hits F10 / bottom-right).
        """
        surf_w, surf_h = self.screen.get_size()
        try:
            win_w, win_h = pygame.display.get_window_size()
        except Exception:
            win_w, win_h = surf_w, surf_h
        out: list[tuple[int, int]] = []

        def add(x: float, y: float) -> None:
            ix, iy = int(x), int(y)
            if 0 <= ix < surf_w and 0 <= iy < surf_h:
                t = (ix, iy)
                if t not in out:
                    out.append(t)

        # Prefer event.pos when already logical — get_pos can lag a frame
        if pos is not None:
            px, py = float(pos[0]), float(pos[1])
            add(px, py)
            # Only rescale when the raw event looks like window pixels
            if (
                win_w > 0
                and win_h > 0
                and (win_w, win_h) != (surf_w, surf_h)
                and not (0 <= px < surf_w and 0 <= py < surf_h)
            ):
                add(px * surf_w / win_w, py * surf_h / win_h)
        mx, my = pygame.mouse.get_pos()
        add(mx, my)
        if (
            win_w > 0
            and win_h > 0
            and (win_w, win_h) != (surf_w, surf_h)
            and not (0 <= mx < surf_w and 0 <= my < surf_h)
        ):
            add(mx * surf_w / win_w, my * surf_h / win_h)
        return out or [(surf_w // 2, surf_h // 2)]

    def handle_console_click(self, kind: str, index: int) -> None:
        """VCR / function-key strip — same paths as physical F-keys."""
        if kind == "vcr":
            if index == 0:
                now = pygame.time.get_ticks()
                if now - self._pause_click_at < 450 and self.mode not in ("settings", "title", "start", "paused"):
                    self._pause_return = self.mode if self.mode in ("flight", "docked", "chart", "orrery", "cargo") else ("docked" if self.docked else "flight")
                    self.time_paused = True
                    self.mode = "settings"
                    self._pause_click_at = 0
                    return
                self._pause_click_at = now
                if self.time_paused:
                    self.time_paused = False
                    self.compression.set_stardreamer(0)
                    self.msg("Stardreamer x1 (normal)")
                else:
                    self.time_paused = True
                    self.msg("Stardreamer paused.")
            else:
                self.time_paused = False
                self.compression.force_realtime = False
                self.compression.set_vcr(index)
                self.msg(f"Stardreamer x{self.compression.selected_factor}")
            return
        if kind == "flightmode":
            self._toggle_autopilot()
            return
        if kind == "weapon":
            items = self._weapon_controls()
            if 0 <= index < len(items):
                self._use_weapon_control(items[index][0])
            return
        self.handle_function_key(index)

    def handle_function_key(self, num: int) -> None:
        """Dispatch F1-F10. Nav (1-6) vs actions (7-10) are separate."""
        if self.mode in ("paused", "settings", "title", "start"):
            return
        mods = pygame.key.get_mods()
        if mods & pygame.KMOD_SHIFT and 1 <= num <= 5:
            self.time_paused = False
            self.compression.set_stardreamer(num - 1)
            self.msg(f"Stardreamer x{self.compression.selected_factor}")
            return
        if (mods & pygame.KMOD_ALT) and num == 8:
            self.msg("Mis-jump abort - drive safeties engaged.")
            return
        if 1 <= num <= 6:
            self.handle_nav_key(num)
        elif 7 <= num <= 10:
            self.handle_action_key(num)

    def handle_nav_key(self, num: int) -> None:
        """F1-F6: view / menu navigation only (never launch or gear)."""
        if num == 1:
            # From any menu the icon is the view you get. Cycling only happens
            # once that view is already on screen.
            names = {"front": "Front View", "rear": "Rear View", "external": "External View"}
            if self.mode == "flight":
                self.view_mode = "rear" if self.view_mode == "front" else "front"
            elif self.view_mode not in ("front", "rear"):
                self.view_mode = "front"
            self.mode = "flight"
            self.map_tools = False
            self.info_page = 2
            self.msg(names.get(self.view_mode, "Front View"))
        elif num == 2:
            pygame.event.set_grab(False)
            pygame.mouse.set_visible(True)
            if self.mode == "chart":
                self.mode = "orrery"
                self.system_page = "bodies"
                self.orrery.look_at(np.zeros(3))
                self.msg("System Map")
            else:
                self.mode = "chart"
                self.chart.look_at(self.system.position_ly)
                self.msg("Galactic Map")
        elif num == 3:
            self._cycle_info_page()
        elif num == 4:
            self._open_comms()
        elif num == 5:
            if self.docked:
                self.mode = "docked"
                self.ui.state.tab = "missions"
                self.ui._default_prompt()
                self.msg("Bulletin Board - local services")
            else:
                self.center_mode = "weapons" if self.center_mode == "scanner" else "scanner"
                self.msg("Weapon control" if self.center_mode == "weapons" else "Scanner")
        elif num == 6:
            pygame.event.set_grab(False)
            pygame.mouse.set_visible(True)
            if self.mode == "orrery":
                self.mode = "chart"
                self.chart.look_at(self.system.position_ly)
                self.msg("Galactic Map")
            else:
                self.mode = "orrery"
                self.system_page = "bodies"
                self.orrery.look_at(np.zeros(3))
                self.msg("Current system")

    def handle_action_key(self, num: int) -> None:
        """F7-F10 follow the icon on the current screen. Map keys never launch."""
        if self.mode == "chart":
            self._chart_action(num)
            return
        if self.mode == "orrery":
            self._system_action(num)
            return
        if num == 7:
            if self.docked:
                if not self.launch_clearance:
                    self.msg("No clearance - F4 Comms, Launch Request first (or F7 again to launch illegally).")
                self.launch()
            elif "autopilot" in self.fitted.equipment:
                self._toggle_autopilot()
        elif num == 8:
            return
        elif num == 9:
            self.landing_gear = not self.landing_gear
            state = "DOWN" if self.landing_gear else "UP"
            if self.docked:
                self.msg(f"Undercarriage / clamps {state}")
            else:
                self.msg(f"Landing gear {state}")
        elif num == 10:
            self.show_id_text = not self.show_id_text
            if self.docked:
                self.msg("Station labels " + ("ON" if self.show_id_text else "OFF"))
            else:
                self.msg("ID text " + ("ON" if self.show_id_text else "OFF"))

    def _chart_action(self, num: int) -> None:
        """Galactic map icons. F7 zooms. It does not launch."""
        if getattr(self, "map_tools", False):
            self._map_tools_action(num)
            return
        if num == 7:
            self.chart.zoom(1.25)
            self.msg("Zoom in")
        elif num == 8:
            self.chart.show_grid = not getattr(self.chart, "show_grid", True)
            self.msg("Grid " + ("ON" if self.chart.show_grid else "OFF"))
        elif num == 9:
            self.map_tools = True
            self.msg("Map tools")
        elif num == 10:
            self.map_target = not getattr(self, "map_target", False)
            self.msg("Targeting " + ("ON" if self.map_target else "OFF"))

    def _system_action(self, num: int) -> None:
        """System data keys. F6 and F10 leave for the galactic map. F9 is empty."""
        if num == 7:
            self.system_page = "bodies" if self.system_page == "trade" else "trade"
        elif num == 8:
            self.system_page = "bodies" if self.system_page == "facts" else "facts"
        elif num == 9:
            return
        elif num == 10:
            self.handle_nav_key(2)

    def _map_tools_action(self, num: int) -> None:
        """Settings icons. F9 cancels. F10 is EXIT."""
        if num == 9:
            self.map_tools = False
            self.msg("Map")
        elif num == 10:
            self.map_tools = False
            self.mode = "docked" if self.docked else "flight"
            self.msg("Exit")
        elif num == 7:
            self.time_paused = False
            self.compression.faster()
            self.msg(f"Forward x{self.compression.selected_factor}")
        elif num == 8:
            self.compression.slower()
            self.msg(f"Rewind x{self.compression.selected_factor}")

    def toggle_fullscreen(self) -> None:
        """Toggle fullscreen with fallbacks — SCALED|FULLSCREEN fails on some GPUs."""
        want = not getattr(self, "fullscreen", False)
        if want:
            attempts: list[tuple[tuple[int, int], int]] = [
                ((WIDTH, HEIGHT), pygame.FULLSCREEN | pygame.SCALED),
                ((WIDTH, HEIGHT), pygame.FULLSCREEN),
                ((WIDTH, HEIGHT), pygame.SCALED | pygame.NOFRAME),
            ]
        else:
            attempts = [
                ((WIDTH, HEIGHT), pygame.SCALED | pygame.RESIZABLE),
                ((WIDTH, HEIGHT), pygame.RESIZABLE),
                ((WIDTH, HEIGHT), 0),
            ]
        for size, flags in attempts:
            try:
                self.screen = pygame.display.set_mode(size, flags)
                self.fullscreen = want
                self.msg("Fullscreen " + ("ON" if want else "OFF"))
                return
            except pygame.error:
                continue
        # Last resort: restore safe windowed mode
        try:
            self.screen = pygame.display.set_mode((WIDTH, HEIGHT), pygame.RESIZABLE)
        except pygame.error:
            self.screen = pygame.display.set_mode((WIDTH, HEIGHT))
        self.fullscreen = False
        self.msg("Fullscreen unavailable on this display.")

    def _handle_settings_key(self, key: int) -> None:
        return

    def _click_options(self, pos: tuple[int, int]) -> None:
        for kind, key, rect in self._opt_hits:
            if not rect.collidepoint(pos):
                continue
            if kind == "detail":
                self.detail_level = int(key)
            elif kind == "track":
                i = int(key)
                self.music_on[i] = not self.music_on[i]
            elif kind == "opt":
                self.opt[str(key)] = not self.opt[str(key)]
            self._save_settings()
            self._music_mode = None
            return

    def _apply_settings_item(self) -> None:
        i = self.settings_sel
        if i == 0:
            self.mode = self._pause_return
        elif i == 1:
            self.toggle_fullscreen()
        elif i == 2:
            self.save()
            self.msg("Game saved.")
        elif i == 3:
            self.mode = "title"
            self.title_phase = "menu"
            self.title_menu = 0
            self._music_mode = None

    def draw_pause_menu(self) -> None:
        self.screen.fill((0, 0, 0))
        t = self.font.render("PAUSED", True, (255, 190, 50))
        self.screen.blit(t, (WIDTH // 2 - t.get_width() // 2, HEIGHT // 2 - 40))
        lines = [
            "Enter / R  Resume",
            "Esc        Settings",
            "S          Save",
            "L          Load",
            "F11        Fullscreen",
            "`          Cheat console",
        ]
        for i, line in enumerate(lines):
            lab = self.small.render(line, True, (180, 160, 80))
            self.screen.blit(lab, (WIDTH // 2 - 70, HEIGHT // 2 - 8 + i * 14))
        self.screen.blit(
            self.small.render(self.stardate.format(), True, (255, 168, 32)),
            (12, HEIGHT - 24),
        )

    def draw_settings(self) -> None:
        """Double-click the pause key. Grey options page from the DOS layout."""
        green = (32, 220, 48)
        white = (236, 236, 236)
        red = (204, 28, 28)
        box = (132, 132, 132)
        self.screen.fill((72, 72, 72))
        hits: list[tuple[str, str | int, pygame.Rect]] = []

        def header(text: str, x: int, y: int) -> None:
            self.screen.blit(self.font.render(text, True, green), (x, y))

        def row(text: str, x: int, y: int, on: bool, kind: str, key: str | int) -> None:
            rect = pygame.Rect(x, y + 2, 10, 10)
            pygame.draw.rect(self.screen, red if on else box, rect)
            self.screen.blit(self.small.render(text, True, white), (x + 16, y))
            hits.append((kind, key, rect.inflate(120, 4)))

        header("Level", 16, 8)
        self.screen.blit(self.font.render("Shape Detail:", True, green), (70, 8))
        for i, name in enumerate(("Low", "Medium", "High", "Very High")):
            row(name, 28, 28 + i * 16, self.detail_level == i, "detail", i)
        header("Sounds:", 250, 8)
        for i, (key, name) in enumerate((
            ("sfx", "Sound effects on"),
            ("engine", "Engine sound"),
            ("station", "Space station"),
            ("wind", "Wind noise"),
            ("beeps", "Icon beeps"),
        )):
            row(name, 250, 28 + i * 16, self.opt[key], "opt", key)
        header("Music:", 460, 8)
        for i, (key, name) in enumerate((
            ("music", "Music on"),
            ("music_hs", "Music after hyperspace"),
            ("music_dock", "Docking music"),
            ("music_battle", "Music in battles"),
            ("music_cont", "Continuous music"),
        )):
            row(name, 460, 28 + i * 16, self.opt[key], "opt", key)
        header("Other Options:", 16, 128)
        for i, (key, name) in enumerate((
            ("dust", "Space dust, clouds etc."),
            ("stars", "Background stars"),
            ("lock_ext", "Lock external view to ship"),
            ("rev_lr", "Reverse left-right controls"),
            ("rev_ud", "Reverse up-down controls"),
            ("elite_ctrl", "Elite control method in space"),
            ("tunnels", "Show targeting tunnels"),
            ("ship_tunnels", "Allow targeting tunnels for ships"),
            ("filenames", "Use filename extensions"),
        )):
            row(name, 16, 150 + i * 16, self.opt[key], "opt", key)
        header("Choose Music From:", 300, 128)
        tracks = (
            "Greig - In the Hall of the Mountain King",
            "Lowe - Frontier Main Theme",
            "Mussorgsky - Baba Yaga",
            "Mussorgsky - Night on the Bare Mountain",
            "Lowe - Frontier Second Theme",
            "Wagner - Ride of the Valkyries",
            "Strauss - Blue Danube",
            "Mussorgsky - Great Gates of Kiev",
        )
        for i, name in enumerate(tracks):
            row(name, 300, 150 + i * 16, self.music_on[i], "track", i)
        self._opt_hits = hits

    def _cheat_items(self) -> list[tuple[str, str]]:
        return [
            ("freebuy", f"Free purchases: {'ON' if self.free_purchases else 'OFF'}  (sales still pay)"),
            ("rubbish", "Put 1t rubbish in the hold"),
            ("cr100k", "+100,000 credits"),
            ("cr1m", "+1,000,000 credits"),
            ("fuel", "Fill tanks and +30t hydrogen"),
            ("milfuel", "+10t military fuel"),
            ("minerals", "+5t minerals"),
            ("repair", "Full repair (hull, shields, energy)"),
            ("god", f"God mode: {'ON' if self.god_mode else 'OFF'}"),
            ("infuel", f"Infinite fuel: {'ON' if self.infinite_fuel else 'OFF'}"),
            ("legal", "Clear fines and go legal"),
            ("wanted", "Become wanted (10,000 Cr fine)"),
            ("clearance", "Grant launch clearance"),
            ("gems", "+10t gem stones"),
            ("kit", "Fit autopilot, scoop, cabin, shields"),
            ("minelaser", "Fit a mining laser on the nose"),
            ("passenger", "Put a passenger in a cabin"),
            ("contract", "Add a test delivery contract"),
            ("rank", "Advance to the next combat rank"),
            ("elite", "Set combat rating to ELITE"),
            ("panther", "Give a fitted Panther Clipper"),
            ("jump", "Jump range 50 ly"),
            ("teleport", "Teleport to nearest station"),
            ("raid", "Spawn a pirate raid"),
            ("wipe", "Destroy every pirate"),
            ("month", "Advance the stardate 30 days"),
            ("close", "Close"),
        ]

    def _apply_cheat(self, cheat_id: str) -> None:
        if cheat_id == "close":
            self.mode = self._cheat_return
            return
        if cheat_id == "freebuy":
            self.free_purchases = not self.free_purchases
            self._free_purchases_armed = True
            self.msg("Cheat: purchases " + ("FREE (sales still pay)" if self.free_purchases else "cost credits again"))
        elif cheat_id == "rubbish":
            self.fitted.cargo["rubbish"] = self.fitted.cargo.get("rubbish", 0) + 1
            self.flight.stats.cargo = dict(self.fitted.cargo)
            self.msg("Cheat: +1t rubbish in the hold")
        elif cheat_id == "cr100k":
            self.credits += 100_000
            self.msg("Cheat: +100,000 Cr")
        elif cheat_id == "cr1m":
            self.credits += 1_000_000
            self.msg("Cheat: +1,000,000 Cr")
        elif cheat_id == "fuel":
            self.fitted.fuel_t = max(self.flight.stats.max_fuel_t, 10.0)
            self._rebuild_player_combatant()
            self.flight.stats.fuel_t = self.flight.stats.max_fuel_t
            self.fitted.fuel_t = self.flight.stats.fuel_t
            self.fitted.cargo["hydrogen_fuel"] = self.fitted.cargo.get("hydrogen_fuel", 0) + 30
            self.flight.stats.cargo = dict(self.fitted.cargo)
            self._rebuild_player_combatant()
            self.msg("Cheat: tanks full, +30t hydrogen fuel in the hold")
        elif cheat_id == "milfuel":
            self.fitted.cargo["military_fuel"] = self.fitted.cargo.get("military_fuel", 0) + 10
            self.flight.stats.cargo = dict(self.fitted.cargo)
            self._rebuild_player_combatant()
            self.msg("Cheat: +10t military fuel")
        elif cheat_id == "minerals":
            self.fitted.cargo["minerals"] = self.fitted.cargo.get("minerals", 0) + 5
            self.flight.stats.cargo = dict(self.fitted.cargo)
            self._rebuild_player_combatant()
            self.msg("Cheat: +5t minerals")
        elif cheat_id == "repair":
            self.fitted.hull = max(self.fitted.max_hull, 100.0)
            self.fitted.max_hull = self.fitted.hull
            self._rebuild_player_combatant()
            self.flight.stats.hull = self.flight.stats.max_hull
            self.flight.stats.shield = self.flight.stats.max_shield
            self.flight.stats.energy = self.flight.stats.max_energy
            self.fitted.hull = self.flight.stats.hull
            self.msg("Cheat: ship fully repaired")
        elif cheat_id == "god":
            self.god_mode = not self.god_mode
            self.combat.god_mode = self.god_mode
            self.msg("Cheat: god mode " + ("ON" if self.god_mode else "OFF"))
        elif cheat_id == "infuel":
            self.infinite_fuel = not self.infinite_fuel
            self.msg("Cheat: infinite fuel " + ("ON" if self.infinite_fuel else "OFF"))
        elif cheat_id == "legal":
            self.legal = True
            self.fines = 0
            self.combat.player_legal = True
            self.combat.fines = 0
            if "player" in self.combat.combatants:
                self.combat.combatants["player"].legal = True
            self.msg("Cheat: record wiped clean")
        elif cheat_id == "wanted":
            self.legal = False
            self.fines = max(self.fines, 10_000)
            self.combat.player_legal = False
            self.combat.fines = self.fines
            if "player" in self.combat.combatants:
                self.combat.combatants["player"].legal = False
            self.msg("Cheat: you are now wanted")
        elif cheat_id == "clearance":
            self.launch_clearance = True
            self.clearance_expires_at = 0.0
            self.msg("Cheat: launch clearance granted")
        elif cheat_id == "gems":
            self.fitted.cargo["gem_stones"] = self.fitted.cargo.get("gem_stones", 0) + 10
            self.flight.stats.cargo = dict(self.fitted.cargo)
            self.msg("Cheat: +10t gem stones")
        elif cheat_id == "kit":
            for eid in (
                "autopilot",
                "auto_refueller",
                "atmo_shield",
                "fuel_scoop",
                "escape_capsule",
                "passenger_cabin",
                "radar_mapper",
                "energy_booster",
                "shield_gen",
                "hull_repair",
            ):
                if eid not in self.fitted.equipment:
                    self.fitted.equipment.append(eid)
            self._rebuild_player_combatant()
            self.msg("Cheat: test fittings installed (they still have mass)")
        elif cheat_id == "minelaser":
            old = self.fitted.guns.get("front")
            if old and old in self.fitted.equipment:
                self.fitted.equipment.remove(old)
            self.fitted.guns["front"] = "mining_laser"
            if "mining_laser" not in self.fitted.equipment:
                self.fitted.equipment.append("mining_laser")
            self._rebuild_player_combatant()
            self.msg("Cheat: 30MW mining laser on the front mount")
        elif cheat_id == "passenger":
            if "passenger_cabin" not in self.fitted.equipment:
                self.fitted.equipment.append("passenger_cabin")
            self.fitted.cargo["passenger"] = self.fitted.cargo.get("passenger", 0) + 1
            self.flight.stats.cargo = dict(self.fitted.cargo)
            self._rebuild_player_combatant()
            self.msg("Cheat: +1 passenger, cabin fitted if it was missing")
        elif cheat_id == "contract":
            from sim.mission import Mission
            neighbours = self.galaxy.neighbors(self.system_id, 40.0)
            if not neighbours:
                self.msg("Cheat: no system in range to contract")
            else:
                dest_id, dist = neighbours[0]
                dest = self.galaxy.get(dest_id)
                self.missions.active.append(Mission(
                    id=f"cheat_{int(self.stardate.days)}_{len(self.missions.active)}",
                    description=f"Deliver 1t computers to {dest.name}",
                    dest_system=dest_id,
                    dest_location=dest.id,
                    dest_name=dest.name,
                    cargo_id="computers",
                    cargo_name="Computers",
                    quantity=1,
                    reward=500,
                    deadline_day=self.stardate.days + 30,
                    taken=True,
                    kind="cargo",
                ))
                self.fitted.cargo["computers"] = self.fitted.cargo.get("computers", 0) + 1
                self.flight.stats.cargo = dict(self.fitted.cargo)
                self._rebuild_player_combatant()
                self.msg(f"Cheat: contract to {dest.name}, 1t computers aboard")
        elif cheat_id == "rank":
            from sim.frontier import COMBAT_RANKS, combat_rank
            nxt = None
            for need, _label in COMBAT_RANKS:
                if need > self.kills:
                    nxt = need
                    break
            if nxt is None:
                self.msg("Cheat: already ELITE")
            else:
                self.kills = nxt
                self.msg(f"Cheat: combat rating {combat_rank(self.kills)} ({self.kills} kills)")
        elif cheat_id == "elite":
            from sim.frontier import combat_rank
            self.kills = 6000
            self.msg(f"Cheat: combat rating {combat_rank(self.kills)}")
        elif cheat_id == "panther":
            self.fitted = FittedShip(
                ship_id="panther",
                equipment=[
                    "drive_class8",
                    "beam_100mw",
                    "beam_20mw",
                    "shield_gen",
                    "shield_gen",
                    "ecm_naval",
                    "docking_computer",
                    "autopilot",
                    "fuel_scoop",
                    "cargo_scoop",
                    "atmo_shield",
                    "life_support",
                    "hull_repair",
                ],
                guns={"front": "beam_100mw", "rear": "beam_20mw"},
                missile_load=["missile_navy"] * 8,
                cargo={"gem_stones": 20, "luxury_goods": 10},
                fuel_t=10.0,
                hull=500.0,
                max_hull=500.0,
            )
            self.cheat_jump_ly = 50.0
            self._rebuild_player_combatant()
            self.msg("Cheat: Panther Clipper delivered, jump 50 ly")
        elif cheat_id == "jump":
            self.cheat_jump_ly = 50.0
            self._rebuild_player_combatant()
            self.msg("Cheat: hyperspace range 50 ly")
        elif cheat_id == "teleport":
            self._cheat_teleport()
        elif cheat_id == "raid":
            if self.docked:
                self.msg("Cheat: launch first — pirates don't spawn in the bay")
            else:
                ais = spawn_traffic(
                    self.frames,
                    self.player_state.parent_id,
                    count=4,
                    seed=int(self.stardate.days * 1000) ^ 0x51DE,
                    near=self.player_state,
                    days=self.stardate.days,
                    roles=["pirate"],
                )
                for c in ais:
                    self.combat.combatants[c.id] = c
                    self.ai_controllers.append(AIController(c))
                self.msg("Cheat: four pirates inbound")
        elif cheat_id == "wipe":
            n = 0
            for c in self.combat.combatants.values():
                if c.role == "pirate" and c.alive:
                    c.alive = False
                    c.stats.hull = 0
                    n += 1
                    self.credits += c.bounty
            self.msg(f"Cheat: {n} pirate{'s' if n != 1 else ''} removed")
        elif cheat_id == "month":
            self.stardate.days += 30.0
            self.msg(f"Cheat: stardate now {self.stardate.format()}")

    def _cheat_teleport(self) -> None:
        if self.docked:
            self.msg("Cheat: undock first")
            return
        target = None
        if self.ap_body_id and self.ap_body_id in self.frames.bodies:
            body = self.frames.bodies[self.ap_body_id]
            if body.is_station or body.is_starport:
                target = self.ap_body_id
        if target is None and self.system.stations:
            target = self.system.stations[0].id
        if target is None:
            for bid, body in self.frames.bodies.items():
                if body.is_starport:
                    target = bid
                    break
        if target is None:
            self.msg("Cheat: no station in this system")
            return
        name = self.frames.bodies[target].name
        self.player_state = FrameState(
            parent_id=target,
            position=np.array([0.0, 0.0, -25.0], dtype=np.float64),
            velocity=np.zeros(3, dtype=np.float64),
        )
        self.ap_enabled = False
        self._rebuild_player_combatant()
        self.msg(f"Cheat: 25 km from {name}")

    def draw_cheats(self) -> None:
        from sim.frontier import combat_rank
        self.screen.fill((6, 4, 12))
        title = self.font.render("CHEAT CONSOLE", True, (48, 230, 72))
        self.screen.blit(title, (WIDTH // 2 - title.get_width() // 2, 16))
        sub = self.small.render(
            f"Cr {self.credits}   Jump {self.flight.stats.jump_range_ly:.1f} ly   "
            f"H2 {int(self.fitted.cargo.get('hydrogen_fuel', 0))}t   "
            f"{combat_rank(self.kills)}   "
            f"{'WANTED' if not self.legal else 'Legal'}",
            True,
            (255, 180, 40),
        )
        self.screen.blit(sub, (WIDTH // 2 - sub.get_width() // 2, 40))
        items = self._cheat_items()
        self.cheat_sel = max(0, min(self.cheat_sel, len(items) - 1))
        cols = 2
        per_col = (len(items) + 1) // cols
        self._cheat_rects = []
        for i, (_cid, label) in enumerate(items):
            col_i = i // per_col
            row = i % per_col
            x = 24 + col_i * 310
            y = 64 + row * 18
            selected = i == self.cheat_sel
            if selected:
                pygame.draw.rect(self.screen, (36, 90, 48), (x - 4, y - 1, 296, 16))
            colour = (48, 230, 72) if selected else (210, 210, 210)
            prefix = "> " if selected else "  "
            self.screen.blit(self.small.render(prefix + label, True, colour), (x, y))
            self._cheat_rects.append(pygame.Rect(x - 4, y - 1, 296, 16))
        hint = self.small.render("` or Esc close    Arrows select    Enter apply", True, (140, 140, 150))
        self.screen.blit(hint, (WIDTH // 2 - hint.get_width() // 2, HEIGHT - 22))

    def _handle_cheat_key(self, key: int) -> None:
        items = self._cheat_items()
        n = len(items)
        if key in (pygame.K_ESCAPE, pygame.K_BACKQUOTE, pygame.K_F12):
            self.mode = self._cheat_return
            return
        if key in (pygame.K_UP, pygame.K_w):
            self.cheat_sel = (self.cheat_sel - 1) % n
        elif key in (pygame.K_DOWN, pygame.K_s):
            self.cheat_sel = (self.cheat_sel + 1) % n
        elif key in (pygame.K_LEFT, pygame.K_a):
            self.cheat_sel = (self.cheat_sel - ((n + 1) // 2)) % n
        elif key in (pygame.K_RIGHT, pygame.K_d):
            self.cheat_sel = (self.cheat_sel + ((n + 1) // 2)) % n
        elif key in (pygame.K_RETURN, pygame.K_SPACE):
            self._apply_cheat(items[self.cheat_sel][0])

    def _relative_to_name(self) -> str:
        body = self.frames.bodies.get(self.player_state.parent_id)
        if body:
            return body.name
        return self.system.name

    def _has_save(self) -> bool:
        return SAVE_PATH.is_file()

    def _fade_to(self, action) -> None:
        if self._fade_phase:
            return
        self._fade_action = action
        self._fade_phase = "out"

    def _tick_fade(self, dt: float) -> None:
        if self._fade_phase == "out":
            self.fade = min(1.0, self.fade + dt / 0.18)
            if self.fade >= 1.0 and self._fade_action:
                action = self._fade_action
                self._fade_action = None
                action()
                self._fade_phase = "in"
        elif self._fade_phase == "in":
            self.fade = max(0.0, self.fade - dt / 0.18)
            if self.fade <= 0.0:
                self._fade_phase = None

    def _open_start(self) -> None:
        self.mode = "start"
        self.start_sel = 0
        self._start_idle = 0.0

    def _show_intro(self) -> None:
        """Hide the position list. The film and intro track keep running."""
        self.mode = "title"
        self.title_phase = "intro"

    def _open_title_menu(self) -> None:
        self.mode = "title"
        self.title_phase = "menu"
        self.title_menu = 0

    def _begin_selected_career(self) -> None:
        sid, dock, _name = self._starts[self.start_sel]
        self.start_new_game(sid, dock)

    def _activate_start(self) -> None:
        if self._fade_phase:
            return
        if self.start_sel < 3:
            self._fade_to(self._begin_selected_career)
        elif self._has_save():
            self._fade_to(self._load_from_title)

    def _load_from_title(self) -> None:
        self.load()

    def _title_menu_count(self) -> int:
        return 3

    def _title_step(self, delta: int) -> None:
        n = self._title_menu_count()
        i = self.title_menu
        for _ in range(n):
            i = (i + delta) % n
            if i != 1 or self._has_save():
                self.title_menu = i
                return

    def _activate_title(self) -> None:
        if self._fade_phase:
            return
        if self.title_menu == 0:
            self._fade_to(self._open_start)
        elif self.title_menu == 1 and self._has_save():
            self._fade_to(self._load_from_title)
        elif self.title_menu == 2:
            self._fade_to(lambda: setattr(self, "_quit_requested", True))

    def _rebuild_player_combatant(self) -> None:
        stats = apply_equipment_flags(
            fitted_to_stats(self.fitted, self.shipyard.ships, self.shipyard.equipment),
            self.fitted,
            self.shipyard.equipment,
        )
        if self.cheat_jump_ly:
            stats.jump_range_ly = self.cheat_jump_ly
        self.flight.stats = stats
        self.combat.god_mode = self.god_mode
        self.combat.combatants["player"] = Combatant(
            id="player",
            name="Commander",
            role="player",
            state=self.player_state,
            stats=stats,
            legal=self.legal,
        )

    def _sync_fitted_from_flight(self) -> None:
        self.fitted.fuel_t = self.flight.stats.fuel_t
        self.fitted.hull = self.flight.stats.hull
        self.fitted.cargo = dict(self.flight.stats.cargo)

    def spawn_local_traffic(self) -> None:
        parent = self.player_state.parent_id
        gov = self.system.government
        if gov == "Anarchy":
            roles = ["pirate", "pirate", "pirate", "trader", "trader"]
        elif gov in ("Democracy", "Corporate"):
            roles = ["police", "police", "trader", "trader", "pirate"] if not self.legal else ["police", "trader", "trader", "trader", "trader"]
        else:
            roles = ["pirate", "trader", "police", "trader", "pirate"]
        ais = spawn_traffic(
            self.frames,
            parent,
            count=5,
            seed=self.system.seed ^ int(self.stardate.days * 10),
            near=self.player_state,
            days=self.stardate.days,
            roles=roles,
        )
        self.ai_controllers = []
        for c in ais:
            self.combat.combatants[c.id] = c
            self.ai_controllers.append(AIController(c))

    def msg(self, text: str) -> None:
        self.messages.append(text)
        if len(self.messages) > 40:
            self.messages = self.messages[-40:]

    # --- Docked actions ---
    def request_launch_clearance(self) -> None:
        """FE2: F4 Comms -> Launch Request. Required before legal takeoff."""
        if not self.docked:
            self.msg("Not docked - no ATC to contact.")
            return
        loc = self.frames.bodies.get(self.dock_location)
        name = loc.name if loc else "Starport"
        self.launch_clearance = True
        # Stays until takeoff or leave station (no timer race with Stardreamer)
        self.clearance_expires_at = 0.0
        pad = ""
        sp = self.system.starport_by_id(self.dock_location)
        if sp and sp.pads:
            idx = min(self.ui.selected_pad, len(sp.pads) - 1)
            pad = f" Pad {sp.pads[idx]}."
        self.ui.state.message = f"Clearance granted.{pad} Press F7 to launch."
        self.msg(f"{name}: Clearance granted.{pad} Press F7 to take off.")

    def launch(self) -> None:
        if not self.docked:
            return
        # FE2: takeoff without clearance = fine + criminal flag
        if not self.launch_clearance:
            fine = self.UNAUTHORIZED_LAUNCH_FINE
            self.fines += fine
            self.legal = False
            if "player" in self.combat.combatants:
                self.combat.combatants["player"].legal = False
            self.combat.player_legal = False
            self.combat.fines = max(getattr(self.combat, "fines", 0), self.fines)
            self.msg(f"UNAUTHORIZED LAUNCH - fined {fine} Cr. Police alerted.")
            self.ui.state.message = f"Illegal takeoff! Fine {fine} Cr."
        else:
            self.msg("Launch clearance confirmed. Cleared for takeoff.")

        self.sfx.launch()
        self.docked = False
        self.mode = "flight"
        self.view_mode = "front"
        self.landing_gear = False  # retract on takeoff
        self.launch_clearance = False
        self.clearance_expires_at = 0.0
        # Prevent instant auto-land back onto the same pad
        self._launch_grace_until = pygame.time.get_ticks() / 1000.0 + 8.0
        body = self.frames.bodies[self.dock_location]
        if body.is_starport and not body.is_station:
            # Surface launch: climb well clear of the 8km auto-land bubble
            days = self.stardate.days
            if body.parent_id:
                port_world = body.world_pos(days)
                planet = self.frames.bodies[body.parent_id]
                rel = port_world - planet.world_pos(days)
                up = rel / max(float(np.linalg.norm(rel)), 1e-9)
                self.player_state = FrameState(
                    parent_id=body.parent_id,
                    position=rel + up * 25.0,  # 25 km above pad
                    velocity=up * 1.2,  # climb-out speed (above auto-land 0.25)
                )
            else:
                self.player_state = FrameState(
                    parent_id=self.dock_location,
                    position=np.array([0.0, 25.0, 0.0], dtype=np.float64),
                    velocity=np.array([0.0, 1.2, 0.0], dtype=np.float64),
                )
            pad = ""
            sp = self.system.starport_by_id(self.dock_location)
            if sp and sp.pads:
                idx = min(self.ui.selected_pad, len(sp.pads) - 1)
                pad = f" from {sp.pads[idx]}"
            self.msg(f"Airborne{pad}. Climbing out - gear UP.")
        else:
            self.player_state = FrameState(
                parent_id=self.dock_location,
                position=np.array([0.0, 0.0, -40.0], dtype=np.float64),
                velocity=np.array([0.0, 0.0, -2.0], dtype=np.float64),
            )
            self.msg("Undocked. Leave the station approaches promptly.")
        self._rebuild_player_combatant()
        self.spawn_local_traffic()
        pygame.event.set_grab(False)
        pygame.mouse.set_visible(True)

    def _nearest_port(self) -> tuple[str, object, float] | None:
        """Station or starport inside the docking-contact range, if any."""
        days = self.stardate.days
        world = self.frames.ship_world_pos(self.player_state, days)
        best = None
        best_d = 1e18
        for bid, body in self.frames.bodies.items():
            if not (body.is_station or body.is_starport):
                continue
            d = float(np.linalg.norm(world - body.world_pos(days)))
            limit = 15.0 if body.is_station else body.radius_km + 5.0
            if d < limit and d < best_d:
                best_d = d
                best = (bid, body, d)
        return best

    def _open_comms(self) -> None:
        """Station services only while docked. In flight, clearance or an emergency call."""
        self.ui.flight_links = None
        if self.docked:
            self.mode = "docked"
            self.ui.state.tab = "comms"
            self.ui.state.selected = 0
            self.ui.state.scroll = 0
            self.ui.state.message = "Which service do you require?"
            return
        links: list[tuple[str, tuple[str, ...]]] = []
        port = self._nearest_port()
        if port is not None:
            _bid, body, _d = port
            kind = "station" if body.is_station else "starport"
            links.append((
                "Request landing clearance",
                (f"(Contact {body.name} {kind})",),
            ))
        if not self.ap_enabled:
            links.append((
                "Send long range emergency signal",
                ("(Manual control only)",),
            ))
        if not links:
            self.msg("No comms link. Manual control for an emergency signal, or close with a station.")
            return
        self.ui.flight_links = links
        self.mode = "docked"
        self.ui.state.tab = "comms"
        self.ui.state.selected = 0
        self.ui.state.scroll = 0
        self.ui.state.message = "Select comms transmission."

    def request_dock(self) -> None:
        if self.docked or (self.cinematic and self.cinematic[0] == "dock"):
            return
        # Find nearest station/starport
        found = self._nearest_port()
        best = found[0] if found else None
        if not best:
            self.msg("No docking clearance — approach a station or starport.")
            return
        speed = float(np.linalg.norm(self.player_state.velocity))
        if speed > 0.5 and not self.flight.stats.has_docking_computer:
            self.msg("Too fast to dock — reduce speed.")
            return
        body = self.frames.bodies[best]
        self.ap_enabled = False
        self.dock_clearance_id = best
        self.flight.controls.set_speed = 0.0
        self.flight.controls.pitch = 0.0
        self.flight.controls.yaw = 0.0
        self.flight.controls.roll = 0.0
        self.cinematic = (
            "dock",
            0.0,
            2.8 if body.is_station else 2.4,
            {"location": best, "is_station": body.is_station, "name": body.name},
        )
        self.mode = "cinematic"
        self.msg(f"{'Docking' if body.is_station else 'Landing'} at {body.name}…")

    def _finish_dock(self, location_id: str) -> None:
        days = self.stardate.days
        self.player_state = self.frames.rebase(self.player_state, days, location_id)
        self.player_state.position = np.array([0.0, 0.0, -2.0], dtype=np.float64)
        self.player_state.velocity[:] = 0
        self.dock_clearance_id = None
        self.dock_location = location_id
        self.cinematic = None
        self.launch_clearance = False
        self.clearance_expires_at = 0.0
        self._open_berth()
        self._sync_fitted_from_flight()
        pygame.event.set_grab(False)
        pygame.mouse.set_visible(True)
        delivered = self.missions.check_delivery(
            self.system_id, location_id, self.fitted.cargo, self.stardate.days
        )
        for m in delivered:
            self.credits += m.reward
            self.msg(f"Delivered: +{m.reward} Cr")
        self.flight.stats.cargo = dict(self.fitted.cargo)
        self.board_missions = self.missions.generate(self.system, int(self.stardate.days))
        name = self.frames.bodies[location_id].name
        if self._berth_is_indoors(location_id):
            self.msg(f"Docked at {name}. Request launch clearance via F4 Comms.")
        else:
            self.msg(f"Landed at {name}.")

    def _cycle_info_page(self) -> None:
        """F3: ship equipment, then commander, then cargo, then ship."""
        self.info_page = (self.info_page + 1) % 3
        if self.info_page == 0:
            self.mode = "ship"
            self.msg("Ship equipment")
        elif self.info_page == 1:
            self.mode = "captain"
            self.msg("Commander")
        else:
            self._open_cargo()
            self.msg("Cargo inventory")

    def _hyperspace_block(self) -> str | None:
        """Why F8 cannot jump, or None when the drive can fire."""
        if getattr(self, "cinematic", None):
            return "Hyperspace unavailable."
        if self.docked:
            return "Cannot hyperspace while docked."
        if float(getattr(self.flight.stats, "_drive_h", 0.0)) <= 0:
            return "No hyperdrive fitted."
        target = self.selected_system
        if not target or target == self.system_id:
            return "No hyperspace destination."
        dist = self.galaxy.distance_ly(self.system_id, target)
        jump = self.flight.stats.jump_range_ly
        if dist > jump + 1e-6:
            return f"Destination out of range ({dist:.2f} ly, drive {jump:.2f} ly)."
        return None

    def hyperspace_ready(self) -> bool:
        return self._hyperspace_block() is None

    def _has_scanner(self) -> bool:
        return "scanner" in self.fitted.equipment

    def _ship_target(self):
        tc = self.combat.combatants.get(self.target_id) if self.target_id else None
        if tc is None or not tc.alive or tc.id == "player":
            return None
        if tc.role not in ("pirate", "police", "trader"):
            return None
        return tc

    def _cloud_in_range(self):
        if not self.hs_clouds or self.docked:
            return None
        me = self.frames.ship_world_pos(self.player_state, self.stardate.days)
        best = None
        best_d = 80.0
        for cloud in self.hs_clouds:
            if cloud.get("system") != self.system_id:
                continue
            if self.stardate.days > cloud["expires"]:
                continue
            d = float(np.linalg.norm(me - cloud["pos"]))
            if d < best_d:
                best = cloud
                best_d = d
        return best

    def _weapon_controls(self) -> list[tuple[str, bool, int]]:
        """Controls for equipment actually fitted. Missiles need ammunition."""
        eq = self.fitted.equipment
        items: list[tuple[str, bool, int]] = []
        if self.fitted.guns:
            items.append(("LASER", True, 0))
        if "ecm" in eq or "ecm_naval" in eq:
            items.append(("ECM", True, 0))
        if "radar_mapper" in eq:
            items.append(("RADAR MAPPER", self._ship_target() is not None, 0))
        missiles = len(self.fitted.missile_load)
        if missiles > 0:
            items.append(("MISSILE", True, missiles))
        bombs = eq.count("energy_bomb")
        if bombs > 0:
            items.append(("ENERGY BOMB", True, bombs))
        if "cloud_analyser" in eq:
            items.append(("CLOUD ANALYSER", self._cloud_in_range() is not None, 0))
        if "military_camera" in eq:
            items.append(("CAMERA", self._ship_target() is not None, 0))
        if "escape_capsule" in eq:
            items.append(("ESCAPE", True, 0))
        return items

    def _use_weapon_control(self, label: str) -> None:
        if label == "LASER":
            self.fire_laser()
        elif label == "ECM":
            self._use_ecm()
        elif label == "RADAR MAPPER":
            self._use_radar_mapper()
        elif label == "MISSILE":
            self.fire_missile()
        elif label == "ENERGY BOMB":
            self._use_energy_bomb()
        elif label == "CLOUD ANALYSER":
            self._use_cloud_analyser()
        elif label == "CAMERA":
            self._use_military_camera()
        elif label == "ESCAPE":
            self._use_escape_capsule()

    def _use_ecm(self) -> None:
        if "ecm" not in self.fitted.equipment and "ecm_naval" not in self.fitted.equipment:
            return
        kept = [p for p in self.combat.projectiles if p.kind != "missile" or p.owner_id == "player"]
        dropped = len(self.combat.projectiles) - len(kept)
        self.combat.projectiles = kept
        self.msg("ECM engaged." if dropped else "ECM — no missiles in range.")

    def _use_energy_bomb(self) -> None:
        if "energy_bomb" not in self.fitted.equipment:
            return
        self.fitted.equipment.remove("energy_bomb")
        me = self.frames.ship_world_pos(self.player_state, self.stardate.days)
        hit = 0
        for c in self.combat.combatants.values():
            if not c.alive or c.id == "player":
                continue
            d = float(np.linalg.norm(self.frames.ship_world_pos(c.state, self.stardate.days) - me))
            if d < 8.0:
                c.stats.hull -= 40
                hit += 1
                if c.stats.hull <= 0:
                    c.alive = False
        self.msg(f"Energy bomb detonated. {hit} ship(s) in range.")

    def _use_radar_mapper(self) -> None:
        if "radar_mapper" not in self.fitted.equipment:
            return
        tc = self._ship_target()
        if tc is None:
            self.mapper_active = False
            self.msg("Radar mapper: no ship targeted.")
            return
        self.mapper_active = not self.mapper_active
        self.msg("Radar mapper on." if self.mapper_active else "Radar mapper off.")

    def _mapper_line(self) -> str:
        if not self.mapper_active or "radar_mapper" not in self.fitted.equipment:
            return ""
        tc = self._ship_target()
        if tc is None:
            self.mapper_active = False
            return ""
        me = self.frames.ship_world_pos(self.player_state, self.stardate.days)
        d = float(np.linalg.norm(self.frames.ship_world_pos(tc.state, self.stardate.days) - me))
        return f"MAPPER  {tc.name}  {tc.role}  hull {tc.stats.hull:.0f}  {d:.1f} km"

    def _use_cloud_analyser(self) -> None:
        if "cloud_analyser" not in self.fitted.equipment:
            return
        cloud = self._cloud_in_range()
        if cloud is None:
            self.msg("Cloud analyser: no hyperspace remnant in range.")
            return
        self.msg(f"Cloud: {cloud['origin']} → {cloud['dest']}")

    def _use_military_camera(self) -> None:
        if "military_camera" not in self.fitted.equipment:
            return
        tc = self._ship_target()
        if tc is None:
            self.camera_line = ""
            self.msg("Military camera: no ship targeted.")
            return
        me = self.frames.ship_world_pos(self.player_state, self.stardate.days)
        d = float(np.linalg.norm(self.frames.ship_world_pos(tc.state, self.stardate.days) - me))
        self.camera_line = f"PHOTO  {tc.name}  {tc.role}  {d:.1f} km"
        self.msg(f"Military camera: {tc.name}.")

    def _use_escape_capsule(self) -> None:
        if "escape_capsule" not in self.fitted.equipment:
            return
        self.msg("Escape capsule launched. The ship is lost.")
        self.fitted = default_eagle()
        self.docked = True
        self.dock_location = self._first_dock(self.system_id)
        self.mode = "docked"
        self.center_mode = "scanner"
        self._rebuild_player_combatant()

    def _toggle_autopilot(self) -> None:
        """Separate from F7. Engages only with a planet or station selected."""
        if self.engines_off:
            self.msg("Engines are off.")
            return
        if self.ap_enabled:
            self.ap_enabled = False
            self.flight.controls.set_speed = None
            self.flight.controls.pitch = 0.0
            self.flight.controls.yaw = 0.0
            self.flight.controls.roll = 0.0
            self.msg("Manual control")
            return
        if self.ap_body_id and self.ap_body_id != "star":
            body = self.frames.bodies.get(self.ap_body_id)
            if body is None:
                self.ap_body_id = None
                self.msg("No destination — F6 System Map, click planet/station")
                return
            self.ap_enabled = True
            self.msg(f"Autopilot engaged -> {self._ap_dest_name()}")
            return
        if self.ap_body_id == "star":
            self.msg("Cannot autopilot to the star — pick a planet or station")
            return
        self.msg("No destination — open the System Map and click a planet or station")

    def _ap_dest_name(self) -> str:
        if not self.ap_body_id:
            return "none"
        if self.ap_body_id == "star":
            return self.system.name
        body = self.frames.bodies.get(self.ap_body_id)
        return body.name if body else self.ap_body_id

    def set_hyperspace_target(self, system_id: str) -> None:
        self.selected_system = system_id
        sys = self.galaxy.get(system_id)
        if system_id == self.system_id:
            self.msg(f"{sys.name} — you are here. Click another star, then H to jump.")
            return
        dist = self.galaxy.distance_ly(self.system_id, system_id)
        jump = self.flight.stats.jump_range_ly
        if dist > jump + 1e-6:
            self.msg(f"Target {sys.name} ({dist:.2f} ly) — OUT OF RANGE (max {jump:.2f} ly).")
        else:
            fuel_need = max(0.1, dist / max(jump, 0.1) * 0.5)
            self.msg(
                f"Hyperspace target: {sys.name} ({dist:.2f} ly, {fuel_need:.1f}t). "
                f"Press H to jump."
            )

    def _chart_nearest_system(
        self, *, other_only: bool = True, in_range_only: bool = False
    ) -> str | None:
        """Nearest catalog star to the chart camera centre."""
        best = None
        best_d = 1e18
        center = self.chart.center
        jump = self.flight.stats.jump_range_ly
        for sid, sys in self.galaxy.systems.items():
            if other_only and sid == self.system_id:
                continue
            if in_range_only:
                if self.galaxy.distance_ly(self.system_id, sid) > jump + 1e-6:
                    continue
            d = float(np.linalg.norm(sys.position_ly - center))
            if d < best_d:
                best_d = d
                best = sid
        return best

    def set_local_destination(self, body_id: str) -> None:
        """Set autopilot destination — planets, moons, stations, starports only."""
        if body_id == "star":
            self.selected_body = body_id
            self.msg("Star locked for view only — pick a planet or station for Autopilot.")
            return
        body = self.frames.bodies.get(body_id)
        if body is None:
            self.msg("Unknown body.")
            return
        # Planets/moons are neither station nor starport; stations/starports OK
        self.selected_body = body_id
        self.ap_body_id = body_id
        kind = "Station" if body.is_station else ("Starport" if body.is_starport else "Planet")
        self.msg(f"{kind} destination: {body.name}. F7 Autopilot to engage.")

    def update_autopilot(self) -> None:
        """Steer + set-speed toward planet/station; auto-dock when close."""
        if not self.ap_enabled or not self.ap_body_id or self.docked:
            return
        if self.ap_body_id == "star":
            self.ap_enabled = False
            self.flight.controls.set_speed = None
            self.msg("Autopilot cancelled — cannot fly to the star. Pick a planet/station.")
            return
        days = self.stardate.days
        me = self.frames.ship_world_pos(self.player_state, days)
        if self.ap_body_id not in self.frames.bodies:
            self.ap_enabled = False
            self.msg("Destination lost.")
            return
        dest = self.frames.world_position(self.ap_body_id, days)
        body = self.frames.bodies[self.ap_body_id]
        delta = dest - me
        dist = float(np.linalg.norm(delta))
        # Aim at a standoff point outside the body, not the centre
        if body.is_station:
            from render.structures import dock_mouth, station_orientation

            dest = dock_mouth(dest, station_orientation(days, self.ap_body_id), body.radius_km)
            delta = dest - me
            dist = float(np.linalg.norm(delta))
            standoff = max(body.radius_km * 3.0, 8.0)
        elif body.is_starport:
            # Manual: autopilot holds 20 km above the pad, then descends.
            parent = self.frames.bodies.get(body.parent_id or "")
            if parent is not None and dist > 22.0:
                up = dest - parent.world_pos(days)
                n = float(np.linalg.norm(up))
                if n > 1e-6:
                    dest = dest + (up / n) * 20.0
                    delta = dest - me
                    dist = float(np.linalg.norm(delta))
            standoff = 1.5
        else:
            standoff = max(body.radius_km * 1.8, 40.0)
        if dist > standoff + 1.0:
            aim = delta * (dist - standoff) / dist
        else:
            aim = delta
        pitch, yaw, roll, align = steer_toward(self.player_state.orientation, aim)
        ctrl = self.flight.controls
        ctrl.pitch = pitch
        ctrl.yaw = yaw
        ctrl.roll = roll
        if body.is_station:
            # Manual: the computer matches your roll to the station's spin.
            from render.structures import station_orientation

            station_up = station_orientation(days, self.ap_body_id)[:, 1]
            roll_err = float((self.player_state.orientation.T @ station_up)[0])
            ctrl.roll = float(np.clip(-roll_err * 1.2, -1.0, 1.0))
        ctrl.main = 0.0
        ctrl.set_speed = ap_speed_for_distance(dist, align)

        # Approach station / starport — dock when near and slow
        if body.is_station or body.is_starport:
            if dist < 80.0:
                self.landing_gear = True
            limit = 12.0 if body.is_station else body.radius_km + 8.0
            spd = float(np.linalg.norm(self.player_state.velocity))
            if dist < limit and spd < 0.6 and align > 0.5:
                self.ap_enabled = False
                self.flight.controls.set_speed = 0.0
                self.request_dock()
                return
        # Near planet — hold outside atmosphere / radius
        if not body.is_station and not body.is_starport and dist < standoff + 5.0:
            self.flight.controls.set_speed = 0.0
            self.flight.controls.pitch = 0.0
            self.flight.controls.yaw = 0.0
            self.flight.controls.roll = 0.0
            self.msg(f"Arrived near {body.name}. Autopilot holding.")
            self.ap_enabled = False

    def hyperspace(self) -> None:
        if getattr(self, "cinematic", None):
            return
        if self.docked:
            self.msg("Cannot hyperspace while docked.")
            return

        # Resolve target: explicit selection, or nearest reachable star on the open chart
        target = self.selected_system
        if (not target or target == self.system_id) and self.mode == "chart":
            near = self._chart_nearest_system(other_only=True, in_range_only=True)
            if near is None:
                near = self._chart_nearest_system(other_only=True, in_range_only=False)
            if near:
                self.set_hyperspace_target(near)
                target = near

        if not target or target == self.system_id:
            if self.mode != "chart":
                self.mode = "chart"
                pygame.event.set_grab(False)
                pygame.mouse.set_visible(True)
                self.chart.look_at(self.system.position_ly)
                self.msg("Hyperspace: click a destination star, then press H again.")
            else:
                self.msg("Click a destination star (not your current system), then H.")
            return

        dist = self.galaxy.distance_ly(self.system_id, target)
        from sim.frontier import jump_fuel_t, jump_hours, plan_route
        h = float(getattr(self.flight.stats, "_drive_h", 0.0))
        if h <= 0:
            self.msg("No hyperdrive fitted. This ship cannot leave the system.")
            return
        jump = self.flight.stats.jump_range_ly
        if dist > jump + 1e-6:
            route = plan_route(
                self.system_id,
                target,
                lambda sid: jump,
                self.galaxy.neighbors,
            )
            via = ""
            if route and len(route) > 2:
                via = f" Next hop {self.galaxy.get(route[1]).name}."
            self.msg(f"Out of range ({dist:.2f} ly, drive {jump:.2f} ly).{via}")
            if self.mode != "chart":
                self.mode = "chart"
                self.chart.look_at(self.galaxy.get(target).position_ly)
            return
        mass = max(self.flight.stats.mass_t, 1.0)
        fuel_need = float(jump_fuel_t(dist, mass))
        kind = getattr(self.flight.stats, "_fuel_goods", "hydrogen_fuel")
        have = int(self.fitted.cargo.get(kind, 0))
        if self.infinite_fuel:
            fuel_need = 0.0
        elif have < fuel_need:
            self.msg(
                f"Need {fuel_need:.0f}t {kind.replace('_', ' ')} in the hold "
                f"({have:.0f}t aboard). Internal tanks are for the engines."
            )
            return
        hours = jump_hours(dist, mass, h)

        # Start FE2 hyperspace tunnel — jump completes when cinematic finishes
        dest_name = self.galaxy.get(target).name
        self.ap_enabled = False
        self.flight.controls.set_speed = None
        self.cinematic = (
            "hyperspace",
            0.0,
            3.2,
            {
                "target": target,
                "dist": dist,
                "fuel_need": fuel_need,
                "fuel_kind": kind,
                "hours": hours,
                "name": dest_name,
            },
        )
        self.mode = "cinematic"
        self.msg(f"Engaging hyperspace drive — {dest_name}…")

    def _finish_hyperspace(self, target: str, dist: float, fuel_need: float, fuel_kind: str = "hydrogen_fuel", hours: float = 0.0) -> None:
        origin_name = self.system.name
        dest_name = self.galaxy.get(target).name
        have = int(self.fitted.cargo.get(fuel_kind, 0))
        self.fitted.cargo[fuel_kind] = have - int(fuel_need)
        if self.fitted.cargo[fuel_kind] <= 0:
            self.fitted.cargo.pop(fuel_kind, None)
        self.flight.stats.cargo = dict(self.fitted.cargo)
        if fuel_kind == "military_fuel":
            self.fitted.cargo["radioactives"] = self.fitted.cargo.get("radioactives", 0) + 1
            self.flight.stats.cargo = dict(self.fitted.cargo)
            self.msg("Military fuel spent — 1t radioactives produced.")
        self.stardate.days += (hours / 24.0) if hours > 0 else dist * 0.15
        self.system_id = target
        self.selected_system = target
        self.system = self.galaxy.get(self.system_id)
        self.frames = build_frame_system(self.system)
        self.ap_body_id = None
        self.ap_enabled = False
        self.flight.controls.set_speed = None
        # Deep space inside the system: not beside a planet, not outside the orbits.
        from sim.kepler import AU_KM
        outers = [
            p.elements.semi_major_au
            for p in self.system.planets
            if p.parent_id is None
        ]
        outer_au = max(outers) if outers else 1.0
        seed = abs(hash((dest_name, int(self.stardate.days * 10))))
        arrive_au = outer_au * (0.35 + (seed % 500) / 1000.0)
        arrive_au = max(arrive_au, min(0.12, outer_au * 0.5))
        ang = (seed % 628) / 100.0
        clear_km = 0.06 * AU_KM

        def _at(angle: float, au: float) -> np.ndarray:
            return np.array(
                [math.cos(angle) * au * AU_KM, ((seed % 7) - 3) * 0.01 * AU_KM, math.sin(angle) * au * AU_KM],
                dtype=np.float64,
            )

        pos = _at(ang, arrive_au)
        for n in range(8):
            near = False
            for p in self.system.planets:
                pw = self.frames.world_position(p.id, self.stardate.days)
                if float(np.linalg.norm(pos - pw)) < clear_km + p.radius_km:
                    near = True
                    break
            if not near:
                break
            ang += 0.8
            arrive_au = min(outer_au * 0.92, arrive_au + outer_au * 0.05)
            pos = _at(ang, arrive_au)
        self.player_state = FrameState(parent_id="star", position=pos)
        self.hs_clouds.append({
            "system": self.system_id,
            "pos": pos.copy(),
            "origin": origin_name,
            "dest": dest_name,
            "expires": self.stardate.days + 2.0,
        })
        self.msg(
            f"Hyperspace complete: {dest_name}. "
            f"Deep space, {arrive_au:.2f} AU from the star. F6 to pick a planet."
        )
        self.docked = False
        self.mode = "flight"
        self.cinematic = None
        self.combat = CombatWorld()
        self._rebuild_player_combatant()
        self.spawn_local_traffic()
        self.selected_body = self.ap_body_id
        pygame.event.set_grab(False)
        pygame.mouse.set_visible(True)

    def _update_cinematic(self, dt: float) -> None:
        if not self.cinematic:
            return
        kind, elapsed, duration, payload = self.cinematic
        elapsed += dt
        self.cinematic = (kind, elapsed, duration, payload)
        if elapsed < duration:
            return
        if kind == "hyperspace":
            self._finish_hyperspace(
                payload["target"],
                payload["dist"],
                payload["fuel_need"],
                payload.get("fuel_kind", "hydrogen_fuel"),
                payload.get("hours", 0.0),
            )
        elif kind == "dock":
            self._finish_dock(payload["location"])
        else:
            self.cinematic = None
            self.mode = "flight"

    def _draw_cinematic(self) -> None:
        if not self.cinematic:
            return
        kind, elapsed, duration, payload = self.cinematic
        if kind == "hyperspace":
            draw_hyperspace(
                self.screen,
                elapsed,
                duration,
                dest_name=payload.get("name", ""),
                font=self.font,
            )
        elif kind == "dock":
            draw_docking(
                self.screen,
                elapsed,
                duration,
                station_name=payload.get("name", ""),
                is_station=bool(payload.get("is_station", True)),
                font=self.font,
            )

    def _open_cargo(self) -> None:
        if self.mode != "cargo":
            self._cargo_return = self.mode
        self.mode = "cargo"
        self.ui.state.selected = 0

    def _dumping_is_illegal(self) -> bool:
        """Dumping waste at a port or in its approach is fined."""
        if self.docked:
            return True
        days = self.stardate.days
        me = self.frames.ship_world_pos(self.player_state, days)
        for body in self.frames.bodies.values():
            if not (body.is_station or body.is_starport):
                continue
            if float(np.linalg.norm(me - body.world_pos(days))) < 40.0:
                return True
        return False

    def expand_hold(self) -> None:
        """Pack a tonne of rubbish into extra bay space. The rubbish stays aboard."""
        have = int(self.fitted.cargo.get("rubbish", 0))
        if have < 1:
            self.msg("Need at least 1t of rubbish aboard to expand the hold.")
            return
        self.fitted.bonus_capacity += 1.0
        self.flight.stats.cargo = dict(self.fitted.cargo)
        self.msg(
            f"Hold expanded +1t (bonus {self.fitted.bonus_capacity:.0f}t). Rubbish kept aboard."
        )

    def jettison(self, good_id: str) -> None:
        if good_id == "fuel":
            if self.fitted.fuel_t < 1:
                self.msg("Nothing to jettison.")
                return
            self.fitted.fuel_t = max(0.0, self.fitted.fuel_t - 1.0)
            self.flight.stats.fuel_t = self.fitted.fuel_t
            self.msg("Jettisoned 1t Hydrogen Fuel.")
            return
        have = int(self.fitted.cargo.get(good_id, 0))
        if have <= 0:
            self.msg("Nothing to jettison.")
            return
        good = self.market.goods_by_id.get(good_id)
        name = good.name if good else good_id
        self.fitted.cargo[good_id] = have - 1
        if self.fitted.cargo[good_id] <= 0:
            del self.fitted.cargo[good_id]
        self.flight.stats.cargo = dict(self.fitted.cargo)
        waste = good_id in ("rubbish", "radioactives")
        if waste and self._dumping_is_illegal():
            fine = 100
            self.fines += fine
            self.legal = False
            self.combat.player_legal = False
            self.combat.fines = max(self.combat.fines, self.fines)
            if "player" in self.combat.combatants:
                self.combat.combatants["player"].legal = False
            self.msg(f"Illegal dump of {name} near a port. Fine {fine} Cr.")
        else:
            self.msg(f"Jettisoned 1t {name}.")

    def draw_cargo_screen(self) -> None:
        names = {g.id: g.name for g in self.market.goods}
        self.console_hits = self.ui.draw_cargo(
            self.screen,
            self.font,
            self.small,
            fitted=self.fitted,
            shipyard=self.shipyard,
            names=names,
            stardate=self.stardate.format(),
            compression_index=self.compression.vcr_slot(),
            time_paused=self.time_paused,
            docked=self.docked,
            credits=self.credits,
            view_mode=self.view_mode,
            engines_off=self.engines_off,
            gear_down=self.landing_gear,
            hud_labels=self.show_id_text,
            hyperspace_ready=self.hyperspace_ready(),
            autopilot=self.ap_enabled,
        )

    def _page_kwargs(self) -> dict:
        return dict(
            fitted=self.fitted,
            shipyard=self.shipyard,
            credits=self.credits,
            stardate=self.stardate.format(),
            compression_index=self.compression.vcr_slot(),
            time_paused=self.time_paused,
            docked=self.docked,
            view_mode=self.view_mode,
            engines_off=self.engines_off,
            gear_down=self.landing_gear,
            hud_labels=self.show_id_text,
            hyperspace_ready=self.hyperspace_ready(),
            autopilot=self.ap_enabled,
        )

    def draw_ship_screen(self) -> None:
        self.console_hits = self.ui.draw_ship_status(
            self.screen, self.font, self.small,
            jump_ly=self.flight.stats.jump_range_ly,
            **self._page_kwargs(),
        )

    def draw_captain_screen(self) -> None:
        from sim.frontier import combat_rank
        self.console_hits = self.ui.draw_commander(
            self.screen, self.font, self.small,
            name=self.commander_name,
            legal=self.legal,
            rank=combat_rank(self.kills),
            **self._page_kwargs(),
        )

    def refuel(self) -> None:
        if not self.docked:
            self.msg("Refuel only while docked.")
            return
        room = self.flight.stats.max_fuel_t - self.fitted.fuel_t
        if room < 0.05:
            self.msg("Tanks are full.")
            return
        tonnes = max(1, int(room))
        good = self.market.goods_by_id.get("hydrogen_fuel")
        unit = int(getattr(good, "base_price", 20) or 20)
        cost = unit * tonnes
        if self.credits < cost and not self.free_purchases:
            self.msg("Not enough cash to refuel.")
            return
        if not self.free_purchases:
            self.credits -= cost
        self.fitted.fuel_t = min(self.flight.stats.max_fuel_t, self.fitted.fuel_t + tonnes)
        self.flight.stats.fuel_t = self.fitted.fuel_t
        self.msg(f"Refuelled. Tanks {self.fitted.fuel_t:.0f}t.")

    def _handle_cargo_key(self, key: int) -> None:
        rows = [gid for gid, n in self.fitted.cargo.items() if n > 0]
        if key in (pygame.K_ESCAPE, pygame.K_i):
            self.mode = getattr(self, "_cargo_return", "docked" if self.docked else "flight")
            return
        if not rows:
            return
        if key in (pygame.K_UP, pygame.K_w):
            self.ui.state.selected = (self.ui.state.selected - 1) % len(rows)
        elif key in (pygame.K_DOWN, pygame.K_s):
            self.ui.state.selected = (self.ui.state.selected + 1) % len(rows)
        elif key in (pygame.K_j, pygame.K_RETURN):
            i = max(0, min(self.ui.state.selected, len(rows) - 1))
            self.jettison(rows[i])

    def handle_docked_action(self, action: str) -> None:
        day = int(self.stardate.days)
        listings = self.market.listings(self.system, day, self.dock_location)
        if action == "launch":
            self.launch()
            return
        if action == "emergency_signal":
            self.msg("Long range emergency signal sent.")
            self.ui.state.message = "Emergency signal transmitted."
            return
        if action == "land_request":
            self.mode = "flight"
            self.ui.flight_links = None
            self.request_dock()
            return
        if action == "launch_request":
            if self.launch_clearance:
                # Second confirm on Launch Request = take off
                self.launch()
            else:
                self.request_launch_clearance()
            return
        if action == "repair":
            deluxe, drive, equip = self.shipyard.service_prices(self.fitted)
            choice = min(self.ui.state.selected, 2)
            cost = (deluxe, drive, equip)[choice]
            if self.credits < cost and not self.free_purchases:
                self.ui.state.message = "Cannot afford service."
                return
            if not self.free_purchases:
                self.credits -= cost
            if choice != 2:
                self.fitted.last_drive_service = self.stardate.days
            if choice != 1:
                self.fitted.last_equip_service = self.stardate.days
            self.fitted.hull = self.fitted.max_hull
            self.flight.stats.hull = self.flight.stats.max_hull
            self.ui.state.message = "Service complete."
            return
        if action == "pay_fine":
            if self.fines <= 0:
                self.ui.state.message = "No fines."
                return
            if self.credits < self.fines:
                self.ui.state.message = "Cannot afford fines."
                return
            self.credits -= self.fines
            self.fines = 0
            self.legal = True
            self.combat.player_legal = True
            self.ui.state.message = "Fines paid. Legal status restored."
            return
        if action == "buy" and self.ui.state.tab == "stock":
            if not listings:
                return
            i = min(self.ui.state.selected, len(listings) - 1)
            L = listings[i]
            has_life = getattr(self.flight.stats, "_has_life_support", False)
            ok, reason = self.market.can_carry(L.good.id, has_life)
            if not ok:
                self.ui.state.message = reason
                return
            free = self.fitted.free_cargo(self.shipyard.ships, self.shipyard.equipment)
            if free < 1:
                self.ui.state.message = "No cargo space."
                return
            if L.stock <= 0:
                self.ui.state.message = "Out of stock."
                return
            # Disposal goods: negative price means you pay them to take it — buying doesn't apply
            pay = L.price
            if pay < 0:
                # Station pays you to haul waste (rubbish, radioactives)
                self.credits += -pay
                self.fitted.cargo[L.good.id] = self.fitted.cargo.get(L.good.id, 0) + 1
                self.flight.stats.cargo = dict(self.fitted.cargo)
                self.ui.state.message = f"Took 1t {L.good.name} (+{-pay} Cr)"
                return
            if self.free_purchases:
                pay = 0
            if self.credits < pay:
                self.ui.state.message = "Insufficient credits."
                return
            self.credits -= pay
            self.fitted.cargo[L.good.id] = self.fitted.cargo.get(L.good.id, 0) + 1
            self.flight.stats.cargo = dict(self.fitted.cargo)
            self.ui.state.message = f"Bought 1t {L.good.name}" + (" (free)" if self.free_purchases else "")
            return
        if action == "sell" and self.ui.state.tab == "stock":
            if not listings:
                return
            i = min(self.ui.state.selected, len(listings) - 1)
            L = listings[i]
            have = self.fitted.cargo.get(L.good.id, 0)
            if have <= 0:
                self.ui.state.message = "None in hold."
                return
            self.fitted.cargo[L.good.id] = have - 1
            if self.fitted.cargo[L.good.id] <= 0:
                del self.fitted.cargo[L.good.id]
            # Disposal: price negative — you pay |price|
            if L.price < 0:
                cost = -L.price
                if self.credits < cost:
                    self.fitted.cargo[L.good.id] = self.fitted.cargo.get(L.good.id, 0) + 1
                    self.ui.state.message = "Cannot afford disposal fee."
                    return
                self.credits -= cost
                self.ui.state.message = f"Disposed 1t {L.good.name} (-{cost} Cr)"
            else:
                self.credits += L.price
                self.ui.state.message = f"Sold 1t {L.good.name}"
            self.flight.stats.cargo = dict(self.fitted.cargo)
            return
        if action == "buy" and self.ui.state.tab == "ships":
            ships = self.shipyard.available_ships(self.system)
            if not ships:
                return
            i = min(self.ui.state.selected, len(ships) - 1)
            new_fitted, new_credits, msg = self.shipyard.buy_ship(
                ships[i].id, self.fitted, self.credits, free=self.free_purchases
            )
            self.ui.state.message = msg
            if new_fitted:
                self.fitted = new_fitted
                self.credits = new_credits
                self._rebuild_player_combatant()
            return
        if action == "fit" and self.ui.state.tab == "outfit":
            items = self.shipyard.available_equipment(self.system)
            if not items:
                return
            i = min(self.ui.state.selected, len(items) - 1)
            e = items[i]
            price = 0 if self.free_purchases else e.price
            if self.credits < price:
                self.ui.state.message = "Insufficient credits."
                return
            ok, msg = self.shipyard.fit(self.fitted, e.id)
            self.ui.state.message = msg + (" (free)" if ok and self.free_purchases else "")
            if ok:
                self.credits -= price
                self._rebuild_player_combatant()
            return
        if action == "remove" and self.ui.state.tab == "outfit":
            items = self.shipyard.available_equipment(self.system)
            if not items:
                return
            i = min(self.ui.state.selected, len(items) - 1)
            e = items[i]
            ok, msg, refund = self.shipyard.remove(self.fitted, e.id)
            self.ui.state.message = msg
            if ok:
                self.credits += refund
                self._rebuild_player_combatant()
            return
        if action == "bbs_yes" and self.ui.state.tab == "videolink":
            if not self.board_missions:
                self.ui.state.tab = "missions"
                return
            i = min(getattr(self.ui, "bbs_index", 0), len(self.board_missions) - 1)
            m = self.board_missions[i]
            if getattr(m, "kind", "cargo") == "passenger":
                cabins = int(getattr(self.flight.stats, "_passenger_cabins", 0))
                aboard = int(self.fitted.cargo.get("passenger", 0))
                if aboard + m.quantity > cabins:
                    self.ui.bbs_note = "You don't have enough passenger space."
                    self.ui.state.message = self.ui.bbs_note
                    return
                self.fitted.cargo["passenger"] = aboard + m.quantity
                self.flight.stats.cargo = dict(self.fitted.cargo)
                self.missions.accept(m)
                self.board_missions.pop(i)
                self.ui.bbs_note = ""
                self.ui.state.tab = "missions"
                self.ui.state.message = f"Passage agreed. ${m.reward} on arrival."
                return
            actions = getattr(self.ui, "bbs_actions", [])
            pick = self.ui.state.selected
            if pick >= len(actions) or actions[pick][0] != "sell":
                return
            qty = actions[pick][1]
            have = int(self.fitted.cargo.get(m.cargo_id, 0))
            sell_n = have if qty < 0 else qty
            unit = m.pay_each or float(m.reward)
            if sell_n <= 0 or have < sell_n:
                self.ui.bbs_note = "You don't have that cargo."
                self.ui.state.message = self.ui.bbs_note
                return
            left = have - sell_n
            if left <= 0:
                del self.fitted.cargo[m.cargo_id]
            else:
                self.fitted.cargo[m.cargo_id] = left
            self.flight.stats.cargo = dict(self.fitted.cargo)
            paid = unit * sell_n
            self.credits += int(paid)
            self.ui.bbs_note = f"Sold {sell_n}t {m.cargo_name} for ${paid:.1f}."
            self.ui.state.message = self.ui.bbs_note
            return
        if action == "accept" and self.ui.state.tab == "missions":
            if not self.board_missions:
                return
            i = min(self.ui.state.selected, len(self.board_missions) - 1)
            m = self.board_missions[i]
            # Need free cargo for delivery goods — give cargo to player for delivery missions
            if getattr(m, "kind", "cargo") == "passenger":
                cabins = int(getattr(self.flight.stats, "_passenger_cabins", 0))
                aboard = int(self.fitted.cargo.get("passenger", 0))
                if aboard + m.quantity > cabins:
                    self.ui.state.message = "No free passenger cabin."
                    return
            else:
                free = self.fitted.free_cargo(self.shipyard.ships, self.shipyard.equipment)
                if free < m.quantity:
                    self.ui.state.message = "Not enough cargo space for contract."
                    return
            self.fitted.cargo[m.cargo_id] = self.fitted.cargo.get(m.cargo_id, 0) + m.quantity
            self.flight.stats.cargo = dict(self.fitted.cargo)
            self.missions.accept(m)
            self.board_missions.pop(i)
            self.ui.state.message = f"Accepted: {m.description}"
            return

    def _save_settings(self) -> None:
        data = {
            "detail_level": self.detail_level,
            "music_on": list(self.music_on),
            "opt": self.opt,
        }
        SETTINGS_PATH.write_text(json.dumps(data, indent=2), encoding="utf-8")

    def _load_settings(self) -> None:
        if not SETTINGS_PATH.is_file():
            return
        try:
            data = json.loads(SETTINGS_PATH.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return
        self.detail_level = int(data.get("detail_level", self.detail_level))
        saved_music = data.get("music_on")
        if isinstance(saved_music, list) and len(saved_music) == 8:
            self.music_on = [bool(v) for v in saved_music]
        for key, value in data.get("opt", {}).items():
            if key in self.opt:
                self.opt[key] = bool(value)

    def save(self) -> None:
        self._sync_fitted_from_flight()
        data = {
            "days": self.stardate.days,
            "system_id": self.system_id,
            "credits": self.credits,
            "legal": self.legal,
            "fines": self.fines,
            "kills": self.kills,
            "mb4_sites": self.mb4_sites,
            "active_missions": [
                {
                    "id": m.id,
                    "description": m.description,
                    "dest_system": m.dest_system,
                    "dest_location": m.dest_location,
                    "dest_name": m.dest_name,
                    "cargo_id": m.cargo_id,
                    "cargo_name": m.cargo_name,
                    "quantity": m.quantity,
                    "reward": m.reward,
                    "deadline_day": m.deadline_day,
                    "kind": getattr(m, "kind", "cargo"),
                }
                for m in self.missions.active
            ],
            "docked": self.docked,
            "dock_location": self.dock_location,
            "fitted": {
                "ship_id": self.fitted.ship_id,
                "equipment": self.fitted.equipment,
                "guns": self.fitted.guns,
                "missile_load": self.fitted.missile_load,
                "cargo": self.fitted.cargo,
                "bonus_capacity": self.fitted.bonus_capacity,
                "fuel_t": self.fitted.fuel_t,
                "hull": self.fitted.hull,
                "max_hull": self.fitted.max_hull,
            },
            "position": self.player_state.position.tolist(),
            "velocity": self.player_state.velocity.tolist(),
            "parent_id": self.player_state.parent_id,
        }
        SAVE_PATH.write_text(json.dumps(data, indent=2), encoding="utf-8")
        self.msg("Game saved.")

    def load(self) -> None:
        if not SAVE_PATH.exists():
            self.msg("No save game.")
            return
        data = json.loads(SAVE_PATH.read_text(encoding="utf-8"))
        self.stardate.days = data["days"]
        self.system_id = data["system_id"]
        self.system = self.galaxy.get(self.system_id)
        self.frames = build_frame_system(self.system)
        self.credits = data["credits"]
        self.legal = data["legal"]
        self.fines = data["fines"]
        self.kills = int(data.get("kills", 0))
        self.mb4_sites = list(data.get("mb4_sites", []))
        self.missions.active = []
        for raw in data.get("active_missions", []):
            from sim.mission import Mission
            self.missions.active.append(Mission(
                id=raw["id"],
                description=raw["description"],
                dest_system=raw["dest_system"],
                dest_location=raw["dest_location"],
                dest_name=raw["dest_name"],
                cargo_id=raw["cargo_id"],
                cargo_name=raw["cargo_name"],
                quantity=raw["quantity"],
                reward=raw["reward"],
                deadline_day=raw["deadline_day"],
                taken=True,
                kind=raw.get("kind", "cargo"),
            ))
        self.docked = data["docked"]
        self.dock_location = data["dock_location"]
        f = data["fitted"]
        self.fitted.ship_id = f["ship_id"]
        self.fitted.equipment = f["equipment"]
        self.fitted.guns = f["guns"]
        self.fitted.missile_load = f["missile_load"]
        self.fitted.cargo = f["cargo"]
        self.fitted.bonus_capacity = float(f.get("bonus_capacity", 0))
        self.fitted.fuel_t = f["fuel_t"]
        self.fitted.hull = f["hull"]
        self.fitted.max_hull = f["max_hull"]
        self.player_state = FrameState(
            parent_id=data["parent_id"],
            position=np.array(data["position"], dtype=np.float64),
            velocity=np.array(data["velocity"], dtype=np.float64),
        )
        self.combat = CombatWorld()
        self._rebuild_player_combatant()
        if self.docked:
            self._open_berth()
        else:
            self.mode = "flight"
        self.msg("Game loaded.")

    def _service_ship(self, sim_dt: float) -> None:
        """Autorefueller, scoop, hull repair, escape capsule. Uses fitted equipment."""
        stats = self.flight.stats
        if "auto_refueller" in self.fitted.equipment and stats.fuel_t < stats.max_fuel_t - 0.05:
            kind = getattr(stats, "_fuel_goods", "hydrogen_fuel")
            held = int(self.fitted.cargo.get(kind, 0))
            if held >= 1:
                self.fitted.cargo[kind] = held - 1
                if self.fitted.cargo[kind] <= 0:
                    del self.fitted.cargo[kind]
                stats.fuel_t = min(stats.max_fuel_t, stats.fuel_t + 1.0)
                self.fitted.fuel_t = stats.fuel_t
                self.flight.stats.cargo = dict(self.fitted.cargo)
                self.msg("Autorefueller transferred 1t into the internal tank.")
        if getattr(stats, "_has_fuel_scoop", False) and not self.docked:
            body = self.frames.bodies.get(self.player_state.parent_id)
            alt = self.frames.altitude(self.player_state, self.stardate.days)
            planet = next((p for p in self.system.planets if p.id == getattr(body, "id", "")), None)
            scooping = bool(
                planet and planet.is_gas_giant and body and body.atmosphere_km > 0 and 0 < alt < body.atmosphere_km
            )
            if scooping:
                self._scoop_bank = getattr(self, "_scoop_bank", 0.0) + sim_dt
                free = self.fitted.free_cargo(self.shipyard.ships, self.shipyard.equipment)
                if free >= 1 and self._scoop_bank >= 8.0:
                    self._scoop_bank = 0.0
                    self.fitted.cargo["hydrogen_fuel"] = self.fitted.cargo.get("hydrogen_fuel", 0) + 1
                    self.flight.stats.cargo = dict(self.fitted.cargo)
                    self.msg("Fuel scoop filled 1t hydrogen into the hold.")
                elif stats.fuel_t < stats.max_fuel_t:
                    stats.fuel_t = min(stats.max_fuel_t, stats.fuel_t + 0.05 * sim_dt)
                    self.fitted.fuel_t = stats.fuel_t
        repair = float(getattr(stats, "_hull_repair", 0.0))
        if repair > 0 and stats.hull < stats.max_hull:
            stats.hull = min(stats.max_hull, stats.hull + repair * sim_dt * 0.01)
            self.fitted.hull = stats.hull
        if stats.hull <= 0 and not self.god_mode:
            if "escape_capsule" in self.fitted.equipment:
                self.msg("Escape capsule launched. The ship is lost.")
                self.fitted = default_eagle()
                self.credits = max(0, self.credits)
                self.docked = True
                self.dock_location = self._first_dock(self.system_id)
                self.mode = "docked"
                self._rebuild_player_combatant()
            else:
                self.msg("Ship destroyed.")
                self.mode = "title"
                self.title_phase = "menu"

    def update_flight(self, dt: float) -> None:
        days = self.stardate.days
        self.flight.controls.engines_off = self.engines_off
        # Destination autopilot sets pitch/yaw/set_speed before integration.
        # Engines-off leaves the existing velocity alone.
        if not self.engines_off:
            self.update_autopilot()
        # Drop to realtime only when actually in danger — not because a pirate
        # exists somewhere in the system, and not for the whole climb-out.
        alt = self.frames.altitude(self.player_state, days)
        me = self.frames.ship_world_pos(self.player_state, days)
        hostile_close = False
        for c in self.combat.combatants.values():
            if not c.alive or c.role not in ("pirate", "police"):
                continue
            if c.role == "police" and self.legal:
                continue
            d = float(np.linalg.norm(self.frames.ship_world_pos(c.state, days) - me))
            if d < 20.0:
                hostile_close = True
                break
        near_surface = 0 < alt < 8.0
        was_locked = self.compression.force_realtime
        self.compression.update_auto(near_surface, hostile_close)
        if self.compression.force_realtime and not was_locked:
            why = "hostile nearby" if hostile_close else "too close to the surface"
            self.msg(f"Stardreamer held at x1 — {why}")

        # Full x10000 is minutes of flight per frame. That many slices locks the game.
        raw = dt * float(self.compression.factor)
        budget = min(raw, 4.0)
        self.stardate.days += budget / 86400.0
        sim_dt = budget
        # At x1000 / x10000 one frame is many seconds. Autopilot must steer
        # again each slice, or a full turn-rate command spins the ship past
        # the destination before the stick is updated.
        remaining = sim_dt
        steps = 0
        while remaining > 1e-6 and steps < 12:
            if self.ap_enabled and not self.engines_off:
                self.update_autopilot()
            if self.docked or getattr(self, "cinematic", None):
                break
            ctrl = self.flight.controls
            turning = max(abs(ctrl.pitch), abs(ctrl.yaw), abs(ctrl.roll))
            if self.ap_enabled and turning > 0.05:
                h = 0.2
            else:
                h = 2.0
            if self.ap_enabled:
                spd = abs(ctrl.set_speed or 0.0)
                if spd > 0.2:
                    h = min(h, max(0.05, 15.0 / spd))
            h = min(h, remaining)
            world = self.frames.ship_world_pos(self.player_state, self.stardate.days)
            star_dist = float(np.linalg.norm(world))
            heat = 0.0
            if star_dist < 200000:
                heat = 50.0 * (200000 - star_dist) / 200000
            self.player_state = self.flight.step(self.player_state, h, heat)
            remaining -= h
            steps += 1

        self._service_ship(sim_dt)
        self.player_state = self.frames.auto_rebase(self.player_state, self.stardate.days)
        self.combat.god_mode = self.god_mode
        if self.infinite_fuel:
            self.flight.stats.fuel_t = self.flight.stats.max_fuel_t
            self.fitted.fuel_t = self.flight.stats.fuel_t
        if self.god_mode:
            self.flight.stats.hull = self.flight.stats.max_hull
            self.flight.stats.shield = self.flight.stats.max_shield
            self.flight.stats.energy = self.flight.stats.max_energy
            self.fitted.hull = self.flight.stats.hull
        self.combat.combatants["player"].state = self.player_state
        self.combat.combatants["player"].legal = self.legal
        self.laser_cool = max(0.0, self.laser_cool - dt)

        # AI
        def fire_ai(c: Combatant) -> None:
            origin = self.frames.ship_world_pos(c.state, self.stardate.days)
            fwd = c.state.orientation[:, 2]
            self.combat.fire_laser(c, fwd, origin)

        player = self.combat.combatants["player"]
        for ai in self.ai_controllers:
            if ai.c.id in self.combat.combatants:
                ai.c = self.combat.combatants[ai.c.id]
                ai.update(min(sim_dt, 0.5), player, self.frames, self.stardate.days, fire_ai)

        def wpos(c: Combatant) -> np.ndarray:
            return self.frames.ship_world_pos(c.state, self.stardate.days)

        for m in self.combat.step(min(sim_dt, 0.5), wpos):
            self.msg(m)
            if m.startswith("Bounty claimed:"):
                try:
                    self.credits += int(m.split(":")[1].split()[0])
                except ValueError:
                    pass
        if self.combat.player_kills:
            from sim.frontier import combat_rank
            before = combat_rank(self.kills)
            self.kills += self.combat.player_kills
            self.combat.player_kills = 0
            after = combat_rank(self.kills)
            if after != before:
                self.msg(f"Combat rating: {after}")
        if self.combat.fines > self.fines:
            self.fines = self.combat.fines
            self.legal = self.combat.player_legal

        # Surface landing at a starport (gear down, slow, not in post-launch grace)
        now = pygame.time.get_ticks() / 1000.0
        if now < getattr(self, "_launch_grace_until", 0.0):
            pass  # just took off — do not snap back to the pad
        elif self.landing_gear:
            days_now = self.stardate.days
            world = self.frames.ship_world_pos(self.player_state, days_now)
            nearest_port = None
            nearest_d = 1e18
            for bid, body in self.frames.bodies.items():
                if not body.is_starport or body.is_station:
                    continue
                d = float(np.linalg.norm(world - body.world_pos(days_now)))
                if d < nearest_d:
                    nearest_d = d
                    nearest_port = bid
            if nearest_port and nearest_d < 5.0:
                spd = float(np.linalg.norm(self.player_state.velocity))
                if spd < 0.15 and not self.cinematic:
                    body = self.frames.bodies[nearest_port]
                    self.ap_enabled = False
                    self.flight.controls.set_speed = 0.0
                    self.cinematic = (
                        "dock",
                        0.0,
                        2.4,
                        {
                            "location": nearest_port,
                            "is_station": False,
                            "name": body.name,
                        },
                    )
                    self.mode = "cinematic"
                    self.msg(f"Landing at {body.name}…")
                    return

        # Atmosphere drag
        body = self.frames.bodies[self.player_state.parent_id]
        if body.atmosphere_km > 0:
            alt = self.frames.altitude(self.player_state, self.stardate.days)
            spd_atm = float(np.linalg.norm(self.player_state.velocity))
            if 0 < alt < body.atmosphere_km:
                drag = 0.02 * (1.0 - alt / body.atmosphere_km)
                self.player_state.velocity *= max(0.0, 1.0 - drag * min(sim_dt, 1.0))
                if (
                    not self.god_mode
                    and not getattr(self.flight.stats, "_has_atmo_shield", False)
                    and spd_atm > 5
                ):
                    self.flight.stats.hull -= 5 * min(sim_dt, 0.5)
                    self.fitted.hull = self.flight.stats.hull

    def handle_flight_input(self, keys: pygame.key.ScancodeWrapper) -> None:
        """FE2 flight: A/Z pitch, ,/. roll, Enter/RShift thrust; RMB mouse look."""
        ctrl = self.flight.controls
        ctrl.engines_off = self.engines_off
        # Thrust — Enter forward, Right Shift reverse (WASD still aliased)
        fwd = 1.0 if (keys[pygame.K_RETURN] or keys[pygame.K_w]) else 0.0
        back = 1.0 if (keys[pygame.K_RSHIFT] or keys[pygame.K_s]) else 0.0
        ctrl.main = 0.0 if self.engines_off else (fwd - back)
        # Pitch — A down, Z up (FE2)
        pitch = 0.0
        if keys[pygame.K_a]:
            pitch -= 1.0
        if keys[pygame.K_z]:
            pitch += 1.0
        # Roll — comma port, period starboard
        roll = 0.0
        if keys[pygame.K_COMMA]:
            roll -= 1.0
        if keys[pygame.K_PERIOD]:
            roll += 1.0
        ctrl.yaw = (-1.0 if keys[pygame.K_q] else 0.0)
        ctrl.vertical = (1.0 if keys[pygame.K_PAGEUP] else 0.0) + (
            -1.0 if keys[pygame.K_PAGEDOWN] else 0.0
        )
        ctrl.lateral = 0.0
        ctrl.pitch = pitch
        ctrl.roll = roll

        # Right mouse button + move = direct orientation (FE2)
        buttons = pygame.mouse.get_pressed(num_buttons=3)
        if buttons[2] and not self.docked and self.mode == "flight":
            mx, my = pygame.mouse.get_rel()
            ctrl.yaw = float(np.clip(mx * 0.02, -1, 1))
            ctrl.pitch = float(np.clip(-my * 0.02, -1, 1))
        elif self.mouse_look and not self.docked and self.mode == "flight" and buttons[2]:
            pass
        else:
            pygame.mouse.get_rel()  # clear delta when not steering

    def fire_laser(self) -> None:
        if self.laser_cool > 0:
            return
        player = self.combat.combatants["player"]
        origin = self.frames.ship_world_pos(self.player_state, self.stardate.days)
        fwd = self.player_state.orientation[:, 2]
        if self.combat.fire_laser(player, fwd, origin):
            rate = getattr(self.flight.stats, "_fire_rate", 1.6)
            self.laser_cool = 1.0 / max(rate, 0.1)
            if getattr(self.flight.stats, "_mining_laser", False):
                self.msg("Mining laser fired.")

    def _mb4_use(self) -> None:
        """FAQ mining: unload an MB4 on a bare world, wait, then collect. Yield falls each visit."""
        if "mining_machine" not in self.fitted.equipment:
            self.msg("No MB4 mining machine fitted.")
            return
        days = self.stardate.days
        me = self.frames.ship_world_pos(self.player_state, days)
        deps = getattr(self, "mb4_sites", None)
        if deps is None:
            self.mb4_sites = []
            deps = self.mb4_sites
        for site in deps:
            if site["system"] != self.system_id:
                continue
            body = self.frames.bodies.get(site["body"])
            if body is None:
                continue
            dist = float(np.linalg.norm(me - body.world_pos(days))) - body.radius_km
            if dist > 40:
                continue
            if days < site["ready_day"]:
                self.msg("The MB4 is still working. Come back in a few months.")
                return
            site["visits"] = site.get("visits", 0) + 1
            self.fitted.equipment.remove("mining_machine")
            # Manual: the MB4 hopper holds 10 t. How long it takes to fill is not stated.
            tonnes = 10
            free = self.fitted.free_cargo(self.shipyard.ships, self.shipyard.equipment)
            take = min(tonnes, int(free))
            if take:
                self.fitted.cargo["minerals"] = self.fitted.cargo.get("minerals", 0) + take
                self.flight.stats.cargo = dict(self.fitted.cargo)
                self.msg(f"MB4 hopper loaded {take}t minerals.")
            else:
                self.msg("MB4 hopper is full but the hold has no space.")
            self._rebuild_player_combatant()
            return
        body_id = self.player_state.parent_id
        planet = next((p for p in self.system.planets if p.id == body_id), None)
        if planet is None or planet.is_gas_giant or planet.starports or planet.inhabited:
            self.msg("MB4 needs an uninhabited world with no starport.")
            return
        self.fitted.equipment.remove("mining_machine")
        self.mb4_sites.append({
            "system": self.system_id,
            "body": body_id,
            "ready_day": days + 60.0,
            "visits": 0,
        })
        self._rebuild_player_combatant()
        self.msg(f"MB4 unloaded on {planet.name}. Return after a few months.")

    def _mine_if_close(self, origin: np.ndarray) -> None:
        days = self.stardate.days
        for body in self.frames.bodies.values():
            if body.is_station or body.is_starport or body.radius_km <= 0 or body.radius_km > 20000:
                continue
            dist = float(np.linalg.norm(origin - body.world_pos(days))) - body.radius_km
            if 0 <= dist < 30.0:
                free = self.fitted.free_cargo(self.shipyard.ships, self.shipyard.equipment)
                if free < 1:
                    self.msg("Hold full — mined material lost.")
                    return
                self.fitted.cargo["minerals"] = self.fitted.cargo.get("minerals", 0) + 1
                self.flight.stats.cargo = dict(self.fitted.cargo)
                self._rebuild_player_combatant()
                self.msg(f"Mined 1t minerals from {body.name}.")
                return

    def fire_missile(self) -> None:
        player = self.combat.combatants["player"]
        origin = self.frames.ship_world_pos(self.player_state, self.stardate.days)
        if self.combat.fire_missile(player, origin, self.target_id):
            if self.fitted.missile_load:
                self.fitted.missile_load.pop()
            self.flight.stats.missiles = len(self.fitted.missile_load)
            self.msg("Missile away.")

    def cycle_target(self) -> None:
        ids = [cid for cid, c in self.combat.combatants.items() if c.alive and cid != "player"]
        if not ids:
            self.target_id = None
            return
        if self.target_id not in ids:
            self.target_id = ids[0]
        else:
            i = ids.index(self.target_id)
            self.target_id = ids[(i + 1) % len(ids)]

    def _flight_camera(self, days: float) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, int]:
        """Ship pose plus the camera for the current front / rear / external view."""
        from render.console_bar import console_bar_height
        ship_pos = self.frames.ship_world_pos(self.player_state, days)
        ship_rot = self.player_state.orientation
        cam_pos = ship_pos.copy()
        cam_rot = ship_rot
        if self.docked and self.dock_location in self.frames.bodies:
            body = self.frames.bodies[self.dock_location]
            if body.is_station:
                from render.structures import station_orientation
                bay = station_orientation(days, body.id)
                cam_pos = body.world_pos(days)
                cam_rot = bay
                ship_rot = bay
            elif body.is_starport and body.parent_id:
                from render.structures import surface_frame
                port = body.world_pos(days)
                planet = self.frames.bodies[body.parent_id].world_pos(days)
                up = port - planet
                ground = surface_frame(up)
                cam_pos = port + ground[:, 1] * 0.04
                cam_rot = ground
                ship_rot = ground
        if self.view_mode == "rear":
            cam_rot = np.column_stack((-ship_rot[:, 0], ship_rot[:, 1], -ship_rot[:, 2]))
        elif self.view_mode == "external":
            # Orbit around the ship. The hull stays in its flight orientation.
            from render.ship_models import ship_scale

            dist = ship_scale(self.fitted.ship_id) * 6.5 * self.ext_zoom
            yaw, pitch = self.ext_yaw, self.ext_pitch
            local = np.array(
                [
                    dist * np.cos(pitch) * np.sin(yaw),
                    dist * np.sin(pitch),
                    -dist * np.cos(pitch) * np.cos(yaw),
                ]
            )
            offset = ship_rot @ local
            cam_pos = ship_pos + offset
            forward = -offset / max(float(np.linalg.norm(offset)), 1e-6)
            up_hint = ship_rot[:, 1]
            right = np.cross(forward, up_hint)
            if float(np.linalg.norm(right)) < 1e-4:
                right = ship_rot[:, 0]
            right = right / max(float(np.linalg.norm(right)), 1e-6)
            up = np.cross(right, forward)
            cam_rot = np.column_stack((right, up, forward))
        view_h = HEIGHT - console_bar_height(HEIGHT)
        return ship_pos, ship_rot, cam_pos, cam_rot, view_h

    def _pick_view_target(self, x: int, y: int) -> tuple[str, str] | None:
        """Click a planet, station, starport, or ship in the flight view."""
        days = self.stardate.days
        _ship_pos, _ship_rot, cam_pos, cam_rot, view_h = self._flight_camera(days)
        if y >= view_h:
            return None
        fov = 200.0
        best: tuple[float, str, str] | None = None

        def consider(world: np.ndarray, kind: str, bid: str, radius_km: float) -> None:
            nonlocal best
            sx, sy, depth, in_front = project_body(world, cam_pos, cam_rot, WIDTH, view_h, fov)
            if in_front and depth > 0.2:
                pix = fov * radius_km / depth
                reach = max(14.0, min(pix, 70.0))
                px, py = sx, sy
            else:
                px, py, _off = edge_clamp(sx, sy, WIDTH, view_h)
                reach = 16.0
            d = math.hypot(px - x, py - y)
            if d <= reach and (best is None or d < best[0]):
                best = (d, kind, bid)

        consider(np.zeros(3), "body", "star", 0.0)
        for p in self.system.planets:
            consider(self.frames.world_position(p.id, days), "body", p.id, p.radius_km)
        for st in self.system.stations:
            consider(self.frames.world_position(st.id, days), "body", st.id, max(st.radius_km, 1.0))
        for bid, b in self.frames.bodies.items():
            if b.is_starport and not b.is_station:
                consider(self.frames.world_position(bid, days), "body", bid, max(b.radius_km, 1.5))
        for cid, c in self.combat.combatants.items():
            if not c.alive or cid == "player":
                continue
            consider(self.frames.ship_world_pos(c.state, days), "ship", cid, 0.6)
        if best is None:
            return None
        return best[1], best[2]

    def draw_flight(self) -> None:
        self.screen.fill((0, 0, 0))
        days = self.stardate.days
        ship_pos, ship_rot, cam_pos, cam_rot, view_h = self._flight_camera(days)
        view = self.screen.subsurface((0, 0, WIDTH, view_h))
        view.fill((0, 0, 8))
        if self.docked and self.dock_location in self.frames.bodies:
            dock_body = self.frames.bodies[self.dock_location]
            if dock_body.is_starport and not dock_body.is_station and dock_body.parent_id:
                planet = self.frames.bodies[dock_body.parent_id]
                ground = planet.color if hasattr(planet, "color") else (90, 70, 50)
                sky = (8, 10, 28)
                view.fill(sky)
                pygame.draw.rect(view, ground, (0, view_h // 2, WIDTH, view_h // 2))
        light = -cam_pos
        if float(np.linalg.norm(light)) < 1:
            light = np.array([0.0, 0.0, 1.0])
        fov = 200.0

        # Nearby stars, wrapped around the ship, so thrust and time compression move them.
        if not hasattr(self, "_fly_stars"):
            rng = np.random.default_rng(154)
            self._fly_stars = rng.uniform(-1.0, 1.0, (320, 3))
        span = 70.0
        half = span * 0.5
        # Current rush is the ×10000 look. Lower Stardreamer rates scale it down.
        rate = self.compression.factor / 10000.0
        vel = self.player_state.velocity
        dt = self.clock.get_time() / 1000.0
        if not hasattr(self, "_star_scroll"):
            self._star_scroll = np.zeros(3)
        self._star_scroll = self._star_scroll + vel * (rate * dt)
        delta = np.mod(self._fly_stars * half - self._star_scroll + half, span) - half
        pos = cam_pos + delta
        speed = float(np.linalg.norm(vel)) * rate
        streak = min(speed * 0.15, half * 0.85) if speed > 0.02 else 0.0
        if streak > 0 and float(np.linalg.norm(vel)) > 1e-6:
            back = pos - vel / np.linalg.norm(vel) * streak
        else:
            back = pos
        star_keep = (40, 80, 160, 320)[min(self.detail_level, 3)]
        if not self.opt.get("stars", True):
            star_keep = 0
        for i in range(min(star_keep, len(pos))):
            rel = (pos[i] - cam_pos) @ cam_rot
            z = float(rel[2])
            if z < 0.4:
                continue
            x = WIDTH / 2 + fov * float(rel[0]) / z
            y = view_h / 2 - fov * float(rel[1]) / z
            if not (0 <= x < WIDTH and 0 <= y < view_h):
                continue
            if streak > 0.5:
                rel_b = (back[i] - cam_pos) @ cam_rot
                zb = float(rel_b[2])
                if zb > 0.4:
                    xb = WIDTH / 2 + fov * float(rel_b[0]) / zb
                    yb = view_h / 2 - fov * float(rel_b[1]) / zb
                    pygame.draw.line(view, (170, 180, 210), (int(xb), int(yb)), (int(x), int(y)), 1)
                else:
                    view.set_at((int(x), int(y)), (220, 220, 235))
            else:
                view.set_at((int(x), int(y)), (180, 180, 200))

        draw_sun(view, np.zeros(3), cam_pos, cam_rot, self.system.star_color(), fov=fov)
        for p in self.system.planets:
            pos = self.frames.world_position(p.id, days)
            draw_sphere_approx(
                view, pos, p.radius_km, cam_pos, cam_rot, p.color, light,
                fov=fov,
                rings=p.has_rings and self.detail_level >= 1,
                atmosphere=p.atmosphere_km > 0 and self.detail_level >= 2,
                gas=p.is_gas_giant, spin=days / max(p.rotation_days, 0.1),
            )
        from render.structures import orbital_station_mesh, starport_mesh, station_orientation, surface_frame

        cleared_id = getattr(self, "dock_clearance_id", None)
        for st in self.system.stations:
            pos = self.frames.world_position(st.id, days)
            ori = station_orientation(days, st.id)
            cleared = cleared_id == st.id
            flash_on = (int(days * 86400.0 * 2.0) % 2) == 0
            draw_mesh(
                view, orbital_station_mesh(cleared, flash_on or cleared), ori, pos, cam_pos, cam_rot, light,
                scale=max(st.radius_km, 0.6), fov=fov,
                interior=self.docked and st.id == self.dock_location,
            )
        for bid, body in self.frames.bodies.items():
            if not body.is_starport:
                continue
            pos = body.world_pos(days)
            parent = self.frames.bodies.get(body.parent_id or "")
            if parent is None:
                continue
            normal = pos - parent.world_pos(days)
            dist = float(np.linalg.norm(pos - cam_pos))
            if dist > 4000:
                continue
            sealed = parent.atmosphere_km <= 0
            draw_mesh(
                view,
                starport_mesh(sealed, cleared_id == bid),
                surface_frame(normal),
                pos, cam_pos, cam_rot, light,
                scale=0.35 if dist > 400 else 0.8, fov=fov,
            )
        for cid, c in self.combat.combatants.items():
            if not c.alive or cid == "player":
                continue
            pos = self.frames.ship_world_pos(c.state, days)
            ship_range = (80.0, 400.0, 2000.0, 1.0e9)[min(self.detail_level, 3)]
            if float(np.linalg.norm(pos - cam_pos)) > ship_range:
                continue
            mesh = mesh_for_ship("viper" if c.role == "police" else "sidewinder" if c.role == "pirate" else "adder")
            draw_mesh(view, mesh, c.state.orientation, pos, cam_pos, cam_rot, light, scale=1.0, fov=fov)
        if self.view_mode == "external":
            from render.ship_models import build_ship_mesh

            mesh, _scale = build_ship_mesh(
                self.fitted.ship_id,
                guns=self.fitted.guns,
                missile_count=len(self.fitted.missile_load),
                equipment=self.fitted.equipment,
                gear_down=self.landing_gear,
            )
            draw_mesh(view, mesh, ship_rot, ship_pos, cam_pos, cam_rot, light, scale=1.0, fov=fov)
        for proj in self.combat.projectiles:
            rel = (proj.position - cam_pos) @ cam_rot
            if rel[2] > 0.1:
                x = WIDTH / 2 + fov * rel[0] / rel[2]
                y = view_h / 2 - fov * rel[1] / rel[2]
                if 0 <= x < WIDTH and 0 <= y < view_h:
                    col = (255, 80, 40) if proj.kind == "laser" else (255, 255, 80)
                    pygame.draw.circle(view, col, (int(x), int(y)), 2)

        # Direction markers: star, planets, moons, stations, starports
        markers: list[tuple[np.ndarray, str, tuple[int, int, int], bool]] = []
        markers.append(
            (np.zeros(3, dtype=np.float64), self.system.name, COL_STAR, self.ap_body_id == "star")
        )
        for p in self.system.planets:
            pos = self.frames.world_position(p.id, days)
            col = COL_MOON if p.parent_id else COL_PLANET
            markers.append((pos, p.name, col, self.ap_body_id == p.id))
        for st in self.system.stations:
            pos = self.frames.world_position(st.id, days)
            markers.append((pos, st.name, COL_STATION, self.ap_body_id == st.id))
        for bid, b in self.frames.bodies.items():
            if b.is_starport and not b.is_station:
                pos = self.frames.world_position(bid, days)
                markers.append((pos, b.name, COL_STARPORT, self.ap_body_id == bid))
        if self.view_mode in ("front", "rear"):
            tunnel_pos = None
            # The destination you are flying toward owns the tunnel.
            # A ship lock is used only when ship tunnels are on and no destination is set.
            if self.ap_body_id and self.ap_body_id != "star" and self.ap_body_id in self.frames.bodies:
                tunnel_pos = self.frames.world_position(self.ap_body_id, days)
            elif (
                self.opt.get("ship_tunnels", False)
                and self.target_id
                and self.target_id in self.combat.combatants
                and self.combat.combatants[self.target_id].alive
            ):
                tc = self.combat.combatants[self.target_id]
                tunnel_pos = self.frames.ship_world_pos(tc.state, days)
            if tunnel_pos is not None and self.opt.get("tunnels", False):
                # "Behind" is behind the view on screen, so a destination you
                # are looking at gets the squares.
                draw_targeting_tunnel(
                    view,
                    tunnel_pos,
                    cam_pos=cam_pos,
                    cam_rot=cam_rot,
                    view_h=view_h,
                    fov=fov,
                    ship_forward=cam_rot[:, 2],
                    font=self.small,
                    behind_label=True,
                )
        if self.show_id_text or self.show_hud:
            draw_direction_markers(
                view,
                self.small,
                markers,
                cam_pos=cam_pos,
                cam_rot=cam_rot,
                view_h=view_h,
                fov=fov,
                show_labels=self.show_id_text,
            )

        body = self.frames.bodies[self.player_state.parent_id]
        alt = self.frames.altitude(self.player_state, days)
        pads = None
        for bid, b in self.frames.bodies.items():
            if b.is_starport and not b.is_station and b.parent_id == self.player_state.parent_id:
                sp = self.system.starport_by_id(bid)
                if sp:
                    pads = sp.pads
                break
        if body.atmosphere_km > 0 and pads and alt < 80:
            draw_surface_approach(view, alt, float(np.linalg.norm(self.player_state.velocity)), pads)
        elif body.atmosphere_km > 0 and 0 < alt < body.atmosphere_km * 3:
            fade = int(40 * max(0, 1 - alt / (body.atmosphere_km * 3)))
            overlay = pygame.Surface((WIDTH, view_h), pygame.SRCALPHA)
            overlay.fill((40, 80, 140, fade))
            view.blit(overlay, (0, 0))

        speed = float(np.linalg.norm(self.player_state.velocity))
        fwd = self.player_state.orientation[:, 2]
        right = self.player_state.orientation[:, 0]
        up = self.player_state.orientation[:, 1]
        contacts = []
        for cid, c in self.combat.combatants.items():
            if not c.alive or cid == "player":
                continue
            rel = self.frames.ship_world_pos(c.state, days) - cam_pos
            col = (255, 60, 40) if c.role == "pirate" else ((80, 160, 255) if c.role == "police" else (80, 220, 80))
            contacts.append((rel, c.name, col))
        # Stations / starports on the FE2 scanner dish
        for bid, b in self.frames.bodies.items():
            if not (b.is_station or b.is_starport):
                continue
            rel = self.frames.world_position(bid, days) - cam_pos
            if float(np.linalg.norm(rel)) > 120:
                continue
            col = (80, 180, 255) if b.is_station else (255, 160, 60)
            if bid == self.ap_body_id:
                col = (48, 230, 72)
            contacts.append((rel, b.name, col))
        tgt_name = ""
        to_tgt = None
        if self.target_id and self.target_id in self.combat.combatants:
            tc = self.combat.combatants[self.target_id]
            if tc.alive:
                tgt_name = tc.name
                to_tgt = self.frames.ship_world_pos(tc.state, days) - cam_pos

        view_names = {"front": "Front View", "rear": "Rear View", "external": "External View"}
        ap_line = ""
        if self.ap_enabled and self.ap_body_id:
            ap_line = f"AP -> {self._ap_dest_name()}"
        elif self.ap_body_id:
            ap_line = f"Dest: {self._ap_dest_name()} (F7)"
        elif self.selected_system and self.selected_system != self.system_id:
            ap_line = f"HS: {self.galaxy.get(self.selected_system).name} (H/F8)"
        hits = draw_hud(
            self.screen, self.font, self.small,
            speed=speed, altitude=alt,
            fuel=self.flight.stats.fuel_t, max_fuel=self.flight.stats.max_fuel_t,
            cabin_temp=self.flight.cabin_temp,
            energy=self.flight.stats.energy, max_energy=self.flight.stats.max_energy,
            shield=self.flight.stats.shield, max_shield=self.flight.stats.max_shield,
            hull=self.flight.stats.hull, max_hull=self.flight.stats.max_hull,
            stardate=self.stardate.format(), compression=self.compression.factor,
            compression_index=self.compression.vcr_slot(),
            system_name=self.system.name, set_speed=self.flight.controls.set_speed,
            target_name=tgt_name, credits=self.credits, legal=self.legal,
            messages=self.messages,
            relative_to=self._relative_to_name(),
            view_name=view_names.get(self.view_mode, "Front View"),
            paused=self.time_paused,
            active_icon=None,
            show_hud=self.show_hud,
            nav_status=ap_line,
            view_mode=self.view_mode,
            map_mode="system" if self.mode == "orrery" else "galaxy",
            info_page=self.info_page,
            engines_off=self.engines_off,
            gear_down=self.landing_gear,
            hud_labels=self.show_id_text,
            hyperspace_ready=self.hyperspace_ready(),
            autopilot=self.ap_enabled,
            has_autopilot="autopilot" in self.fitted.equipment,
            center_mode=self.center_mode,
            has_scanner=self._has_scanner(),
            weapon_items=self._weapon_controls(),
            mapper_line=self._mapper_line(),
            camera_line=self.camera_line if "military_camera" in self.fitted.equipment else "",
        )
        self.console_hits = hits
        if self.show_hud and self._has_scanner() and self.center_mode == "scanner":
            draw_scanner(self.screen, contacts, fwd, right, up)
        draw_compass(self.screen, fwd, to_tgt)

    def draw_docked(self) -> None:
        loc = self.frames.bodies.get(self.dock_location)
        name = loc.name if loc else self.dock_location
        listings = self.market.listings(self.system, int(self.stardate.days), self.dock_location)
        sp = self.system.starport_by_id(self.dock_location)
        pads = sp.pads if sp else None
        planet_name = ""
        if sp:
            pl = self.system.planet_by_id(sp.planet_id)
            planet_name = pl.name if pl else ""
            if pl and pl.parent_id:
                giant = self.system.planet_by_id(pl.parent_id)
                if giant:
                    planet_name = f"{pl.name} (moon of {giant.name})"
        self.ui.draw(
            self.screen,
            self.font,
            self.small,
            system=self.system,
            location_name=name,
            credits=self.credits,
            fitted=self.fitted,
            shipyard=self.shipyard,
            listings=listings,
            missions=self.board_missions,
            fines=self.fines,
            legal=self.legal,
            stardate=self.stardate.format(),
            starport_pads=pads,
            planet_name=planet_name,
            compression_index=self.compression.vcr_slot(),
            time_paused=self.time_paused,
            launch_clearance=self.launch_clearance,
            free_purchases=self.free_purchases,
            view_mode=self.view_mode,
            info_page=self.info_page,
            engines_off=self.engines_off,
            gear_down=self.landing_gear,
            hud_labels=self.show_id_text,
            hyperspace_ready=self.hyperspace_ready(),
            autopilot=self.ap_enabled,
        )
        self.console_hits = self.ui.console_hits

    def run(self) -> None:
        running = True
        while running:
            dt = self.clock.tick(60) / 1000.0
            self.title_t += dt
            self._tick_fade(dt)
            if self.mode == "start":
                self._start_idle += dt
                if self._start_idle >= 20.0:
                    self._show_intro()
            if self._quit_requested:
                running = False
            for event in pygame.event.get():
                if event.type == pygame.QUIT:
                    running = False
                elif event.type == pygame.KEYDOWN:
                    if self.mode == "title":
                        self._open_start()
                        continue
                    if self.mode == "start":
                        self._start_idle = 0.0
                        if event.key in (pygame.K_UP, pygame.K_w):
                            self.start_sel = (self.start_sel - 1) % 5
                        elif event.key in (pygame.K_DOWN, pygame.K_s):
                            self.start_sel = (self.start_sel + 1) % 5
                        elif event.key in (pygame.K_1, pygame.K_2, pygame.K_3, pygame.K_4, pygame.K_5):
                            self.start_sel = event.key - pygame.K_1
                            self._activate_start()
                        elif event.key in (pygame.K_RETURN, pygame.K_SPACE):
                            self._activate_start()
                        elif event.key == pygame.K_ESCAPE:
                            self._show_intro()
                        continue
                    if self.mode == "cinematic":
                        # Space / Enter / Esc skips to the end of the sequence
                        if event.key in (pygame.K_SPACE, pygame.K_RETURN, pygame.K_ESCAPE):
                            if self.cinematic:
                                kind, _e, duration, payload = self.cinematic
                                self.cinematic = (kind, duration, duration, payload)
                        continue
                    if event.key in (pygame.K_BACKQUOTE, pygame.K_F12) and self.mode != "title":
                        if self.mode == "cheats":
                            self.mode = self._cheat_return
                        else:
                            self._cheat_return = self.mode
                            self.mode = "cheats"
                        continue
                    if self.mode == "cheats":
                        self._handle_cheat_key(event.key)
                        continue
                    if event.key == pygame.K_ESCAPE:
                        if self.mode == "settings":
                            self.mode = self._pause_return
                            self.time_paused = False
                            self._esc_armed = 0.0
                        elif self.mode == "paused":
                            # Esc again while paused -> settings
                            self.mode = "settings"
                            self.settings_sel = 0
                            self._esc_armed = 0.0
                        elif self.mode in ("orrery", "chart"):
                            self.mode = "docked" if self.docked else "flight"
                            self._esc_armed = 0.0
                        elif self.mode in ("ship", "captain", "cargo"):
                            self.mode = "flight"
                            self.info_page = 2
                            self._esc_armed = 0.0
                        else:
                            # FE2: Esc twice opens pause
                            now = pygame.time.get_ticks() / 1000.0
                            if now - self._esc_armed < 0.6:
                                self._pause_return = "docked" if self.docked else "flight"
                                self.mode = "paused"
                                self._esc_armed = 0.0
                                self.msg("Paused - Esc for settings")
                            else:
                                self._esc_armed = now
                                self.msg("Esc again to pause")
                    elif event.key == pygame.K_F11:
                        self.toggle_fullscreen()
                    elif pygame.K_F1 <= event.key <= pygame.K_F10:
                        self.handle_function_key(event.key - pygame.K_F1 + 1)
                    elif event.key == pygame.K_EQUALS or event.key == pygame.K_PLUS:
                        if self.mode == "chart":
                            self.chart.zoom(1.15)
                        elif self.mode == "orrery":
                            self.orrery.zoom(1.2)
                        else:
                            self.compression.faster()
                    elif event.key == pygame.K_MINUS:
                        if self.mode == "chart":
                            self.chart.zoom(1 / 1.15)
                        elif self.mode == "orrery":
                            self.orrery.zoom(1 / 1.2)
                        else:
                            self.compression.slower()
                    elif event.key == pygame.K_h or event.key == pygame.K_j:
                        self.hyperspace()
                    elif event.key == pygame.K_c and self.mode == "chart":
                        self.chart.look_at(self.system.position_ly)
                        self.msg(f"Centred on {self.system.name}")
                    elif event.key == pygame.K_c and self.mode == "orrery":
                        self.orrery.look_at(np.zeros(3))
                        self.msg(f"Centred on {self.system.name}")
                    elif event.key == pygame.K_INSERT and self.mode == "orrery":
                        self.orrery.show_stems = not self.orrery.show_stems
                        self.msg("Orbital grid " + ("ON" if self.orrery.show_stems else "OFF"))
                    elif self.mode == "paused":
                        if event.key in (pygame.K_RETURN, pygame.K_r):
                            self.mode = self._pause_return
                        elif event.key == pygame.K_s:
                            self.save()
                        elif event.key == pygame.K_l:
                            self.load()
                    elif self.mode == "settings":
                        self._handle_settings_key(event.key)
                    elif self.mode == "cargo":
                        self._handle_cargo_key(event.key)
                    elif self.mode == "docked":
                        if event.key == pygame.K_i:
                            self._open_cargo()
                        else:
                            action = self.ui.handle_key(event.key)
                            if action:
                                self.handle_docked_action(action)
                    elif self.mode == "flight":
                        if self.view_mode == "external" and event.key in (
                            pygame.K_LEFT, pygame.K_RIGHT, pygame.K_UP, pygame.K_DOWN,
                            pygame.K_LEFTBRACKET, pygame.K_RIGHTBRACKET,
                        ):
                            if event.key == pygame.K_LEFT:
                                self.ext_yaw -= 0.25
                            elif event.key == pygame.K_RIGHT:
                                self.ext_yaw += 0.25
                            elif event.key == pygame.K_UP:
                                self.ext_pitch = min(1.2, self.ext_pitch + 0.12)
                            elif event.key == pygame.K_DOWN:
                                self.ext_pitch = max(-0.4, self.ext_pitch - 0.12)
                            elif event.key == pygame.K_RIGHTBRACKET:
                                self.ext_zoom = max(0.45, self.ext_zoom / 1.15)
                            else:
                                self.ext_zoom = min(3.5, self.ext_zoom * 1.15)
                            continue
                        if event.key == pygame.K_i:
                            self._open_cargo()
                            continue
                        if event.key == pygame.K_LEFTBRACKET:
                            self.flight.adjust_set_speed(-1.0)
                        elif event.key == pygame.K_RIGHTBRACKET:
                            self.flight.adjust_set_speed(1.0)
                        elif event.key == pygame.K_SPACE:
                            self.fire_laser()
                        elif event.key == pygame.K_m:
                            self.fire_missile()
                        elif event.key == pygame.K_TAB:
                            self.cycle_target()
                        elif event.key == pygame.K_l:
                            self.request_dock()
                        elif event.key == pygame.K_e:
                            self._use_ecm()
                        elif event.key == pygame.K_b:
                            self._use_energy_bomb()
                        elif event.key == pygame.K_x:
                            self._use_escape_capsule()
                        elif event.key == pygame.K_r:
                            self._use_radar_mapper()
                        elif event.key == pygame.K_SLASH and (pygame.key.get_mods() & pygame.KMOD_SHIFT):
                            self._use_cloud_analyser()
                        elif event.key == pygame.K_d:
                            if "military_camera" in self.fitted.equipment:
                                self._use_military_camera()
                            elif "mining_machine" in self.fitted.equipment:
                                self._mb4_use()
                        elif event.key == pygame.K_c and self.flight.stats.has_docking_computer:
                            self.flight.controls.set_speed = 0.0
                            self.request_dock()
                    elif self.mode == "chart":
                        if event.key == pygame.K_LEFT:
                            self.chart.rot_y -= 0.15
                        elif event.key == pygame.K_RIGHT:
                            self.chart.rot_y += 0.15
                        elif event.key == pygame.K_UP:
                            self.chart.rot_x = max(-1.2, self.chart.rot_x - 0.1)
                        elif event.key == pygame.K_DOWN:
                            self.chart.rot_x = min(1.2, self.chart.rot_x + 0.1)
                        elif event.key == pygame.K_HOME:
                            self.chart.look_at(self.system.position_ly)
                            self.msg(f"Centred on {self.system.name}")
                        elif event.key == pygame.K_RETURN:
                            self.hyperspace()
                    elif self.mode == "orrery":
                        if event.key == pygame.K_LEFT:
                            self.orrery.rot_y -= 0.15
                        elif event.key == pygame.K_RIGHT:
                            self.orrery.rot_y += 0.15
                        elif event.key == pygame.K_UP:
                            self.orrery.rot_x = max(0.08, self.orrery.rot_x - 0.1)
                        elif event.key == pygame.K_DOWN:
                            self.orrery.rot_x = min(1.45, self.orrery.rot_x + 0.1)
                        elif event.key == pygame.K_HOME:
                            self.orrery.look_at(np.zeros(3))
                            self.set_local_destination("star")
                        elif event.key == pygame.K_RETURN:
                            if self.selected_body:
                                self.set_local_destination(self.selected_body)
                        elif event.key == pygame.K_t:
                            self.orrery.show_stems = not self.orrery.show_stems
                elif event.type == pygame.MOUSEWHEEL and self.mode == "flight" and self.view_mode == "external":
                    if event.y > 0:
                        self.ext_zoom = max(0.45, self.ext_zoom / 1.12)
                    elif event.y < 0:
                        self.ext_zoom = min(3.5, self.ext_zoom * 1.12)
                elif event.type == pygame.MOUSEWHEEL and self.mode == "chart":
                    self.chart.zoom(1.15 if event.y > 0 else 1 / 1.15)
                elif event.type == pygame.MOUSEWHEEL and self.mode == "orrery":
                    self.orrery.zoom(1.2 if event.y > 0 else 1 / 1.2)
                elif event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
                    if self.mode == "title":
                        self._open_start()
                        continue
                    if self.mode == "start":
                        self._start_idle = 0.0
                        pos = self._logical_pos(event.pos)
                        for i, rect in enumerate(self._start_rects):
                            if rect.collidepoint(pos):
                                self.start_sel = i
                                self._activate_start()
                                break
                        continue
                    if self.mode == "settings":
                        self._click_options(self._logical_pos(event.pos))
                        continue
                    if self.mode == "cheats":
                        pos = self._logical_pos(event.pos)
                        for i, rect in enumerate(getattr(self, "_cheat_rects", [])):
                            if rect.collidepoint(pos):
                                self.cheat_sel = i
                                self._apply_cheat(self._cheat_items()[i][0])
                                break
                        continue
                    if self.mode == "cinematic":
                        if self.cinematic:
                            kind, _e, duration, payload = self.cinematic
                            self.cinematic = (kind, duration, duration, payload)
                        continue
                    variants = self._click_pos_variants(event.pos)
                    from render.console_bar import console_bar_height
                    map_bottom = HEIGHT - console_bar_height(HEIGHT)

                    # Console strip first — try every coord interpretation
                    hits = None
                    if self.mode == "docked":
                        hits = self.ui.console_hits
                    elif self.mode in ("chart", "orrery", "flight", "paused", "settings", "cargo", "ship", "captain"):
                        hits = self.console_hits
                    if hits is None and self.mode == "docked":
                        hits = getattr(self, "console_hits", None)
                    if hits:
                        for pos in variants:
                            hit = hit_console(hits, pos)
                            if hit:
                                self.handle_console_click(*hit)
                                break
                        else:
                            hit = None
                        if hit:
                            continue

                    if self.mode == "cargo":
                        for p in variants:
                            if self.ui.refuel_rect and self.ui.refuel_rect.collidepoint(p):
                                self.refuel()
                                break
                            for gid, rect in self.ui.jettison_glitch_rects:
                                if gid == "rubbish" and rect.collidepoint(p):
                                    self.expand_hold()
                                    break
                            else:
                                for gid, rect in self.ui.jettison_rects:
                                    if rect.collidepoint(p):
                                        self.jettison(gid)
                                        break
                                else:
                                    continue
                            break
                        continue

                    pos = variants[0]
                    # Map picks (above console)
                    if self.mode == "chart":
                        picked = None
                        for p in variants:
                            if p[1] >= map_bottom:
                                continue
                            picked = self.chart.pick(p[0], p[1], radius=22.0)
                            if picked:
                                pos = p
                                break
                        if picked:
                            self.chart.look_at(self.galaxy.get(picked).position_ly)
                            if self.map_target:
                                self.set_hyperspace_target(picked)
                        elif pos[1] < map_bottom:
                            self.chart.begin_drag(pos)
                        continue
                    if self.mode == "orrery":
                        picked = None
                        for p in variants:
                            if p[1] >= map_bottom:
                                continue
                            picked = self.orrery.pick(p[0], p[1], radius=22.0)
                            if picked:
                                pos = p
                                break
                        if picked:
                            self.selected_body = picked
                            if self.map_target and picked != "star":
                                self.set_local_destination(picked)
                            if picked == "star":
                                self.orrery.look_at(np.zeros(3))
                            elif self.system.planet_by_id(picked):
                                self.orrery.look_at(
                                    body_world_km(
                                        self.system, picked, self.stardate.days, self.orrery.scale
                                    )
                                )
                            else:
                                from render.orrery3d import station_world_km
                                pos_km = None
                                for st in self.system.stations:
                                    if st.id == picked:
                                        pos_km = station_world_km(
                                            self.system, st, self.stardate.days, self.orrery.scale
                                        )
                                        break
                                if pos_km is None and picked in self.frames.bodies:
                                    parent = self.frames.bodies[picked].parent_id or "star"
                                    if parent == "star":
                                        pos_km = np.zeros(3)
                                    elif self.system.planet_by_id(parent):
                                        pos_km = body_world_km(
                                            self.system, parent, self.stardate.days, self.orrery.scale
                                        )
                                if pos_km is not None:
                                    self.orrery.look_at(pos_km)
                        elif pos[1] < map_bottom:
                            self.selected_body = None
                            if self.map_target:
                                self.ap_body_id = None
                        continue

                    if self.mode == "docked":
                        for p in variants:
                            action = self.ui.handle_click(p)
                            if action:
                                self.handle_docked_action(action)
                                break
                        continue
                    if self.mode == "flight" and not self.docked:
                        picked = None
                        for p in variants:
                            if p[1] >= map_bottom:
                                continue
                            picked = self._pick_view_target(p[0], p[1])
                            if picked:
                                break
                        if picked:
                            kind, bid = picked
                            if kind == "ship":
                                self.target_id = bid
                                name = self.combat.combatants[bid].name
                                self.msg(f"Target: {name}.")
                            else:
                                self.set_local_destination(bid)
                        else:
                            self.fire_laser()
                        continue
                elif event.type == pygame.MOUSEBUTTONDOWN and event.button == 3:
                    if self.mode == "chart":
                        self.chart.begin_drag(event.pos)
                    elif self.mode == "orrery":
                        self.orrery.begin_drag(event.pos, "pan")
                elif event.type == pygame.MOUSEBUTTONUP and event.button in (1, 3):
                    self.chart.end_drag()
                    self.orrery.end_drag()
                elif event.type == pygame.MOUSEMOTION:
                    if self.mode == "chart" and self.chart.dragging:
                        self.chart.drag(event.pos)
                    elif self.mode == "orrery" and self.orrery.dragging:
                        self.orrery.drag(event.pos)

            if self.mode == "cinematic":
                self._update_cinematic(dt)
            elif self.mode == "flight" and not self.docked:
                keys = pygame.key.get_pressed()
                if not self.ap_enabled:
                    self.handle_flight_input(keys)
                else:
                    # Keep thrust aliases available; AP owns pitch/yaw/set_speed
                    pygame.mouse.get_rel()
                if keys[pygame.K_SPACE]:
                    self.fire_laser()
                if not self.time_paused:
                    self.update_flight(dt)
            elif self.mode in ("docked", "orrery", "chart", "cargo", "ship", "captain") and not self.time_paused:
                # Realtime lock is only for close combat / landing
                self.compression.force_realtime = False
                self.stardate.advance(dt, self.compression.factor)

            self._sync_music()

            if self.mode in ("title", "start"):
                self.intro.advance(dt)
                self.intro.draw(self.screen)
            if self.mode == "start":
                self._start_rects = draw_start(
                    self.screen,
                    self.font,
                    self.small,
                    self.title_t,
                    self.start_sel,
                    load_ok=self._has_save(),
                )
            elif self.mode == "cinematic":
                self._draw_cinematic()
            elif self.mode == "flight":
                self.draw_flight()
            elif self.mode == "cargo":
                self.draw_cargo_screen()
            elif self.mode == "ship":
                self.draw_ship_screen()
            elif self.mode == "captain":
                self.draw_captain_screen()
            elif self.mode == "docked":
                self.draw_docked()
            elif self.mode == "orrery":
                self.console_hits = draw_orrery(
                    self.screen, self.font, self.small, self.system, self.stardate.days,
                    self.selected_body,
                    self.orrery,
                    stardate=self.stardate.format(),
                    compression_index=self.compression.vcr_slot(),
                    paused=self.time_paused,
                    docked=self.docked,
                    view_mode=self.view_mode,
                    info_page=self.info_page,
                    engines_off=self.engines_off,
                    gear_down=self.landing_gear,
                    hud_labels=self.show_id_text,
                    hyperspace_ready=self.hyperspace_ready(),
                    autopilot=self.ap_enabled,
                    map_tools=getattr(self, "map_tools", False),
                    system_page=self.system_page,
                    system_index=list(self.galaxy.systems).index(self.system_id) + 1,
                )
            elif self.mode == "chart":
                self.console_hits = draw_galactic_chart(
                    self.screen,
                    self.font,
                    self.small,
                    self.galaxy,
                    self.system_id,
                    self.selected_system,
                    self.flight.stats.jump_range_ly,
                    self.chart,
                    stardate=self.stardate.format(),
                    fuel=self.flight.stats.fuel_t,
                    compression_index=self.compression.vcr_slot(),
                    paused=self.time_paused,
                    docked=self.docked,
                    view_mode=self.view_mode,
                    info_page=self.info_page,
                    engines_off=self.engines_off,
                    gear_down=self.landing_gear,
                    hud_labels=self.show_id_text,
                    hyperspace_ready=self.hyperspace_ready(),
                    autopilot=self.ap_enabled,
                    map_tools=getattr(self, "map_tools", False),
                )
            elif self.mode == "cheats":
                self.draw_cheats()
            elif self.mode == "paused":
                self.draw_pause_menu()
            elif self.mode == "settings":
                self.draw_settings()
            if self.fade > 0.01:
                shade = pygame.Surface((WIDTH, HEIGHT))
                shade.fill((0, 0, 0))
                shade.set_alpha(int(self.fade * 255))
                self.screen.blit(shade, (0, 0))
            pygame.display.flip()

        pygame.quit()


def main() -> None:
    Game().run()


if __name__ == "__main__":
    main()
