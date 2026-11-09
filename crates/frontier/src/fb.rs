//! Software framebuffer: the Rust equivalent of a pygame Surface.

use crate::palette::Rgb;
use image::RgbaImage;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
    clip: Rect,
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        Self {
            w,
            h,
            px: vec![0; (w * h * 4) as usize],
            clip: Rect::new(0, 0, w, h),
        }
    }

    pub fn set_clip(&mut self, r: Rect) {
        let x0 = r.x.max(0);
        let y0 = r.y.max(0);
        let x1 = r.right().min(self.w);
        let y1 = r.bottom().min(self.h);
        self.clip = Rect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0));
    }

    pub fn reset_clip(&mut self) {
        self.clip = Rect::new(0, 0, self.w, self.h);
    }

    pub fn clear(&mut self, c: Rgb) {
        for p in self.px.chunks_exact_mut(4) {
            p[0] = c[0];
            p[1] = c[1];
            p[2] = c[2];
            p[3] = 255;
        }
    }

    #[inline]
    pub fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if !self.clip.contains(x, y) {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        self.px[i] = c[0];
        self.px[i + 1] = c[1];
        self.px[i + 2] = c[2];
        self.px[i + 3] = 255;
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Rgb {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return [0, 0, 0];
        }
        let i = ((y * self.w + x) * 4) as usize;
        [self.px[i], self.px[i + 1], self.px[i + 2]]
    }

    pub fn hline(&mut self, x0: i32, x1: i32, y: i32, c: Rgb) {
        if y < self.clip.y || y >= self.clip.bottom() {
            return;
        }
        let a = x0.min(x1).max(self.clip.x);
        let b = x0.max(x1).min(self.clip.right() - 1);
        for x in a..=b {
            self.put(x, y, c);
        }
    }

    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, c: Rgb) {
        for y in y0.min(y1)..=y0.max(y1) {
            self.put(x, y, c);
        }
    }

    pub fn fill_rect(&mut self, r: Rect, c: Rgb) {
        for y in r.y..r.bottom() {
            self.hline(r.x, r.right() - 1, y, c);
        }
    }

    pub fn rect(&mut self, r: Rect, c: Rgb) {
        self.hline(r.x, r.right() - 1, r.y, c);
        self.hline(r.x, r.right() - 1, r.bottom() - 1, c);
        self.vline(r.x, r.y, r.bottom() - 1, c);
        self.vline(r.right() - 1, r.y, r.bottom() - 1, c);
    }

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        // Guard against wildly off-screen projections.
        let lim = 20_000;
        if x0.abs() > lim || y0.abs() > lim || x1.abs() > lim || y1.abs() > lim {
            return;
        }
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.put(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Even-odd scanline polygon fill (flat shading, no anti-aliasing).
    pub fn fill_poly(&mut self, pts: &[(f64, f64)], c: Rgb) {
        if pts.len() < 3 {
            return;
        }
        let mut ymin = f64::MAX;
        let mut ymax = f64::MIN;
        for &(_, y) in pts {
            ymin = ymin.min(y);
            ymax = ymax.max(y);
        }
        let y0 = (ymin.ceil() as i32).max(self.clip.y);
        let y1 = (ymax.floor() as i32).min(self.clip.bottom() - 1);
        let mut xs: Vec<f64> = Vec::with_capacity(8);
        for y in y0..=y1 {
            let fy = y as f64 + 0.5;
            xs.clear();
            for i in 0..pts.len() {
                let (ax, ay) = pts[i];
                let (bx, by) = pts[(i + 1) % pts.len()];
                if (ay <= fy && by > fy) || (by <= fy && ay > fy) {
                    xs.push(ax + (fy - ay) / (by - ay) * (bx - ax));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in xs.chunks_exact(2) {
                let xa = pair[0].round() as i32;
                let xb = pair[1].round() as i32 - 1;
                if xb >= xa {
                    self.hline(xa, xb, y, c);
                }
            }
        }
    }

    pub fn blit(&mut self, img: &RgbaImage, x: i32, y: i32) {
        for (ix, iy, p) in img.enumerate_pixels() {
            if p[3] > 127 {
                self.put(x + ix as i32, y + iy as i32, [p[0], p[1], p[2]]);
            }
        }
    }

    /// Nearest-neighbour scaled blit, with an optional per-pixel recolour.
    pub fn blit_scaled(
        &mut self,
        img: &RgbaImage,
        dst: Rect,
        recolor: Option<&dyn Fn(Rgb) -> Rgb>,
    ) {
        if dst.w <= 0 || dst.h <= 0 || img.width() == 0 || img.height() == 0 {
            return;
        }
        for dy in 0..dst.h {
            let sy = (dy as u64 * img.height() as u64 / dst.h as u64) as u32;
            for dx in 0..dst.w {
                let sx = (dx as u64 * img.width() as u64 / dst.w as u64) as u32;
                let p = img.get_pixel(sx, sy);
                if p[3] <= 127 {
                    continue;
                }
                let mut c = [p[0], p[1], p[2]];
                if let Some(f) = recolor {
                    c = f(c);
                }
                self.put(dst.x + dx, dst.y + dy, c);
            }
        }
    }

    pub fn to_image(&self) -> RgbaImage {
        RgbaImage::from_raw(self.w as u32, self.h as u32, self.px.clone()).expect("canvas size")
    }
}
