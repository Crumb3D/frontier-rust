//! Low-poly flat-shaded meshes. Every face carries an explicit outward normal,
//! so back-face culling never depends on vertex winding.

use crate::palette::Rgb;
use glam::DVec3;

pub struct Face {
    pub idx: Vec<usize>,
    pub color: Rgb,
    pub normal: DVec3,
    pub center: DVec3,
    /// Decals (windows) sort with, and just in front of, this face.
    pub parent: Option<usize>,
}

/// A wireframe line, e.g. the geodesic domes at Sirocco.
pub struct Edge {
    pub a: usize,
    pub b: usize,
    pub color: Rgb,
}

#[derive(Default)]
pub struct Mesh {
    pub verts: Vec<DVec3>,
    pub faces: Vec<Face>,
    pub edges: Vec<Edge>,
}

impl Mesh {
    fn add_vert(&mut self, v: DVec3) -> usize {
        self.verts.push(v);
        self.verts.len() - 1
    }

    /// Add a polygon; the normal is flipped if needed so it points away from `inside`.
    pub fn add_face(&mut self, idx: Vec<usize>, color: Rgb, inside: DVec3) {
        let p: Vec<DVec3> = idx.iter().map(|&i| self.verts[i]).collect();
        let center = p.iter().copied().sum::<DVec3>() / p.len() as f64;
        let mut n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
        if n.dot(center - inside) < 0.0 {
            n = -n;
        }
        self.faces.push(Face { idx, color, normal: n, center, parent: None });
    }

    pub fn add_face_normal(&mut self, idx: Vec<usize>, color: Rgb, normal: DVec3) {
        let center = idx.iter().map(|&i| self.verts[i]).sum::<DVec3>() / idx.len() as f64;
        self.faces.push(Face { idx, color, normal, center, parent: None });
    }

    /// A small quad lying on face `parent`, centred at `c` with half-extents along `u` and `v`.
    pub fn add_decal(&mut self, parent: usize, c: DVec3, u: DVec3, v: DVec3, color: Rgb) {
        let normal = self.faces[parent].normal;
        let c = c + normal * 1e-4;
        let b = self.verts.len();
        self.verts.extend([c - u - v, c + u - v, c + u + v, c - u + v]);
        self.faces.push(Face { idx: vec![b, b + 1, b + 2, b + 3], color, normal, center: c, parent: Some(parent) });
    }

    pub fn add_edge(&mut self, a: DVec3, b: DVec3, color: Rgb) {
        let ia = self.add_vert(a);
        let ib = self.add_vert(b);
        self.edges.push(Edge { a: ia, b: ib, color });
    }

    pub fn add_box(&mut self, min: DVec3, max: DVec3, color: Rgb) {
        let c = [
            DVec3::new(min.x, min.y, min.z),
            DVec3::new(max.x, min.y, min.z),
            DVec3::new(max.x, max.y, min.z),
            DVec3::new(min.x, max.y, min.z),
            DVec3::new(min.x, min.y, max.z),
            DVec3::new(max.x, min.y, max.z),
            DVec3::new(max.x, max.y, max.z),
            DVec3::new(min.x, max.y, max.z),
        ];
        let b = self.verts.len();
        for v in c {
            self.add_vert(v);
        }
        let inside = (min + max) * 0.5;
        // Roofs a shade lighter than walls, like the FE2 cities.
        let roof = crate::palette::scale(color, 1.15);
        for (q, col) in [
            ([0, 1, 2, 3], color),
            ([4, 5, 6, 7], color),
            ([0, 1, 5, 4], color),
            ([3, 2, 6, 7], roof),
            ([0, 3, 7, 4], color),
            ([1, 2, 6, 5], color),
        ] {
            self.add_face(q.iter().map(|i| b + i).collect(), col, inside);
        }
    }

