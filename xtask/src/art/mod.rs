//! `cargo xtask art-import [--game-dir DIR] [--out DIR]`: extracts still pictures from the
//! user's own game install for the TUI (personal use only; nothing here is ever committed).
//!
//! The `.pck` is copied once to `~/.cache/spirezero-data/` (the install is on a slow mount).
//! Output, in `~/.cache/spirezero-data/art/` by default:
//! - `<kind>/<ID>.webp|jpg`: one picture per entity, `ID` being the engine's `ModelId.Entry`
//!   (lossless WebP with transparency; JPEG when opaque, as card art is).
//! - `index.tsv`: `kind  id  file  width  height` per picture.
//! - `art.bundle`: all pictures in one file, the form the TUI reads first and a portable build
//!   embeds. Layout (little-endian): `STS2ART1`, a `u32` count, then per entry a `u16` name
//!   length, the name `kind/ID`, a `u32` offset from the end of the table and a `u32` length;
//!   then the image files' bytes.
//!
//!
//! - `font/<name>.ttf`: the game's fonts ([`ui`]).
//!
//! It also copies the game's English text (`localization/eng/*.json` in the `.pck`) to
//! `~/.cache/spirezero-data/text/`, which a portable build embeds (`--only text` does just that).
//!
//! Monsters are Spine skeletons with no still image in the game, so each is drawn once in its
//! setup pose ([`spine`], [`raster`]).
#![allow(
    clippy::many_single_char_names,
    clippy::suboptimal_flops,
    reason = "geometry and colour maths read best in its usual letters and plain arithmetic"
)]
#![allow(
    clippy::float_arithmetic,
    reason = "image processing, not game logic: no float reaches the engine"
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    reason = "pixel coordinates and sizes are small and non-negative"
)]

mod ctex;
mod godot;
mod pck;
mod raster;
mod spine;
mod ui;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use image::{Rgba32FImage, RgbaImage, imageops};

use crate::coverage;
use godot::Resource;
use pck::Pck;

/// Where the user's game is installed, relative to `$HOME`: the Steam library of the OS
/// (Linux's native one, macOS's under `Library/Application Support`).
const DEFAULT_GAME_DIR: &str = if cfg!(target_os = "macos") {
    "Library/Application Support/Steam/steamapps/common/Slay the Spire 2"
} else {
    ".local/share/Steam/steamapps/common/Slay the Spire 2"
};
/// The `.pck`'s name in the local copy, and where it sits in an install: at the top on Linux
/// (and Windows), inside the app bundle on macOS, under its display name.
const PCK_NAME: &str = "SlayTheSpire2.pck";
const PCK_IN_INSTALL: [&str; 2] = [
    PCK_NAME,
    "SlayTheSpire2.app/Contents/Resources/Slay the Spire 2.pck",
];
pub const BUNDLE_MAGIC: &[u8; 8] = b"STS2ART1";

/// The longest side, in pixels, of each kind's pictures: about what a terminal can show.
const MONSTER_PX: u32 = 256;
const CHARACTER_PX: u32 = 256;
const CARD_PX: u32 = 200;
const ICON_PX: u32 = 64;
/// Event and ancient illustrations: wide paintings, a phone column wide at most twice over.
const EVENT_PX: u32 = 800;

/// The characters, whose creature scenes are their combat models.
const CHARACTERS: [&str; 5] = ["ironclad", "silent", "defect", "regent", "necrobinder"];

/// Skins the game picks in code (`MonsterModel.SetupSkins` overrides, or a relic's first
/// `SkinOptions` for pets), by scene; where the game picks at random, the first option.
const FIXED_SKINS: &[(&str, &[&str])] = &[
    ("battle_friend_v1", &["v1"]),
    ("battle_friend_v2", &["v2"]),
    ("battle_friend_v3", &["v3"]),
    ("bowlbug_egg", &["cocoon"]),
    ("bowlbug_nectar", &["goop"]),
    ("bowlbug_rock", &["rock"]),
    ("bowlbug_silk", &["web"]),
    ("byrdpip", &["version1"]),
    ("calcified_cultist", &["coral"]),
    ("cubex_construct", &["diamondeye", "moss1"]),
    ("damp_cultist", &["slug"]),
    ("kin_follower", &["hair_1"]),
    ("paels_legion", &["eyes"]),
    ("phantasmal_gardener", &["short"]),
    ("scroll_of_biting", &["skin1"]),
    ("toadpole", &["eye1", "pattern1"]),
    ("tough_egg", &["egg1"]),
    ("two_tailed_rat", &["barnacles1", "head1"]),
];

