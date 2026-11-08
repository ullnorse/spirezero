//! Spine 4.2 binary skeletons (`.skel`) and texture atlases, posed in their setup pose.
//!
//! Only what a still picture needs: bones, slots, skins and their region, mesh, linked-mesh and
//! clipping attachments, and the first frame of the idle animation (the pose the game shows
//! first). Constraints are skipped.

use std::collections::HashMap;

/// A big-endian Spine binary reader (`SkeletonInput`).
struct Input<'a> {
    data: &'a [u8],
    at: usize,
    strings: Vec<String>,
}

type R<T> = Result<T, String>;

impl<'a> Input<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            at: 0,
            strings: Vec::new(),
        }
    }

    fn byte(&mut self) -> R<u8> {
        let b = *self
            .data
            .get(self.at)
            .ok_or_else(|| format!("skeleton truncated at {}", self.at))?;
        self.at += 1;
        Ok(b)
    }

    fn bool(&mut self) -> R<bool> {
        Ok(self.byte()? != 0)
    }

    fn bytes<const N: usize>(&mut self) -> R<[u8; N]> {
        let slice = self
            .data
            .get(self.at..self.at + N)
            .ok_or_else(|| format!("skeleton truncated at {}", self.at))?;
        self.at += N;
        Ok(slice.try_into().unwrap_or([0; N]))
    }

    fn int(&mut self) -> R<i32> {
        Ok(i32::from_be_bytes(self.bytes()?))
    }

    fn float(&mut self) -> R<f32> {
        Ok(f32::from_be_bytes(self.bytes()?))
    }

    /// `readInt(true)`: a varint, 7 bits per byte, low bits first.
    fn varint(&mut self) -> R<usize> {
        let mut result = 0u32;
        for shift in [0, 7, 14, 21] {
            let b = self.byte()?;
            result |= u32::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(result as usize);
            }
        }
        result |= u32::from(self.byte()?) << 28;
        Ok(result as usize)
    }

    /// `readInt(true)` of a value that may be negative (written as its 32-bit pattern).
    fn signed_varint(&mut self) -> R<isize> {
        Ok(self.varint()? as u32 as i32 as isize)
    }

    fn string(&mut self) -> R<Option<String>> {
        let len = self.varint()?;
        if len == 0 {
            return Ok(None);
        }
        let bytes = self
            .data
            .get(self.at..self.at + len - 1)
            .ok_or("skeleton truncated in a string")?;
        self.at += len - 1;
        Ok(Some(String::from_utf8_lossy(bytes).into_owned()))
    }

    fn string_ref(&mut self) -> R<Option<String>> {
        let index = self.varint()?;
        Ok(match index {
            0 => None,
            i => self.strings.get(i - 1).cloned(),
        })
    }

    fn color(&mut self) -> R<[f32; 4]> {
        let [r, g, b, a] = self.bytes::<4>()?;
        Ok([r, g, b, a].map(|c| f32::from(c) / 255.0))
    }

    fn floats(&mut self, n: usize) -> R<Vec<f32>> {
        (0..n).map(|_| self.float()).collect()
    }

    fn shorts(&mut self, n: usize) -> R<Vec<usize>> {
        (0..n).map(|_| self.varint()).collect()
    }

    fn skip_varints(&mut self, n: usize) -> R<()> {
        for _ in 0..n {
            self.varint()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct BoneData {
    parent: Option<usize>,
    rotation: f32,
    x: f32,
    y: f32,
    scale_x: f32,
    scale_y: f32,
    shear_x: f32,
    shear_y: f32,
    inherit: u8,
}

#[derive(Debug, Clone)]
struct SlotData {
    bone: usize,
    color: [f32; 4],
    attachment: Option<String>,
    blend: Blend,
}

/// A slot's blend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Normal,
    Additive,
    Multiply,
    Screen,
}

/// Vertex positions: plain (x, y) pairs in the slot bone's space, or weighted.
#[derive(Debug, Clone, Default)]
struct Vertices {
    /// Per vertex: the bone count, then that many bone indices.
    bones: Option<Vec<usize>>,
    /// Plain: x, y per vertex. Weighted: x, y, weight per bone per vertex.
    values: Vec<f32>,
}

#[derive(Debug, Clone)]
struct Sequence {
    start: usize,
    digits: usize,
    setup: usize,
}

impl Sequence {
    fn path(&self, base: &str) -> String {
        format!(
            "{base}{:0width$}",
            self.start + self.setup,
            width = self.digits
        )
    }
}

#[derive(Debug, Clone)]
enum Attachment {
    Region {
        path: String,
        color: [f32; 4],
        x: f32,
        y: f32,
        rotation: f32,
        scale_x: f32,
        scale_y: f32,
        width: f32,
        height: f32,
    },
    Mesh {
        path: String,
        color: [f32; 4],
        vertices: Vertices,
        uvs: Vec<f32>,
        triangles: Vec<usize>,
    },
    /// A mesh sharing its parent's geometry, resolved after all skins are read.
    Linked {
        path: String,
        color: [f32; 4],
        skin: usize,
        parent: String,
    },
    Clipping {
        end_slot: usize,
        vertices: Vertices,
    },
    /// Bounding boxes, paths and points: nothing to draw, but animations may deform them.
    Invisible(Vertices),
}

#[derive(Debug, Default)]
struct Skin {
    name: String,
    attachments: HashMap<(usize, String), Attachment>,
}

/// A parsed skeleton.
#[derive(Debug)]
pub struct Skeleton {
    bones: Vec<BoneData>,
    slots: Vec<SlotData>,
    skins: Vec<Skin>,
    /// Vertex offsets (weighted) or positions (plain) set by the posed animation, by slot and
    /// attachment name.
    deforms: HashMap<(usize, String), Vec<f32>>,
    /// The slot draw order, when the posed animation changes it.
    draw_order: Option<Vec<usize>>,
}

impl Skeleton {
    /// Parses a skeleton and, with `animate`, poses it in the first frame of `idle_loop` (else
    /// of the first animation whose name has `idle`); else it stays in the setup pose.
    pub fn parse(data: &[u8], animate: bool) -> R<Self> {
        let mut input = Input::new(data);
        input.bytes::<8>()?; // hash
        let version = input.string()?.unwrap_or_default();
        if !version.starts_with("4.2") {
            return Err(format!("Spine {version} (want 4.2)"));
        }
        input.floats(5)?; // x, y, width, height, reference scale
        let nonessential = input.bool()?;
        if nonessential {
            input.float()?; // fps
            input.string()?; // images path
            input.string()?; // audio path
        }
        let n = input.varint()?;
        for _ in 0..n {
            let s = input.string()?.unwrap_or_default();
            input.strings.push(s);
        }
        let bones = read_bones(&mut input, nonessential)?;
        let slots = read_slots(&mut input, nonessential)?;
        skip_constraints(&mut input)?;
        let mut skins = Vec::new();
        let mut linked = Vec::new();
        // The default skin is index 0 even when the file has none, so a linked mesh's skin
        // index (which counts only the skins present) is shifted by one then.
        let has_default = if let Some(skin) = read_skin(&mut input, 0, nonessential, &mut linked)? {
            skins.push(skin);
            true
        } else {
            skins.push(Skin {
                name: "default".to_owned(),
                ..Skin::default()
            });
            false
        };
        for _ in 0..input.varint()? {
            let index = skins.len();
            if let Some(skin) = read_skin(&mut input, index, nonessential, &mut linked)? {
                skins.push(skin);
            }
        }
        if !has_default {
            for skin in &mut skins {
                for attachment in skin.attachments.values_mut() {
                    if let Attachment::Linked { skin, .. } = attachment {
                        *skin += 1;
                    }
                }
            }
        }
        let mut skeleton = Self {
            bones,
            slots,
            skins,
            deforms: HashMap::new(),
            draw_order: None,
        };
        skeleton.resolve_linked(&linked);
        let event_audio = read_events(&mut input)?;
        let mut idle: Option<FirstFrame> = None;
        for _ in 0..input.varint()? {
            let name = input.string()?.unwrap_or_default();
            let frame = skeleton.read_animation(&mut input, &event_audio, has_default)?;
            let better = match &idle {
                None => name.contains("idle"),
                Some(_) => name == "idle_loop",
            };
            if better {
                idle = Some(frame);
            }
            if name == "idle_loop" {
                break;
            }
        }
        if let Some(frame) = idle.filter(|_| animate) {
            skeleton.apply(frame);
        }
        Ok(skeleton)
    }

    /// Reads one animation, keeping the values it sets at time 0.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per timeline type, as in the format"
    )]
    fn read_animation(
        &self,
        input: &mut Input<'_>,
        event_audio: &[bool],
        has_default: bool,
    ) -> R<FirstFrame> {
        let mut first = FirstFrame::default();
        input.varint()?; // timeline count
        // Slot timelines.
        for _ in 0..input.varint()? {
            let slot = input.varint()?;
            for _ in 0..input.varint()? {
                let kind = input.byte()?;
                let frames = input.varint()?;
                if kind == 0 {
                    for frame in 0..frames {
                        let time = input.float()?;
                        let name = input.string_ref()?;
                        if frame == 0 && time <= 0.0 {
                            first.attachments.push((slot, name));
                        }
                    }
                    continue;
                }
                input.varint()?; // bezier count
                // Colour bytes per frame: RGBA, RGB, RGBA + RGB, RGB + RGB, alpha.
                let (bytes, curves) = match kind {
                    1 => (4, 4),
                    2 => (3, 3),
                    3 => (7, 7),
                    4 => (6, 6),
                    _ => (1, 1),
                };
                for frame in 0..frames {
                    let time = input.float()?;
                    let mut color = [0u8; 7];
                    for c in color.iter_mut().take(bytes) {
                        *c = input.byte()?;
                    }
                    if frame == 0 && time <= 0.0 {
                        let c = color.map(|c| f32::from(c) / 255.0);
                        let old = self.slots.get(slot).map_or([1.0; 4], |s| s.color);
                        let rgba = match kind {
                            1 | 3 => [c[0], c[1], c[2], c[3]],
                            2 | 4 => [c[0], c[1], c[2], old[3]],
                            _ => [old[0], old[1], old[2], c[0]],
                        };
                        first.colors.push((slot, rgba));
                    }
                    // The curve from the previous frame follows each later frame's values.
                    if frame > 0 {
                        skip_curve(input, curves)?;
                    }
                }
            }
        }
        // Bone timelines.
        for _ in 0..input.varint()? {
            let bone = input.varint()?;
            for _ in 0..input.varint()? {
                let kind = input.byte()?;
                let frames = input.varint()?;
                if kind == 10 {
                    for frame in 0..frames {
                        let time = input.float()?;
                        let inherit = input.byte()?;
                        if frame == 0 && time <= 0.0 {
                            first.bones.push((bone, BoneValue::Inherit(inherit)));
                        }
                    }
                    continue;
                }
                input.varint()?; // bezier count
                let values = if matches!(kind, 1 | 4 | 7) { 2 } else { 1 };
                let (time, a, b) = read_curve_timeline(input, frames, values)?;
                if time <= 0.0 {
                    let value = match kind {
                        0 => BoneValue::Rotate(a),
                        1 => BoneValue::Translate(Some(a), Some(b)),
                        2 => BoneValue::Translate(Some(a), None),
                        3 => BoneValue::Translate(None, Some(a)),
                        4 => BoneValue::Scale(Some(a), Some(b)),
                        5 => BoneValue::Scale(Some(a), None),
                        6 => BoneValue::Scale(None, Some(a)),
                        7 => BoneValue::Shear(Some(a), Some(b)),
                        8 => BoneValue::Shear(Some(a), None),
                        _ => BoneValue::Shear(None, Some(a)),
                    };
                    first.bones.push((bone, value));
                }
            }
        }
        // IK constraint timelines.
        for _ in 0..input.varint()? {
            input.varint()?; // constraint
            let frames = input.varint()?;
            input.varint()?; // bezier count
            for frame in 0..frames {
                let flags = input.byte()?;
                input.float()?; // time
                if flags & 1 != 0 && flags & 2 != 0 {
                    input.float()?;
                }
                if flags & 4 != 0 {
                    input.float()?;
                }
                // A frame's flags also say how the curve from the previous frame bends.
                if frame > 0 && flags & 128 != 0 {
                    input.floats(8)?;
                }
            }
        }
        // Transform constraint timelines.
        for _ in 0..input.varint()? {
            input.varint()?;
            let frames = input.varint()?;
            input.varint()?;
            read_curve_timeline(input, frames, 6)?;
        }
        // Path constraint timelines.
        for _ in 0..input.varint()? {
            input.varint()?;
            for _ in 0..input.varint()? {
                let kind = input.byte()?;
                let frames = input.varint()?;
                input.varint()?;
                read_curve_timeline(input, frames, if kind == 2 { 3 } else { 1 })?;
            }
        }
        // Physics timelines.
        for _ in 0..input.varint()? {
            input.varint()?;
            for _ in 0..input.varint()? {
                let kind = input.byte()?;
                let frames = input.varint()?;
                if kind == 8 {
                    input.floats(frames)?;
                    continue;
                }
                input.varint()?;
                read_curve_timeline(input, frames, 1)?;
            }
        }
        // Attachment (deform, sequence) timelines.
        for _ in 0..input.varint()? {
            let skin = input.varint()? + usize::from(!has_default);
            for _ in 0..input.varint()? {
                let slot = input.varint()?;
                for _ in 0..input.varint()? {
                    let name = input.string_ref()?.unwrap_or_default();
                    let kind = input.byte()?;
                    let frames = input.varint()?;
                    if kind == 1 {
                        for _ in 0..frames {
                            input.float()?;
                            input.int()?;
                            input.float()?;
                        }
                        continue;
                    }
                    let vertices = match self
                        .skins
                        .get(skin)
                        .and_then(|s| s.attachments.get(&(slot, name.clone())))
                    {
                        Some(
                            Attachment::Mesh { vertices, .. }
                            | Attachment::Clipping { vertices, .. }
                            | Attachment::Invisible(vertices),
                        ) => vertices.clone(),
                        _ => return Err(format!("deform of unknown attachment {name}")),
                    };
                    let length = if vertices.bones.is_some() {
                        vertices.values.len() / 3 * 2
                    } else {
                        vertices.values.len()
                    };
                    input.varint()?; // bezier count
                    let time = input.float()?;
                    for frame in 0..frames {
                        let end = input.varint()?;
                        let mut deform = vec![0.0; length];
                        if end > 0 {
                            let start = input.varint()?;
                            for d in deform.iter_mut().skip(start).take(end) {
                                *d = input.float()?;
                            }
                        }
                        if frame == 0 && time <= 0.0 {
                            if vertices.bones.is_none() {
                                for (d, v) in deform.iter_mut().zip(&vertices.values) {
                                    *d += v;
                                }
                            }
                            first.deforms.push(((slot, name.clone()), deform));
                        }
                        if frame + 1 < frames {
                            input.float()?; // next time
                            skip_curve(input, 1)?;
                        }
                    }
                }
            }
        }
        // Draw order timeline.
        let slot_count = self.slots.len();
        for frame in 0..input.varint()? {
            let time = input.float()?;
            let offsets = input.varint()?;
            let mut order = vec![usize::MAX; slot_count];
            let mut unchanged = Vec::with_capacity(slot_count);
            let mut original = 0;
            for _ in 0..offsets {
                let slot = input.varint()?;
                while original != slot && original < slot_count {
                    unchanged.push(original);
                    original += 1;
                }
                let at = original.checked_add_signed(input.signed_varint()?);
                if let Some(o) = at.and_then(|at| order.get_mut(at)) {
                    *o = original;
                }
                original += 1;
            }
            unchanged.extend(original..slot_count);
            for o in order.iter_mut().rev() {
                if *o == usize::MAX {
                    *o = unchanged.pop().unwrap_or(0);
                }
            }
            if frame == 0 && time <= 0.0 {
                first.draw_order = Some(order);
            }
        }
        // Event timeline.
        for _ in 0..input.varint()? {
            input.float()?;
            let event = input.varint()?;
            input.varint()?; // int (zigzag)
            input.float()?;
            input.string()?;
            if event_audio.get(event).copied().unwrap_or(false) {
                input.floats(2)?;
            }
        }
        Ok(first)
    }

    /// Poses the skeleton in `frame` (`MixBlend.setup`, full alpha).
    fn apply(&mut self, frame: FirstFrame) {
        for (bone, value) in frame.bones {
            let Some(b) = self.bones.get_mut(bone) else {
                continue;
            };
            match value {
                BoneValue::Rotate(r) => b.rotation += r,
                BoneValue::Translate(x, y) => {
                    b.x += x.unwrap_or(0.0);
                    b.y += y.unwrap_or(0.0);
                }
                BoneValue::Scale(x, y) => {
                    b.scale_x *= x.unwrap_or(1.0);
                    b.scale_y *= y.unwrap_or(1.0);
                }
                BoneValue::Shear(x, y) => {
                    b.shear_x += x.unwrap_or(0.0);
                    b.shear_y += y.unwrap_or(0.0);
                }
                BoneValue::Inherit(i) => b.inherit = i,
            }
        }
        for (slot, name) in frame.attachments {
            if let Some(s) = self.slots.get_mut(slot) {
                s.attachment = name;
            }
        }
        for (slot, color) in frame.colors {
            if let Some(s) = self.slots.get_mut(slot) {
                s.color = color;
            }
        }
        self.deforms = frame.deforms.into_iter().collect();
        self.draw_order = frame.draw_order;
    }

    /// Gives each linked mesh its parent's geometry.
    fn resolve_linked(&mut self, linked: &[(usize, usize, String)]) {
        for (skin, slot, name) in linked {
            let Some(Attachment::Linked {
                path,
                color,
                skin: parent_skin,
                parent,
            }) = self.skins[*skin]
                .attachments
                .get(&(*slot, name.clone()))
                .cloned()
            else {
                continue;
            };
            let parent = self
                .skins
                .get(parent_skin)
                .and_then(|s| s.attachments.get(&(*slot, parent)));
            let resolved = match parent {
                Some(Attachment::Mesh {
                    vertices,
                    uvs,
                    triangles,
                    ..
                }) => Attachment::Mesh {
                    path,
                    color,
                    vertices: vertices.clone(),
                    uvs: uvs.clone(),
                    triangles: triangles.clone(),
                },
                _ => Attachment::Invisible(Vertices::default()),
            };
            self.skins[*skin]
                .attachments
                .insert((*slot, name.clone()), resolved);
        }
    }

    /// The names of the non-default skins.
    pub fn skin_names(&self) -> Vec<&str> {
        self.skins.iter().skip(1).map(|s| s.name.as_str()).collect()
    }

    /// The setup pose with the default skin plus the skins named in `skins` (missing names are
    /// ignored), as triangles in world space, back to front.
    pub fn draw_list(&self, skins: &[&str], atlas: &Atlas) -> DrawList {
        let world = self.world_transforms();
        let active: Vec<&Skin> = self
            .skins
            .iter()
            .skip(1)
            .filter(|s| skins.contains(&s.name.as_str()))
            .collect();
        let mut list = DrawList::default();
        let mut clip_end = None;
        let order: Vec<usize> = self
            .draw_order
            .clone()
            .unwrap_or_else(|| (0..self.slots.len()).collect());
        for index in order {
            let Some(slot) = self.slots.get(index) else {
                continue;
            };
            let deform = slot
                .attachment
                .as_ref()
                .and_then(|name| self.deforms.get(&(index, name.clone())));
            let attachment = slot.attachment.as_ref().and_then(|name| {
                let key = (index, name.clone());
                active
                    .iter()
                    .rev()
                    .find_map(|s| s.attachments.get(&key))
                    .or_else(|| self.skins[0].attachments.get(&key))
            });
            let bone = world[slot.bone];
            let attachment = attachment.filter(|a| !is_left_out(a));
            match attachment {
                Some(Attachment::Region {
                    path,
                    color,
                    x,
                    y,
                    rotation,
                    scale_x,
                    scale_y,
                    width,
                    height,
                }) => match atlas.region(path) {
                    Some(region) => {
                        let (offsets, uvs) = region_quad(
                            region, *x, *y, *rotation, *scale_x, *scale_y, *width, *height,
                        );
                        list.items.push(DrawItem::Triangles(Triangles {
                            page: region.page,
                            points: offsets.iter().map(|&p| bone.apply(p)).collect(),
                            uvs,
                            indices: vec![0, 1, 2, 2, 3, 0],
                            color: mul(slot.color, *color),
                            blend: slot.blend,
                        }));
                    }
                    None => list.missing.push(path.clone()),
                },
                Some(Attachment::Mesh {
                    path,
                    color,
                    vertices,
                    uvs,
                    triangles,
                }) => match atlas.region(path) {
                    Some(region) => {
                        list.items.push(DrawItem::Triangles(Triangles {
                            page: region.page,
                            points: world_vertices(vertices, deform, bone, &world),
                            uvs: mesh_uvs(region, uvs, &atlas.pages[region.page]),
                            indices: triangles.clone(),
                            color: mul(slot.color, *color),
                            blend: slot.blend,
                        }));
                    }
                    None => list.missing.push(path.clone()),
                },
                Some(Attachment::Clipping { end_slot, vertices }) => {
                    list.items.push(DrawItem::ClipStart(world_vertices(
                        vertices, deform, bone, &world,
                    )));
                    clip_end = Some(*end_slot);
                }
                _ => {}
            }
            if clip_end == Some(index) {
                list.items.push(DrawItem::ClipEnd);
                clip_end = None;
            }
        }
        list
    }

    /// Each bone's world transform in the setup pose (`Bone.updateWorldTransform`; the
    /// skeleton itself is unscaled at the origin).
    fn world_transforms(&self) -> Vec<Affine> {
        let mut world: Vec<Affine> = Vec::with_capacity(self.bones.len());
        for bone in &self.bones {
            let rx = (bone.rotation + bone.shear_x).to_radians();
            let ry = (bone.rotation + 90.0 + bone.shear_y).to_radians();
            let local = [
                rx.cos() * bone.scale_x,
                ry.cos() * bone.scale_y,
                rx.sin() * bone.scale_x,
                ry.sin() * bone.scale_y,
            ];
            let Some(parent) = bone.parent.map(|p| world[p]) else {
                world.push(Affine {
                    m: local,
                    x: bone.x,
                    y: bone.y,
                });
                continue;
            };
            let (x, y) = parent.apply((bone.x, bone.y));
            let [pa, pb, pc, pd] = parent.m;
            let m = match bone.inherit {
                // onlyTranslation
                1 => local,
                // noRotationOrReflection
                2 => {
                    let s = pa * pa + pc * pc;
                    let (pa, pb, pc, pd, prx) = if s > 0.0001 {
                        let s = (pa * pd - pb * pc).abs() / s;
                        (pa, pc * s, pc, pa * s, pc.atan2(pa).to_degrees())
                    } else {
                        (0.0, pb, 0.0, pd, 90.0 - pd.atan2(pb).to_degrees())
                    };
                    let rx = (bone.rotation + bone.shear_x - prx).to_radians();
                    let ry = (bone.rotation + bone.shear_y - prx + 90.0).to_radians();
                    let la = rx.cos() * bone.scale_x;
                    let lb = ry.cos() * bone.scale_y;
                    let lc = rx.sin() * bone.scale_x;
                    let ld = ry.sin() * bone.scale_y;
                    [
                        pa * la - pb * lc,
                        pa * lb - pb * ld,
                        pc * la + pd * lc,
                        pc * lb + pd * ld,
                    ]
                }
                // noScale, noScaleOrReflection
                3 | 4 => {
                    let r = bone.rotation.to_radians();
                    let (cos, sin) = (r.cos(), r.sin());
                    let mut za = pa * cos + pb * sin;
                    let mut zc = pc * cos + pd * sin;
                    let mut s = za.hypot(zc);
                    if s > 0.00001 {
                        s = 1.0 / s;
                    }
                    za *= s;
                    zc *= s;
                    s = za.hypot(zc);
                    if bone.inherit == 3 && pa * pd - pb * pc < 0.0 {
                        s = -s;
                    }
                    let r = std::f32::consts::FRAC_PI_2 + zc.atan2(za);
                    let zb = r.cos() * s;
                    let zd = r.sin() * s;
                    let sx = bone.shear_x.to_radians();
                    let sy = (90.0 + bone.shear_y).to_radians();
                    let la = sx.cos() * bone.scale_x;
                    let lb = sy.cos() * bone.scale_y;
                    let lc = sx.sin() * bone.scale_x;
                    let ld = sy.sin() * bone.scale_y;
                    [
                        za * la + zb * lc,
                        za * lb + zb * ld,
                        zc * la + zd * lc,
                        zc * lb + zd * ld,
                    ]
                }
                _ => {
                    let [la, lb, lc, ld] = local;
                    [
                        pa * la + pb * lc,
                        pa * lb + pb * ld,
                        pc * la + pd * lc,
                        pc * lb + pd * ld,
                    ]
                }
            };
            world.push(Affine { m, x, y });
        }
        world
    }
}