    /// Flat quad in the XZ plane facing +Y (landing pads, ground markings).
    pub fn add_ground_quad(&mut self, cx: f64, cz: f64, hx: f64, hz: f64, y: f64, color: Rgb) {
        let b = self.verts.len();
        self.add_vert(DVec3::new(cx - hx, y, cz - hz));
        self.add_vert(DVec3::new(cx + hx, y, cz - hz));
        self.add_vert(DVec3::new(cx + hx, y, cz + hz));
        self.add_vert(DVec3::new(cx - hx, y, cz + hz));
        self.add_face_normal(vec![b, b + 1, b + 2, b + 3], color, DVec3::Y);
    }
}

/// Coriolis-style orbital station: a cuboctahedron with the docking slot on +Z.
pub fn station(radius_km: f64) -> Mesh {
    let mut m = Mesh::default();
    let s = radius_km;
    let vi = |m: &mut Mesh, x: f64, y: f64, z: f64| m.add_vert(DVec3::new(x, y, z) * s);
    // 12 vertices: permutations of (±1, ±1, 0)
    let mut v = std::collections::HashMap::new();
    for &a in &[-1.0, 1.0] {
        for &b in &[-1.0, 1.0] {
            v.insert(("xy", a as i32, b as i32), vi(&mut m, a, b, 0.0));
            v.insert(("xz", a as i32, b as i32), vi(&mut m, a, 0.0, b));
            v.insert(("yz", a as i32, b as i32), vi(&mut m, 0.0, a, b));
        }
    }
    let shades: [Rgb; 3] = [[204, 204, 204], [170, 170, 170], [136, 136, 136]];
    // Square faces on each axis.
    for &sgn in &[-1, 1] {
        m.add_face(
            vec![v[&("xy", sgn, 1)], v[&("xz", sgn, 1)], v[&("xy", sgn, -1)], v[&("xz", sgn, -1)]],
            shades[0],
            DVec3::ZERO,
        );
        m.add_face(
            vec![v[&("xy", 1, sgn)], v[&("yz", sgn, 1)], v[&("xy", -1, sgn)], v[&("yz", sgn, -1)]],
            shades[1],
            DVec3::ZERO,
        );
        m.add_face(
            vec![v[&("xz", 1, sgn)], v[&("yz", 1, sgn)], v[&("xz", -1, sgn)], v[&("yz", -1, sgn)]],
            shades[0],
            DVec3::ZERO,
        );
    }
    // Triangles in each octant.
    for &sx in &[-1, 1] {
        for &sy in &[-1, 1] {
            for &sz in &[-1, 1] {
                m.add_face(
                    vec![v[&("xy", sx, sy)], v[&("xz", sx, sz)], v[&("yz", sy, sz)]],
                    shades[2],
                    DVec3::ZERO,
                );
            }
        }
    }
    // Docking slot, just proud of the +Z square face.
    let z = 1.001 * s;
    let (hx, hy) = (0.32 * s, 0.11 * s);
    let b = m.verts.len();
    m.verts.extend([
        DVec3::new(-hx, -hy, z),
        DVec3::new(hx, -hy, z),
        DVec3::new(hx, hy, z),
        DVec3::new(-hx, hy, z),
    ]);
    m.add_face_normal(vec![b, b + 1, b + 2, b + 3], [17, 17, 34], DVec3::Z);
    m
}

/// Wireframe geodesic dome: latitude rings joined in a zig-zag, squashed by `hk`.
fn add_dome(m: &mut Mesh, c: DVec3, r: f64, hk: f64, color: Rgb) {
    use std::f64::consts::{FRAC_PI_2, TAU};
    let (rings, segs) = (4usize, 14usize);
    let pt = |i: usize, j: isize| {
        let lat = i as f64 / rings as f64 * FRAC_PI_2;
        let off = if i % 2 == 1 { 0.5 } else { 0.0 };
        let lon = (j as f64 + off) / segs as f64 * TAU;
        c + DVec3::new(r * lat.cos() * lon.cos(), r * lat.sin() * hk, r * lat.cos() * lon.sin())
    };
    let apex = c + DVec3::Y * r * hk;
    for i in 0..rings {
        for j in 0..segs as isize {
            m.add_edge(pt(i, j), pt(i, j + 1), color);
            if i + 1 < rings {
                let k = if i % 2 == 0 { j - 1 } else { j + 1 };
                m.add_edge(pt(i, j), pt(i + 1, j), color);
                m.add_edge(pt(i, j), pt(i + 1, k), color);
            } else {
                m.add_edge(pt(i, j), apex, color);
            }
        }
    }
}