/// Creatures whose drawing needs Spine constraints (not implemented: path constraints bend
/// their tentacles): the game's own still image of them instead.
const STILL_IMAGES: &[(&str, &str)] = &[("vantom", "images/ui/run_history/vantom_boss.png")];

/// Creatures without a model of their own, drawn as part of another's: (id, picture's id).
const ALIASES: &[(&str, &str)] = &[
    // The Kaiser Crab's claws.
    ("CRUSHER", "KAISER_CRAB_BOSS"),
    ("ROCKET", "KAISER_CRAB_BOSS"),
];

/// Creatures whose idle animation starts faded out: drawn in the setup pose.
const SETUP_POSE: &[&str] = &["soul_nexus"];

#[derive(Debug)]
struct Options {
    game_dir: PathBuf,
    out: PathBuf,
    /// Only this kind (`monster`, `card`, ...): for working on one importer.
    only: Option<String>,
    /// Print this text file of the `.pck` (a scene, a resource) and stop: for finding out how
    /// the game lays a screen out.
    cat: Option<String>,
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| "no HOME".into())
}

/// `$SPIREZERO_DATA` (or the old `$SPIREBOT_DATA`), else `~/.cache/spirezero-data`, or the old
/// `~/.cache/spirezero-data` while only that one exists.
pub fn data_dir() -> Result<PathBuf, String> {
    for var in ["SPIREZERO_DATA", "SPIREBOT_DATA"] {
        if let Some(dir) = std::env::var_os(var).filter(|v| !v.is_empty()) {
            return Ok(PathBuf::from(dir));
        }
    }
    let cache = home()?.join(".cache");
    let (new, old) = (cache.join("spirezero-data"), cache.join("spirebot-data"));
    Ok(if !new.exists() && old.is_dir() {
        old
    } else {
        new
    })
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut game_dir = home()?.join(DEFAULT_GAME_DIR);
    let mut out = data_dir()?.join("art");
    let mut only = None;
    let mut cat = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--game-dir" => game_dir = PathBuf::from(value()?),
            "--out" => out = PathBuf::from(value()?),
            "--only" => only = Some(value()?),
            "--cat" => cat = Some(value()?),
            _ => {
                return Err(
                    "usage: cargo xtask art-import [--game-dir DIR] [--out DIR] [--only KIND] [--cat PATH]"
                        .into(),
                );
            }
        }
    }
    Ok(Options {
        game_dir,
        out,
        only,
        cat,
    })
}

/// The local copy of the game's `.pck`, refreshed when the install's differs in size.
fn local_pck(game_dir: &Path, lines: &mut Vec<String>) -> Result<PathBuf, String> {
    let source = PCK_IN_INSTALL
        .iter()
        .map(|p| game_dir.join(p))
        .find(|p| p.is_file())
        .unwrap_or_else(|| game_dir.join(PCK_NAME));
    let copy = data_dir()?.join(PCK_NAME);
    let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).ok();
    match (size(&source), size(&copy)) {
        (Some(a), Some(b)) if a == b => {}
        (Some(_), _) => {
            std::fs::create_dir_all(data_dir()?).map_err(|e| e.to_string())?;
            let partial = copy.with_extension("pck.partial");
            std::fs::copy(&source, &partial)
                .map_err(|e| format!("copy {}: {e}", source.display()))?;
            std::fs::rename(&partial, &copy).map_err(|e| e.to_string())?;
            lines.push(format!("copied {} to {}", source.display(), copy.display()));
        }
        (None, Some(_)) => {}
        (None, None) => return Err(format!("no {PCK_NAME} in {}", game_dir.display())),
    }
    Ok(copy)
}

/// Where the game keeps its English text.
const TEXT_PREFIX: &str = "localization/eng/";

/// Copies every `localization/eng/*.json` of `pck` to `dir`; returns how many.
fn text(pck: &mut Pck, dir: &Path) -> Result<usize, String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    let names: Vec<String> = pck
        .paths()
        .filter_map(|p| p.strip_prefix(TEXT_PREFIX))
        .filter(|name| !name.contains('/') && has_extension(name, "json"))
        .map(str::to_owned)
        .collect();
    for name in &names {
        let bytes = pck
            .read(&format!("{TEXT_PREFIX}{name}"))
            .ok_or_else(|| format!("cannot read {TEXT_PREFIX}{name}"))?;
        std::fs::write(dir.join(name), bytes).map_err(io)?;
    }
    if names.is_empty() {
        return Err(format!("no {TEXT_PREFIX}*.json in the .pck"));
    }
    Ok(names.len())
}