/// A bone value an animation sets.
#[derive(Debug, Clone, Copy)]
enum BoneValue {
    Rotate(f32),
    Translate(Option<f32>, Option<f32>),
    Scale(Option<f32>, Option<f32>),
    Shear(Option<f32>, Option<f32>),
    Inherit(u8),
}

/// What an animation sets at time 0.
#[derive(Debug, Default)]
struct FirstFrame {
    bones: Vec<(usize, BoneValue)>,
    attachments: Vec<(usize, Option<String>)>,
    colors: Vec<(usize, [f32; 4])>,
    deforms: Vec<((usize, String), Vec<f32>)>,
    draw_order: Option<Vec<usize>>,
}

/// Skips a frame's curve: its type byte and, for a Bézier, four floats per value.
fn skip_curve(input: &mut Input<'_>, values: usize) -> R<()> {
    if input.byte()? == 2 {
        input.floats(4 * values)?;
    }
    Ok(())
}

/// Reads a curve timeline of `values` floats per frame; returns the first frame's time and
/// first two values.
fn read_curve_timeline(input: &mut Input<'_>, frames: usize, values: usize) -> R<(f32, f32, f32)> {
    let time = input.float()?;
    let first = input.floats(values)?;
    for _ in 1..frames {
        input.float()?;
        input.floats(values)?;
        skip_curve(input, values)?;
    }
    Ok((
        time,
        first.first().copied().unwrap_or(0.0),
        first.get(1).copied().unwrap_or(0.0),
    ))
}

