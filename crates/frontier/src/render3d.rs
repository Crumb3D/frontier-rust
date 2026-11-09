//! Software 3D: starfield, per-pixel shaded planets, painter-sorted flat polygons.
//! Port of render/soft.py, with planets ray-cast so they also work from low orbit.

use crate::fb::{Canvas, Rect};
use crate::meshes::Mesh;
use crate::palette::{self, Rgb};
use glam::{DMat3, DVec3};

/// Projection for the 3D view rectangle.
#[derive(Clone, Copy)]
pub struct View {
    pub rect: Rect,
    pub focal: f64,
}

impl View {
    pub fn new(rect: Rect) -> Self {
        Self { rect, focal: rect.w as f64 * 0.58 }
    }

    fn cx(&self) -> f64 {
        self.rect.x as f64 + self.rect.w as f64 * 0.5
    }
    fn cy(&self) -> f64 {
        self.rect.y as f64 + self.rect.h as f64 * 0.5
    }

    /// Camera-space point (x right, y up, z forward) to screen.
    pub fn project(&self, p: DVec3) -> Option<(f64, f64)> {
        if p.z <= 1e-6 {
            return None;
        }
        Some((self.cx() + p.x * self.focal / p.z, self.cy() - p.y * self.focal / p.z))
    }

    fn ray(&self, sx: f64, sy: f64) -> DVec3 {
        DVec3::new((sx - self.cx()) / self.focal, -(sy - self.cy()) / self.focal, 1.0).normalize()
    }
}

/// World-to-camera: `rot` columns are the camera's right, up and forward in world axes.
pub fn to_cam(rot: &DMat3, v: DVec3) -> DVec3 {
    rot.transpose() * v
}

pub fn starfield(seed: u64, n: usize) -> Vec<(DVec3, u8)> {
    let mut s = seed | 1;
    let mut rnd = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s % 1_000_000) as f64 / 1_000_000.0
    };
    (0..n)
        .map(|_| {
            let z = rnd() * 2.0 - 1.0;
            let a = rnd() * std::f64::consts::TAU;
            let r = (1.0 - z * z).sqrt();
            let b = [68u8, 102, 136, 170, 221][(rnd() * 5.0) as usize % 5];
            (DVec3::new(r * a.cos(), z, r * a.sin()), b)
        })
        .collect()
}