/// One extracted picture.
#[derive(Debug)]
struct Picture {
    kind: &'static str,
    id: String,
    image: RgbaImage,
}

pub fn run(args: &[String]) -> Result<Vec<String>, String> {
    let options = options(args)?;
    let mut lines = Vec::new();
    let pck_path = local_pck(&options.game_dir, &mut lines)?;
    let mut pck = Pck::open(&pck_path)?;
    if let Some(path) = &options.cat {
        let text = pck
            .text(path)
            .ok_or_else(|| format!("no {path} in the .pck"))?;
        lines.extend(text.lines().map(str::to_owned));
        return Ok(lines);
    }
    if options.only.as_deref().is_none_or(|o| o == "text") {
        let dir = data_dir()?.join("text");
        let count = text(&mut pck, &dir)?;
        lines.push(format!("wrote {count} text tables to {}", dir.display()));
    }
    if options.only.as_deref() == Some("text") {
        return Ok(lines);
    }
    let mut pictures = Vec::new();
    let mut notes = Vec::new();
    let wanted = |kind: &str| options.only.as_deref().is_none_or(|o| o == kind);
    if wanted("monster") {
        monsters(&mut pck, &mut pictures, &mut notes);
    }
    if wanted("character") {
        characters(&mut pck, &mut pictures);
    }
    if wanted("card") {
        cards(&mut pck, &mut pictures);
    }
    if wanted("event") {
        events(&mut pck, &mut pictures);
    }
    if wanted("icon") {
        icons(&mut pck, &mut pictures);
    }
    if wanted("ui") {
        ui::pieces(&mut pck, &mut pictures, &mut notes);
        ui::tinted(&mut pck, &mut pictures, &mut notes);
    }
    let bundle_size = write(&options.out, &pictures)?;
    if wanted("ui") {
        let dir = options.out.join("font");
        let count = ui::fonts(&mut pck, &dir, &mut notes)?;
        lines.push(format!("wrote {count} fonts to {}", dir.display()));
    }

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for p in &pictures {
        *counts.entry(p.kind).or_default() += 1;
    }
    lines.push(
        counts
            .iter()
            .map(|(k, n)| format!("{k} {n}"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    for (kind, dir) in [
        ("monster", "monsters"),
        ("card", "cards"),
        ("relic", "relics"),
        ("potion", "potions"),
        ("power", "powers"),
    ] {
        let have: BTreeSet<String> = pictures
            .iter()
            .filter(|p| p.kind == kind)
            .map(|p| coverage::normalize("", &p.id))
            .collect();
        let ported = coverage::entity_variants(dir)?;
        let missing: Vec<&String> = ported
            .iter()
            .filter(|v| {
                let id = coverage::normalize_variant(v);
                !have.contains(&id) && !have.contains(&format!("{id}power"))
            })
            .collect();
        let mut line = format!(
            "{kind}: {}/{} ported have a picture",
            ported.len() - missing.len(),
            ported.len()
        );
        if !missing.is_empty() {
            let names: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
            let _ = write!(line, "; missing: {}", names.join(" "));
        }
        lines.push(line);
    }
    lines.extend(notes);
    lines.push(format!(
        "wrote {} ({} KB bundle)",
        options.out.display(),
        bundle_size / 1024
    ));
    Ok(lines)
}

/// Writes the pictures, the index and the bundle; returns the bundle's size.
fn write(out: &Path, pictures: &[Picture]) -> Result<usize, String> {
    let io = |e: std::io::Error| format!("{}: {e}", out.display());
    // Only ever replace a directory this command made.
    if out.exists() {
        if !out.join("index.tsv").exists() {
            return Err(format!(
                "{} exists and is not an art-import output",
                out.display()
            ));
        }
        std::fs::remove_dir_all(out).map_err(io)?;
    }
    std::fs::create_dir_all(out).map_err(io)?;
    let mut index = String::new();
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for p in pictures {
        let (bytes, extension) = encode(p.kind, &p.image)?;
        let dir = out.join(p.kind);
        std::fs::create_dir_all(&dir).map_err(io)?;
        let file = format!("{}/{}.{extension}", p.kind, p.id);
        std::fs::write(out.join(&file), &bytes).map_err(io)?;
        let _ = writeln!(
            index,
            "{}\t{}\t{file}\t{}\t{}",
            p.kind,
            p.id,
            p.image.width(),
            p.image.height()
        );
        entries.push((format!("{}/{}", p.kind, p.id), bytes));
    }
    std::fs::write(out.join("index.tsv"), index).map_err(io)?;
    let bundle = bundle(&entries);
    std::fs::write(out.join("art.bundle"), &bundle).map_err(io)?;
    Ok(bundle.len())
}

/// JPEG quality of opaque pictures.
const JPEG_QUALITY: u8 = 80;

/// Card art as JPEG (rectangles of painting; the few with a ragged edge go on black), else
/// lossless WebP (keeps the transparency around creatures and icons); with the file extension.
fn encode(kind: &str, image: &RgbaImage) -> Result<(Vec<u8>, &'static str), String> {
    let mut bytes = Vec::new();
    if kind == "card" || image.pixels().all(|p| p.0[3] == 255) {
        let rgb = image::RgbImage::from_fn(image.width(), image.height(), |x, y| {
            let [r, g, b, a] = image.get_pixel(x, y).0;
            let over_black = |c: u8| (u16::from(c) * u16::from(a) / 255) as u8;
            image::Rgb([over_black(r), over_black(g), over_black(b)])
        });
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY)
            .encode_image(&rgb)
            .map_err(|e| e.to_string())?;
        return Ok((bytes, "jpg"));
    }
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::WebP,
        )
        .map_err(|e| e.to_string())?;
    Ok((bytes, "webp"))
}

/// The bundle bytes (layout in the module docs). Identical pictures share their bytes.
fn bundle(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut table = Vec::new();
    let mut data: Vec<u8> = Vec::new();
    let mut stored: BTreeMap<&[u8], usize> = BTreeMap::new();
    for (name, bytes) in entries {
        let offset = *stored.entry(bytes).or_insert_with(|| {
            data.extend(bytes);
            data.len() - bytes.len()
        });
        table.extend((name.len() as u16).to_le_bytes());
        table.extend(name.as_bytes());
        table.extend((offset as u32).to_le_bytes());
        table.extend((bytes.len() as u32).to_le_bytes());
    }
    let mut out = BUNDLE_MAGIC.to_vec();
    out.extend((entries.len() as u32).to_le_bytes());
    out.extend(table);
    out.extend(data);
    out
}

fn has_extension(path: &str, extension: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(extension))
}

