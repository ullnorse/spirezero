//! The game's own interface pieces (kind `ui`): map room icons and backgrounds, the top bar's
//! icons, card frames and cost gems, the combat widgets, reward and rest-site icons, buttons; and
//! its fonts (`font/<name>.ttf`, Kreon, Spectral and Bitter, which are open-licence).
//!
//! A piece is either a loose texture (`images/.../x.png`) or a region of an atlas, kept as an
//! `AtlasTexture` resource (`images/atlases/<atlas>.sprites/.../x.tres`: the atlas texture and a
//! `region = Rect2(x, y, w, h)`, and a `margin` when it was trimmed). Each is named by its file
//! stem (`map_monster`, `top_bar_heart`, `card_frame_attack_s`); a loose texture wins over an atlas sprite of its
//! stem (the atlas copy of a boss's map icon is a different, older drawing).

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use image::{RgbaImage, imageops};

use super::godot::Resource;
use super::pck::Pck;
use super::{Picture, fit, has_extension, stem, texture};

/// Atlas sprites: the directory, and whether its subdirectories count.
const SPRITES: &[(&str, bool)] = &[
    ("images/atlases/ui_atlas.sprites/map/", true),
    ("images/atlases/ui_atlas.sprites/top_bar/", true),
    ("images/atlases/ui_atlas.sprites/card/", true),
    ("images/atlases/compressed.sprites/map/", true),
    // The buttons (proceed, back, confirm, ...) at the atlas's top level.
    ("images/atlases/ui_atlas.sprites/", false),
];

/// Loose textures, each directory with its subdirectories.
const IMAGES: &[&str] = &[
    "images/packed/map/icons/",
    "images/packed/map/ancients/",
    "images/packed/map/map_bgs/",
    "images/map/placeholder/",
    "images/packed/combat_ui/",
    "images/ui/combat/",
    "images/ui/reward_screen/",
    "images/ui/rest_site/",
    "images/packed/sprite_fonts/",
];

/// The fonts, by their file stem under `fonts/`.
const FONTS: &[&str] = &[
    "kreon_regular",
    "kreon_bold",
    "spectral_bold",
    "bitter_medium_italic",
];

/// The longest side of a piece: a map background is a page, a card frame is shown large on a
/// high-density phone, the rest are icons.
fn max_px(path: &str) -> u32 {
    if path.contains("/map_bgs/") {
        1024
    } else if [
        "card_frame",
        "card_banner",
        "portrait_border",
        "ancient_banner",
    ]
    .iter()
    .any(|k| path.contains(k))
    {
        512
    } else {
        256
    }
}

/// Every interface piece of `pck`.
pub(super) fn pieces(pck: &mut Pck, pictures: &mut Vec<Picture>, notes: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    let mut atlases: HashMap<String, Option<RgbaImage>> = HashMap::new();
    let mut add = |path: &str, image: RgbaImage, pictures: &mut Vec<Picture>| {
        let id = stem(path).to_ascii_lowercase();
        if seen.insert(id.clone()) {
            pictures.push(Picture {
                kind: "ui",
                id,
                image: fit(image, max_px(path)),
            });
        }
    };
    for dir in IMAGES {
        let mut paths: Vec<String> = pck
            .paths()
            .filter_map(|p| p.strip_suffix(".import"))
            .filter(|p| p.starts_with(dir) && has_extension(p, "png"))
            .map(str::to_owned)
            .collect();
        paths.sort();
        for path in paths {
            match texture(pck, &path) {
                Some(image) => add(&path, image, pictures),
                None => notes.push(format!("ui: {path}: undecodable")),
            }
        }
    }
    for &(dir, deep) in SPRITES {
        let mut paths: Vec<String> = pck
            .paths()
            .filter(|p| {
                p.strip_prefix(dir).is_some_and(|rest| {
                    (deep || !rest.contains('/')) && has_extension(rest, "tres")
                })
            })
            .map(str::to_owned)
            .collect();
        paths.sort();
        for path in paths {
            match sprite(pck, &path, &mut atlases) {
                Ok(image) => add(&path, image, pictures),
                Err(e) => notes.push(format!("ui: {path}: {e}")),
            }
        }
    }
}

