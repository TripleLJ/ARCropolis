use std::{fmt::Write, path::PathBuf};

use serde::{de::DeserializeOwned, Serialize};
use skyline::{
    hooks::{getRegionAddress, Region},
    nn,
};

#[macro_use]
extern crate log;

pub struct Pattern {
    pub bytes: &'static str,
    pub adjust: isize,
}

pub fn text() -> &'static [u8] {
    unsafe {
        let ptr = getRegionAddress(Region::Text) as *const u8;
        let size = (getRegionAddress(Region::Rodata) as usize) - (ptr as usize);
        std::slice::from_raw_parts(ptr, size)
    }
}

pub fn find(text: &[u8], pattern: &Pattern) -> Option<usize> {
    let found = lazysimd::find_pattern_neon(text.as_ptr(), text.len(), pattern.bytes)? as isize;
    Some((found + pattern.adjust) as usize)
}

fn find_unique(text: &[u8], pattern: &Pattern) -> Option<isize> {
    let found = lazysimd::find_pattern_neon(text.as_ptr(), text.len(), pattern.bytes)? as usize;
    let after = found + 1;
    if after < text.len() && lazysimd::find_pattern_neon(text[after..].as_ptr(), text.len() - after, pattern.bytes).is_some() {
        return None;
    }
    Some(found as isize)
}

pub fn find_any(text: &[u8], patterns: &[Pattern]) -> Option<usize> {
    patterns.iter().find_map(|pattern| find_unique(text, pattern).map(|found| (found + pattern.adjust) as usize))
}

pub fn find_bytes(text: &[u8], (bytes, adjust): (&[u8], isize)) -> Option<usize> {
    let mut pattern = String::new();
    for byte in bytes {
        write!(&mut pattern, "{:02X} ", byte).unwrap();
    }
    pattern.push_str("??");

    let found = lazysimd::find_pattern_neon(text.as_ptr(), text.len(), pattern)? as isize;
    Some((found + adjust) as usize)
}

fn instruction(text: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(text[offset..offset + 4].try_into().unwrap())
}

pub fn adrp_page(text: &[u8], adrp_offset: usize) -> usize {
    let adrp = instruction(text, adrp_offset);
    let immhi = (adrp & 0b0000_0000_1111_1111_1111_1111_1110_0000) >> 3;
    let immlo = (adrp & 0b0110_0000_0000_0000_0000_0000_0000_0000) >> 29;
    let imm = ((immhi | immlo) << 12) as i32 as usize;
    (adrp_offset & !0xfff).wrapping_add(imm)
}

pub fn ldr_imm(text: &[u8], ldr_offset: usize) -> usize {
    let ldr = instruction(text, ldr_offset);
    let size = (ldr & 0b1100_0000_0000_0000_0000_0000_0000_0000) >> 30;
    let imm = (ldr & 0b0000_0000_0011_1111_1111_1100_0000_0000) >> 10;
    (imm as usize) << size
}

pub fn add_imm(text: &[u8], offset: usize) -> usize {
    ((instruction(text, offset) & 0b0000_0000_0011_1111_1111_1100_0000_0000) >> 10) as usize
}

pub fn adrp_ldr(text: &[u8], anchor: &Pattern) -> Option<usize> {
    let adrp = find(text, anchor)?;
    Some(adrp_page(text, adrp) + ldr_imm(text, adrp + 4))
}

pub fn adrp_add(text: &[u8], anchor: &Pattern) -> Option<usize> {
    let adrp = find(text, anchor)?;
    Some(adrp_page(text, adrp) + add_imm(text, adrp + 4))
}

pub fn adrp_ldr_any(text: &[u8], anchors: &[Pattern]) -> Option<usize> {
    let adrp = find_any(text, anchors)?;
    Some(adrp_page(text, adrp) + ldr_imm(text, adrp + 4))
}

pub fn adrp_add_any(text: &[u8], anchors: &[Pattern]) -> Option<usize> {
    let adrp = find_any(text, anchors)?;
    Some(adrp_page(text, adrp) + add_imm(text, adrp + 4))
}

pub fn offset_to_addr(offset: usize) -> *const () {
    unsafe { (getRegionAddress(Region::Text) as *const u8).add(offset) as _ }
}

pub fn game_version() -> String {
    unsafe {
        let mut version = nn::oe::DisplayVersion { name: [0; 16] };
        nn::oe::GetDisplayVersion(&mut version);
        skyline::from_c_str(version.name.as_ptr())
    }
}

pub fn cache_dir() -> PathBuf {
    PathBuf::from("sd:/ultimate/arcropolis/cache").join(game_version())
}

pub fn load_or_build<T: Serialize + DeserializeOwned>(file: &str, build: impl FnOnce() -> Option<T>) -> T {
    let path = cache_dir().join(file);

    if let Ok(string) = std::fs::read_to_string(&path) {
        match toml::from_str(&string) {
            Ok(table) => return table,
            Err(err) => warn!("Ignoring '{}': {}", path.display(), err),
        }
    }

    let table = build().unwrap_or_else(|| panic!("a pattern for '{}' was not found in this game version", file));

    match toml::to_string_pretty(&table) {
        Ok(string) => {
            let _ = std::fs::create_dir_all(cache_dir());
            if let Err(err) = std::fs::write(&path, string) {
                error!("Unable to write '{}': {}", path.display(), err);
            }
        },
        Err(err) => error!("Unable to serialize '{}': {}", path.display(), err),
    }

    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_any_skips_misses_and_duplicates() {
        let text = [0xAA, 0xBB, 0xCC, 0x11, 0x22, 0xDD, 0x11, 0x22, 0xEE];
        let duplicate = Pattern { bytes: "11 22", adjust: 0 };
        let missing = Pattern { bytes: "FF FF", adjust: 0 };
        let unique = Pattern { bytes: "AA BB CC", adjust: 5 };

        assert_eq!(find_any(&text, &[duplicate, missing, unique]), Some(5));

        let only_duplicate = Pattern { bytes: "11 22", adjust: 0 };
        let still_missing = Pattern { bytes: "FF FF", adjust: 0 };
        assert_eq!(find_any(&text, &[only_duplicate, still_missing]), None);
    }
}