/// `chomper` → `CHOMPER`.
fn entity_id(stem: &str) -> String {
    stem.to_ascii_uppercase()
}

/// The file name without directory and extension.
fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.split('.').next().unwrap_or(name)
}

/// Decodes the texture `path` (`res://` path of the source `.png`).
fn texture(pck: &mut Pck, path: &str) -> Option<RgbaImage> {
    ctex::decode(&pck.imported(path)?).ok()
}

/// Scales `image` down (never up) to fit `max` × `max`, filtering in premultiplied alpha.
fn fit(image: RgbaImage, max: u32) -> RgbaImage {
    let (w, h) = image.dimensions();
    if w <= max && h <= max {
        return image;
    }
    let ratio = f64::from(max) / f64::from(w.max(h));
    let nw = ((f64::from(w) * ratio).round() as u32).max(1);
    let nh = ((f64::from(h) * ratio).round() as u32).max(1);
    let mut pre = Rgba32FImage::new(w, h);
    for (dst, src) in pre.pixels_mut().zip(image.pixels()) {
        let [r, g, b, a] = src.0.map(|c| f32::from(c) / 255.0);
        dst.0 = [r * a, g * a, b * a, a];
    }
    let small = imageops::resize(&pre, nw, nh, imageops::FilterType::Lanczos3);
    let mut out = RgbaImage::new(nw, nh);
    for (dst, src) in out.pixels_mut().zip(small.pixels()) {
        let a = src.0[3].clamp(0.0, 1.0);
        let un = |v: f32| {
            if a > 0.0 {
                ((v / a).clamp(0.0, 1.0) * 255.0).round() as u8
            } else {
                0
            }
        };
        dst.0 = [
            un(src.0[0]),
            un(src.0[1]),
            un(src.0[2]),
            (a * 255.0).round() as u8,
        ];
    }
    out
}

