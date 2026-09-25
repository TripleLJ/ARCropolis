use std::{
    collections::{HashSet, VecDeque},
    ffi::CStr,
    fs,
    ptr::{addr_of_mut, NonNull},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

use smash_arc::Hash40;

use crate::{
    data::{
        mods::{self, ModCategory, ModEntry},
        workspaces,
    },
    game::{
        hash40,
        layout::{
            debug_name, play_animation, with_nul, write_text, Buttons, LayoutViewHandle, Pane, PaneHandle, PartsHandle, Picture, VirtualButton,
            ANIM_FRAME_STILL, ASSIGN_STANDARD, INT_FMT, TEXT_FMT,
        },
        list::{ListScrollerParams, RowBinder, RowBinderVtable, Scroller, ITEM_NONE},
        scene::{Scene, SceneChanger, SceneInitContext},
        scene_vtable::{SceneImpl, SceneVtable},
        texture::MemoryTexture,
    },
    labels,
    preview::webp_to_bntx,
    screen::{ExitStep, HeaderStyle, Screen, ScreenEvent},
    scenes::EXIT_BACK,
};

static VTABLE: SceneVtable = SceneVtable::of::<ArcadiaScene>();

static BINDER_VTABLE: RowBinderVtable = RowBinderVtable::new(bind_row);

static DESC_BINDER_VTABLE: RowBinderVtable = RowBinderVtable::new(bind_desc_line);

static FRAME: AtomicU64 = AtomicU64::new(0);
static RETIRED: Mutex<VecDeque<(u64, MemoryTexture)>> = Mutex::new(VecDeque::new());
const RETIRE_FRAMES: u64 = 10;

const LAYOUT_PATH: &str = "ui/layout/menu/arcadia/arcadia/layout.arc";

const _: () = assert!(hash40(LAYOUT_PATH) == 0x29_5847_8e96);

static SCROLL_GROUP: &[u8] = b"scroll\0";

static DESC_GROUP: &[u8] = b"scroll_00\0";

static TITLE_PANE: &[u8] = b"set_txt_topic_title\0";

static ROW_NUM_PANE: &[u8] = b"set_txt_num\0";
static ROW_ON: &str = "ON";
static ROW_OFF: &str = "OFF";

static ROW_COL_FIGHTER: &[u8] = b"col_fighter\0";
static ROW_COL_STAGE: &[u8] = b"col_stage\0";
static ROW_COL_EFFECTS: &[u8] = b"col_effects\0";
static ROW_COL_UI: &[u8] = b"col_ui\0";
static ROW_COL_PARAM: &[u8] = b"col_param\0";
static ROW_COL_AUDIO: &[u8] = b"col_audio\0";
static ROW_COL_MISC: &[u8] = b"col_misc\0";

static ROW_ICONS_OFF: &[u8] = b"fighter_icon_off\0";

static CATEGORY_PANE: &[u8] = b"set_txt_topic_cat_com\0";

static TOPIC_NUM_PANE: &[u8] = b"set_txt_topic_num\0";

static TAB_PANE: &[u8] = b"set_txt_list\0";

static TAB_ANIM_ALL: &[u8] = b"cat_com_mode_all\0";
static TAB_ANIM_ONE: &[u8] = b"cat_com\0";
const TAB_ANIM_END: f32 = 9.0;

static MOVIE_PARTS: &[u8] = b"set_movie_preview\0";
static MOVIE_PIC_PANE: &[u8] = b"set_rep_movie\0";

static EXPLANATION_PANES: [&[u8]; 11] = [
    b"set_txt_explanation_00\0",
    b"set_txt_explanation_01\0",
    b"set_txt_explanation_02\0",
    b"set_txt_explanation_03\0",
    b"set_txt_explanation_04\0",
    b"set_txt_explanation_05\0",
    b"set_txt_explanation_06\0",
    b"set_txt_explanation_07\0",
    b"set_txt_explanation_08\0",
    b"set_txt_explanation_09\0",
    b"set_txt_explanation_10\0",
];

static KEYHELP_PARTS: &[u8] = b"set_keyhelp\0";
static KEYHELP_ICON_PANE: &[u8] = b"set_txt_icon\0";
static KEYHELP_TEXT_PANE: &[u8] = b"set_txt_help\0";
static KEYHELP_ICON_LABEL: &[u8] = b"mnu_technique_scroll_guide\0";

const WRAP_COLUMNS: usize = 60;

const MAX_DESC_LINES: usize = 60;

const PREVIEW_SETTLE_FRAMES: u32 = 2;

static TABS: [(&str, Option<ModCategory>); 8] = [
    ("All", None),
    ("Fighter", Some(ModCategory::Fighter)),
    ("Stage", Some(ModCategory::Stage)),
    ("Effects", Some(ModCategory::Effects)),
    ("UI", Some(ModCategory::Ui)),
    ("Param", Some(ModCategory::Param)),
    ("Audio", Some(ModCategory::Audio)),
    ("Misc", Some(ModCategory::Misc)),
];

static ASSIGN_EXTRA: [(VirtualButton, Buttons); 2] = [(VirtualButton::Extra0, Buttons::L), (VirtualButton::Extra1, Buttons::R)];

#[repr(C)]
pub struct ArcadiaScene {
    base: Scene,

    preview: Option<MemoryTexture>,

    desc_scroller: Option<Scroller>,

    scroller: Option<Scroller>,
    screen: Screen,

    binder: RowBinder,

    desc_binder: RowBinder,

    desc_lines: Vec<String>,

    mods: Vec<ModEntry>,

    preset: HashSet<Hash40>,

    workspace_name: String,

    filtered: Vec<u32>,

    tab: usize,

    movie_root: Option<NonNull<Pane>>,
    movie_picture: Option<NonNull<Picture>>,

    keyhelp_icon: Option<NonNull<Pane>>,
    keyhelp_help: Option<NonNull<Pane>>,

    revealed: bool,
    exit_requested: bool,

    dirty: bool,

    last_index: i32,

    pending_index: i32,
    pending_frames: u32,
}

unsafe impl SceneImpl for ArcadiaScene {
    const NAME: &'static CStr = c"ArcadiaScene";

    fn create() -> Box<ArcadiaScene> {
        Box::new(ArcadiaScene {
            base: Scene::new(&VTABLE),
            preview: None,
            desc_scroller: None,
            scroller: None,

            screen: Screen::new("mod manager", LAYOUT_PATH, HeaderStyle::HEADER_HELP, labels::HDR_SCENE_MODS, false),
            binder: RowBinder::empty(),
            desc_binder: RowBinder::empty(),
            desc_lines: Vec::new(),
            mods: Vec::new(),
            preset: HashSet::new(),
            workspace_name: String::new(),
            filtered: Vec::new(),
            tab: 0,
            movie_root: None,
            movie_picture: None,
            keyhelp_icon: None,
            keyhelp_help: None,
            revealed: false,
            exit_requested: false,
            dirty: false,
            last_index: ITEM_NONE,
            pending_index: ITEM_NONE,
            pending_frames: 0,
        })
    }

    fn initialize(&mut self, ctx: &SceneInitContext) {
        self.screen.initialize(ctx);

        self.screen.set_view_count(2);

        let param = ctx.init_code().unwrap_or(0);
        self.load_workspace(param);
        self.mods = mods::scan(&self.preset);

        info!(
            "Mod manager on workspace '{}', {} mods, {} in the preset",
            self.workspace_name,
            self.mods.len(),
            self.preset.len()
        );
    }

    fn update(&mut self, _changer: &mut SceneChanger) {
        FRAME.fetch_add(1, Ordering::Relaxed);
        Self::free_retired();

        match self.screen.poll() {
            ScreenEvent::LayoutReady => {
                self.build_widgets();
                self.screen.activate();
                self.screen.reveal();
                self.revealed = true;
            },
            ScreenEvent::GaveUp => self.screen.leave(EXIT_BACK),
            ScreenEvent::Nothing => {},
        }

        if self.revealed {
            self.screen.tick();
            self.poll_tab();
            self.poll_input();
        }
    }

    fn request_exit(&mut self) {
        self.screen.request_exit();
        self.exit_requested = true;
    }

    fn is_exit_finished(&mut self) -> bool {
        if !self.exit_requested {
            return false;
        }

        match self.screen.exit_step() {
            ExitStep::Waiting => false,
            ExitStep::Teardown => {
                self.retire_preview();
                self.desc_scroller = None;
                self.scroller = None;
                self.movie_root = None;
                self.movie_picture = None;
                self.keyhelp_icon = None;
                self.keyhelp_help = None;
                self.screen.teardown();
                true
            },
            ExitStep::Done => true,
        }
    }
}

impl ArcadiaScene {
    fn load_workspace(&mut self, param: u32) {
        let name = if param == 0 {
            workspaces::active_name().unwrap_or_else(|_| "Default".to_string())
        } else {
            match workspaces::list() {
                Ok(list) => match list.get(param as usize - 1) {
                    Some(workspace) => workspace.name.clone(),
                    None => {
                        warn!("Mod manager: workspace {} is off the end of the list, using the active one", param);
                        workspaces::active_name().unwrap_or_else(|_| "Default".to_string())
                    },
                },
                Err(err) => {
                    warn!("Mod manager: could not read the workspace list, {}", err);
                    "Default".to_string()
                },
            }
        };

        self.preset = match workspaces::preset(&name) {
            Ok(preset) => preset,
            Err(err) => {
                warn!("Mod manager: could not read preset '{}', starting empty, {}", name, err);
                HashSet::new()
            },
        };
        self.workspace_name = name;
    }

    fn build_widgets(&mut self) {
        let this = self as *mut ArcadiaScene;

        let Some(cell) = self.screen.root_mut() else {
            return;
        };
        if let Some(view) = cell.view0() {
            for &(button, mask) in ASSIGN_STANDARD.iter().chain(ASSIGN_EXTRA.iter()) {
                view.set_assign(button, mask);
            }
        }

        match cell.view_at(1) {
            Some(view1) => {
                view1.clear_assign();
                view1.set_assign(VirtualButton::Up, Buttons::X);
                view1.set_assign(VirtualButton::Down, Buttons::Y);
            },
            None => warn!("Mod manager: no view 1, the description will not scroll"),
        }

        unsafe { cell.set_enable_input(true) };

        self.binder.vtable = &BINDER_VTABLE;
        self.binder.owner = this.cast();
        self.desc_binder.vtable = &DESC_BINDER_VTABLE;
        self.desc_binder.owner = this.cast();

        self.setup_keyhelp();
        self.find_movie_pane();

        self.filter_mods();
        self.build_scroller();
    }

    fn build_scroller(&mut self) {
        let binder_ptr = addr_of_mut!(self.binder);
        let Some(cell) = self.screen.root_mut() else {
            return;
        };
        let view_handle = cell.view_handle();
        let items = self.filtered.len() as i32;

        let scroller = self.scroller.get_or_insert_with(Scroller::new);
        if scroller.is_empty() {
            if !unsafe { scroller.setup_list(view_handle, SCROLL_GROUP, items, 0, true, binder_ptr) } {
                warn!("Mod manager scroller got nothing, group '{}' is probably not in the bflyt", debug_name(SCROLL_GROUP));
                return;
            }
        } else {
            unsafe { scroller.resize(items, 0) };
        }

        self.last_index = ITEM_NONE;
        if let Some(view) = self.root_view() {
            unsafe { write_text(view, TAB_PANE, TABS[self.tab].0) };
        }
        info!("Mod manager list up with {} rows in tab '{}'", items, TABS[self.tab].0);
    }

    fn filter_mods(&mut self) {
        let (_, wanted) = TABS[self.tab];
        self.filtered = self
            .mods
            .iter()
            .enumerate()
            .filter(|(_, entry)| wanted.is_none_or(|category| category == entry.category))
            .map(|(index, _)| index as u32)
            .collect();
    }

    fn switch_tab(&mut self, step: i32) {
        let count = TABS.len() as i32;
        self.tab = ((self.tab as i32 + step).rem_euclid(count)) as usize;

        self.filter_mods();

        let (name, _) = TABS[self.tab];
        if let Some(view) = self.root_view() {
            unsafe { write_text(view, TAB_PANE, name) };
        }

        self.hide_preview();
        self.pending_index = ITEM_NONE;
        self.pending_frames = 0;

        self.build_scroller();

        if self.filtered.is_empty() {
            self.last_index = ITEM_NONE;
            if let Some(view) = self.root_view() {
                unsafe {
                    write_text(view, TITLE_PANE, name);
                    write_text(view, CATEGORY_PANE, "");
                }
            }
            self.set_description(&[String::from("No mods in this category.")]);
        }

        if let Some(payload) = self.screen.root_mut().map(|cell| cell.view_payload()) {
            let tag: &[u8] = if self.tab == 0 { TAB_ANIM_ALL } else { TAB_ANIM_ONE };
            unsafe { play_animation(payload, tag, TAB_ANIM_END) };
        }

        debug!("Mod manager tab '{}' step {} rows {}", name, step, self.filtered.len());
    }

    fn poll_tab(&mut self) {
        let (prev, next) = {
            let Some(view) = self.screen.root_mut().and_then(|cell| cell.view0()) else {
                return;
            };
            if self.scroller.as_ref().is_none_or(Scroller::is_empty) {
                return;
            }
            (view.pressed(VirtualButton::Extra0), view.pressed(VirtualButton::Extra1))
        };

        if prev {
            self.switch_tab(-1);
        } else if next {
            self.switch_tab(1);
        }
    }

    fn poll_input(&mut self) {
        let (current, decided, cancel) = {
            let Some(view) = self.screen.root_mut().and_then(|cell| cell.view0()) else {
                return;
            };
            let Some(scroller) = self.scroller.as_ref() else {
                return;
            };
            if scroller.is_empty() {
                return;
            }
            unsafe { (scroller.current_index(), scroller.decided_index(), view.pressed(VirtualButton::Cancel)) }
        };

        if current != self.last_index {
            self.last_index = current;

            self.pending_index = current;
            self.pending_frames = 0;
            self.show_mod_text(current);
        } else if self.pending_index >= 0 {
            self.pending_frames += 1;
            if self.pending_frames >= PREVIEW_SETTLE_FRAMES {
                let row = self.pending_index;
                self.pending_index = ITEM_NONE;
                self.show_preview(row);
            }
        }

        if decided != ITEM_NONE {
            self.toggle_mod(decided);
        }

        if cancel {
            self.save_and_leave();
        }
    }

    fn toggle_mod(&mut self, item: i32) {
        let Some(&row) = self.filtered.get(usize::try_from(item).unwrap_or(usize::MAX)) else {
            return;
        };
        let Some(entry) = self.mods.get(row as usize) else {
            return;
        };
        let hash = entry.hash;

        let now_on = if self.preset.remove(&hash) {
            false
        } else {
            self.preset.insert(hash);
            true
        };
        self.mods[row as usize].enabled = now_on;
        self.dirty = true;

        self.rebind_row(item);
        debug!("Mod manager toggled '{}' -> {}", self.mods[row as usize].folder, if now_on { "ON" } else { "OFF" });
    }

    fn rebind_row(&mut self, item: i32) {
        let binder = addr_of_mut!(self.binder);
        let Some(scroller) = self.scroller.as_ref() else {
            return;
        };
        if let Some((row_view, extra)) = unsafe { scroller.row_for_item(item) } {
            unsafe { bind_row(binder, item, row_view, extra) };
        }
    }

    fn show_mod_text(&mut self, index: i32) {
        let Some(entry) = self.entry(index) else {
            return;
        };
        let display = entry.display_name.clone();
        let category = entry.category.label();
        let description = entry.description.clone();
        let version = entry.version.clone();
        let authors = entry.authors.clone();

        if let Some(view) = self.root_view() {
            unsafe {
                write_text(view, TITLE_PANE, &display);
                write_text(view, CATEGORY_PANE, category);
                let mut num = PaneHandle::get(view, TOPIC_NUM_PANE);
                if !num.is_empty() {
                    num.set_text_format_int(INT_FMT, index + 1);
                }
            }
        }

        let mut lines = wrap_lines(&description, WRAP_COLUMNS, MAX_DESC_LINES);
        lines.push(String::new());
        lines.push(format!("Version: {}", version));
        lines.push(format!("Authors: {}", authors));
        self.set_description(&lines);
    }

    fn set_description(&mut self, lines: &[String]) {
        self.desc_lines = lines.to_vec();

        if let Some(view) = self.root_view() {
            for (slot, pane) in EXPLANATION_PANES.iter().enumerate() {
                let text = self.desc_lines.get(slot).map(String::as_str).unwrap_or("");
                unsafe { write_text(view, pane, text) };
            }
        }

        self.build_desc_scroller();

        if self.desc_scroller.as_ref().is_none_or(Scroller::is_empty) {
            self.show_desc_hint(false);
            return;
        }

        let can_scroll = self.desc_scroller.as_ref().is_some_and(|scroller| unsafe { scroller.can_scroll() });
        self.show_desc_hint(can_scroll);
    }

    fn build_desc_scroller(&mut self) {
        let binder_ptr = addr_of_mut!(self.desc_binder);
        let Some(cell) = self.screen.root_mut() else {
            return;
        };
        let view_handle = cell.view_handle();
        let items = self.desc_lines.len() as i32;

        let mut params = ListScrollerParams::text_scroll(items, 1, 1);

        let scroller = self.desc_scroller.get_or_insert_with(Scroller::new);
        unsafe { scroller.setup(view_handle, DESC_GROUP, &mut params, binder_ptr) };

        if scroller.is_empty() {
            debug!("Mod manager: no '{}' group, the description gets no bar and no stick", debug_name(DESC_GROUP));
            return;
        }

        unsafe {
            scroller.set_focus(true);
            scroller.refresh_rows();
        }
        debug!("Mod manager description up with {} lines, scrolls {}", items, unsafe { scroller.can_scroll() });
    }

    fn show_desc_hint(&self, on: bool) {
        unsafe {
            if let Some(mut pane) = self.keyhelp_icon {
                pane.as_mut().set_visible(on);
            }
            if let Some(mut pane) = self.keyhelp_help {
                pane.as_mut().set_visible(on);
            }
        }
    }

    fn setup_keyhelp(&mut self) {
        let Some(view) = self.root_view() else {
            return;
        };

        let Some(mut parts) = (unsafe { PartsHandle::get_checked(view, KEYHELP_PARTS, 0) }) else {
            warn!("Mod manager: no '{}' part, no stick hint this run", debug_name(KEYHELP_PARTS));
            return;
        };
        let part_view = parts.view_handle();

        unsafe { write_text(part_view, KEYHELP_TEXT_PANE, "") };
        self.keyhelp_help = NonNull::new(unsafe { PaneHandle::get(part_view, KEYHELP_TEXT_PANE).pane() });

        let mut icon = unsafe { PaneHandle::get(part_view, KEYHELP_ICON_PANE) };
        self.keyhelp_icon = NonNull::new(unsafe { icon.pane() });
        if icon.is_empty() || unsafe { icon.text_box() }.is_null() {
            warn!(
                "Mod manager: no '{}' text box in '{}', no stick glyph this run",
                debug_name(KEYHELP_ICON_PANE),
                debug_name(KEYHELP_PARTS)
            );
            return;
        }

        unsafe { icon.set_text_label(KEYHELP_ICON_LABEL) };
        self.show_desc_hint(false);
        debug!("Mod manager stick glyph label set on '{}'", debug_name(KEYHELP_ICON_PANE));
    }

    fn find_movie_pane(&mut self) {
        let Some(view) = self.root_view() else {
            return;
        };

        let Some(mut parts) = (unsafe { PartsHandle::get_checked(view, MOVIE_PARTS, 0) }) else {
            warn!("Mod manager: no '{}', no preview this run", debug_name(MOVIE_PARTS));
            return;
        };
        let root = unsafe { parts.root_pane() };

        let pane = unsafe { PaneHandle::get(parts.view_handle(), MOVIE_PIC_PANE) };
        if pane.is_empty() {
            warn!("Mod manager: no '{}', no preview this run", debug_name(MOVIE_PIC_PANE));
            return;
        }
        let picture = unsafe { pane.picture() };

        self.movie_root = NonNull::new(root);
        self.movie_picture = NonNull::new(picture);
        self.set_preview_visible(false);

        debug!("Mod manager preview pane found, picture {:#x}", picture as u64);
    }

    fn show_preview(&mut self, index: i32) {
        let Some(movie_picture) = self.movie_picture else {
            return;
        };
        if self.movie_root.is_none() {
            return;
        }

        self.hide_preview();

        let Some(entry) = self.entry(index) else {
            return;
        };
        let Some(preview_path) = entry.preview.clone() else {
            debug!("Mod manager: row {} has no preview.webp", index);
            return;
        };

        let bytes = match fs::read(&preview_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                warn!("Mod manager: could not read '{}', {}", preview_path, err);
                return;
            },
        };

        let decoded = match webp_to_bntx(&bytes) {
            Ok(decoded) => decoded,
            Err(err) => {
                warn!("Mod manager: row {} preview decode failed, {}", index, err);
                return;
            },
        };

        let Some(texture) = (unsafe { MemoryTexture::create(&decoded.bntx) }) else {
            warn!("Mod manager: no memory for a {} byte preview", decoded.bntx.len());
            return;
        };

        if !unsafe { texture.slot_registered() } {
            warn!("Mod manager: row {} preview got no descriptor slot, staying hidden", index);
            return;
        }

        unsafe { texture.bind(movie_picture.as_ptr()) };
        self.set_preview_visible(true);
        self.preview = Some(texture);
        info!("Mod manager: row {} preview {}x{} shown", index, decoded.width, decoded.height);
    }

    fn hide_preview(&mut self) {
        self.set_preview_visible(false);
        self.retire_preview();
    }

    fn retire_preview(&mut self) {
        if let Some(texture) = self.preview.take() {
            if let Ok(mut retired) = RETIRED.lock() {
                retired.push_back((FRAME.load(Ordering::Relaxed), texture));
            }
        }
    }

    fn free_retired() {
        let now = FRAME.load(Ordering::Relaxed);
        if let Ok(mut retired) = RETIRED.lock() {
            while retired.front().is_some_and(|(at, _)| now.saturating_sub(*at) >= RETIRE_FRAMES) {
                retired.pop_front();
            }
        }
    }

    fn set_preview_visible(&mut self, visible: bool) {
        if let Some(mut root) = self.movie_root {
            unsafe { root.as_mut().set_visible(visible) };
        }
    }

    fn save_and_leave(&mut self) {
        if self.dirty {
            match workspaces::save_preset(&self.workspace_name, &self.preset) {
                Ok(()) => info!("Mod manager saved preset '{}', {} mods on", self.workspace_name, self.preset.len()),
                Err(err) => warn!("Mod manager: could not save preset '{}', {}", self.workspace_name, err),
            }
        }
        self.screen.leave(EXIT_BACK);
    }

    fn entry(&self, item: i32) -> Option<&ModEntry> {
        let row = *self.filtered.get(usize::try_from(item).ok()?)?;
        self.mods.get(row as usize)
    }

    fn root_view(&mut self) -> Option<*mut LayoutViewHandle> {
        self.screen.root_mut().map(|cell| cell.view_handle())
    }
}

