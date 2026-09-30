pub const HDR_MODE: &str = "mnu_arcadia_hdr_mode";

pub const HDR_SCENE_MODS: &str = "mnu_arcadia_hdr_scene_mods";
pub const HDR_SCENE_CONFIG: &str = "mnu_arcadia_hdr_scene_config";
pub const HDR_SCENE_LEVEL: &str = "mnu_arcadia_hdr_scene_level";
pub const HDR_SCENE_WORKSPACE: &str = "mnu_arcadia_hdr_scene_workspace";
pub const HDR_SCENE_NOTES: &str = "mnu_arcadia_hdr_scene_notes";

pub const FTR_UPDATE: &str = "mnu_arcadia_ftr_update";
pub const FTR_NOTES_UPDATE: &str = "mnu_arcadia_ftr_notes_update";
pub const FTR_NOTES_READ: &str = "mnu_arcadia_ftr_notes_read";

pub const WS_FOOTER_CREATE: &str = "mnu_arcadia_ftr_ws_create";
pub const WS_FOOTER_DEFAULT: &str = "mnu_arcadia_ftr_ws_default";
pub const WS_FOOTER_NAMED: &str = "mnu_arcadia_ftr_ws_named";
pub const WS_FOOTER_BUBBLE: &str = "mnu_arcadia_ftr_ws_bubble";

pub const CTX_EDIT: &str = "mnu_arcadia_ctx_edit";
pub const CTX_RENAME: &str = "mnu_arcadia_ctx_rename";
pub const CTX_DELETE: &str = "mnu_arcadia_ctx_delete";

pub const POP_WS_DELETE: &str = "mnu_arcadia_pop_ws_delete";

static HUB_FOOTERS: [&str; 3] = ["mnu_arcadia_ftr_hub_0", "mnu_arcadia_ftr_hub_1", "mnu_arcadia_ftr_hub_2"];

static CONFIG_FOOTERS: [&str; 8] = [
    "mnu_arcadia_ftr_cfg_0",
    "mnu_arcadia_ftr_cfg_1",
    "mnu_arcadia_ftr_cfg_2",
    "mnu_arcadia_ftr_cfg_3",
    "mnu_arcadia_ftr_cfg_4",
    "mnu_arcadia_ftr_cfg_5",
    "mnu_arcadia_ftr_cfg_6",
    "mnu_arcadia_ftr_cfg_7",
];

static LEVEL_FOOTERS: [&str; 6] = [
    "mnu_arcadia_ftr_lvl_0",
    "mnu_arcadia_ftr_lvl_1",
    "mnu_arcadia_ftr_lvl_2",
    "mnu_arcadia_ftr_lvl_3",
    "mnu_arcadia_ftr_lvl_4",
    "mnu_arcadia_ftr_lvl_5",
];

pub fn hub_footer(row: i32) -> &'static str {
    lookup(&HUB_FOOTERS, row)
}

pub fn config_footer(row: i32) -> &'static str {
    lookup(&CONFIG_FOOTERS, row)
}

pub fn level_footer(row: i32) -> &'static str {
    lookup(&LEVEL_FOOTERS, row)
}

fn lookup(table: &[&'static str], row: i32) -> &'static str {
    usize::try_from(row).ok().and_then(|row| table.get(row)).copied().unwrap_or("")
}