/// Crops away fully transparent borders.
fn trim(image: RgbaImage) -> RgbaImage {
    let (w, h) = image.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (x, y, p) in image.enumerate_pixels() {
        if p.0[3] > 8 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    if x1 <= x0 || y1 <= y0 {
        return image;
    }
    imageops::crop_imm(&image, x0, y0, x1 - x0, y1 - y0).to_image()
}

/// One picture per creature scene: its Spine skeleton drawn in the setup pose.
fn monsters(pck: &mut Pck, pictures: &mut Vec<Picture>, notes: &mut Vec<String>) {
    let mut scenes: Vec<String> = pck
        .paths()
        .filter(|p| p.starts_with("scenes/creature_visuals/") && has_extension(p, "tscn"))
        .map(str::to_owned)
        .collect();
    scenes.sort();
    let mut failed = Vec::new();
    for scene in scenes {
        let name = stem(&scene).to_owned();
        match creature(pck, &scene, &name) {
            Ok(image) => pictures.push(Picture {
                kind: "monster",
                id: entity_id(&name),
                image,
            }),
            Err(e) => failed.push(format!("{name} ({e})")),
        }
    }
    for (id, of) in ALIASES {
        if let Some(image) = pictures
            .iter()
            .find(|p| p.id == *of)
            .map(|p| p.image.clone())
        {
            pictures.push(Picture {
                kind: "monster",
                id: (*id).to_owned(),
                image,
            });
        }
    }
    failed.retain(|f| {
        !ALIASES
            .iter()
            .any(|(id, _)| f.starts_with(&id.to_ascii_lowercase()))
    });
    if !failed.is_empty() {
        notes.push(format!("scenes without a picture: {}", failed.join(", ")));
    }
}

/// The skins to draw with the default one: the ones the game's code picks, else the scene's
/// preview skin, the `normal` one (not `phobia`), else the first.
fn chosen_skins(name: &str, preview: Option<&str>, available: &[&str]) -> Vec<String> {
    if let Some((_, skins)) = FIXED_SKINS.iter().find(|(n, _)| *n == name) {
        return skins.iter().map(|s| (*s).to_owned()).collect();
    }
    [preview, Some("normal"), available.first().copied()]
        .into_iter()
        .flatten()
        .find(|s| available.contains(s))
        .map(|s| vec![s.to_owned()])
        .unwrap_or_default()
}

fn creature(pck: &mut Pck, scene: &str, name: &str) -> Result<RgbaImage, String> {
    if let Some((_, still)) = STILL_IMAGES.iter().find(|(n, _)| *n == name) {
        let image = texture(pck, still).ok_or("no still image")?;
        return Ok(fit(trim(image), MONSTER_PX));
    }
    let scene_text = pck.text(scene).ok_or("unreadable scene")?;
    let scene_res = Resource::parse(&scene_text);
    let sprite = scene_res
        .nodes("SpineSprite")
        .next()
        .ok_or("no SpineSprite")?;
    let data_path = sprite
        .prop("skeleton_data_res")
        .and_then(|v| scene_res.ext_path(v))
        .ok_or("no skeleton data")?;
    let preview = sprite
        .prop("preview_skin")
        .map(|v| v.trim_matches('"').to_owned());
    spine_picture(
        pck,
        &data_path,
        name,
        preview.as_deref(),
        MONSTER_PX,
        !SETUP_POSE.contains(&name),
    )
}

/// A Spine skeleton's picture (its `skeleton_data` resource at `data_path`), drawn in the setup
/// pose, cropped to what shows and at most `max` pixels on a side.
fn spine_picture(
    pck: &mut Pck,
    data_path: &str,
    name: &str,
    preview: Option<&str>,
    max: u32,
    animate: bool,
) -> Result<RgbaImage, String> {
    let data = Resource::parse(&pck.text(data_path).ok_or("unreadable skeleton data")?);
    let main = data.main().ok_or("empty skeleton data")?;
    let atlas_path = main
        .prop("atlas_res")
        .and_then(|v| data.ext_path(v))
        .ok_or("no atlas")?;
    let skel_path = main
        .prop("skeleton_file_res")
        .and_then(|v| data.ext_path(v))
        .ok_or("no skeleton file")?;
    let skeleton = spine::Skeleton::parse(&pck.imported(&skel_path).ok_or("no .skel")?, animate)?;
    let atlas_json = pck.imported(&atlas_path).ok_or("no .atlas")?;
    let atlas_json = crate::json::parse(&String::from_utf8_lossy(&atlas_json))?;
    let atlas_text = atlas_json
        .get("atlas_data")
        .and_then(crate::json::Value::as_str)
        .ok_or("atlas without data")?;
    let atlas = spine::Atlas::parse(&unescape(atlas_text));
    let dir = atlas_path.rsplit_once('/').map_or("", |(d, _)| d);
    let pages: Vec<Option<raster::Texture>> = atlas
        .pages
        .iter()
        .map(|page| {
            texture(pck, &format!("{dir}/{}", page.name))
                .map(|t| raster::Texture::new(&t, page.pma))
        })
        .collect();
    let available = skeleton.skin_names();
    let skins = chosen_skins(name, preview, &available);
    let skins: Vec<&str> = skins.iter().map(String::as_str).collect();
    let list = skeleton.draw_list(&skins, &atlas);
    let world = raster::bounds(&list).ok_or("nothing to draw")?;
    // A rough pass finds what is visible, then the final one fills the picture with it.
    let rough_scale = 256.0 / (world.2 - world.0).max(world.3 - world.1);
    let rough = raster::render(&list, &pages, world, rough_scale);
    let visible = visible_world(&rough, world, rough_scale).ok_or("fully transparent")?;
    let scale = 3.0 * max as f32 / (visible.2 - visible.0).max(visible.3 - visible.1);
    let image = raster::render(&list, &pages, visible, scale);
    Ok(fit(trim(image), max))
}

/// Undoes JSON string escapes (`crate::json` keeps strings raw).
fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) if other != 'r' => out.push(other),
            _ => {}
        }
    }
    out
}