unsafe extern "C" fn bind_row(binder: *mut RowBinder, item: i32, row_view: *mut LayoutViewHandle, _extra: *mut u8) {
    if binder.is_null() || row_view.is_null() {
        return;
    }
    let this = (*binder).owner as *const ArcadiaScene;
    if this.is_null() {
        return;
    }
    let scene = &*this;
    let Some(entry) = scene.entry(item) else {
        return;
    };

    {
        let title = PaneHandle::get(row_view, TITLE_PANE);
        if !title.is_empty() {
            title.set_text(&with_nul(&entry.display_name));
        }
    }
    {
        let mut num = PaneHandle::get(row_view, ROW_NUM_PANE);
        if !num.is_empty() {
            num.set_text_format_str(TEXT_FMT, &with_nul(if entry.enabled { ROW_ON } else { ROW_OFF }));
        }
    }

    let payload = (*row_view).payload();
    play_animation(payload, row_colour_tag(entry.category), ANIM_FRAME_STILL);
    play_animation(payload, ROW_ICONS_OFF, ANIM_FRAME_STILL);
}

unsafe extern "C" fn bind_desc_line(binder: *mut RowBinder, item: i32, row_view: *mut LayoutViewHandle, _extra: *mut u8) {
    if binder.is_null() || row_view.is_null() {
        return;
    }
    let this = (*binder).owner as *mut ArcadiaScene;
    if this.is_null() {
        return;
    }

    let scene = &mut *this;

    let Some(scroller) = scene.desc_scroller.as_ref() else {
        return;
    };
    let Some(slot) = scroller.slot_for_view(row_view) else {
        return;
    };
    let Some(&pane_name) = EXPLANATION_PANES.get(slot) else {
        return;
    };

    let Some(root_view) = scene.root_view() else {
        return;
    };

    let line = scene.desc_lines.get(usize::try_from(item).unwrap_or(usize::MAX)).map(String::as_str).unwrap_or("");

    let handle = PaneHandle::get(root_view, pane_name);
    if !handle.is_empty() && !handle.text_box().is_null() {
        handle.set_text(&with_nul(line));
    }
}

