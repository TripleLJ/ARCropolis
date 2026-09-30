use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("could not write the setting: {0}")]
    Write(String),
    #[error("{0} is not one of the six logging levels")]
    BadLevel(usize),
}

pub const FLAG_ROWS: [(&str, &str); 7] = [
    ("auto_update", "Check for updates on boot"),
    ("beta_updates", "Beta updates"),
    ("log_to_file", "Log to file"),
    ("skip_cutscene", "Skip opening movie"),
    ("skip_title_scene", "Skip title screen"),
    ("legacy_discovery", "Legacy mod discovery"),
    ("use_folder_name", "Folder names in the mod manager"),
];

pub fn flag(key: &str) -> bool {
    config::GLOBAL_CONFIG.lock().unwrap().get_flag(key)
}

pub fn set_flag(key: &str, value: bool) -> Result<(), SettingsError> {
    config::GLOBAL_CONFIG
        .lock()
        .unwrap()
        .set_flag(key, value)
        .map_err(|err| SettingsError::Write(err.to_string()))
}

pub const LEVELS: [&str; 6] = ["Trace", "Debug", "Info", "Warn", "Error", "Off"];

pub fn level_label(index: usize) -> &'static str {
    match LEVELS.get(index).copied() {
        Some("Warn") => "Warning",
        Some(level) => level,
        None => "",
    }
}

pub fn logging_level_index() -> usize {
    let stored = config::logger_level();
    LEVELS.iter().position(|level| *level == stored).unwrap_or(3)
}

pub fn set_logging_level(index: usize) -> Result<(), SettingsError> {
    let Some(level) = LEVELS.get(index) else {
        return Err(SettingsError::BadLevel(index));
    };

    config::GLOBAL_CONFIG
        .lock()
        .unwrap()
        .set_field("logging_level", *level)
        .map_err(|err| SettingsError::Write(err.to_string()))
}