/// The share of a picture's opacity that may lie outside the crop on each side: drops stray
/// specks of effects far from the creature.
const CROP_TAIL: f64 = 0.001;

/// The range of indices holding all but [`CROP_TAIL`] of `mass` at each end.
fn core_range(mass: &[f64]) -> Option<(usize, usize)> {
    let total: f64 = mass.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let cut = total * CROP_TAIL;
    let mut sum = 0.0;
    let start = mass.iter().position(|m| {
        sum += m;
        sum > cut
    })?;
    sum = 0.0;
    let end = mass.len()
        - mass.iter().rev().position(|m| {
            sum += m;
            sum > cut
        })?;
    Some((start, end))
}

/// The world bounds of the opaque core of a render of `world` at `scale` ([`core_range`]),
/// with a pixel of margin.
fn visible_world(
    image: &RgbaImage,
    world: (f32, f32, f32, f32),
    scale: f32,
) -> Option<(f32, f32, f32, f32)> {
    let (w, h) = image.dimensions();
    let mut columns = vec![0.0; w as usize];
    let mut rows = vec![0.0; h as usize];
    for (x, y, p) in image.enumerate_pixels() {
        let a = f64::from(p.0[3]);
        columns[x as usize] += a;
        rows[y as usize] += a;
    }
    let (x0, x1) = core_range(&columns)?;
    let (y0, y1) = core_range(&rows)?;
    let px = |v: usize| v as f32 / scale;
    Some((
        world.0 + px(x0.saturating_sub(1)),
        world.3 - px((y1 + 1).min(h as usize)),
        world.0 + px((x1 + 1).min(w as usize)),
        world.3 - px(y0.saturating_sub(1)),
    ))
}

/// The character select portraits.
fn characters(pck: &mut Pck, pictures: &mut Vec<Picture>) {
    for name in CHARACTERS {
        let path = format!("images/packed/character_select/char_select_{name}.png");
        if let Some(image) = texture(pck, &path) {
            pictures.push(Picture {
                kind: "character",
                id: entity_id(name),
                image: fit(image, CHARACTER_PX),
            });
        }
    }
}

/// Card portraits: `images/packed/card_portraits/<pool>/<card>.png`, else the `beta/` art (some
/// cards have only that).
fn cards(pck: &mut Pck, pictures: &mut Vec<Picture>) {
    let mut paths: Vec<String> = pck
        .paths()
        .filter_map(|p| p.strip_suffix(".import"))
        .filter(|p| p.starts_with("images/packed/card_portraits/") && has_extension(p, "png"))
        .map(str::to_owned)
        .collect();
    // Final art first.
    paths.sort_by_key(|p| (p.contains("/beta/"), p.clone()));
    let mut seen = BTreeSet::new();
    for path in paths {
        let id = entity_id(stem(&path));
        if seen.contains(&id) {
            continue;
        }
        if let Some(image) = texture(pck, &path) {
            seen.insert(id.clone());
            pictures.push(Picture {
                kind: "card",
                id,
                image: fit(image, CARD_PX),
            });
        }
    }
}