/// A city block with lit windows on its four walls.
fn add_block(m: &mut Mesh, min: DVec3, max: DVec3, wall: Rgb, lit: bool) {
    let f0 = m.faces.len();
    m.add_box(min, max, wall);
    if !lit {
        return;
    }
    let size = max - min;
    let (wx, wy) = (0.006, 0.007);
    let rows = ((size.y - 0.01) / 0.025).floor().max(1.0) as i32;
    let walls = [
        (f0, DVec3::new(0.0, 0.0, min.z), DVec3::X, size.x),
        (f0 + 1, DVec3::new(0.0, 0.0, max.z), DVec3::X, size.x),
        (f0 + 4, DVec3::new(min.x, 0.0, 0.0), DVec3::Z, size.z),
        (f0 + 5, DVec3::new(max.x, 0.0, 0.0), DVec3::Z, size.z),
    ];
    for (face, plane, along, len) in walls {
        let cols = ((len - 0.01) / 0.022).floor().max(1.0) as i32;
        for row in 0..rows {
            for col in 0..cols {
                // Leave some windows dark, deterministically.
                if (row * 7 + col * 3 + face as i32) % 4 == 0 {
                    continue;
                }
                let t = (col as f64 + 0.5) / cols as f64;
                let y = min.y + 0.012 + row as f64 * 0.025;
                let mut c = min + along * (t * len) + DVec3::Y * (y - min.y);
                if along == DVec3::X {
                    c.z = plane.z;
                } else {
                    c.x = plane.x;
                }
                m.add_decal(face, c, along * wx, DVec3::Y * wy, crate::palette::WINDOW_YELLOW);
            }
        }
    }
}

/// Surface starport on a local ground plane (+Y up, +Z ahead of the pad).
/// Laid out after Sirocco in the DOS opening still: big pink wireframe domes
/// around the pad, dark lit-window blocks on the skyline.
pub fn starport(seed: u32) -> Mesh {
    let mut m = Mesh::default();
    let pad: Rgb = [76, 76, 76];
    for &(px, pz) in &[(0.0, 0.0), (0.22, -0.05), (-0.22, -0.05), (0.0, -0.25)] {
        m.add_ground_quad(px, pz, 0.05, 0.05, 0.0005, pad);
    }
    let pink = crate::palette::DOME_PINK;
    add_dome(&mut m, DVec3::new(-0.62, 0.0, 1.25), 0.62, 0.62, pink);
    add_dome(&mut m, DVec3::new(1.05, 0.0, 1.35), 0.55, 0.62, pink);

    let mut r = seed.max(1);
    let mut rnd = move || {
        r ^= r << 13;
        r ^= r >> 17;
        r ^= r << 5;
        (r % 1000) as f64 / 1000.0
    };
    let walls: [Rgb; 4] = [[108, 108, 108], [140, 140, 140], [92, 92, 92], [76, 76, 76]];
    // Skyline to the left: tall blocks with windows.
    for i in 0..6 {
        let ang = -0.62 + i as f64 * 0.05 + rnd() * 0.03;
        let d = 3.0 + rnd() * 0.6;
        let (x, z) = (ang.sin() * d, ang.cos() * d);
        let w = 0.04 + rnd() * 0.04;
        let h = 0.08 + rnd() * 0.14;
        add_block(&mut m, DVec3::new(x - w, 0.0, z - w), DVec3::new(x + w, h, z + w), walls[i % 4], true);
    }
    // Low dark sheds to the right.
    for i in 0..5 {
        let ang = 0.25 + i as f64 * 0.06 + rnd() * 0.03;
        let d = 2.6 + rnd() * 0.5;
        let (x, z) = (ang.sin() * d, ang.cos() * d);
        let w = 0.05 + rnd() * 0.05;
        let col: Rgb = if i % 2 == 0 { [60, 60, 60] } else { [12, 12, 12] };
        add_block(&mut m, DVec3::new(x - w, 0.0, z - w), DVec3::new(x + w, 0.015 + rnd() * 0.02, z + w), col, false);
    }
    m
}