/// Reads the events; returns whether each has audio (its keys then carry volume and balance).
fn read_events(input: &mut Input<'_>) -> R<Vec<bool>> {
    let mut audio = Vec::new();
    for _ in 0..input.varint()? {
        input.string()?; // name
        input.varint()?; // int
        input.float()?;
        input.string()?; // string
        let has_audio = input.string()?.is_some();
        if has_audio {
            input.floats(2)?;
        }
        audio.push(has_audio);
    }
    Ok(audio)
}

/// Attachments left out of a portrait: smoke textures that the game feeds to a shader (drawn
/// plainly they show their "SMOKE" placeholder lettering), and the shadow on the ground, which
/// would leave the creature small in a tall picture.
fn is_left_out(attachment: &Attachment) -> bool {
    match attachment {
        Attachment::Region { path, .. } | Attachment::Mesh { path, .. } => {
            let path = path.to_ascii_lowercase();
            let name = path.rsplit('/').next().unwrap_or(&path);
            path.contains("smoke")
                || name
                    .trim_start_matches("ground")
                    .trim_start_matches(['_', ' '])
                    .trim_end_matches(|c: char| c.is_ascii_digit() || c == '_')
                    == "shadow"
        }
        _ => false,
    }
}

fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]]
}

fn read_bones(input: &mut Input<'_>, nonessential: bool) -> R<Vec<BoneData>> {
    let n = input.varint()?;
    let mut bones = Vec::with_capacity(n);
    for i in 0..n {
        input.string()?;
        let parent = if i == 0 { None } else { Some(input.varint()?) };
        let [rotation, x, y, scale_x, scale_y, shear_x, shear_y, _length] =
            [(); 8].map(|()| input.float().unwrap_or(0.0));
        let inherit = input.byte()?;
        input.bool()?; // skin required
        if nonessential {
            input.int()?; // color
            input.string()?; // icon
            input.bool()?; // visible
        }
        if parent.is_some_and(|p| p >= i) {
            return Err("bone parent after its child".to_owned());
        }
        bones.push(BoneData {
            parent,
            rotation,
            x,
            y,
            scale_x,
            scale_y,
            shear_x,
            shear_y,
            inherit,
        });
    }
    Ok(bones)
}

