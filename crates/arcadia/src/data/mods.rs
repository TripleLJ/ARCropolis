use std::{collections::HashSet, fs};

use camino::{Utf8Path, Utf8PathBuf};
use log::warn;
use serde::Deserialize;
use smash_arc::Hash40;

use super::paths;

#[derive(Debug, Deserialize, Default)]
pub struct ModInfo {
    pub display_name: Option<String>,
    pub authors: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
}

pub fn read_info(path: &Utf8Path, folder_name: &str) -> ModInfo {
    let text = match fs::read_to_string(path.join("info.toml")) {
        Ok(text) => text,
        Err(_) => return ModInfo::default(),
    };

    match toml::from_str(&text) {
        Ok(info) => info,
        Err(err) => {
            warn!("'{}' has an info.toml that does not parse: {}", folder_name, err);
            ModInfo::default()
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModCategory {
    Fighter,
    Stage,
    Effects,
    Ui,
    Param,
    Audio,
    Misc,
}

impl ModCategory {
    pub fn from_str(value: &str) -> ModCategory {
        match value.to_ascii_lowercase().as_str() {
            "fighter" => ModCategory::Fighter,
            "stage" => ModCategory::Stage,
            "effects" => ModCategory::Effects,
            "ui" => ModCategory::Ui,
            "param" => ModCategory::Param,
            "audio" | "music" => ModCategory::Audio,
            _ => ModCategory::Misc,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ModCategory::Fighter => "Fighter",
            ModCategory::Stage => "Stage",
            ModCategory::Effects => "Effects",
            ModCategory::Ui => "UI",
            ModCategory::Param => "Param",
            ModCategory::Audio => "Audio",
            ModCategory::Misc => "Misc",
        }
    }
}

pub struct ModEntry {
    pub display_name: String,
    pub folder: String,
    pub category: ModCategory,
    pub description: String,
    pub version: String,
    pub authors: String,

    pub hash: Hash40,
    pub enabled: bool,
    pub preview: Option<Utf8PathBuf>,
}

pub fn find_preview(path: &Utf8Path) -> Option<Utf8PathBuf> {
    let preview = path.join("preview.webp");
    preview.exists().then_some(preview)
}

pub fn scan(preset: &HashSet<Hash40>) -> Vec<ModEntry> {
    let use_folder_name = config::use_folder_name();
    let dir = paths::mods();

    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) => {
            warn!("could not list the mods folder '{}': {}", dir, err);
            return Vec::new();
        },
    };

    let mut mods: Vec<ModEntry> = entries
        .flatten()
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .filter_map(|entry| {
            let folder = entry.file_name().into_string().ok()?;
            if folder.starts_with('.') {
                return None;
            }

            let path = dir.join(&folder);
            let hash = Hash40::from(path.as_str());
            let info = read_info(&path, &folder);
            let preview = find_preview(&path);

            let display_name = if use_folder_name {
                folder.clone()
            } else {
                info.display_name.clone().unwrap_or_else(|| folder.clone())
            };

            Some(ModEntry {
                display_name,
                folder,
                category: info.category.as_deref().map(ModCategory::from_str).unwrap_or(ModCategory::Misc),
                description: info.description.unwrap_or_default(),
                version: info.version.unwrap_or_else(|| "???".to_string()),
                authors: info.authors.unwrap_or_else(|| "???".to_string()),
                hash,
                enabled: preset.contains(&hash),
                preview,
            })
        })
        .collect();

    mods.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
    mods
}

pub fn installed_hashes() -> Vec<Hash40> {
    let dir = paths::mods();

    let mut hashes: Vec<Hash40> = fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|folder| !folder.starts_with('.'))
                .map(|folder| Hash40::from(dir.join(folder).as_str()))
                .collect()
        })
        .unwrap_or_default();

    hashes.sort_unstable();
    hashes
}
