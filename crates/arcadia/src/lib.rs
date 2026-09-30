use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use camino::Utf8Path;

#[macro_use] extern crate log;

pub mod data;
pub mod game;
mod hooks;
pub mod labels;
pub mod offsets;
pub mod preview;
pub mod scenes;
pub mod screen;

pub const REQUIRED_FILES: &[&str] = &[
    "config.json",
    "ui/layout/menu/arcadia/arcadia/layout.arc",
    "ui/layout/menu/arcadia/arcadia_top/layout.arc",
    "ui/layout/menu/arcadia/arcadia_config/layout.arc",
    "ui/layout/menu/arcadia/arcadia_loglevel/layout.arc",
    "ui/layout/menu/arcadia/arcadia_workspace/layout.arc",
    "ui/layout/menu/arcadia/arcadia_changelog/layout.arc",
    "ui/message/msg_menu.xmsbt",
];

pub fn missing_resources(root: &Utf8Path) -> Vec<String> {
    REQUIRED_FILES
        .iter()
        .filter(|file| !root.join(file).exists())
        .map(|file| (*file).to_string())
        .collect()
}

pub enum Request {
    Hub,
    ModManager,
    Config,
    Changelog,
}

static REQUEST: Mutex<Option<Request>> = Mutex::new(None);

static PENDING_NOTES: Mutex<Option<(data::changelog::MainEntry, bool)>> = Mutex::new(None);

static CHANGELOG_CHOICE: Mutex<Option<bool>> = Mutex::new(None);

pub fn show_changelog(notes: data::changelog::MainEntry, offer_update: bool) {
    *PENDING_NOTES.lock().unwrap() = Some((notes, offer_update));
    *CHANGELOG_CHOICE.lock().unwrap() = None;
    request(Request::Changelog);
}

pub fn take_changelog_choice() -> Option<bool> {
    CHANGELOG_CHOICE.lock().unwrap().take()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UpdateProgress {
    Idle,
    Downloading,
    Failed,
}

static UPDATE_PROGRESS: Mutex<UpdateProgress> = Mutex::new(UpdateProgress::Idle);

pub fn update_progress() -> UpdateProgress {
    *UPDATE_PROGRESS.lock().unwrap()
}

pub fn set_update_progress(progress: UpdateProgress) {
    *UPDATE_PROGRESS.lock().unwrap() = progress;
}

pub(crate) fn has_pending_notes() -> bool {
    PENDING_NOTES.lock().unwrap().is_some()
}

pub(crate) fn take_pending_notes() -> Option<(data::changelog::MainEntry, bool)> {
    PENDING_NOTES.lock().unwrap().take()
}

pub(crate) fn set_changelog_choice(update: bool) {
    *CHANGELOG_CHOICE.lock().unwrap() = Some(update);
}

pub fn request(request: Request) {
    *REQUEST.lock().unwrap() = Some(request);
}

fn requested() -> bool {
    REQUEST.lock().unwrap().is_some()
}

fn take_request() -> Option<Request> {
    REQUEST.lock().unwrap().take()
}

pub fn open_from_menu() -> bool {
    hooks::leave_main_menu()
}

static INSTALLED: AtomicBool = AtomicBool::new(false);

pub fn installed() -> bool {
    INSTALLED.load(Ordering::Acquire)
}

pub fn install() {
    let _ = offsets::scene_queue_push_front();

    skyline::install_hooks!(hooks::scene_queue_push_front, hooks::menu_sequence_scene_on_enter, hooks::menu_sequence_scene_push_main_menu);

    INSTALLED.store(true, Ordering::Release);
}
