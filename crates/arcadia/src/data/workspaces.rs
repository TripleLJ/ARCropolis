use std::{cmp::Ordering, collections::HashSet};

use config::{presets::PresetError, workspaces::WorkspaceError};
use smash_arc::Hash40;

use super::mods;

const DEFAULT_NAME: &str = "Default";

pub struct Workspace {
    pub name: String,
    pub active: bool,
    pub mod_count: usize,
}

pub fn list() -> Result<Vec<Workspace>, WorkspaceError> {
    let all = config::workspaces::get_list()?;
    let active_name = config::workspaces::get_active_workspace_name().unwrap_or_else(|_| DEFAULT_NAME.to_string());
    let installed = mods::installed_hashes();

    let mut entries: Vec<Workspace> = all
        .into_iter()
        .map(|(name, _preset_field)| {
            let mod_count = config::presets::get_preset(&name)
                .map(|preset| preset.iter().filter(|hash| installed.binary_search(hash).is_ok()).count())
                .unwrap_or(0);
            let active = name == active_name;

            Workspace { name, active, mod_count }
        })
        .collect();

    entries.sort_by(|a, b| match (a.name == DEFAULT_NAME, b.name == DEFAULT_NAME) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}

pub fn create(name: &str) -> Result<(), WorkspaceError> {
    config::workspaces::create_new_workspace(name.to_string())
}

pub fn rename(from: &str, to: &str) -> Result<(), WorkspaceError> {
    config::workspaces::rename_workspace(from, to)
}

pub fn set_active(name: &str) -> Result<(), WorkspaceError> {
    config::workspaces::set_active_workspace(name.to_string())
}

pub fn active_name() -> Result<String, WorkspaceError> {
    config::workspaces::get_active_workspace_name()
}

pub fn delete(name: &str) -> Result<(), WorkspaceError> {
    config::workspaces::delete_workspace(name)
}

pub fn preset(name: &str) -> Result<HashSet<Hash40>, PresetError> {
    config::presets::get_preset(name)
}

pub fn save_preset(name: &str, preset: &HashSet<Hash40>) -> Result<(), PresetError> {
    config::presets::replace_preset(name, preset)
}