/// The frame colours (`materials/cards/frames/card_frame_<colour>_mat.tres`), the rarities
/// (`materials/cards/banners/card_banner_<rarity>_mat.tres`) and the card types with a frame or
/// a portrait border of their own (`CardModel.FramePath`, `PortraitBorderPath`).
const FRAME_COLOURS: &[&str] = &[
    "red",
    "green",
    "blue",
    "orange",
    "pink",
    "colorless",
    "curse",
    "quest",
];
const RARITIES: &[&str] = &[
    "common", "uncommon", "rare", "curse", "status", "event", "quest",
];
const FRAME_TYPES: &[&str] = &["attack", "skill", "power", "quest"];
const BORDER_TYPES: &[&str] = &["attack", "skill", "power"];

/// The card pieces the game tints with its HSV shader (`shaders/hsv.gdshader`), baked once per
/// material: `card_frame_<type>_<colour>` by the pool's frame material, and by the rarity's
/// banner material `card_banner_<rarity>`, `card_portrait_border_<type>_<rarity>` and
/// `card_plaque_<rarity>` (`NCard.UpdatePortrait`, `UpdateTypePlaque`).
pub(super) fn tinted(pck: &mut Pck, pictures: &mut Vec<Picture>, notes: &mut Vec<String>) {
    let base = |id: &str| {
        pictures
            .iter()
            .find(|p| p.kind == "ui" && p.id == id)
            .map(|p| p.image.clone())
    };
    let mut material = |path: String| {
        let res = Resource::parse(&pck.text(&path)?);
        let main = res.main()?;
        let param = |k: &str| {
            main.prop(&format!("shader_parameter/{k}"))?
                .parse::<f64>()
                .ok()
        };
        Some((param("h")?, param("s")?, param("v")?))
    };
    let mut out = Vec::new();
    for colour in FRAME_COLOURS {
        let Some(hsv) = material(format!(
            "materials/cards/frames/card_frame_{colour}_mat.tres"
        )) else {
            notes.push(format!("ui: no frame material {colour}"));
            continue;
        };
        for ty in FRAME_TYPES {
            if let Some(image) = base(&format!("card_frame_{ty}_s")) {
                out.push((format!("card_frame_{ty}_{colour}"), hsv_shift(&image, hsv)));
            }
        }
    }
    for rarity in RARITIES {
        let Some(hsv) = material(format!(
            "materials/cards/banners/card_banner_{rarity}_mat.tres"
        )) else {
            notes.push(format!("ui: no banner material {rarity}"));
            continue;
        };
        if let Some(image) = base("card_banner") {
            out.push((format!("card_banner_{rarity}"), hsv_shift(&image, hsv)));
        }
        if let Some(image) = base("card_portrait_border_plaque_s") {
            out.push((format!("card_plaque_{rarity}"), hsv_shift(&image, hsv)));
        }
        for ty in BORDER_TYPES {
            if let Some(image) = base(&format!("card_portrait_border_{ty}_s")) {
                out.push((
                    format!("card_portrait_border_{ty}_{rarity}"),
                    hsv_shift(&image, hsv),
                ));
            }
        }
    }
    pictures.extend(out.into_iter().map(|(id, image)| Picture {
        kind: "ui",
        id,
        image,
    }));
}