pub fn draw_stars(cv: &mut Canvas, view: &View, rot: &DMat3, stars: &[(DVec3, u8)]) {
    for &(d, b) in stars {
        let c = to_cam(rot, d);
        if let Some((x, y)) = view.project(c) {
            cv.put(x as i32, y as i32, [b, b, b]);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum BodyKind {
    Star,
    Rocky,
    Gas,
}

pub struct SphereBody {
    /// Centre relative to the camera, world axes (km).
    pub center: DVec3,
    pub radius: f64,
    pub color: Rgb,
    /// Sea colour for rocky bodies; `None` tints the land toward blue.
    pub sea: Option<Rgb>,
    pub kind: BodyKind,
    pub seed: u32,
    /// World-to-body rotation (planet spin), used for surface detail.
    pub spin: DMat3,
    pub rings: Option<Rings>,
    /// Set when the camera is down in this body's atmosphere.
    pub near: Option<NearGround>,
}

pub struct Rings {
    /// Ring-plane normal, world axes.
    pub normal: DVec3,
    pub inner: f64,
    pub outer: f64,
    pub colors: [Rgb; 2],
}

/// How the ground looks from inside the atmosphere: flat colour with a haze strip
/// at the horizon, measured on the DOS opening still.
pub struct NearGround {
    /// Local vertical at the camera, world axes.
    pub up: DVec3,
    pub ground: Rgb,
    pub haze: [Rgb; 2],
    /// 0..1 daylight at the camera.
    pub day: f64,
}

/// Sky band edges above the horizon, radians (from PIC002 at the DOS focal length).
const SKY_EDGES: [f64; 4] = [0.253, 0.150, 0.080, 0.032];
const HAZE_EDGES: [f64; 2] = [0.0108, 0.059];

/// Atmosphere seen from inside: hard colour bands by elevation, faded toward black
/// with altitude (`fade` 1 on the ground, 0 at the top of the atmosphere).
pub fn draw_sky(cv: &mut Canvas, view: &View, rot: &DMat3, up: DVec3, bands: &[Rgb; 5], fade: f64) {
    let r = view.rect;
    let cols: Vec<Rgb> = bands.iter().map(|&b| palette::dos(palette::scale(b, fade))).collect();
    // Like the original, each screen row is one band (taken at the centre column),
    // so the bands are straight stripes rather than curved shells.
    let cx = r.x as f64 + r.w as f64 * 0.5;
    for py in r.y..r.bottom() {
        let d = *rot * view.ray(cx, py as f64 + 0.5);
        let e = d.dot(up).clamp(-1.0, 1.0).asin();
        let i = SKY_EDGES.iter().position(|&edge| e > edge).unwrap_or(4);
        cv.hline(r.x, r.right() - 1, py, cols[i]);
    }
}

fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f64 {
    let mut h = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ (z as u32).wrapping_mul(2_147_483_647)
        ^ seed.wrapping_mul(2_654_435_761);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    (h ^ (h >> 16)) as f64 / u32::MAX as f64
}

fn value_noise(p: DVec3, seed: u32) -> f64 {
    let f = p.floor();
    let t = p - f;
    let s = t * t * (DVec3::splat(3.0) - 2.0 * t);
    let (ix, iy, iz) = (f.x as i32, f.y as i32, f.z as i32);
    let mut acc = 0.0;
    for dz in 0..2 {
        for dy in 0..2 {
            for dx in 0..2 {
                let w = (if dx == 1 { s.x } else { 1.0 - s.x })
                    * (if dy == 1 { s.y } else { 1.0 - s.y })
                    * (if dz == 1 { s.z } else { 1.0 - s.z });
                acc += w * hash3(ix + dx, iy + dy, iz + dz, seed);
            }
        }
    }
    acc
}

fn surface_color(body: &SphereBody, local_n: DVec3) -> Rgb {
    match body.kind {
        // The DOS still shows Aster as one flat colour.
        BodyKind::Star | BodyKind::Gas => body.color,
        BodyKind::Rocky => {
            let n = value_noise(local_n * 2.2, body.seed) * 0.65
                + value_noise(local_n * 5.0, body.seed ^ 0x9e37) * 0.35;
            match body.sea {
                // Flat land and sea, like Merlin in the DOS view after launch.
                Some(sea) => {
                    if n > 0.5 {
                        body.color
                    } else {
                        sea
                    }
                }
                None if n > 0.52 => palette::scale(body.color, 0.85 + (n - 0.52) * 1.2),
                None => palette::mix(body.color, [34, 51, 119], 0.55),
            }
        }
    }
}

/// Ray-cast spheres far-to-near. `light` is the star position relative to the camera.
pub fn draw_spheres(cv: &mut Canvas, view: &View, rot: &DMat3, bodies: &mut [SphereBody], light: DVec3) {
    bodies.sort_by(|a, b| b.center.length().partial_cmp(&a.center.length()).unwrap());
    let r = view.rect;
    for body in bodies.iter() {
        let dist = body.center.length();
        let c_cam = to_cam(rot, body.center);
        let extent = body.rings.as_ref().map_or(body.radius, |rg| rg.outer);
        // Screen bounds: full view when close, otherwise the projected disc.
        let mut bx = (r.x, r.y, r.right(), r.bottom());
        if dist > extent * 1.05 {
            let ang = (extent / dist).asin();
            if c_cam.z <= 0.0 && ang < 1.2 {
                continue;
            }
            if let Some((sx, sy)) = view.project(c_cam) {
                let pr = view.focal * extent / (dist * dist - extent * extent).sqrt() * 1.15
                    / (c_cam.z / dist).max(0.2);
                if pr < 0.75 {
                    let lit = if body.kind == BodyKind::Star { body.color } else { palette::scale(body.color, 0.7) };
                    cv.put(sx as i32, sy as i32, palette::dos(lit));
                    continue;
                }
                bx = (
                    ((sx - pr) as i32 - 1).max(r.x),
                    ((sy - pr) as i32 - 1).max(r.y),
                    ((sx + pr) as i32 + 2).min(r.right()),
                    ((sy + pr) as i32 + 2).min(r.bottom()),
                );
            }
        }
        let l_dir = (light - body.center).normalize_or_zero();
        let c2 = body.center.length_squared() - body.radius * body.radius;
        let near_cols = body.near.as_ref().map(|g| {
            let k = 0.25 + 0.75 * ((g.day * 4.0).ceil() / 4.0);
            [
                palette::dos(palette::scale(g.haze[0], k)),
                palette::dos(palette::scale(g.haze[1], k)),
                palette::dos(palette::scale(g.ground, k)),
            ]
        });
        let mid_x = r.x as f64 + r.w as f64 * 0.5;
        for py in bx.1..bx.3 {
            let row_d = *rot * view.ray(mid_x, py as f64 + 0.5);
            for px in bx.0..bx.2 {
                let d = *rot * view.ray(px as f64 + 0.5, py as f64 + 0.5);
                let b = d.dot(body.center);
                let disc = b * b - c2;
                let t_sphere = if disc >= 0.0 && b - disc.sqrt() > 0.0 { Some(b - disc.sqrt()) } else { None };

                if let Some(rg) = &body.rings {
                    let denom = d.dot(rg.normal);
                    if denom.abs() > 1e-9 {
                        let t = body.center.dot(rg.normal) / denom;
                        let rr = (d * t - body.center).length();
                        if t > 0.0 && rr >= rg.inner && rr <= rg.outer && t_sphere.map_or(true, |ts| t < ts) {
                            let f = (rr - rg.inner) / (rg.outer - rg.inner);
                            // Two flat bands with a gap, as in the DOS captures.
                            if !(0.42..0.58).contains(&f) {
                                cv.put(px, py, palette::dos(rg.colors[if f < 0.5 { 0 } else { 1 }]));
                                continue;
                            }
                        }
                    }
                }
                let Some(t) = t_sphere else { continue };

                if let (Some(g), Some(cols)) = (&body.near, &near_cols) {
                    let dip = -row_d.dot(g.up).clamp(-1.0, 1.0).asin();
                    let c = if dip < HAZE_EDGES[0] {
                        cols[0]
                    } else if dip < HAZE_EDGES[1] {
                        cols[1]
                    } else {
                        cols[2]
                    };
                    cv.put(px, py, c);
                    continue;
                }

                let n = (d * t - body.center) / body.radius;
                let base = surface_color(body, body.spin * n);
                let col = if body.kind == BodyKind::Star {
                    let limb = 0.75 + 0.25 * (-n.dot(d)).max(0.0);
                    palette::scale(base, limb)
                } else if body.kind == BodyKind::Gas {
                    // Gas giants are one flat colour on the day side (Aster in PIC002).
                    let lam = n.dot(l_dir);
                    palette::scale(base, if lam > 0.0 { 1.0 } else if lam > -0.08 { 0.5 } else { 0.12 })
                } else {
                    // FE2 posterised terminator: a few hard bands, near-black night side.
                    let lam = n.dot(l_dir).max(0.0);
                    let q = (lam * 6.0).ceil() / 6.0;
                    palette::scale(base, 0.06 + 0.94 * q)
                };
                cv.put(px, py, palette::dos(col));
            }
        }
    }
}

/// A mesh placed in the world, relative to the camera.
pub struct Instance<'a> {
    pub mesh: &'a Mesh,
    pub pos: DVec3,
    pub orient: DMat3,
}

/// Is the segment camera→point blocked by any sphere?
fn occluded(point: DVec3, spheres: &[(DVec3, f64)]) -> bool {
    let len = point.length();
    if len < 1e-9 {
        return false;
    }
    let d = point / len;
    spheres.iter().any(|&(c, r)| {
        let b = d.dot(c);
        let disc = b * b - (c.length_squared() - r * r);
        if disc < 0.0 {
            return false;
        }
        let t = b - disc.sqrt();
        t > 0.0 && t < len * 0.999
    })
}

pub fn draw_meshes(
    cv: &mut Canvas,
    view: &View,
    rot: &DMat3,
    instances: &[Instance],
    light: DVec3,
    occluders: &[(DVec3, f64)],
) {
    let near = 1e-4;
    let mut polys: Vec<(f64, Vec<(f64, f64)>, Rgb)> = Vec::new();
    for inst in instances {
        if occluded(inst.pos, occluders) {
            continue;
        }
        let cam: Vec<DVec3> = inst
            .mesh
            .verts
            .iter()
            .map(|v| to_cam(rot, inst.pos + inst.orient * *v))
            .collect();
        let depth_of = |idx: &[usize]| idx.iter().map(|&i| cam[i].z).sum::<f64>() / idx.len() as f64;
        for face in &inst.mesh.faces {
            let center_w = inst.pos + inst.orient * face.center;
            let n_w = inst.orient * face.normal;
            if n_w.dot(-center_w) <= 0.0 {
                continue;
            }
            if face.idx.iter().any(|&i| cam[i].z <= near) {
                continue;
            }
            let pts: Vec<(f64, f64)> = face.idx.iter().filter_map(|&i| view.project(cam[i])).collect();
            if pts.len() < 3 {
                continue;
            }
            let (depth, col) = match face.parent {
                // Windows are self-lit and drawn straight after their wall.
                Some(p) => (depth_of(&inst.mesh.faces[p].idx) * (1.0 - 1e-9), face.color),
                None => {
                    let l = (light - center_w).normalize_or_zero();
                    let f = 0.3 + 0.7 * n_w.dot(l).max(0.0);
                    (depth_of(&face.idx), palette::scale(face.color, f))
                }
            };
            polys.push((depth, pts, palette::dos(col)));
        }
        for e in &inst.mesh.edges {
            let (mut a, mut b) = (cam[e.a], cam[e.b]);
            if a.z <= near && b.z <= near {
                continue;
            }
            if a.z <= near {
                a = a + (b - a) * ((near - a.z) / (b.z - a.z));
            } else if b.z <= near {
                b = b + (a - b) * ((near - b.z) / (a.z - b.z));
            }
            if let (Some(pa), Some(pb)) = (view.project(a), view.project(b)) {
                polys.push(((a.z + b.z) * 0.5, vec![pa, pb], palette::dos(e.color)));
            }
        }
    }
    polys.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    for (_, pts, col) in &polys {
        if pts.len() == 2 {
            cv.line(pts[0].0 as i32, pts[0].1 as i32, pts[1].0 as i32, pts[1].1 as i32, *col);
        } else {
            cv.fill_poly(pts, *col);
        }
    }
}
