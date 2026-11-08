//! Draws a posed skeleton ([`DrawList`]) into an image: textured triangles with bilinear
//! sampling, slot tint and blend modes, and clipping polygons.

use image::{Rgba, RgbaImage};

use super::spine::{Blend, DrawItem, DrawList};

/// A texture page as premultiplied RGBA floats.
#[derive(Debug)]
pub struct Texture {
    width: usize,
    height: usize,
    texels: Vec<[f32; 4]>,
}

impl Texture {
    /// `pma`: the image's colours are already multiplied by alpha.
    pub fn new(image: &RgbaImage, pma: bool) -> Self {
        let texels = image
            .pixels()
            .map(|p| {
                let [r, g, b, a] = p.0.map(|c| f32::from(c) / 255.0);
                if pma {
                    [r, g, b, a]
                } else {
                    [r * a, g * a, b * a, a]
                }
            })
            .collect();
        Self {
            width: image.width() as usize,
            height: image.height() as usize,
            texels,
        }
    }

    fn texel(&self, x: isize, y: isize) -> [f32; 4] {
        let x = x.clamp(0, self.width as isize - 1) as usize;
        let y = y.clamp(0, self.height as isize - 1) as usize;
        self.texels[y * self.width + x]
    }

    /// Bilinear sample at texel coordinates (texel centres at +0.5).
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let (x, y) = (u - 0.5, v - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (x0, y0) = (x0 as isize, y0 as isize);
        let t = [
            self.texel(x0, y0),
            self.texel(x0 + 1, y0),
            self.texel(x0, y0 + 1),
            self.texel(x0 + 1, y0 + 1),
        ];
        std::array::from_fn(|i| {
            let top = t[0][i] + (t[1][i] - t[0][i]) * fx;
            let bottom = t[2][i] + (t[3][i] - t[2][i]) * fx;
            top + (bottom - top) * fy
        })
    }
}

/// The world-space bounds (min x, min y, max x, max y) of everything drawn.
pub fn bounds(list: &DrawList) -> Option<(f32, f32, f32, f32)> {
    let mut b: Option<(f32, f32, f32, f32)> = None;
    for item in &list.items {
        let DrawItem::Triangles(t) = item else {
            continue;
        };
        for &(x, y) in &t.points {
            b = Some(b.map_or((x, y, x, y), |(x0, y0, x1, y1)| {
                (x0.min(x), y0.min(y), x1.max(x), y1.max(y))
            }));
        }
    }
    b.filter(|(x0, y0, x1, y1)| x1 > x0 && y1 > y0)
}

/// Draws `list` with `scale` pixels per world unit; world y points up, image y down.
pub fn render(
    list: &DrawList,
    pages: &[Option<Texture>],
    world: (f32, f32, f32, f32),
    scale: f32,
) -> RgbaImage {
    let (min_x, min_y, max_x, max_y) = world;
    let width = (((max_x - min_x) * scale).ceil() as usize).max(1);
    let height = (((max_y - min_y) * scale).ceil() as usize).max(1);
    let mut canvas = vec![[0.0f32; 4]; width * height];
    let to_px = |(x, y): (f32, f32)| ((x - min_x) * scale, (max_y - y) * scale);
    let mut clip: Option<Vec<(f32, f32)>> = None;
    for item in &list.items {
        let tris = match item {
            DrawItem::ClipStart(poly) => {
                clip = Some(poly.iter().map(|&p| to_px(p)).collect());
                continue;
            }
            DrawItem::ClipEnd => {
                clip = None;
                continue;
            }
            DrawItem::Triangles(t) => t,
        };
        let Some(Some(texture)) = pages.get(tris.page) else {
            continue;
        };
        let points: Vec<(f32, f32)> = tris.points.iter().map(|&p| to_px(p)).collect();
        for tri in tris.indices.as_chunks::<3>().0 {
            let (Some(&a), Some(&b), Some(&c)) =
                (points.get(tri[0]), points.get(tri[1]), points.get(tri[2]))
            else {
                continue;
            };
            let (Some(&ta), Some(&tb), Some(&tc)) = (
                tris.uvs.get(tri[0]),
                tris.uvs.get(tri[1]),
                tris.uvs.get(tri[2]),
            ) else {
                continue;
            };
            let area = edge(a, b, c);
            if area.abs() < 1e-6 {
                continue;
            }
            let x0 = a.0.min(b.0).min(c.0).floor().max(0.0) as usize;
            let y0 = a.1.min(b.1).min(c.1).floor().max(0.0) as usize;
            let x1 = (a.0.max(b.0).max(c.0).ceil() as usize).min(width);
            let y1 = (a.1.max(b.1).max(c.1).ceil() as usize).min(height);
            for py in y0..y1 {
                for px in x0..x1 {
                    let p = (px as f32 + 0.5, py as f32 + 0.5);
                    let wa = edge(b, c, p) / area;
                    let wb = edge(c, a, p) / area;
                    let wc = 1.0 - wa - wb;
                    if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                        continue;
                    }
                    if clip.as_ref().is_some_and(|poly| !inside(poly, p)) {
                        continue;
                    }
                    let u = ta.0 * wa + tb.0 * wb + tc.0 * wc;
                    let v = ta.1 * wa + tb.1 * wb + tc.1 * wc;
                    let s = texture.sample(u, v);
                    let [tr, tg, tb_, ta_] = tris.color;
                    let src = [
                        s[0] * tr * ta_,
                        s[1] * tg * ta_,
                        s[2] * tb_ * ta_,
                        s[3] * ta_,
                    ];
                    blend(&mut canvas[py * width + px], src, tris.blend);
                }
            }
        }
    }
    let mut out = RgbaImage::new(width as u32, height as u32);
    for (pixel, c) in out.pixels_mut().zip(&canvas) {
        let a = c[3].clamp(0.0, 1.0);
        let un = |v: f32| {
            if a > 0.0 {
                ((v / a).clamp(0.0, 1.0) * 255.0).round() as u8
            } else {
                0
            }
        };
        *pixel = Rgba([un(c[0]), un(c[1]), un(c[2]), (a * 255.0).round() as u8]);
    }
    out
}

fn edge(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> f32 {
    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
}

/// Even-odd point-in-polygon.
fn inside(poly: &[(f32, f32)], p: (f32, f32)) -> bool {
    let mut inside = false;
    let mut j = poly.len().wrapping_sub(1);
    for (i, &a) in poly.iter().enumerate() {
        let b = poly[j];
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Blends a premultiplied source over the premultiplied canvas pixel.
fn blend(dst: &mut [f32; 4], src: [f32; 4], mode: Blend) {
    match mode {
        // Glows add light but no coverage: on a transparent background they only brighten
        // what is already drawn.
        Blend::Additive | Blend::Screen => {
            for i in 0..3 {
                dst[i] = (dst[i] + src[i] * dst[3]).min(dst[3]);
            }
        }
        Blend::Multiply => {
            for i in 0..3 {
                dst[i] = src[i] * dst[i] + dst[i] * (1.0 - src[3]);
            }
        }
        Blend::Normal => {
            for i in 0..4 {
                dst[i] = src[i] + dst[i] * (1.0 - src[3]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_in_polygon() {
        let square = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)];
        assert!(inside(&square, (1.0, 1.0)));
        assert!(!inside(&square, (3.0, 1.0)));
    }
}