fn read_slots(input: &mut Input<'_>, nonessential: bool) -> R<Vec<SlotData>> {
    let n = input.varint()?;
    let mut slots = Vec::with_capacity(n);
    for _ in 0..n {
        input.string()?;
        let bone = input.varint()?;
        let color = input.color()?;
        input.int()?; // dark color
        let attachment = input.string_ref()?;
        let blend = match input.varint()? {
            1 => Blend::Additive,
            2 => Blend::Multiply,
            3 => Blend::Screen,
            _ => Blend::Normal,
        };
        if nonessential {
            input.bool()?; // visible
        }
        slots.push(SlotData {
            bone,
            color,
            attachment,
            blend,
        });
    }
    Ok(slots)
}

/// Reads past the IK, transform, path and physics constraints.
fn skip_constraints(input: &mut Input<'_>) -> R<()> {
    // IK.
    for _ in 0..input.varint()? {
        input.string()?;
        input.varint()?; // order
        let n = input.varint()?;
        input.skip_varints(n + 1)?; // bones, target
        let flags = input.byte()?;
        if flags & 32 != 0 && flags & 64 != 0 {
            input.float()?;
        }
        if flags & 128 != 0 {
            input.float()?;
        }
    }
    // Transform.
    for _ in 0..input.varint()? {
        input.string()?;
        input.varint()?;
        let n = input.varint()?;
        input.skip_varints(n + 1)?;
        let flags = input.byte()?;
        let floats = (3..8).filter(|bit| flags & (1 << bit) != 0).count();
        input.floats(floats)?;
        let flags = input.byte()?;
        let floats = (0..7).filter(|bit| flags & (1 << bit) != 0).count();
        input.floats(floats)?;
    }
    // Path.
    for _ in 0..input.varint()? {
        input.string()?;
        input.varint()?;
        input.bool()?;
        let n = input.varint()?;
        input.skip_varints(n + 1)?; // bones, target slot
        let flags = input.byte()?;
        if flags & 128 != 0 {
            input.float()?;
        }
        input.floats(5)?; // position, spacing, mix rotate, x, y
    }
    // Physics.
    for _ in 0..input.varint()? {
        input.string()?;
        input.varint()?;
        input.varint()?; // bone
        let flags = input.byte()?;
        let floats = (1..7).filter(|bit| flags & (1 << bit) != 0).count();
        input.floats(floats)?;
        input.byte()?; // step
        input.floats(3)?; // inertia, strength, damping
        if flags & 128 != 0 {
            input.float()?;
        }
        input.floats(2)?; // wind, gravity
        if input.byte()? & 128 != 0 {
            input.float()?;
        }
    }
    Ok(())
}

