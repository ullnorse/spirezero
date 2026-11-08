//! A Godot 4 `.pck` archive (pack format 2 and 3, unencrypted): the file directory, and reads of
//! single files by their `res://` path.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

const MAGIC: &[u8; 4] = b"GDPC";
/// `PACK_DIR_ENCRYPTED`.
const DIR_ENCRYPTED: u32 = 1;
/// `PACK_REL_FILEBASE`: file offsets are relative to the file base.
const REL_FILEBASE: u32 = 2;
/// `PACK_FILE_ENCRYPTED` on a directory entry.
const FILE_ENCRYPTED: u32 = 1;

#[derive(Debug)]
pub struct Pck {
    file: File,
    /// Path without `res://` → (absolute offset, size).
    entries: HashMap<String, (u64, u64)>,
}

fn u32_at(r: &mut impl Read) -> std::io::Result<u32> {
    let mut b = [0; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn u64_at(r: &mut impl Read) -> std::io::Result<u64> {
    let mut b = [0; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

impl Pck {
    /// Opens `path` and reads its directory.
    pub fn open(path: &Path) -> Result<Self, String> {
        let fail = |e: std::io::Error| format!("{}: {e}", path.display());
        let file = File::open(path).map_err(fail)?;
        let mut r = BufReader::new(file.try_clone().map_err(fail)?);
        let mut magic = [0; 4];
        r.read_exact(&mut magic).map_err(fail)?;
        if &magic != MAGIC {
            return Err(format!("{}: not a Godot .pck", path.display()));
        }
        let version = u32_at(&mut r).map_err(fail)?;
        if !(2..=3).contains(&version) {
            return Err(format!("unsupported .pck format {version} (want 2 or 3)"));
        }
        // Engine version major, minor, patch.
        for _ in 0..3 {
            u32_at(&mut r).map_err(fail)?;
        }
        let flags = u32_at(&mut r).map_err(fail)?;
        if flags & DIR_ENCRYPTED != 0 {
            return Err("the .pck directory is encrypted".to_owned());
        }
        let file_base = u64_at(&mut r).map_err(fail)?;
        let dir_offset = if version >= 3 {
            Some(u64_at(&mut r).map_err(fail)?)
        } else {
            None
        };
        // 16 reserved words.
        for _ in 0..16 {
            u32_at(&mut r).map_err(fail)?;
        }
        if let Some(offset) = dir_offset {
            r.seek(SeekFrom::Start(offset)).map_err(fail)?;
        }
        let base = if flags & REL_FILEBASE != 0 {
            file_base
        } else {
            0
        };
        let count = u32_at(&mut r).map_err(fail)?;
        let mut entries = HashMap::with_capacity(count as usize);
        for _ in 0..count {
            let len = u32_at(&mut r).map_err(fail)? as usize;
            let mut name = vec![0; len];
            r.read_exact(&mut name).map_err(fail)?;
            let offset = u64_at(&mut r).map_err(fail)?;
            let size = u64_at(&mut r).map_err(fail)?;
            let mut md5 = [0; 16];
            r.read_exact(&mut md5).map_err(fail)?;
            let file_flags = u32_at(&mut r).map_err(fail)?;
            if file_flags & FILE_ENCRYPTED != 0 {
                continue;
            }
            let name = String::from_utf8_lossy(&name);
            let name = name.trim_end_matches('\0');
            let name = name.strip_prefix("res://").unwrap_or(name).to_owned();
            entries.insert(name, (base + offset, size));
        }
        Ok(Self { file, entries })
    }

    /// Every file path in the archive (without `res://`).
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// The bytes of the file at `path` (with or without `res://`).
    pub fn read(&mut self, path: &str) -> Option<Vec<u8>> {
        let path = path.strip_prefix("res://").unwrap_or(path);
        let &(offset, size) = self.entries.get(path)?;
        let mut data = vec![0; usize::try_from(size).ok()?];
        self.file.seek(SeekFrom::Start(offset)).ok()?;
        self.file.read_exact(&mut data).ok()?;
        Some(data)
    }

    /// The text file at `path`.
    pub fn text(&mut self, path: &str) -> Option<String> {
        self.read(path)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    }

    /// The imported form of the resource `path` (`.ctex`, `.spatlas`, ...), found through its
    /// `.import` remap file.
    pub fn imported(&mut self, path: &str) -> Option<Vec<u8>> {
        let path = path.strip_prefix("res://").unwrap_or(path);
        let remap = self.text(&format!("{path}.import"))?;
        let target = super::godot::import_target(&remap)?;
        self.read(&target)
    }
}