/// Event illustrations (`images/events/<event>.png`, the picture an event's page shows over its
/// text), the ancients' (`images/ancients/<ancient>_placeholder.png`, as `DARV`) and the
/// treasure room's chest (as `CHEST`).
fn events(pck: &mut Pck, pictures: &mut Vec<Picture>) {
    let found: Vec<(String, String)> = pck
        .paths()
        .filter_map(|p| p.strip_suffix(".import"))
        .filter(|p| has_extension(p, "png"))
        .filter_map(|p| {
            let name = stem(p);
            if let Some(rest) = p.strip_prefix("images/events/") {
                let flat = !rest.contains('/');
                let own = !name.ends_with("_phobia_mode") && !name.ends_with("_foreground");
                return (flat && own).then(|| (entity_id(name), p.to_owned()));
            }
            let ancient = p
                .strip_prefix("images/ancients/")?
                .strip_suffix("_placeholder.png")?;
            Some((entity_id(ancient), p.to_owned()))
        })
        .collect();
    // The treasure room's chest (Spine; the act's room), as `CHEST`.
    if let Ok(image) = spine_picture(
        pck,
        "animations/backgrounds/treasure_room/chest_room_act_1_skel_data.tres",
        "chest",
        None,
        EVENT_PX,
        true,
    ) {
        pictures.push(Picture {
            kind: "event",
            id: "CHEST".to_owned(),
            image,
        });
    }
    for (id, path) in found {
        if let Some(image) = texture(pck, &path) {
            pictures.push(Picture {
                kind: "event",
                id,
                image: fit(image, EVENT_PX),
            });
        }
    }
}

/// Relic, potion, power and intent icons.
fn icons(pck: &mut Pck, pictures: &mut Vec<Picture>) {
    for (kind, dir, prefix) in [
        ("relic", "images/relics/", ""),
        ("potion", "images/potions/large/", ""),
        ("power", "images/powers/", ""),
        ("intent", "images/packed/intents/", "intent_"),
    ] {
        let mut paths: Vec<String> = pck
            .paths()
            .filter_map(|p| p.strip_suffix(".import"))
            .filter(|p| {
                p.strip_prefix(dir)
                    .is_some_and(|rest| !rest.contains('/') && has_extension(rest, "png"))
            })
            .map(str::to_owned)
            .collect();
        paths.sort();
        for path in paths {
            let name = stem(&path);
            let name = name.strip_prefix(prefix).unwrap_or(name);
            if let Some(image) = texture(pck, &path) {
                pictures.push(Picture {
                    kind,
                    id: entity_id(name),
                    image: fit(trim(image), ICON_PX),
                });
            }
        }
    }
    // Yummy Cookie's icon depends on the character: the Ironclad's.
    if let Some(image) = pictures
        .iter()
        .find(|p| p.kind == "relic" && p.id == "YUMMY_COOKIE_IRONCLAD")
        .map(|p| p.image.clone())
    {
        pictures.push(Picture {
            kind: "relic",
            id: "YUMMY_COOKIE".to_owned(),
            image,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_layout() {
        let b = bundle(&[
            ("monster/A".to_owned(), vec![1, 2]),
            ("card/B".to_owned(), vec![3]),
            ("monster/C".to_owned(), vec![1, 2]),
        ]);
        assert_eq!(&b[..8], BUNDLE_MAGIC);
        assert_eq!(u32::from_le_bytes([b[8], b[9], b[10], b[11]]), 3);
        // The data follows the table: [1, 2] then [3]; C shares A's bytes.
        assert_eq!(&b[b.len() - 3..], [1, 2, 3]);
    }

    #[test]
    fn skins_follow_the_game() {
        assert_eq!(
            chosen_skins("bowlbug_rock", None, &["rock", "goop"]),
            ["rock"]
        );
        assert_eq!(chosen_skins("x", None, &["phobia", "normal"]), ["normal"]);
        assert_eq!(chosen_skins("x", Some("b"), &["a", "b"]), ["b"]);
        assert_eq!(chosen_skins("x", None, &[]), Vec::<String>::new());
    }
}