/// Reads skin number `skin_index` (0: the default skin); linked meshes are noted as (skin index, slot, name) in `linked`.
fn read_skin(
    input: &mut Input<'_>,
    skin_index: usize,
    nonessential: bool,
    linked: &mut Vec<(usize, usize, String)>,
) -> R<Option<Skin>> {
    let mut skin = Skin::default();
    let slot_count = if skin_index == 0 {
        let n = input.varint()?;
        if n == 0 {
            return Ok(None);
        }
        "default".clone_into(&mut skin.name);
        n
    } else {
        skin.name = input.string()?.unwrap_or_default();
        if nonessential {
            input.int()?;
        }
        for _ in 0..5 {
            let n = input.varint()?;
            input.skip_varints(n)?; // bones, then the four constraint kinds
        }
        input.varint()?
    };
    for _ in 0..slot_count {
        let slot = input.varint()?;
        for _ in 0..input.varint()? {
            let name = input.string_ref()?.unwrap_or_default();
            let attachment = read_attachment(input, &name, nonessential)?;
            if matches!(attachment, Attachment::Linked { .. }) {
                linked.push((skin_index, slot, name.clone()));
            }
            skin.attachments.insert((slot, name), attachment);
        }
    }
    Ok(Some(skin))
}

fn read_vertices(input: &mut Input<'_>, weighted: bool) -> R<Vertices> {
    let count = input.varint()?;
    if !weighted {
        return Ok(Vertices {
            bones: None,
            values: input.floats(count * 2)?,
        });
    }
    let mut bones = Vec::new();
    let mut values = Vec::new();
    for _ in 0..count {
        let n = input.varint()?;
        bones.push(n);
        for _ in 0..n {
            bones.push(input.varint()?);
            values.extend(input.floats(3)?);
        }
    }
    Ok(Vertices {
        bones: Some(bones),
        values,
    })
}

