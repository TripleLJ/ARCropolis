pub mod changelog;
pub mod config;
pub mod log_level;
pub mod mods;
pub mod sequence;
pub mod top;
pub mod workspace;

use crate::scene_table;

pub const EXIT_BACK: u32 = 0;
pub const EXIT_MOD_MANAGER: u32 = 1;
pub const EXIT_WORKSPACE: u32 = 2;
pub const EXIT_CONFIG: u32 = 3;

pub const EXIT_LOG_LEVEL: u32 = 1;

pub const EXIT_WORKSPACE_EDIT_BASE: u32 = 1;

scene_table!(pub static MENU_TABLE = [sequence::ArcadiaSequenceScene]);

scene_table!(pub static HUB_TABLE = [
    top::ArcadiaTopScene,
    config::ArcadiaConfigScene,
    log_level::ArcadiaLogLevelScene,
    workspace::ArcadiaWorkspaceScene,
    mods::ArcadiaScene,
    changelog::ArcadiaChangelogScene,
]);