/// A thin box from `a` to `b`. `inside` is any point the sides should face away from.
fn add_beam(m: &mut Mesh, a: DVec3, b: DVec3, half: f64, color: Rgb) {
    let dir = b - a;
    let len = dir.length();
    if len < 1e-6 {
        return;
    }
    let dir = dir / len;
    let mut side = dir.cross(DVec3::Y);
    if side.length_squared() < 1e-8 {
        side = dir.cross(DVec3::X);
    }
    side = side.normalize() * half;
    let up = dir.cross(side).normalize() * half;
    let corners = [
        a + side + up,
        a - side + up,
        a - side - up,
        a + side - up,
        b + side + up,
        b - side + up,
        b - side - up,
        b + side - up,
    ];
    let base = m.verts.len();
    for c in corners {
        m.add_vert(c);
    }
    let mid = (a + b) * 0.5;
    for idx in [[0, 1, 2, 3], [4, 7, 6, 5], [0, 4, 5, 1], [3, 2, 6, 7], [0, 3, 7, 4], [1, 5, 6, 2]] {
        m.add_face(idx.into_iter().map(|i| base + i).collect(), color, mid);
    }
}

/// A squashed octahedron, the sensor pods on the Eagle's mast.
fn add_pod(m: &mut Mesh, c: DVec3, rx: f64, ry: f64, rz: f64, color: Rgb) {
    let base = m.verts.len();
    for p in [
        c + DVec3::X * rx,
        c - DVec3::X * rx,
        c + DVec3::Y * ry,
        c - DVec3::Y * ry,
        c + DVec3::Z * rz,
        c - DVec3::Z * rz,
    ] {
        m.add_vert(p);
    }
    for t in [[0, 2, 4], [0, 4, 3], [0, 3, 5], [0, 5, 2], [1, 4, 2], [1, 3, 4], [1, 5, 3], [1, 2, 5]] {
        m.add_face(t.into_iter().map(|i| base + i).collect(), color, c);
    }
}