fn read_sequence(input: &mut Input<'_>) -> R<Sequence> {
    input.varint()?; // count
    Ok(Sequence {
        start: input.varint()?,
        digits: input.varint()?,
        setup: input.varint()?,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per attachment type, as in the format"
)]
fn read_attachment(input: &mut Input<'_>, slot_name: &str, nonessential: bool) -> R<Attachment> {
    let flags = input.byte()?;
    let name = if flags & 8 != 0 {
        input.string_ref()?.unwrap_or_default()
    } else {
        slot_name.to_owned()
    };
    let path_and_color = |input: &mut Input<'_>| -> R<(String, [f32; 4], Option<Sequence>)> {
        let path = if flags & 16 != 0 {
            input.string_ref()?
        } else {
            None
        };
        let color = if flags & 32 != 0 {
            input.color()?
        } else {
            [1.0; 4]
        };
        let sequence = if flags & 64 != 0 {
            Some(read_sequence(input)?)
        } else {
            None
        };
        let path = path.unwrap_or_else(|| name.clone());
        let path = sequence
            .as_ref()
            .map_or_else(|| path.clone(), |s| s.path(&path));
        Ok((path, color, sequence))
    };
    Ok(match flags & 7 {
        // Region.
        0 => {
            let (path, color, _) = path_and_color(input)?;
            let rotation = if flags & 128 != 0 {
                input.float()?
            } else {
                0.0
            };
            let [x, y, scale_x, scale_y, width, height] =
                [(); 6].map(|()| input.float().unwrap_or(0.0));
            Attachment::Region {
                path,
                color,
                x,
                y,
                rotation,
                scale_x,
                scale_y,
                width,
                height,
            }
        }
        // Bounding box.
        1 => {
            let vertices = read_vertices(input, flags & 16 != 0)?;
            if nonessential {
                input.int()?;
            }
            Attachment::Invisible(vertices)
        }
        // Mesh.
        2 => {
            let (path, color, _) = path_and_color(input)?;
            let hull = input.varint()?;
            let vertices = read_vertices(input, flags & 128 != 0)?;
            let count = if let Some(bones) = &vertices.bones {
                vertex_count(bones)
            } else {
                vertices.values.len() / 2
            };
            let uvs = input.floats(count * 2)?;
            let triangles = input.shorts((count * 2 - hull - 2) * 3)?;
            if nonessential {
                let n = input.varint()?;
                input.skip_varints(n)?; // edges
                input.floats(2)?;
            }
            Attachment::Mesh {
                path,
                color,
                vertices,
                uvs,
                triangles,
            }
        }
        // Linked mesh.
        3 => {
            let (path, color, _) = path_and_color(input)?;
            let skin = input.varint()?;
            let parent = input.string_ref()?.unwrap_or_default();
            if nonessential {
                input.floats(2)?;
            }
            Attachment::Linked {
                path,
                color,
                skin,
                parent,
            }
        }
        // Path.
        4 => {
            let vertices = read_vertices(input, flags & 64 != 0)?;
            let count = vertices
                .bones
                .as_ref()
                .map_or_else(|| vertices.values.len() / 2, |bones| vertex_count(bones));
            input.floats(count * 2 / 6)?;
            if nonessential {
                input.int()?;
            }
            Attachment::Invisible(vertices)
        }
        // Point.
        5 => {
            input.floats(3)?;
            if nonessential {
                input.int()?;
            }
            Attachment::Invisible(Vertices::default())
        }
        // Clipping.
        6 => {
            let end_slot = input.varint()?;
            let vertices = read_vertices(input, flags & 16 != 0)?;
            if nonessential {
                input.int()?;
            }
            Attachment::Clipping { end_slot, vertices }
        }
        other => return Err(format!("unknown attachment type {other}")),
    })
}

