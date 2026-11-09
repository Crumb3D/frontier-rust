"""Navigation helpers — autopilot steer + on-screen direction markers."""

from __future__ import annotations

import math

import numpy as np
import pygame

# Marker colours (FE2-ish)
COL_STAR = (255, 220, 80)
COL_PLANET = (80, 200, 120)
COL_MOON = (160, 180, 200)
COL_STATION = (80, 180, 255)
COL_STARPORT = (255, 160, 60)
COL_AP = (48, 230, 72)


def project_body(
    world_pos: np.ndarray,
    cam_pos: np.ndarray,
    cam_rot: np.ndarray,
    view_w: int,
    view_h: int,
    fov: float,
) -> tuple[float, float, float, bool]:
    """Return (sx, sy, depth, in_front). sx/sy may be off-screen."""
    rel = (world_pos - cam_pos) @ cam_rot
    z = float(rel[2])
    if abs(z) < 1e-6:
        z = 1e-6
    sx = view_w / 2 + fov * float(rel[0]) / z
    sy = view_h / 2 - fov * float(rel[1]) / z
    return sx, sy, z, z > 0.05


def edge_clamp(sx: float, sy: float, view_w: int, view_h: int, margin: int = 14) -> tuple[int, int, bool]:
    """Clamp to view edge; returns (x, y, was_clamped)."""
    on = margin <= sx < view_w - margin and margin <= sy < view_h - margin
    if on:
        return int(sx), int(sy), False
    cx, cy = view_w / 2, view_h / 2
    dx, dy = sx - cx, sy - cy
    if abs(dx) < 1e-6 and abs(dy) < 1e-6:
        return int(cx), margin, True
    # Intersect with inset rectangle
    hw, hh = view_w / 2 - margin, view_h / 2 - margin
    scale = min(hw / max(abs(dx), 1e-6), hh / max(abs(dy), 1e-6))
    return int(cx + dx * scale), int(cy + dy * scale), True


def _tunnel_spacing(dist_km: float) -> float:
    """World gap between squares. Shrinks as the target gets closer, so they pass faster."""
    if dist_km > 20000:
        return 2500.0
    if dist_km > 4000:
        return 500.0
    if dist_km > 800:
        return 100.0
    if dist_km > 150:
        return 20.0
    if dist_km > 30:
        return 4.0
    if dist_km > 6:
        return 0.8
    return 0.2