/// The Eagle long range fighter, traced from the original's F3 ship box.
///
/// Nose is +Z, up is +Y. The three F3 pages share their palette with the hull,
/// so the same mesh is teal, green or grey depending on the page; the wingtips,
/// bays, engine and mast stay their own colours.
pub fn eagle_in([hull, side, belly]: [Rgb; 3]) -> Mesh {
    let mut m = Mesh::default();
    let origin = DVec3::ZERO;
    let tip: Rgb = [174, 174, 174];
    let tip_dark: Rgb = [109, 109, 109];
    let white: Rgb = [239, 239, 239];
    let black: Rgb = [12, 12, 12];
    let cyan: Rgb = [12, 239, 239];
    let yellow: Rgb = [239, 239, 12];
    let rust: Rgb = [174, 109, 109];
    let pod: Rgb = [158, 158, 158];

    let at = |m: &mut Mesh, x: f64, y: f64, z: f64| m.add_vert(DVec3::new(x, y, z));

    // Fuselage: a long wedge, thick only over the rear half. Each ring is
    // top, right, bottom, left.
    let nose = at(&mut m, 0.0, 0.01, 1.32);
    let mut rings: Vec<[usize; 4]> = Vec::new();
    for (z, hx, yt, yb) in [
        (0.92, 0.06, 0.045, -0.025),
        (0.48, 0.11, 0.08, -0.045),
        (0.08, 0.17, 0.12, -0.07),
        (-0.38, 0.15, 0.11, -0.075),
        (-0.72, 0.09, 0.07, -0.05),
    ] {
        rings.push([
            at(&mut m, 0.0, yt, z),
            at(&mut m, hx, (yt + yb) * 0.2, z),
            at(&mut m, 0.0, yb, z),
            at(&mut m, -hx, (yt + yb) * 0.2, z),
        ]);
    }
    let tail = at(&mut m, 0.0, 0.0, -0.9);

    let mut cockpit_face = 0;
    let r0 = rings[0];
    m.add_face(vec![nose, r0[1], r0[0]], hull, origin);
    m.add_face(vec![nose, r0[0], r0[3]], hull, origin);
    m.add_face(vec![nose, r0[2], r0[1]], belly, origin);
    m.add_face(vec![nose, r0[3], r0[2]], belly, origin);
    for i in 0..rings.len() - 1 {
        let (a, b) = (rings[i], rings[i + 1]);
        if i == 0 {
            cockpit_face = m.faces.len();
        }
        m.add_face(vec![a[0], b[0], b[1], a[1]], hull, origin);
        m.add_face(vec![a[0], a[3], b[3], b[0]], hull, origin);
        m.add_face(vec![a[1], b[1], b[2], a[2]], side, origin);
        m.add_face(vec![a[3], a[2], b[2], b[3]], side, origin);
        m.add_face(vec![a[2], b[2], b[3], a[3]], belly, origin);
    }
    let rear = rings[rings.len() - 1];
    let rear_face = m.faces.len();
    m.add_face(vec![rear[0], tail, rear[1]], hull, origin);
    m.add_face(vec![rear[0], rear[3], tail], hull, origin);
    m.add_face(vec![rear[1], tail, rear[2]], belly, origin);
    m.add_face(vec![rear[3], rear[2], tail], belly, origin);
    // The engine glow, stuck out the back so it reads from the side as well.
    let ex = m.faces.len();
    let el = at(&mut m, -0.045, -0.01, -0.86);
    let er = at(&mut m, 0.045, -0.01, -0.86);
    let et = at(&mut m, 0.0, 0.045, -0.86);
    let eb = at(&mut m, 0.0, -0.06, -0.86);
    let tip_v = at(&mut m, 0.0, -0.005, -1.12);
    for idx in [vec![el, er, tip_v], vec![er, eb, tip_v], vec![eb, el, tip_v], vec![et, er, el], vec![et, tip_v, er], vec![et, el, tip_v]] {
        m.add_face(idx, cyan, DVec3::new(0.0, 0.0, -0.9));
    }
    for f in &mut m.faces[ex..] {
        f.parent = Some(rear_face);
    }

    // Canopy: a dark oval with one yellow pixel of glint, just aft of the nose.
    m.add_decal(cockpit_face, DVec3::new(0.0, 0.06, 0.78), DVec3::X * 0.035, DVec3::Z * 0.07, black);
    m.add_decal(cockpit_face, DVec3::new(0.012, 0.065, 0.8), DVec3::X * 0.012, DVec3::Z * 0.012, yellow);

    // Wings. The plan is a swept crescent: tips further aft than the root
    // trailing edge, grey for the outer fifth, as on the original's top view.
    let (ty, by) = (0.055, -0.028);
    for sign in [1.0, -1.0] {
        let mut p = |x: f64, y: f64, z: f64| at(&mut m, sign * x, y, z);
        let le_root = p(0.12, ty, 0.36);
        let le_mid = p(0.62, ty, -0.02);
        let le_out = p(1.05, ty, -0.32);
        let tip_pt = p(1.42, ty, -0.66);
        let te_out = p(1.12, ty, -0.8);
        let te_mid = p(0.55, ty, -0.4);
        let te_root = p(0.13, ty, -0.62);
        let le_root_b = p(0.12, by, 0.36);
        let le_mid_b = p(0.62, by, -0.02);
        let le_out_b = p(1.05, by, -0.32);
        let tip_b = p(1.42, by, -0.66);
        let te_out_b = p(1.12, by, -0.8);
        let te_mid_b = p(0.55, by, -0.4);
        let te_root_b = p(0.13, by, -0.62);

        let top = if sign > 0.0 {
            vec![le_root, le_mid, te_mid, te_root]
        } else {
            vec![le_root, te_root, te_mid, le_mid]
        };
        let wing_face = m.faces.len();
        m.add_face(top, hull, origin);
        m.add_face(
            if sign > 0.0 { vec![le_mid, le_out, te_mid] } else { vec![le_mid, te_mid, le_out] },
            hull,
            origin,
        );
        m.add_face(
            if sign > 0.0 { vec![le_out, tip_pt, te_out, te_mid] } else { vec![le_out, te_mid, te_out, tip_pt] },
            tip,
            origin,
        );
        m.add_face(
            if sign > 0.0 { vec![le_root_b, te_root_b, te_mid_b, le_mid_b] } else { vec![le_root_b, le_mid_b, te_mid_b, te_root_b] },
            belly,
            origin,
        );
        m.add_face(
            if sign > 0.0 { vec![le_mid_b, te_mid_b, le_out_b] } else { vec![le_mid_b, le_out_b, te_mid_b] },
            belly,
            origin,
        );
        m.add_face(
            if sign > 0.0 { vec![le_out_b, te_mid_b, te_out_b, tip_b] } else { vec![le_out_b, tip_b, te_out_b, te_mid_b] },
            tip_dark,
            origin,
        );
        // Rim, so the wing has an edge when seen from the side.
        let rim = [
            (le_root, le_mid, le_mid_b, le_root_b),
            (le_mid, le_out, le_out_b, le_mid_b),
            (le_out, tip_pt, tip_b, le_out_b),
            (tip_pt, te_out, te_out_b, tip_b),
            (te_out, te_mid, te_mid_b, te_out_b),
            (te_mid, te_root, te_root_b, te_mid_b),
        ];
        for (a, b, c, d) in rim {
            m.add_face(vec![a, b, c, d], side, origin);
        }

        // The dark bay with the white cross, and the rust strip along its front.
        let bay = DVec3::new(sign * 0.5, ty, -0.2);
        let across = DVec3::X * sign * 0.2;
        let along = DVec3::Z * 0.11;
        m.add_decal(wing_face, bay + DVec3::Z * 0.13, across, DVec3::Z * 0.018, rust);
        m.add_decal(wing_face, bay, across, along, black);
        let x1 = (across + along).normalize();
        let x2 = (across - along).normalize();
        m.add_decal(wing_face, bay, x1 * 0.16, x2 * 0.018, white);
        m.add_decal(wing_face, bay, x2 * 0.16, x1 * 0.018, white);
    }

    // Mast over the rear fuselage: two crossed white struts, a bar, four pods.
    let foot_y = 0.12;
    add_beam(&mut m, DVec3::new(0.0, foot_y, -0.08), DVec3::new(0.0, 0.46, -0.42), 0.012, white);
    add_beam(&mut m, DVec3::new(0.0, foot_y, -0.46), DVec3::new(0.0, 0.46, -0.1), 0.012, white);
    add_beam(&mut m, DVec3::new(0.0, 0.5, -0.02), DVec3::new(0.0, 0.5, -0.56), 0.035, pod);
    for z in [-0.08, -0.22, -0.36, -0.5] {
        add_pod(&mut m, DVec3::new(0.0, 0.57, z), 0.055, 0.045, 0.055, pod);
    }
    m
}