fn row_colour_tag(category: ModCategory) -> &'static [u8] {
    match category {
        ModCategory::Fighter => ROW_COL_FIGHTER,
        ModCategory::Stage => ROW_COL_STAGE,
        ModCategory::Effects => ROW_COL_EFFECTS,
        ModCategory::Ui => ROW_COL_UI,
        ModCategory::Param => ROW_COL_PARAM,
        ModCategory::Audio => ROW_COL_AUDIO,
        ModCategory::Misc => ROW_COL_MISC,
    }
}

fn wrap_lines(text: &str, columns: usize, max_lines: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for paragraph in text.split('\n') {
        let mut line = String::new();

        for word in paragraph.split_whitespace() {
            let mut word = word;

            while word.chars().count() > columns {
                let cut = word.char_indices().nth(columns).map_or(word.len(), |(at, _)| at);
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                }
                out.push(word[..cut].to_string());
                word = &word[cut..];
            }

            let width = line.chars().count();
            if width == 0 {
                line.push_str(word);
            } else if width + 1 + word.chars().count() <= columns {
                line.push(' ');
                line.push_str(word);
            } else {
                out.push(std::mem::replace(&mut line, word.to_string()));
            }
        }

        out.push(line);
    }

    while out.last().map(String::is_empty).unwrap_or(false) {
        out.pop();
    }

    out.truncate(max_lines);
    out
}