/// The vertex count of weighted vertices' bone list.
const fn vertex_count(bones: &[usize]) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i < bones.len() {
        i += bones[i] + 1;
        count += 1;
    }
    count
}

/// An (x, y) pair.
type Point = (f32, f32);

/// A bone's world transform: `[a, b, c, d]` and the translation.
#[derive(Debug, Clone, Copy)]
struct Affine {
    m: [f32; 4],
    x: f32,
    y: f32,
}

impl Affine {
    fn apply(self, (x, y): (f32, f32)) -> (f32, f32) {
        let [a, b, c, d] = self.m;
        (x * a + y * b + self.x, x * c + y * d + self.y)
    }
}

/// `VertexAttachment.computeWorldVertices`; `deform` holds the animation's positions (plain) or
/// offsets (weighted).
fn world_vertices(
    vertices: &Vertices,
    deform: Option<&Vec<f32>>,
    slot_bone: Affine,
    world: &[Affine],
) -> Vec<(f32, f32)> {
    let Some(bones) = &vertices.bones else {
        let values = deform
            .filter(|d| d.len() == vertices.values.len())
            .unwrap_or(&vertices.values);
        return values
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&p| slot_bone.apply(p.into()))
            .collect();
    };
    let deform = deform.filter(|d| d.len() == vertices.values.len() / 3 * 2);
    let mut out = Vec::new();
    let (mut i, mut v, mut f) = (0, 0, 0);
    while i < bones.len() {
        let n = bones[i];
        let (mut wx, mut wy) = (0.0, 0.0);
        for &bone in &bones[i + 1..=i + n] {
            let (dx, dy) = deform.map_or((0.0, 0.0), |d| (d[f], d[f + 1]));
            f += 2;
            let (x, y, weight) = (
                vertices.values[v] + dx,
                vertices.values[v + 1] + dy,
                vertices.values[v + 2],
            );
            let (px, py) = world.get(bone).map_or((x, y), |b| b.apply((x, y)));
            wx += px * weight;
            wy += py * weight;
            v += 3;
        }
        out.push((wx, wy));
        i += n + 1;
    }
    out
}

/// One page of an atlas: its image file name and size.
#[derive(Debug)]
pub struct Page {
    pub name: String,
    pub width: f32,
    pub height: f32,
    pub pma: bool,
}

#[derive(Debug, Clone, Copy)]
struct Region {
    page: usize,
    /// Position and size in the texture as packed (before any rotation).
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    offset_x: f32,
    offset_y: f32,
    original_width: f32,
    original_height: f32,
    degrees: u16,
}

/// A Spine texture atlas (`.atlas` text).
#[derive(Debug, Default)]
pub struct Atlas {
    pub pages: Vec<Page>,
    regions: HashMap<String, Region>,
}

impl Atlas {
    pub fn parse(text: &str) -> Self {
        let mut atlas = Self::default();
        let mut region: Option<(String, Region, i32)> = None;
        let mut new_page = true;
        let finish = |atlas: &mut Self, region: Option<(String, Region, i32)>| {
            if let Some((name, mut r, index)) = region {
                if r.original_width == 0.0 && r.original_height == 0.0 {
                    r.original_width = r.width;
                    r.original_height = r.height;
                }
                if index >= 0 {
                    atlas.regions.entry(format!("{name}{index}")).or_insert(r);
                }
                atlas.regions.entry(name).or_insert(r);
            }
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                finish(&mut atlas, region.take());
                new_page = true;
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                finish(&mut atlas, region.take());
                if new_page {
                    atlas.pages.push(Page {
                        name: line.to_owned(),
                        width: 0.0,
                        height: 0.0,
                        pma: false,
                    });
                    new_page = false;
                } else {
                    region = Some((
                        line.to_owned(),
                        Region {
                            page: atlas.pages.len().saturating_sub(1),
                            left: 0.0,
                            top: 0.0,
                            width: 0.0,
                            height: 0.0,
                            offset_x: 0.0,
                            offset_y: 0.0,
                            original_width: 0.0,
                            original_height: 0.0,
                            degrees: 0,
                        },
                        -1,
                    ));
                }
                continue;
            };
            let nums: Vec<f32> = value
                .split(',')
                .filter_map(|n| n.trim().parse().ok())
                .collect();
            let n = |i: usize| nums.get(i).copied().unwrap_or(0.0);
            match (&mut region, key.trim()) {
                (None, "size") => {
                    if let Some(page) = atlas.pages.last_mut() {
                        page.width = n(0);
                        page.height = n(1);
                    }
                }
                (None, "pma") => {
                    if let Some(page) = atlas.pages.last_mut() {
                        page.pma = value.trim() == "true";
                    }
                }
                (Some((_, r, _)), "bounds" | "xy" | "size") => {
                    if key.trim() == "size" {
                        (r.width, r.height) = (n(0), n(1));
                    } else if key.trim() == "xy" {
                        (r.left, r.top) = (n(0), n(1));
                    } else {
                        (r.left, r.top, r.width, r.height) = (n(0), n(1), n(2), n(3));
                    }
                }
                (Some((_, r, _)), "offsets") => {
                    (r.offset_x, r.offset_y, r.original_width, r.original_height) =
                        (n(0), n(1), n(2), n(3));
                }
                (Some((_, r, _)), "rotate") => {
                    r.degrees = match value.trim() {
                        "true" => 90,
                        "false" => 0,
                        v => v.parse().unwrap_or(0),
                    };
                }
                (Some((_, _, index)), "index") => {
                    *index = value.trim().parse().unwrap_or(-1);
                }
                _ => {}
            }
        }
        finish(&mut atlas, region);
        atlas
    }

    fn region(&self, name: &str) -> Option<&Region> {
        self.regions.get(name)
    }
}

