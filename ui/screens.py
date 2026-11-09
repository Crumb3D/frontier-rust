"""Amiga FE2 station screens — shipyard layout matching the original UI."""

from __future__ import annotations

from dataclasses import dataclass

import pygame

from render.console_bar import (
    ConsoleHits,
    console_bar_height,
    draw_docked_console,
    draw_status_strip,
    status_strip_height,
)
from render.portrait import draw_merchant_portrait
from sim.galaxy import StarSystem
from sim.market import MarketListing
from sim.mission import Mission
from sim.shipyard import EquipDef, FittedShip, ShipDef, Shipyard

# Amiga FE2 palette (from shipyard screenshot)
PURPLE = (168, 72, 200)
PURPLE_DIM = (100, 48, 130)
ORANGE = (255, 168, 32)
GREEN = (48, 220, 64)
WHITE = (248, 248, 248)
GREY = (140, 140, 150)
LOCKED = (72, 72, 80)
RED = (220, 48, 48)
BG = (0, 0, 0)
PANEL = (12, 8, 18)


@dataclass
class MenuState:
    tab: str = "comms"
    scroll: int = 0
    selected: int = 0
    message: str = "Comms Link - select Launch Request for clearance."
    mode: str = "fit"  # fit | remove  (which checkbox column is active)


