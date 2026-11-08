//! Just enough of Godot's text formats (`.import`, `.tres`, `.tscn`) to follow resources.

use std::collections::HashMap;

/// The first quoted string in `s`.
fn quoted(s: &str) -> Option<&str> {
    let start = s.find('"')? + 1;
    let len = s[start..].find('"')?;
    Some(&s[start..start + len])
}

/// `key="value"` inside a section header such as `[ext_resource ... path="..." id="1"]`.
fn attr<'a>(header: &'a str, key: &str) -> Option<&'a str> {
    let at = header.find(&format!(" {key}=\""))?;
    quoted(&header[at..])
}

/// The imported file an `.import` remap points to: `path=`, else the first `path.<format>=`
/// (the desktop formats come first).
pub fn import_target(remap: &str) -> Option<String> {
    let mut fallback = None;
    for line in remap.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key == "path" {
            return quoted(value).map(|p| p.trim_start_matches("res://").to_owned());
        }
        if key.starts_with("path.") && fallback.is_none() {
            fallback = quoted(value).map(|p| p.trim_start_matches("res://").to_owned());
        }
    }
    fallback
}

/// The contents of a Godot compressed file (`RSCC`: a `u32` mode, block size and total size,
/// then each block's compressed size, then the blocks), or `data` itself when it is not one.
/// Only zstd (mode 2), the mode the game's resources use.
pub fn uncompressed(data: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, String> {
    use std::io::Read as _;
    if !data.starts_with(b"RSCC") {
        return Ok(std::borrow::Cow::Borrowed(data));
    }
    let u32_at = |at: usize| {
        data.get(at..at + 4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_le_bytes)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or("truncated RSCC header")
    };
    let (mode, block, total) = (u32_at(4)?, u32_at(8)?, u32_at(12)?);
    if mode != 2 {
        return Err(format!("RSCC mode {mode} (only zstd, 2)"));
    }
    if block == 0 {
        return Err("RSCC block size 0".into());
    }
    let blocks = total / block + 1;
    let mut at = 16 + 4 * blocks;
    let mut out = Vec::with_capacity(total);
    for i in 0..blocks {
        let size = u32_at(16 + 4 * i)?;
        let chunk = data.get(at..at + size).ok_or("truncated RSCC block")?;
        at += size;
        if size == 0 {
            continue;
        }
        let mut decoder =
            ruzstd::decoding::StreamingDecoder::new(chunk).map_err(|e| e.to_string())?;
        decoder.read_to_end(&mut out).map_err(|e| e.to_string())?;
    }
    if out.len() != total {
        return Err(format!("RSCC gave {} bytes, not {total}", out.len()));
    }
    Ok(std::borrow::Cow::Owned(out))
}

/// A section of a `.tres`/`.tscn` file: its header line and its `key = value` properties.
#[derive(Debug, Default)]
pub struct Section {
    pub header: String,
    pub props: HashMap<String, String>,
}

impl Section {
    /// The section kind: `node`, `resource`, `ext_resource`, ...
    pub fn kind(&self) -> &str {
        self.header
            .trim_start_matches('[')
            .split([' ', ']'])
            .next()
            .unwrap_or_default()
    }

    pub fn attr(&self, key: &str) -> Option<&str> {
        attr(&self.header, key)
    }

    pub fn prop(&self, key: &str) -> Option<&str> {
        self.props.get(key).map(String::as_str)
    }
}

/// A parsed `.tres`/`.tscn` file.
#[derive(Debug, Default)]
pub struct Resource {
    pub sections: Vec<Section>,
}

impl Resource {
    pub fn parse(text: &str) -> Self {
        let mut sections: Vec<Section> = Vec::new();
        for line in text.lines() {
            if line.starts_with('[') {
                sections.push(Section {
                    header: line.to_owned(),
                    props: HashMap::new(),
                });
            } else if let Some((key, value)) = line.split_once(" = ")
                && let Some(section) = sections.last_mut()
            {
                section.props.insert(key.to_owned(), value.to_owned());
            }
        }
        Self { sections }
    }

    /// The path (without `res://`) of the external resource a value such as
    /// `ExtResource("1_abc")` refers to.
    pub fn ext_path(&self, value: &str) -> Option<String> {
        let id = quoted(value.strip_prefix("ExtResource(")?)?;
        self.sections
            .iter()
            .filter(|s| s.kind() == "ext_resource")
            .find(|s| s.attr("id") == Some(id))
            .and_then(|s| s.attr("path"))
            .map(|p| p.trim_start_matches("res://").to_owned())
    }

    /// The `[resource]` section of a `.tres`.
    pub fn main(&self) -> Option<&Section> {
        self.sections.iter().find(|s| s.kind() == "resource")
    }

    /// The nodes of a `.tscn` whose `type` is `ty`.
    pub fn nodes<'a>(&'a self, ty: &'a str) -> impl Iterator<Item = &'a Section> + 'a {
        self.sections
            .iter()
            .filter(move |s| s.kind() == "node" && s.attr("type") == Some(ty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_remaps_and_ext_resources() {
        let remap = "[remap]\n\nimporter=\"texture\"\npath.bptc=\"res://.godot/imported/a.bptc.ctex\"\npath.astc=\"res://b\"\n";
        assert_eq!(
            import_target(remap).as_deref(),
            Some(".godot/imported/a.bptc.ctex")
        );
        let tres = "[gd_resource type=\"AtlasTexture\" format=3]\n\n[ext_resource type=\"Texture2D\" path=\"res://images/x.png\" id=\"1\"]\n\n[resource]\natlas = ExtResource(\"1\")\nregion = Rect2(2017, 2875, 250, 190)\n";
        let res = Resource::parse(tres);
        let main = res.main().expect("resource section");
        assert_eq!(
            res.ext_path(main.prop("atlas").expect("atlas")).as_deref(),
            Some("images/x.png")
        );
        assert_eq!(main.prop("region"), Some("Rect2(2017, 2875, 250, 190)"));
    }
}