impl Region {
    /// (u, v, u2, v2) of the packed rectangle in the page, in texels.
    fn texels(&self) -> (f32, f32, f32, f32) {
        let (w, h) = if self.degrees == 90 {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        };
        (self.left, self.top, self.left + w, self.top + h)
    }

    /// `packedWidth`/`packedHeight`: the size in the page.
    const fn packed(&self) -> (f32, f32) {
        if self.degrees == 90 {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }
}

/// A region attachment's corners in bone space and their texel coordinates
/// (`RegionAttachment.updateRegion`), in the order br, bl, ul, ur.
#[expect(clippy::too_many_arguments, reason = "the attachment's fields")]
fn region_quad(
    region: &Region,
    x: f32,
    y: f32,
    rotation: f32,
    scale_x: f32,
    scale_y: f32,
    width: f32,
    height: f32,
) -> (Vec<Point>, Vec<Point>) {
    let (packed_w, packed_h) = region.packed();
    let mut x2 = width / 2.0;
    let mut y2 = height / 2.0;
    let mut x1 = -x2 + region.offset_x / region.original_width * width;
    let mut y1 = -y2 + region.offset_y / region.original_height * height;
    let rotated = region.degrees == 90;
    if rotated {
        x2 -= (region.original_width - region.offset_x - packed_h) / region.original_width * width;
        y2 -=
            (region.original_height - region.offset_y - packed_w) / region.original_height * height;
    } else {
        x2 -= (region.original_width - region.offset_x - packed_w) / region.original_width * width;
        y2 -=
            (region.original_height - region.offset_y - packed_h) / region.original_height * height;
    }
    x1 *= scale_x;
    y1 *= scale_y;
    x2 *= scale_x;
    y2 *= scale_y;
    let r = rotation.to_radians();
    let (cos, sin) = (r.cos(), r.sin());
    let corner = |lx: f32, ly: f32| (lx * cos - ly * sin + x, lx * sin + ly * cos + y);
    let offsets = vec![
        corner(x2, y1),
        corner(x1, y1),
        corner(x1, y2),
        corner(x2, y2),
    ];
    let (u, v, u2, v2) = region.texels();
    let uvs = if rotated {
        vec![(u2, v), (u2, v2), (u, v2), (u, v)]
    } else {
        vec![(u2, v2), (u, v2), (u, v), (u2, v)]
    };
    (offsets, uvs)
}

/// A mesh's texel coordinates (`MeshAttachment.updateRegion`).
fn mesh_uvs(region: &Region, region_uvs: &[f32], page: &Page) -> Vec<(f32, f32)> {
    let (tw, th) = (page.width.max(1.0), page.height.max(1.0));
    let (packed_w, packed_h) = region.packed();
    let (mut u, mut v, _, _) = region.texels();
    u /= tw;
    v /= th;
    let pairs = region_uvs.as_chunks::<2>().0.iter();
    let uv: Vec<(f32, f32)> = match region.degrees {
        90 => {
            u -= (region.original_height - region.offset_y - packed_w) / tw;
            v -= (region.original_width - region.offset_x - packed_h) / th;
            let (w, h) = (region.original_height / tw, region.original_width / th);
            pairs
                .map(|p| (u + p[1] * w, v + (1.0 - p[0]) * h))
                .collect()
        }
        180 => {
            u -= (region.original_width - region.offset_x - packed_w) / tw;
            v -= region.offset_y / th;
            let (w, h) = (region.original_width / tw, region.original_height / th);
            pairs
                .map(|p| (u + (1.0 - p[0]) * w, v + (1.0 - p[1]) * h))
                .collect()
        }
        270 => {
            u -= region.offset_y / tw;
            v -= region.offset_x / th;
            let (w, h) = (region.original_height / tw, region.original_width / th);
            pairs
                .map(|p| (u + (1.0 - p[1]) * w, v + p[0] * h))
                .collect()
        }
        _ => {
            u -= region.offset_x / tw;
            v -= (region.original_height - region.offset_y - packed_h) / th;
            let (w, h) = (region.original_width / tw, region.original_height / th);
            pairs.map(|p| (u + p[0] * w, v + p[1] * h)).collect()
        }
    };
    uv.into_iter().map(|(u, v)| (u * tw, v * th)).collect()
}

/// Textured triangles of one attachment, in world space; `uvs` are in texels of `page`.
#[derive(Debug)]
pub struct Triangles {
    pub page: usize,
    pub points: Vec<(f32, f32)>,
    pub uvs: Vec<(f32, f32)>,
    pub indices: Vec<usize>,
    pub color: [f32; 4],
    pub blend: Blend,
}

#[derive(Debug)]
pub enum DrawItem {
    Triangles(Triangles),
    /// Clip what follows to this polygon (world space) until [`DrawItem::ClipEnd`].
    ClipStart(Vec<(f32, f32)>),
    ClipEnd,
}

/// What to draw for a pose, and the attachment paths the atlas lacked.
#[derive(Debug, Default)]
pub struct DrawList {
    pub items: Vec<DrawItem>,
    pub missing: Vec<String>,
}