def draw_targeting_tunnel(
    surface: pygame.Surface,
    world_pos: np.ndarray,
    *,
    cam_pos: np.ndarray,
    cam_rot: np.ndarray,
    view_h: int,
    fov: float = 200.0,
    ship_forward: np.ndarray | None = None,
    font: pygame.font.Font | None = None,
    behind_label: bool = True,
) -> None:
    """Squares at real distances along the line from the ship to the target.

    The lattice is anchored on absolute range, so flying toward the target
    makes the near square grow and pass, stopping freezes it, and reversing
    sends it back out. Pitch, yaw and roll move the projected squares because
    they are rebuilt from the current target vector every frame.
    """
    w = surface.get_width()
    to = world_pos - cam_pos
    dist = float(np.linalg.norm(to))
    if dist < 0.05:
        return
    direction = to / dist
    facing = float(np.dot(direction, ship_forward)) if ship_forward is not None else float((to @ cam_rot)[2] / dist)
    col = (48, 200, 72)
    if facing <= 0.02:
        if behind_label and font is not None:
            label = font.render("TARGET BEHIND", False, col)
            surface.blit(label, (w // 2 - label.get_width() // 2, view_h // 2 - 8))
        return
    sx, sy, _depth, in_front = project_body(world_pos, cam_pos, cam_rot, w, view_h, fov)
    if not in_front:
        return
    _tx, _ty, off = edge_clamp(sx, sy, w, view_h, margin=6)
    if off:
        return

    spacing = _tunnel_spacing(dist)
    # Nearest square sits (dist mod spacing) km ahead. That remainder shrinks
    # as range falls, so the square comes at the ship at the closing speed.
    gap = dist % spacing
    if gap < spacing * 0.12:
        gap += spacing
    half_world = spacing * 0.18
    for i in range(7):
        along = gap + i * spacing
        if along >= dist * 0.96 or along < 0.2:
            break
        point = cam_pos + direction * along
        px, py, pz, front = project_body(point, cam_pos, cam_rot, w, view_h, fov)
        if not front or pz < 0.15:
            continue
        half = fov * half_world / pz
        if half < 2.0 or half > view_h * 0.45:
            continue
        pygame.draw.rect(
            surface, col,
            (int(px - half), int(py - half), int(half * 2), int(half * 2)),
            1,
        )


def draw_direction_markers(
    surface: pygame.Surface,
    small: pygame.font.Font,
    markers: list[tuple[np.ndarray, str, tuple[int, int, int], bool]],
    *,
    cam_pos: np.ndarray,
    cam_rot: np.ndarray,
    view_h: int,
    fov: float = 200.0,
    show_labels: bool = True,
) -> None:
    """Draw boxed markers / edge arrows for star, planets, stations.

    markers: (world_pos, label, colour, is_ap_target)
    """
    w = surface.get_width()
    # Project first so we can keep only the nearest labels and avoid pile-ups
    drawn: list[tuple[float, int, int, bool, tuple[int, int, int], str, bool]] = []
    for world_pos, label, col, is_ap in markers:
        sx, sy, _depth, in_front = project_body(world_pos, cam_pos, cam_rot, w, view_h, fov)
        if not in_front:
            # Park the marker off the side you must turn, not in the middle of the view.
            rel = (world_pos - cam_pos) @ cam_rot
            sx = w / 2 + float(rel[0]) * 1e6
            sy = view_h / 2 - float(rel[1]) * 1e6
        x, y, clamped = edge_clamp(sx, sy, w, view_h)
        dist = float(np.linalg.norm(world_pos - cam_pos))
        drawn.append((dist, x, y, clamped or not in_front, col, label, is_ap))

    drawn.sort(key=lambda t: (not t[6], t[0]))
    labelled = 0
    occupied: list[pygame.Rect] = []
    for dist, x, y, off, col, label, is_ap in drawn:
        accent = COL_AP if is_ap else col
        size = 5 if is_ap else 3
        if off:
            pygame.draw.polygon(
                surface,
                accent,
                [(x, y - size - 2), (x - size, y + size), (x + size, y + size)],
                1,
            )
        else:
            pygame.draw.rect(surface, accent, (x - size, y - size, size * 2, size * 2), 1)
            if is_ap:
                pygame.draw.circle(surface, accent, (x, y), size + 3, 1)
        # Labels: autopilot target always; otherwise nearest few that are on screen
        want_label = show_labels and label and (is_ap or (not off and labelled < 4))
        if not want_label:
            continue
        if dist >= 1_000_000:
            dtxt = f"{dist/1_000_000:.2f}Gm"
        elif dist >= 1000:
            dtxt = f"{dist/1000:.1f}Mm"
        elif dist >= 1:
            dtxt = f"{dist:.0f}km"
        else:
            dtxt = f"{dist*1000:.0f}m"
        text = f"{label} {dtxt}"
        lab = small.render(text[:24], True, accent)
        lx = min(max(2, x + 8), max(2, w - lab.get_width() - 2))
        ly = min(max(2, y - lab.get_height() - 2), max(2, view_h - lab.get_height() - 2))
        rect = pygame.Rect(lx - 1, ly - 1, lab.get_width() + 2, lab.get_height() + 2)
        # Push down until it clears other labels
        for _ in range(8):
            if not any(rect.colliderect(o) for o in occupied):
                break
            rect.y += lab.get_height() + 2
            if rect.bottom > view_h - 2:
                rect.y = 2
                break
        bg = pygame.Surface((rect.w, rect.h), pygame.SRCALPHA)
        bg.fill((0, 0, 0, 160))
        surface.blit(bg, rect.topleft)
        surface.blit(lab, (rect.x + 1, rect.y + 1))
        occupied.append(rect)
        labelled += 1


def steer_toward(
    orientation: np.ndarray,
    desired_dir: np.ndarray,
) -> tuple[float, float, float, float]:
    """Return (pitch, yaw, roll, align) to face desired_dir (world).

    orient columns: right, up, forward (+Z nose).
    align is 1 when nose is on target, 0 when 90deg+ off.
    """
    n = float(np.linalg.norm(desired_dir))
    if n < 1e-9:
        return 0.0, 0.0, 0.0, 1.0
    desired = desired_dir / n
    # Body-frame: x=right, y=up, z=forward
    body = orientation.T @ desired
    bx, by, bz = float(body[0]), float(body[1]), float(body[2])

    # atan2 gives stable signed errors in (-pi, pi]
    yaw_err = math.atan2(bx, bz)
    pitch_err = math.atan2(by, math.hypot(bx, bz) if bz > -0.05 else max(bz, 0.01))

    # When target is behind, prioritise yaw; hold pitch
    if bz < 0.0:
        pitch_err *= max(0.0, bz + 1.0)  # fade pitch while turning around

    # Gentle proportional gain — avoid saturating until ~70deg off
    gain = 0.85
    yaw = float(np.clip(yaw_err * gain, -1.0, 1.0))
    pitch = float(np.clip(pitch_err * gain, -1.0, 1.0))

    # Deadband to kill jitter on boresight
    if abs(yaw_err) < 0.03:
        yaw = 0.0
    if abs(pitch_err) < 0.03:
        pitch = 0.0

    # Minimal roll — only to level wings slightly when aligned
    roll = 0.0
    if bz > 0.85:
        # Keep belly down: world up projected onto body right should be ~0
        world_up = np.array([0.0, 1.0, 0.0], dtype=np.float64)
        body_up_err = float((orientation.T @ world_up)[0])
        roll = float(np.clip(-body_up_err * 0.4, -0.25, 0.25))

    align = float(np.clip(bz, 0.0, 1.0))
    return pitch, yaw, roll, align


def ap_speed_for_distance(dist_km: float, align: float = 1.0) -> float:
    """FE2-ish set-speed ladder; zero until roughly facing the destination."""
    if align < 0.72:
        return 0.0
    if dist_km > 50000:
        spd = 60.0
    elif dist_km > 5000:
        spd = 20.0
    elif dist_km > 500:
        spd = 6.0
    elif dist_km > 50:
        spd = 2.0
    elif dist_km > 15:
        spd = 0.6
    elif dist_km > 5:
        spd = 0.2
    else:
        spd = 0.05
    return spd * float(np.clip((align - 0.72) / 0.28, 0.0, 1.0))