class DockedUI:
    # FE2 services: Comms (F4) is the ATC / launch-request channel
    TABS = ["comms", "stock", "outfit", "ships", "missions", "starport", "status", "police"]
    COMMS_OPTIONS = [
        "Launch Request",
        "Shipyard",
        "Bulletin Board",
        "Stockmarket",
    ]

    def __init__(self) -> None:
        self.state = MenuState()
        self.state.tab = "comms"
        self.state.message = "Comms Link - select Launch Request for clearance."
        self.selected_pad = 0
        self.console_hits: ConsoleHits | None = None
        self.tab_rects: list[tuple[str, pygame.Rect]] = []
        self.comms_row_rects: list[tuple[int, pygame.Rect]] = []
        self.back_rect: pygame.Rect | None = None
        self.bbs_index = 0
        self.bbs_note = ""
        self.bbs_actions: list[tuple] = []
        self.flight_links: list[tuple[str, tuple[str, ...]]] | None = None
        self.more_rect: pygame.Rect | None = None
        self.jettison_rects: list[tuple[str, pygame.Rect]] = []
        self.jettison_glitch_rects: list[tuple[str, pygame.Rect]] = []
        self.refuel_rect: pygame.Rect | None = None
        self.cargo_rows: list[str] = []

    def handle_key(self, key: int) -> str | None:
        if key == pygame.K_BACKSPACE:
            self._go_back()
            return None
        elif key == pygame.K_UP:
            if self.state.tab == "starport":
                self.selected_pad = max(0, self.selected_pad - 1)
            else:
                self.state.selected = max(0, self.state.selected - 1)
                if self.state.selected < self.state.scroll:
                    self.state.scroll = self.state.selected
        elif key == pygame.K_DOWN:
            if self.state.tab == "starport":
                self.selected_pad += 1
            else:
                self.state.selected += 1
        elif key == pygame.K_f:
            self.state.mode = "fit"
            if self.state.tab == "outfit":
                return "fit" if self._buy_allowed() else None
            return "buy" if self._buy_allowed() else None
        elif key == pygame.K_r:
            self.state.mode = "remove"
            return "remove" if self.state.tab == "outfit" else None
        elif key in (pygame.K_RETURN, pygame.K_KP_ENTER, pygame.K_SPACE):
            if self.state.tab == "comms":
                return self._comms_action()
            if self.state.tab == "yard":
                return self._yard_action()
            if self.state.tab == "repair":
                return "repair" if self._buy_allowed() else None
            if self.state.tab == "police":
                self._go_back()
                return None
            if self.state.tab == "outfit":
                if self.state.mode == "fit":
                    return "fit" if self._buy_allowed() else None
                return "remove"
            if self.state.tab == "stock":
                return "buy" if self._buy_allowed() else None
            if self.state.tab == "ships":
                return "buy" if self._buy_allowed() else None
            if self.state.tab == "missions":
                self._open_video_link()
                return None
            if self.state.tab == "videolink":
                return self._bbs_pick(self.state.selected)
            return None
        elif key == pygame.K_1 and self.state.tab == "comms" and not self.flight_links:
            self.state.selected = 0
            return "launch_request"
        elif key == pygame.K_s and self.state.tab == "stock":
            self.state.mode = "remove"
            return "sell"
        elif key == pygame.K_b and self.state.tab == "stock":
            self.state.mode = "fit"
            return "buy" if self._buy_allowed() else None
        elif key == pygame.K_p and self.state.tab == "police":
            return "pay_fine" if getattr(self, "_fine_ok", True) else None
        elif key == pygame.K_a and self.state.tab == "missions":
            self._open_video_link()
            return None
        elif key == pygame.K_l:
            return "launch"
        elif key == pygame.K_PAGEUP:
            self.state.scroll = max(0, self.state.scroll - 5)
        elif key == pygame.K_PAGEDOWN:
            self.state.scroll += 5
        return None

    def _comms_action(self) -> str | None:
        if self.flight_links:
            i = min(self.state.selected, len(self.flight_links) - 1)
            name = self.flight_links[i][0]
            if name.startswith("Request landing"):
                return "land_request"
            if name.startswith("Send long range"):
                return "emergency_signal"
            return None
        opts = self.COMMS_OPTIONS
        i = min(self.state.selected, len(opts) - 1)
        name = opts[i]
        if name == "Launch Request":
            # If clearance already held, Enter launches (FE2: request then take off)
            return "launch_request"
        if name == "Contact Police":
            self.state.tab = "police"
            self._default_prompt()
            return None
        if name == "Shipyard":
            self.state.tab = "yard"
            self.state.selected = 0
            self.state.scroll = 0
            self._default_prompt()
            return None
        if name == "Stockmarket":
            self.state.tab = "stock"
            self.state.selected = 0
            self.state.scroll = 0
            self._default_prompt()
            return None
        if name == "Bulletin Board":
            self.state.tab = "missions"
            self._default_prompt()
            return None
        return None

    YARD_OPTIONS = [
        ("Upgrades", "(Ship equipment fitted while you wait)", "outfit"),
        ("Repairs and Servicing", "(In our fully automated workshop)", "repair"),
        ("New and Reconditioned Ships", "(Ships part exchanged)", "ships"),
        ("Contact Local Police", "(For paying fines, etc)", "police"),
    ]

    def _yard_action(self) -> str | None:
        i = min(self.state.selected, len(self.YARD_OPTIONS) - 1)
        _label, _note, tab = self.YARD_OPTIONS[i]
        self.state.tab = tab
        self.state.selected = 0
        self.state.scroll = 0
        self._default_prompt()
        return None

    def _go_back(self) -> None:
        back = {
            "outfit": "yard",
            "repair": "yard",
            "ships": "yard",
            "shipview": "ships",
            "police": "yard",
            "yard": "comms",
            "stock": "comms",
            "missions": "comms",
            "videolink": "missions",
        }
        self.state.tab = back.get(self.state.tab, "comms")
        self.state.selected = 0
        self.state.scroll = 0
        self._default_prompt()

    def _default_prompt(self) -> None:
        prompts = {
            "comms": "Which service do you require?",
            "yard": "Which service do you require?",
            "repair": "Select desired repair or service.",
            "police": "We have no business with you at the moment.",
            "stock": "Choose commodity to Buy or Sell.",
            "outfit": "Choose Fit or Remove for required item.",
            "ships": "Select a ship to purchase.",
            "missions": "Please choose advertisement. Direct link will be made with advertiser.",
            "starport": "ARROWS = Select Pad | F4 = Request clearance | F7 = Launch",
            "status": "Ship inventory and status.",
        }
        self.state.message = prompts.get(self.state.tab, "")

    def draw(
        self,
        surface: pygame.Surface,
        font: pygame.font.Font,
        small: pygame.font.Font,
        *,
        system: StarSystem,
        location_name: str,
        credits: int,
        fitted: FittedShip,
        shipyard: Shipyard,
        listings: list[MarketListing],
        missions: list[Mission],
        fines: int,
        legal: bool,
        stardate: str,
        starport_pads: list[str] | None = None,
        planet_name: str = "",
        compression_index: int = 1,
        time_paused: bool = False,
        launch_clearance: bool = False,
        free_purchases: bool = False,
        view_mode: str = "front",
        info_page: int = 0,
        engines_off: bool = False,
        gear_down: bool = True,
        hud_labels: bool = True,
        hyperspace_ready: bool = False,
        autopilot: bool = False,
    ) -> ConsoleHits | None:
        w, h = surface.get_size()
        ch = console_bar_height(h)
        sh = status_strip_height()
        work_h = h - ch - sh
        # FE2 menus: deep purple working area above the console
        if self.state.tab != "starport":
            surface.fill((85, 68, 153))
            pygame.draw.rect(surface, BG, (0, work_h, w, ch + sh))
        else:
            surface.fill(BG)

        yard_name = location_name if len(location_name) <= 28 else location_name[:26] + ".."
        titles = {
            "comms": f"{yard_name} Station Services",
            "yard": f"{yard_name} Station Shipyard",
            "outfit": f"{yard_name} Station Shipyard",
            "repair": f"{yard_name} Station Shipyard",
            "ships": f"{yard_name} Station Shipyard",
            "shipview": f"{yard_name} Station Shipyard",
            "police": f"{yard_name} Station Shipyard",
            "stock": f"{yard_name} Station Stockmarket",
            "missions": f"{yard_name} Station Bulletin Board",
            "videolink": f"{yard_name} Video Link",
        }
        surface.blit(small.render(titles.get(self.state.tab, yard_name), True, WHITE), (8, 4))
        self.tab_rects = []

        faces = {
            "comms": "comms.png",
            "missions": "bulliten board.png",
            "videolink": "bulliten board.png",
            "yard": "shipyard.png",
            "outfit": "shipyard.png",
            "repair": "shipyard.png",
            "ships": "shipyard.png",
            "shipview": "shipyard.png",
            "police": "cop.png",
            "stock": "shipyard.png",
        }
        face = faces.get(self.state.tab)
        left_w = 230
        if face or self.state.tab in ("stock", "outfit", "ships", "repair", "yard", "comms", "missions", "police", "videolink"):
            free = fitted.free_cargo(shipyard.ships, shipyard.equipment)
            used = fitted.cargo_mass() + fitted.fuel_t
            cabins = sum(int(shipyard.equipment[e].raw.get("passengers", 0)) for e in fitted.equipment)
            aboard = int(fitted.cargo.get("passenger", 0))
            by = work_h - 38
            if face:
                self._blit_face(surface, face, 8, 18, left_w - 16, max(120, by - 26))
            surface.blit(small.render("Remaining", True, GREY), (62, by))
            surface.blit(small.render("Used", True, GREY), (118, by))
            surface.blit(small.render("Cargo Space", True, WHITE), (8, by + 12))
            surface.blit(small.render(f"{free:4.0f}", True, WHITE), (62, by + 12))
            surface.blit(small.render(f"{used:4.0f}", True, WHITE), (118, by + 12))
            surface.blit(small.render("Cash", True, WHITE), (168, by + 12))
            surface.blit(small.render("Cabins", True, WHITE), (8, by + 24))
            surface.blit(small.render(f"{max(0, cabins - aboard):4d}", True, WHITE), (62, by + 24))
            surface.blit(small.render(f"{aboard:4d}", True, WHITE), (118, by + 24))
            surface.blit(small.render(f"${credits}", True, WHITE), (168, by + 24))
            list_x = left_w + 8
        else:
            list_x = 8

        list_w = w - list_x - 8
        list_top = 22
        list_bottom = work_h - 1

        show_more = False
        self.action_rects = []
        if self.state.tab == "comms":
            show_more = self._draw_comms(surface, small, list_x, list_top, list_w, list_bottom, launch_clearance)
        elif self.state.tab == "yard":
            self._draw_yard(surface, small, list_x, list_top, list_w)
        elif self.state.tab == "repair":
            self._draw_repair(
                surface, small, fitted, shipyard, list_x, list_top, list_w,
                credits=credits, free_purchases=free_purchases,
            )
        elif self.state.tab == "outfit":
            show_more = self._draw_outfit_table(
                surface, small, shipyard, system, fitted, list_x, list_top, list_w, list_bottom,
                free_purchases=free_purchases, credits=credits,
            )
        elif self.state.tab == "stock":
            show_more = self._draw_stock_table(
                surface, small, listings, fitted, list_x, list_top, list_w, list_bottom,
                free_purchases=free_purchases, credits=credits,
            )
        elif self.state.tab == "ships":
            self._draw_ships_table(
                surface, small, shipyard.available_ships(system), list_x, list_top, list_w, list_bottom,
                free_purchases=free_purchases, credits=credits,
            )
        elif self.state.tab == "shipview":
            self._draw_ship_view(surface, small, shipyard.available_ships(system), list_x, list_top)
        elif self.state.tab == "missions":
            show_more = self._draw_missions(surface, small, missions, list_x, list_top, list_w, list_bottom)
        elif self.state.tab == "videolink":
            self._draw_video_link(surface, small, missions, fitted, list_x, list_top)
        elif self.state.tab == "starport" and starport_pads:
            from render.maps import draw_starport_map
            # Draw into the purple work area only (above status strip / console)
            pad_surf = surface.subsurface((0, 0, w, work_h))
            self.selected_pad = min(self.selected_pad, max(0, len(starport_pads) - 1))
            draw_starport_map(
                pad_surf, font, small, location_name, planet_name or "planet",
                starport_pads, self.selected_pad, credits,
            )
        elif self.state.tab == "status":
            self._draw_status(surface, small, fitted, shipyard, legal, fines, list_x, list_top)
        elif self.state.tab == "police":
            surface.blit(small.render("Suggested Responses:", True, WHITE), (list_x + 8, list_top + 4))
            hang = pygame.Rect(list_x + 8, list_top + 22, 120, 14)
            self.action_rects.append(("hangup", 0, hang))
            self._checkbox(surface, hang.x, hang.y + 2, False)
            surface.blit(small.render("Hang up.", True, WHITE), (hang.x + 14, hang.y))
            self._fine_ok = False
            if fines > 0:
                can_pay = free_purchases or credits >= fines
                self._fine_ok = can_pay
                pay = pygame.Rect(list_x + 8, list_top + 40, 160, 14)
                if can_pay:
                    self.action_rects.append(("pay_fine", 0, pay))
                self._checkbox(surface, pay.x, pay.y + 2, False, enabled=can_pay)
                surface.blit(small.render(f"Pay fine  Cr{fines}", True, WHITE if can_pay else LOCKED), (pay.x + 14, pay.y))
            self.state.message = "We have no business with you at the moment." if fines <= 0 else f"Outstanding fine Cr{fines}."

        # Dedicated hint strip above console — never overlaps list / More...
        hint = {
            "comms": "Enter = Request / Launch   F7 = Take off",
            "outfit": "F=Fit  R=Remove  Enter=Confirm",
            "stock": "B=Buy  S=Sell  I=Cargo inventory",
            "ships": "Enter=Purchase ship",
            "missions": "Enter = open the video link",
            "videolink": "Enter = reply    Hang up returns to the board",
            "starport": "F4 Comms = clearance  then F7 = Launch",
            "status": "Ship equipment status",
            "police": "P=Pay fine",
        }.get(self.state.tab, "")
        self.back_rect = None
        self.more_rect = None
        if show_more:
            lab = small.render("More...", True, WHITE)
            self.more_rect = pygame.Rect(list_x + 8, work_h - 16, lab.get_width() + 28, 14)
            pygame.draw.polygon(surface, WHITE, ((list_x + 10, work_h - 14), (list_x + 18, work_h - 14), (list_x + 14, work_h - 8)))
            surface.blit(lab, (self.more_rect.x + 16, self.more_rect.y))
        strip = "" if self.state.tab in ("videolink", "comms") else self.state.message
        draw_status_strip(surface, small, strip, more=False)

        tab_page = {"status": 0, "outfit": 2, "missions": 3}.get(self.state.tab, info_page)
        self.console_hits = draw_docked_console(
            surface,
            small,
            stardate=stardate,
            message=self.state.message,
            docked=True,
            comms=True,
            compression_index=compression_index,
            paused=time_paused,
            warn_launch=not launch_clearance,
            view_mode=view_mode,
            map_mode="galaxy",
            info_page=tab_page,
            engines_off=engines_off,
            gear_down=gear_down,
            hud_labels=hud_labels,
            hyperspace_ready=hyperspace_ready,
            autopilot=autopilot,
            bare_left=self.state.tab == "comms",
            bare_right=self.state.tab == "comms",
        )
        return self.console_hits

    def _draw_comms(
        self,
        surface: pygame.Surface,
        small: pygame.font.Font,
        x: int,
        y: int,
        lw: int,
        bottom: int,
        clearance: bool,
    ) -> bool:
        if self.flight_links:
            opts = [name for name, _note in self.flight_links]
            notes = {name: note for name, note in self.flight_links}
        else:
            opts = self.COMMS_OPTIONS
            notes = {
            "Launch Request": ("(Request launch permission from traffic control)",),
            "Shipyard": (
                "(Ships refitted, repaired, bought and sold)",
                "(Also contact with local Police)",
            ),
            "Bulletin Board": ("(Advertisements for goods and services)",),
            "Stockmarket": ("(Direct trading on the local market)",),
        }
        self.state.selected = min(self.state.selected, len(opts) - 1)
        self.comms_row_rects = []
        yy = y + 2
        for i, name in enumerate(opts):
            extra = notes.get(name, ())
            block = 12 + 11 * len(extra)
            if yy + block > bottom:
                break
            row = pygame.Rect(x, yy, lw, block + 4)
            self.comms_row_rects.append((i, row))
            selected = i == self.state.selected
            self._checkbox(surface, x + 2, yy + 1, False)
            surface.blit(small.render(name, True, WHITE if selected else (196, 196, 204)), (x + 16, yy))
            for n, line in enumerate(extra):
                surface.blit(small.render(line, True, (176, 176, 186)), (x + 16, yy + 12 + n * 11))
            yy += block + 6
        del clearance
        return False

    def _blit_face(self, surface: pygame.Surface, filename: str, x: int, y: int, w: int, h: int) -> None:
        from pathlib import Path
        cache = getattr(self, "_faces", None)
        if cache is None:
            cache = {}
            self._faces = cache
        if filename not in cache:
            path = Path(__file__).resolve().parents[1] / "assets" / "Images" / filename
            cache[filename] = pygame.image.load(str(path)).convert() if path.is_file() else None
        src = cache[filename]
        if src is None:
            return
        surface.blit(pygame.transform.scale(src, (w, h)), (x, y))

    def _draw_yard(self, surface, small, x, y, lw) -> None:
        self.comms_row_rects = []
        for i, (name, note, _tab) in enumerate(self.YARD_OPTIONS):
            yy = y + i * 36
            row = pygame.Rect(x + 4, yy, lw - 8, 32)
            self.comms_row_rects.append((i, row))
            self._checkbox(surface, x + 8, yy + 2, False)
            col = WHITE if i == self.state.selected else GREY
            surface.blit(small.render(name, True, col), (x + 22, yy))
            surface.blit(small.render(note, True, GREY), (x + 22, yy + 12))

    def _can_pay(self, price: int, *, credits: int, free_purchases: bool) -> bool:
        return free_purchases or credits >= price

    def _buy_allowed(self) -> bool:
        ok = getattr(self, "_afford_buy", None)
        if ok is None:
            return True
        return self.state.selected in ok

    def _service_date(self, day: float | None) -> str:
        if day is None:
            return "31-Dec-3199"
        total = int(day)
        year = 3200 + total // 365
        rem = total % 365
        months = ("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec")
        return f"{rem % 30 + 1}-{months[min(rem // 30, 11)]}-{year}"

    def _draw_repair(self, surface, small, fitted, shipyard, x, y, lw, *, credits: int = 0, free_purchases: bool = False) -> None:
        deluxe, drive, equip = shipyard.service_prices(fitted)
        rows = (
            ("Deluxe service (drive and equipment)", deluxe),
            (f"Drive only service (last: {self._service_date(fitted.last_drive_service)})", drive),
            (f"Equipment only service (last: {self._service_date(fitted.last_equip_service)})", equip),
        )
        self.comms_row_rects = []
        self._afford_buy = set()
        surface.blit(small.render("Repair Item", True, WHITE), (x + 8, y))
        surface.blit(small.render("Cost", True, WHITE), (x + lw - 50, y))
        for i, (name, price) in enumerate(rows):
            yy = y + 18 + i * 16
            can_pay = self._can_pay(price, credits=credits, free_purchases=free_purchases)
            if can_pay:
                row = pygame.Rect(x + 4, yy, lw - 8, 14)
                self.comms_row_rects.append((i, row))
                self._afford_buy.add(i)
            self._checkbox(surface, x + 8, yy + 2, False, enabled=can_pay)
            col = (WHITE if i == self.state.selected else GREY) if can_pay else LOCKED
            surface.blit(small.render(name, True, col), (x + 22, yy))
            surface.blit(small.render(f"Cr{price}", True, col), (x + lw - 50, yy))

    def handle_click(self, pos: tuple[int, int]) -> str | None:
        """Click a menu row or Go Back. Returns a docked action, or None."""
        if self.back_rect is not None and self.back_rect.collidepoint(pos):
            self._go_back()
            return None
        if self.more_rect is not None and self.more_rect.collidepoint(pos):
            self.state.scroll += 5
            return None
        for action, i, rect in getattr(self, "action_rects", []):
            if rect.collidepoint(pos):
                if action == "hangup":
                    self.state.tab = "comms"
                    self.state.selected = 0
                    self.state.scroll = 0
                    self._default_prompt()
                    return None
                self.state.selected = i
                if action == "viewship":
                    self.view_ship = i
                    self.state.tab = "shipview"
                    self._default_prompt()
                    return None
                self.state.mode = "remove" if action in ("remove", "sell") else "fit"
                return action
        if self.state.tab == "missions":
            for i, rect in self.comms_row_rects:
                if rect.collidepoint(pos):
                    self.state.selected = i
                    self._open_video_link()
                    return None
        if self.state.tab == "videolink":
            for i, rect in self.comms_row_rects:
                if rect.collidepoint(pos):
                    self.state.selected = i
                    return self._bbs_pick(i)
        if self.state.tab in ("comms", "yard", "repair"):
            for i, rect in self.comms_row_rects:
                if rect.collidepoint(pos):
                    self.state.selected = i
                    if self.state.tab == "yard":
                        return self._yard_action()
                    if self.state.tab == "repair":
                        return "repair"
                    return self._comms_action()
        return None

    def _checkbox(self, surface, x, y, on: bool, active_col=WHITE, enabled: bool = True) -> None:
        """Raised bevel. Pressed flips the light and dark edges."""
        del active_col
        size = 9
        face = (85, 68, 153)
        light = (214, 214, 220) if enabled else LOCKED
        dark = (0, 0, 0) if enabled else (28, 20, 48)
        pygame.draw.rect(surface, face, (x, y, size, size))
        top_left, bottom_right = (dark, light) if on and enabled else (light, dark)
        pygame.draw.line(surface, top_left, (x, y), (x + size - 1, y))
        pygame.draw.line(surface, top_left, (x, y), (x, y + size - 1))
        pygame.draw.line(surface, bottom_right, (x + size - 1, y), (x + size - 1, y + size - 1))
        pygame.draw.line(surface, bottom_right, (x, y + size - 1), (x + size - 1, y + size - 1))

    def _draw_outfit_table(
        self, surface, small, shipyard, system, fitted, x, y, lw, bottom, *, free_purchases: bool = False, credits: int = 0,
    ) -> bool:
        items = shipyard.available_equipment(system)
        # Leave no footer inside the list — hints live in the status strip
        visible = max(1, (bottom - y - 14) // 10)
        self.state.selected = min(self.state.selected, max(0, len(items) - 1))
        if self.state.selected >= self.state.scroll + visible:
            self.state.scroll = self.state.selected - visible + 1
        if self.state.selected < self.state.scroll:
            self.state.scroll = self.state.selected

        # Headers
        hy = y + 2
        surface.blit(small.render("Fit", True, GREY), (x + 4, hy))
        surface.blit(small.render("Remove", True, GREY), (x + 28, hy))
        surface.blit(small.render("Item", True, GREY), (x + 70, hy))
        # Right-aligned column headers
        for label, col_r in (("cr to Fit", x + lw - 78), ("cr Removal", x + lw - 28), ("Wt", x + lw - 2)):
            lab = small.render(label, True, GREY)
            surface.blit(lab, (col_r - lab.get_width(), hy))
        pygame.draw.line(surface, PURPLE_DIM, (x, hy + 11), (x + lw, hy + 11), 1)

        fit_col = x + lw - 78
        rem_col = x + lw - 28
        wt_col = x + lw - 2
        free = fitted.free_cargo(shipyard.ships, shipyard.equipment)
        self._afford_buy = set()
        for i, e in enumerate(items):
            fit_price = 0 if free_purchases else e.price
            stackable = bool(e.raw.get("stackable"))
            held = fitted.has(e.id) and not stackable
            if (not held) and e.mass_t <= free + 1e-6 and self._can_pay(fit_price, credits=credits, free_purchases=free_purchases):
                self._afford_buy.add(i)

        for row, i in enumerate(range(self.state.scroll, min(len(items), self.state.scroll + visible))):
            e = items[i]
            yy = y + 14 + row * 10
            selected = i == self.state.selected
            if selected:
                pygame.draw.rect(surface, PURPLE_DIM, (x + 1, yy - 1, lw - 2, 10))
            owned = fitted.has(e.id)
            stackable = bool(e.raw.get("stackable"))
            fit_price = 0 if free_purchases else e.price
            can_fit = i in self._afford_buy
            can_remove = owned
            if (stackable or not owned) and e.mass_t <= free + 1e-6:
                box = pygame.Rect(x + 8, yy + 1, 8, 8)
                if can_fit:
                    self.action_rects.append(("fit", i, box))
                    self._checkbox(surface, box.x, box.y, selected and self.state.mode == "fit")
                else:
                    self._checkbox(surface, box.x, box.y, False, enabled=False)
            if can_remove:
                box = pygame.Rect(x + 40, yy + 1, 8, 8)
                self.action_rects.append(("remove", i, box))
                self._checkbox(surface, box.x, box.y, selected and self.state.mode == "remove")
            name = e.name
            if stackable:
                n = fitted.count(e.id)
                if n:
                    name = f"{name} x{n}"
            if len(name) > 26:
                name = name[:24] + ".."
            col = WHITE if selected else GREY
            if owned and not selected:
                col = (200, 200, 220)
            if not can_fit and not owned:
                col = LOCKED
            surface.blit(small.render(name, True, col), (x + 70, yy))
            fit_s = small.render(f"{fit_price}", True, col)
            rem_s = small.render(f"{e.price // 2}", True, col)
            wt_s = small.render(f"{e.mass_t:.0f}", True, col)
            surface.blit(fit_s, (fit_col - fit_s.get_width(), yy))
            surface.blit(rem_s, (rem_col - rem_s.get_width(), yy))
            surface.blit(wt_s, (wt_col - wt_s.get_width(), yy))

        return self.state.scroll + visible < len(items)

    def _draw_stock_table(self, surface, small, listings, fitted, x, y, lw, bottom, *, free_purchases: bool = False, credits: int = 0):
        visible = max(1, (bottom - y - 16) // 10)
        self.state.selected = min(self.state.selected, max(0, len(listings) - 1))
        if self.state.selected >= self.state.scroll + visible:
            self.state.scroll = self.state.selected - visible + 1
        hy = y + 2
        surface.blit(small.render("Buy", True, GREY), (x + 4, hy))
        surface.blit(small.render("Sell", True, GREY), (x + 28, hy))
        surface.blit(small.render("Commodity", True, GREY), (x + 60, hy))
        surface.blit(small.render("Price", True, GREY), (x + lw - 90, hy))
        surface.blit(small.render("Stock", True, GREY), (x + lw - 50, hy))
        surface.blit(small.render("Hold", True, GREY), (x + lw - 22, hy))
        pygame.draw.line(surface, PURPLE_DIM, (x, hy + 11), (x + lw, hy + 11), 1)
        self._afford_buy = set()
        for i, L in enumerate(listings):
            show_price = 0 if free_purchases and L.price > 0 else L.price
            if L.stock > 0 and self._can_pay(show_price, credits=credits, free_purchases=free_purchases):
                self._afford_buy.add(i)
        for row, i in enumerate(range(self.state.scroll, min(len(listings), self.state.scroll + visible))):
            L = listings[i]
            yy = y + 14 + row * 10
            selected = i == self.state.selected
            if selected:
                pygame.draw.rect(surface, PURPLE_DIM, (x + 1, yy - 1, lw - 2, 10))
            hold = fitted.cargo.get(L.good.id, 0)
            show_price = 0 if free_purchases and L.price > 0 else L.price
            can_buy = i in self._afford_buy
            if L.stock > 0:
                box = pygame.Rect(x + 8, yy + 1, 8, 8)
                if can_buy:
                    self.action_rects.append(("buy", i, box))
                    self._checkbox(surface, box.x, box.y, selected and self.state.mode == "fit")
                else:
                    self._checkbox(surface, box.x, box.y, False, enabled=False)
            if hold > 0:
                box = pygame.Rect(x + 36, yy + 1, 8, 8)
                self.action_rects.append(("sell", i, box))
                self._checkbox(surface, box.x, box.y, selected and self.state.mode == "remove")
            col = RED if not L.good.legal else (WHITE if selected else GREY)
            if L.stock > 0 and not can_buy:
                col = LOCKED
            name = L.good.name[:24]
            surface.blit(small.render(name, True, col), (x + 60, yy))
            surface.blit(small.render(f"{show_price}", True, col), (x + lw - 90, yy))
            surface.blit(small.render(f"{L.stock}", True, col), (x + lw - 50, yy))
            surface.blit(small.render(f"{hold}", True, col), (x + lw - 18, yy))
        surface.blit(small.render("B/Enter=Buy  S=Sell", True, GREY), (x + 4, bottom - 12))

    def _draw_ships_table(self, surface, small, ships: list[ShipDef], x, y, lw, bottom, *, free_purchases: bool = False, credits: int = 0):
        visible = max(1, (bottom - y - 16) // 10)
        self.state.selected = min(self.state.selected, max(0, len(ships) - 1))
        hy = y + 2
        surface.blit(small.render("Buy", True, GREY), (x + 4, hy))
        surface.blit(small.render("View", True, GREY), (x + 28, hy))
        surface.blit(small.render("Ship", True, GREY), (x + 64, hy))
        surface.blit(small.render("Mass", True, GREY), (x + lw - 140, hy))
        surface.blit(small.render("Cap", True, GREY), (x + lw - 100, hy))
        surface.blit(small.render("Jump", True, GREY), (x + lw - 70, hy))
        surface.blit(small.render("Price", True, GREY), (x + lw - 40, hy))
        pygame.draw.line(surface, PURPLE_DIM, (x, hy + 11), (x + lw, hy + 11), 1)
        self._afford_buy = {
            i for i, s in enumerate(ships)
            if self._can_pay(0 if free_purchases else s.price, credits=credits, free_purchases=free_purchases)
        }
        for row, i in enumerate(range(self.state.scroll, min(len(ships), self.state.scroll + visible))):
            s = ships[i]
            yy = y + 14 + row * 10
            selected = i == self.state.selected
            price = 0 if free_purchases else s.price
            can_buy = i in self._afford_buy
            if selected:
                pygame.draw.rect(surface, PURPLE_DIM, (x + 1, yy - 1, lw - 2, 10))
            col = (WHITE if selected else GREY) if can_buy else LOCKED
            buy = pygame.Rect(x + 6, yy, 10, 10)
            view = pygame.Rect(x + 32, yy, 10, 10)
            if can_buy:
                self.action_rects.append(("buy", i, buy))
            self.action_rects.append(("viewship", i, view))
            self._checkbox(surface, buy.x, buy.y, False, enabled=can_buy)
            self._checkbox(surface, view.x, view.y, False)
            surface.blit(small.render(s.name[:22], True, col), (x + 64, yy))
            surface.blit(small.render(f"{s.mass_t:.0f}", True, col), (x + lw - 140, yy))
            surface.blit(small.render(f"{s.capacity_t:.0f}", True, col), (x + lw - 100, yy))
            surface.blit(small.render(f"{s.jump_ly_base:.1f}", True, col), (x + lw - 70, yy))
            surface.blit(small.render(f"{price}", True, col), (x + lw - 40, yy))

    def _draw_ship_view(self, surface, small, ships: list[ShipDef], x, y) -> None:
        if not ships:
            return
        s = ships[min(getattr(self, "view_ship", 0), len(ships) - 1)]
        lines = [
            s.name,
            f"Mass (fully laden)  {s.mass_t + s.capacity_t:.0f}t",
            f"Internal capacity (no drive)  {s.capacity_t:.0f}t",
            f"Gun mountings  {len(s.gun_mounts)}",
            f"Missile pylons  {s.missiles}",
            f"Crew  {s.crew}",
            f"Main thruster acceleration  {s.thrust_g:.1f} Earth g",
            f"Retro thruster acceleration  {s.retro_g:.1f} Earth g",
            f"Drive fitted  {s.drive}",
            f"Laden jump  {s.jump_ly_base:.2f} ly",
        ]
        for i, line in enumerate(lines):
            surface.blit(small.render(line, True, WHITE if i == 0 else GREY), (x + 8, y + i * 14))

    def _wrap(self, small, text: str, width: int) -> list[str]:
        words = text.split()
        lines: list[str] = []
        cur = ""
        for word in words:
            trial = word if not cur else f"{cur} {word}"
            if small.size(trial)[0] <= width:
                cur = trial
            else:
                if cur:
                    lines.append(cur)
                cur = word
        if cur:
            lines.append(cur)
        return lines or [""]

    def _draw_missions(self, surface, small, missions: list[Mission], x, y, lw, bottom) -> bool:
        if not missions:
            surface.blit(small.render("No advertisements.", True, GREY), (x + 8, y + 8))
            return False
        self.comms_row_rects = []
        self.state.selected = min(self.state.selected, max(0, len(missions) - 1))
        yy = y + 4
        more = False
        for i in range(self.state.scroll, len(missions)):
            m = missions[i]
            lines = self._wrap(small, m.description, lw - 36)
            block = 8 + len(lines) * 11
            if yy + block > bottom - 18:
                more = True
                break
            self.comms_row_rects.append((i, pygame.Rect(x + 4, yy, lw - 8, block)))
            self._checkbox(surface, x + 8, yy + 2, False)
            col = WHITE if i == self.state.selected else GREY
            for n, line in enumerate(lines):
                surface.blit(small.render(line, True, col), (x + 22, yy + n * 11))
            yy += block + 8
        return more

    def _open_video_link(self) -> None:
        self.bbs_index = self.state.selected
        self.bbs_note = ""
        self.state.tab = "videolink"
        self.state.selected = 0

    def _bbs_pick(self, index: int) -> str | None:
        actions = getattr(self, "bbs_actions", [])
        if index < 0 or index >= len(actions) or actions[index][0] == "hangup":
            self.bbs_note = ""
            self.state.tab = "missions"
            self._default_prompt()
            return None
        self.state.selected = index
        return "bbs_yes"

    def _draw_video_link(self, surface, small, missions: list[Mission], fitted, x, y) -> None:
        if not missions:
            self.state.tab = "missions"
            return
        m = missions[min(getattr(self, "bbs_index", 0), len(missions) - 1)]
        hold = int(fitted.cargo.get(m.cargo_id, 0))
        who = m.speaker or "The advertiser"
        if m.kind == "passenger":
            offer = (
                f"I'm {who}. I need transport to {m.dest_name} "
                f"before day {m.deadline_day:.0f}. I'll pay [${m.reward}]."
            )
            options = [f"Agree to transport to {m.dest_name} for ${m.reward}"]
            self.bbs_actions = [("agree",)]
        else:
            unit = m.pay_each or float(m.reward)
            offer = (
                f"I'm {who}. I need {m.cargo_name} urgently, I'll pay twice "
                f"the market price [${unit:.1f}] for them."
            )
            options = []
            self.bbs_actions = []
            for qty in (1, 10):
                word = "tonne" if qty == 1 else "tonnes"
                options.append(f"Sell {qty} {word} of {m.cargo_name} for ${unit * qty:.1f}")
                self.bbs_actions.append(("sell", qty))
            options[0] += f"\n({hold} tonnes in cargo bay)"
            all_pay = unit * hold
            options.append(f"Sell all {m.cargo_name} for ${all_pay:.1f}")
            self.bbs_actions.append(("sell", -1))
        options.append("Hang up.")
        self.bbs_actions.append(("hangup",))
        self.state.selected = min(self.state.selected, len(options) - 1)
        self.state.message = self.bbs_note or offer
        surface.blit(small.render("Suggested Responses:", True, WHITE), (x + 4, y))
        self.comms_row_rects = []
        yy = y + 18
        for i, line in enumerate(options):
            wrapped = line.split("\n")
            block = max(14, 11 * len(wrapped))
            self.comms_row_rects.append((i, pygame.Rect(x + 4, yy, 400, block)))
            self._checkbox(surface, x + 8, yy + 1, False)
            col = WHITE if i == self.state.selected else (196, 196, 204)
            for n, part in enumerate(wrapped):
                surface.blit(small.render(part, True, col), (x + 22, yy + n * 11))
            yy += block + 8

    def _draw_status(self, surface, small, fitted, shipyard, legal, fines, x, y):
        hull = shipyard.ships[fitted.ship_id]
        lines = [
            f"Ship: {hull.name}",
            f"Hull {hull.mass_t:.0f}t  Cap {hull.capacity_t:.0f}t  Laden {hull.mass_t + hull.capacity_t:.0f}t",
            f"Main {hull.thrust_g:.1f}g  Retro {hull.retro_g:.1f}g  Crew {hull.crew}",
            f"Laden jump {hull.jump_ly_base:.2f} ly  Missiles {hull.missiles}",
            f"Fuel {fitted.fuel_t:.2f}t  Hull {fitted.hull:.0f}/{fitted.max_hull:.0f}",
            f"Legal: {'yes' if legal else 'NO'}  Fines: {fines}",
            "Equipment:",
        ]
        for i, line in enumerate(lines):
            surface.blit(small.render(line, True, WHITE if i == 0 else GREY), (x + 8, y + 8 + i * 10))
        cy = y + 8 + len(lines) * 10
        for eid in fitted.equipment:
            e = shipyard.equipment[eid]
            surface.blit(small.render(f"  {e.name}", True, GREY), (x + 8, cy))
            cy += 9
            if cy > surface.get_height() - 70:
                break

    def _ship_column(self, surface, small, fitted, shipyard, credits, docked: bool) -> None:
        """Ship render on the right, hold figures underneath it."""
        w, h = surface.get_size()
        ch = console_bar_height(h)
        from render.ship_models import blit_ship_preview

        blit_ship_preview(
            surface,
            pygame.Rect(w - 210, 28, 190, 168),
            fitted.ship_id,
            guns=fitted.guns,
            missile_count=len(fitted.missile_load),
            equipment=fitted.equipment,
            gear_down=docked,
        )
        free = fitted.free_cargo(shipyard.ships, shipyard.equipment)
        used = fitted.cargo_mass() + fitted.fuel_t
        cabins = sum(int(shipyard.equipment[e].raw.get("passengers", 0)) for e in fitted.equipment)
        aboard = int(fitted.cargo.get("passenger", 0))
        by = h - ch - 52
        x = w - 200
        surface.blit(small.render("Remaining", True, GREY), (x + 54, by))
        surface.blit(small.render("Used", True, GREY), (x + 112, by))
        surface.blit(small.render("Cargo Space", True, WHITE), (x, by + 12))
        surface.blit(small.render(f"{free:4.0f}", True, WHITE), (x + 54, by + 12))
        surface.blit(small.render(f"{used:4.0f}", True, WHITE), (x + 112, by + 12))
        surface.blit(small.render("Cash", True, WHITE), (x + 150, by + 12))
        surface.blit(small.render("Cabins", True, WHITE), (x, by + 24))
        surface.blit(small.render(f"{max(0, cabins - aboard):4d}", True, WHITE), (x + 54, by + 24))
        surface.blit(small.render(f"{aboard:4d}", True, WHITE), (x + 112, by + 24))
        surface.blit(small.render(f"${credits:.1f}", True, WHITE), (x + 150, by + 24))

    def _console(self, surface, small, *, stardate, message, docked, compression_index, time_paused, view_mode, info_page, engines_off, gear_down, hud_labels, hyperspace_ready, autopilot):
        return draw_docked_console(
            surface,
            small,
            stardate=stardate,
            message=message,
            docked=docked,
            comms=False,
            compression_index=compression_index,
            paused=time_paused,
            view_mode=view_mode,
            map_mode="galaxy",
            info_page=info_page,
            engines_off=engines_off,
            gear_down=gear_down,
            hud_labels=hud_labels,
            hyperspace_ready=hyperspace_ready,
            autopilot=autopilot,
        )

    def draw_ship_status(
        self, surface, font, small, *, fitted, shipyard, credits, stardate, compression_index,
        time_paused, docked, view_mode="front", engines_off=False, gear_down=True,
        hud_labels=True, hyperspace_ready=False, autopilot=False, jump_ly=0.0,
    ):
        """Equipment list on the left, ship on the right."""
        surface.fill((85, 68, 153))
        hull = shipyard.ships[fitted.ship_id]
        drive = next((shipyard.equipment[e] for e in fitted.equipment if shipyard.equipment[e].slot == "drive"), None)
        drive_name = drive.name if drive else "None"
        shields = sum(1 for e in fitted.equipment if e == "shield_gen")
        y = 16
        surface.blit(font.render(f"SHIP EQUIPMENT STATUS - {hull.name}", True, WHITE), (12, y))
        y += 22
        lines = [
            f"Drive System:   {drive_name} (max.{hull.jump_ly_base:.2f} ly)",
            f"                         (current {jump_ly:.2f} ly)",
            f"Hull Condition:   {100.0 * fitted.hull / max(1.0, fitted.max_hull):.1f}%",
            f"Shield Generators: {shields} unit(s)",
            "",
        ]
        for eid in fitted.equipment:
            e = shipyard.equipment[eid]
            if e.slot in ("drive", "gun", "missile") or eid == "shield_gen":
                continue
            lines.append(e.name)
        lines.append("")
        for mount, gid in fitted.guns.items():
            lines.append(f"{mount.title()} Gun Mounting:   {shipyard.equipment[gid].name}")
        for line in lines:
            surface.blit(small.render(line, True, WHITE), (12, y))
            y += 14
        self._ship_column(surface, small, fitted, shipyard, credits, docked)
        return self._console(
            surface, small, stardate=stardate, message="", docked=docked,
            compression_index=compression_index, time_paused=time_paused, view_mode=view_mode,
            info_page=0, engines_off=engines_off, gear_down=gear_down, hud_labels=hud_labels,
            hyperspace_ready=hyperspace_ready, autopilot=autopilot,
        )

    def draw_commander(
        self, surface, font, small, *, fitted, shipyard, credits, name, legal, rank, stardate,
        compression_index, time_paused, docked, view_mode="front", engines_off=False,
        gear_down=True, hud_labels=True, hyperspace_ready=False, autopilot=False,
    ):
        """Commander profile. Ranks we do not simulate stay at the starting titles."""
        surface.fill((96, 96, 96))
        surface.blit(font.render(f"PROFILE FOR COMMANDER {name.upper()}", True, WHITE), (12, 16))
        legal_word = "Clean" if legal else "Offender"
        rows = [
            (36, "MILITARY STANDING", "LEGAL STATUS"),
            (54, f"Federal Rank: None", f"Federal:  {legal_word}"),
            (68, "Imperial Title: Outsider", f"Imperial:  {legal_word}"),
            (82, f"Elite Rating:  {rank}", f"Interpol:  {legal_word}"),
            (104, "FEDERAL CRIMINAL RECORD", ""),
            (140, "MEDALS AND PERMITS", ""),
        ]
        for y, left, right in rows:
            if left:
                surface.blit(small.render(left, True, WHITE), (12, y))
            if right:
                surface.blit(small.render(right, True, WHITE), (250, y))
        self._ship_column(surface, small, fitted, shipyard, credits, docked)
        return self._console(
            surface, small, stardate=stardate, message="", docked=docked,
            compression_index=compression_index, time_paused=time_paused, view_mode=view_mode,
            info_page=1, engines_off=engines_off, gear_down=gear_down, hud_labels=hud_labels,
            hyperspace_ready=hyperspace_ready, autopilot=autopilot,
        )

    def draw_cargo(
        self,
        surface: pygame.Surface,
        font: pygame.font.Font,
        small: pygame.font.Font,
        *,
        fitted: FittedShip,
        shipyard: Shipyard,
        names: dict[str, str],
        stardate: str,
        compression_index: int,
        time_paused: bool,
        docked: bool,
        credits: float = 0,
        view_mode: str = "front",
        engines_off: bool = False,
        gear_down: bool = True,
        hud_labels: bool = True,
        hyperspace_ready: bool = False,
        autopilot: bool = False,
    ) -> ConsoleHits | None:
        """Cargo list on the left. Jettison is a small checkbox beside each line."""
        h = surface.get_height()
        ch = console_bar_height(h)
        surface.fill((168, 88, 80))
        surface.blit(font.render("CARGO INVENTORY", True, WHITE), (12, 14))
        self.refuel_rect = pygame.Rect(250, 16, 9, 9)
        self._checkbox(surface, self.refuel_rect.x, self.refuel_rect.y, False)
        surface.blit(small.render("Refuel", True, WHITE), (264, 14))

        self.jettison_rects = []
        self.jettison_glitch_rects = []
        lines: list[tuple[str, str]] = []
        if fitted.fuel_t >= 0.05:
            lines.append(("fuel", f"Hydrogen Fuel   {fitted.fuel_t:.0f}t"))
        for gid, tonnes in fitted.cargo.items():
            if tonnes > 0:
                lines.append((gid, f"{names.get(gid, gid)}   {tonnes:.0f}t"))
        self.cargo_rows = [gid for gid, _ in lines if gid != "fuel"]
        yy = 40
        for gid, label in lines:
            if yy > h - ch - 56:
                break
            box = pygame.Rect(12, yy, 9, 9)
            self._checkbox(surface, box.x, box.y, False)
            surface.blit(small.render("Jettison", True, RED), (26, yy - 1))
            surface.blit(small.render(label, True, WHITE), (110, yy - 1))
            self.jettison_rects.append((gid, box))
            if gid == "rubbish":
                self.jettison_glitch_rects.append((gid, pygame.Rect(box.x, box.bottom, box.w, 6)))
            yy += 16

        self._ship_column(surface, small, fitted, shipyard, credits, docked)
        return self._console(
            surface, small, stardate=stardate, message="", docked=docked,
            compression_index=compression_index, time_paused=time_paused, view_mode=view_mode,
            info_page=2, engines_off=engines_off, gear_down=gear_down, hud_labels=hud_labels,
            hyperspace_ready=hyperspace_ready, autopilot=autopilot,
        )