/// `hsv.gdshader` on `image`: to YIQ, the chroma turned by `(1 - h)` of a full turn and scaled
/// by `s`, all scaled by `v`, back to RGB (on the stored values, as a 2D canvas shader sees them).
fn hsv_shift(image: &RgbaImage, (h, s, v): (f64, f64, f64)) -> RgbaImage {
    let to_yiq = [
        [0.2989, 0.5870, 0.1140],
        [0.5959, -0.2774, -0.3216],
        [0.2115, -0.5229, 0.3114],
    ];
    let from_yiq = inverse(to_yiq);
    let turn = (1.0 - h) * std::f64::consts::TAU;
    let (sin, cos) = turn.sin_cos();
    let mut out = image.clone();
    for px in out.pixels_mut() {
        let rgb = [0, 1, 2].map(|i| f64::from(px.0[i]) / 255.0);
        let [y, i, q] = to_yiq.map(|row| row[0] * rgb[0] + row[1] * rgb[1] + row[2] * rgb[2]);
        // `col.rgb *= hue_shift`: the row vector times the matrix whose columns are
        // `(1, 0, 0)`, `(0, cos, -sin)`, `(0, sin, cos)`.
        let (i, q) = (cos * i - sin * q, sin * i + cos * q);
        let yiq = [y * v, i * s * v, q * s * v];
        let back = from_yiq.map(|row| row[0] * yiq[0] + row[1] * yiq[1] + row[2] * yiq[2]);
        for (c, value) in px.0.iter_mut().zip(back) {
            *c = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    out
}

/// The inverse of a 3×3 matrix (the YIQ one is well conditioned).
fn inverse(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let cof = |r: usize, c: usize| {
        let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
        let (c1, c2) = ((c + 1) % 3, (c + 2) % 3);
        m[r1][c1] * m[r2][c2] - m[r1][c2] * m[r2][c1]
    };
    let det = m[0][0] * cof(0, 0) + m[0][1] * cof(0, 1) + m[0][2] * cof(0, 2);
    let mut inv = [[0.0; 3]; 3];
    for (r, row) in inv.iter_mut().enumerate() {
        for (c, value) in row.iter_mut().enumerate() {
            *value = cof(c, r) / det;
        }
    }
    inv
}

/// The region of its atlas an `AtlasTexture` resource names; atlases are decoded once.
fn sprite(
    pck: &mut Pck,
    path: &str,
    atlases: &mut HashMap<String, Option<RgbaImage>>,
) -> Result<RgbaImage, String> {
    let res = Resource::parse(&pck.text(path).ok_or("unreadable")?);
    let main = res.main().ok_or("no [resource]")?;
    let atlas_path = main
        .prop("atlas")
        .and_then(|v| res.ext_path(v))
        .ok_or("no atlas")?;
    let (x, y, w, h) = main.prop("region").and_then(rect).ok_or("no region")?;
    let atlas = atlases
        .entry(atlas_path.clone())
        .or_insert_with(|| texture(pck, &atlas_path))
        .as_ref()
        .ok_or_else(|| format!("undecodable atlas {atlas_path}"))?;
    if w == 0 || h == 0 || x + w > atlas.width() || y + h > atlas.height() {
        return Err(format!("region outside {atlas_path}"));
    }
    let cut = imageops::crop_imm(atlas, x, y, w, h).to_image();
    // A sprite trimmed into the atlas keeps its canvas as `margin = Rect2(left, top, extra
    // width, extra height)`: put it back on that canvas, so it lines up with its outline.
    let Some((left, top, more_w, more_h)) = main.prop("margin").and_then(rect) else {
        return Ok(cut);
    };
    let mut canvas = RgbaImage::new(w + more_w, h + more_h);
    imageops::overlay(&mut canvas, &cut, i64::from(left), i64::from(top));
    Ok(canvas)
}

/// `Rect2(2017, 2875, 250, 190)` as whole pixels.
fn rect(value: &str) -> Option<(u32, u32, u32, u32)> {
    let inner = value.strip_prefix("Rect2(")?.strip_suffix(')')?;
    let n: Vec<u32> = inner
        .split(',')
        .map(|s| s.trim().parse::<f64>().ok().map(|v| v.round() as u32))
        .collect::<Option<_>>()?;
    match n[..] {
        [x, y, w, h] => Some((x, y, w, h)),
        _ => None,
    }
}

/// Writes the fonts to `dir` as `.ttf`; returns how many.
pub(super) fn fonts(pck: &mut Pck, dir: &Path, notes: &mut Vec<String>) -> Result<usize, String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    let mut count = 0;
    for name in FONTS {
        let Some(data) = pck.imported(&format!("fonts/{name}.ttf")) else {
            notes.push(format!("font {name}: not in the .pck"));
            continue;
        };
        let data = match super::godot::uncompressed(&data) {
            Ok(d) => d,
            Err(e) => {
                notes.push(format!("font {name}: {e}"));
                continue;
            }
        };
        if let Some(font) = sfnt(&data) {
            std::fs::write(dir.join(format!("{name}.ttf")), font).map_err(io)?;
            count += 1;
        } else {
            notes.push(format!(
                "font {name}: no font file in its {} bytes",
                data.len()
            ));
        }
    }
    Ok(count)
}

/// The TrueType/OpenType file embedded in a Godot font resource: found by its header, its end
/// the furthest of its tables.
fn sfnt(data: &[u8]) -> Option<&[u8]> {
    let be16 = |b: &[u8], at: usize| Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?));
    let be32 = |b: &[u8], at: usize| Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?));
    (0..data.len().saturating_sub(12)).find_map(|start| {
        let head = data.get(start..start + 4)?;
        if head != [0, 1, 0, 0] && head != b"OTTO" {
            return None;
        }
        let font = &data[start..];
        let tables = usize::from(be16(font, 4)?);
        if !(4..=64).contains(&tables) {
            return None;
        }
        let mut end = 12 + 16 * tables;
        for i in 0..tables {
            let record = 12 + 16 * i;
            let tag = font.get(record..record + 4)?;
            if !tag.iter().all(|c| c.is_ascii_graphic() || *c == b' ') {
                return None;
            }
            let offset = usize::try_from(be32(font, record + 8)?).ok()?;
            let length = usize::try_from(be32(font, record + 12)?).ok()?;
            end = end.max(offset.checked_add(length)?);
        }
        font.get(..end)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hsv_shader_keeps_grey_and_turns_colour() {
        let yiq = [
            [0.2989, 0.5870, 0.1140],
            [0.5959, -0.2774, -0.3216],
            [0.2115, -0.5229, 0.3114],
        ];
        let id = inverse(yiq);
        let back = (0..3)
            .map(|r| {
                (0..3)
                    .map(|c| (0..3).map(|k| id[r][k] * yiq[k][c]).sum::<f64>())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for (r, row) in back.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                assert!((v - if r == c { 1.0 } else { 0.0 }).abs() < 1e-9);
            }
        }
        let image = RgbaImage::from_pixel(1, 1, image::Rgba([128, 128, 128, 200]));
        // h = 1 is no turn, s = 0 drops colour: grey stays grey, alpha untouched.
        let out = hsv_shift(&image, (0.3, 0.0, 1.0));
        assert_eq!(out.get_pixel(0, 0).0, [128, 128, 128, 200]);
        let red = RgbaImage::from_pixel(1, 1, image::Rgba([200, 40, 40, 255]));
        assert_eq!(
            hsv_shift(&red, (1.0, 1.0, 1.0)).get_pixel(0, 0).0,
            [200, 40, 40, 255]
        );
        let turned = hsv_shift(&red, (0.5, 1.0, 1.0)).get_pixel(0, 0).0;
        assert!(
            turned[0] < turned[2],
            "half a turn takes red towards blue-green: {turned:?}"
        );
    }

    #[test]
    fn regions_and_embedded_fonts() {
        assert_eq!(
            rect("Rect2(2017, 2875, 250, 190)"),
            Some((2017, 2875, 250, 190))
        );
        assert_eq!(rect("Rect2(1.5, 2, 3, 4)"), Some((2, 2, 3, 4)));
        assert_eq!(rect("Rect2(1, 2)"), None);
        // A one-table font after some resource bytes: its header, the record, the table.
        let mut font = vec![0, 1, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0];
        for (i, tag) in [b"cmap", b"head", b"hhea", b"name"].iter().enumerate() {
            font.extend(*tag);
            font.extend([0; 4]);
            font.extend(u32::try_from(76 + 4 * i).unwrap_or(0).to_be_bytes());
            font.extend(4_u32.to_be_bytes());
        }
        font.extend([7; 16]);
        let mut data = b"RSRC junk".to_vec();
        data.extend(&font);
        data.extend(b"trailing");
        assert_eq!(sfnt(&data), Some(&font[..]));
    }
}
