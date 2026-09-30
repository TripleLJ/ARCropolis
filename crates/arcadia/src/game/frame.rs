use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    ptr::{self, addr_of_mut},
};

use super::{
    scene::FixedBaseString64,
    selector::{select_button_in_slot, set_selector_focus, ButtonSelector},
};
use crate::offsets;

#[derive(Clone, Copy)]
#[repr(i32)]
pub enum HeaderCategory {
    Melee = 0,
    Other = 1,
    Spirits = 2,
    Collection = 3,
    Options = 4,
    Online = 5,
    Help = 6,
    Standard = 7,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum MenuBackground {
    None = -1,
    Melee = 0,
    Spirits = 1,
    Other = 2,
    Collection = 3,
    Online = 4,
    Local = 5,
    Option = 6,
    Help = 7,
    Black = 9,
}

#[repr(i32)]
pub enum FooterButtonKind {
    None = 0,
    Medium100 = 5,
    Large100 = 10,
}

pub const MENU_BG_NONE: i32 = MenuBackground::None as i32;

const MENU_BG_LAST: i32 = MenuBackground::Black as i32;

const HEADER_SIZE_SMALL: i32 = 0;

pub const SHOW_ANIMATED: i32 = 0;
pub const SHOW_INSTANT: i32 = 1;

const FOOTER_SEL_MEDIUM: i32 = 1;
const FOOTER_SEL_LARGE: i32 = 2;
const FOOTER_SEL_PARK: i32 = 3;

#[skyline::from_offset(offsets::com_frame_set_title_text())]
unsafe fn com_frame_set_title_text(actor: *mut ComFrameActor, label: *const FixedBaseString64);

#[skyline::from_offset(offsets::com_frame_set_title_format())]
unsafe fn com_frame_set_title_format(actor: *mut ComFrameActor, label: *const FixedBaseString64);

#[skyline::from_offset(offsets::com_frame_set_scene_text())]
unsafe fn com_frame_set_scene_label(actor: *mut ComFrameActor, label: *const FixedBaseString64);

#[skyline::from_offset(offsets::com_frame_set_header_size())]
unsafe fn com_frame_set_header_size(actor: *mut ComFrameActor, size: i32);

#[skyline::from_offset(offsets::com_frame_set_footer_message())]
unsafe fn com_frame_set_footer_message(actor: *mut ComFrameActor, label: *const FixedBaseString64);

#[skyline::from_offset(offsets::com_frame_show_frame())]
unsafe fn com_frame_show_frame(manager: *mut ComFrameManager, view_buf: *mut ShowFrameOptions, visible: u8, mode: i32);

#[skyline::from_offset(offsets::com_frame_build_show_frame_options())]
unsafe fn com_frame_build_show_view(dest: *mut ShowFrameOptions, src: *mut ShowFrameOptions);

#[skyline::from_offset(offsets::com_frame_colour_header())]
unsafe fn com_frame_colour_header(actor: *mut ComFrameActor, category: i32);

#[skyline::from_offset(offsets::com_frame_play_footer_help_anim())]
unsafe fn com_frame_play_footer_anim(actor: *mut ComFrameActor, visible: u8, instant: u8);

#[skyline::from_offset(offsets::com_frame_set_footer_button())]
unsafe fn com_frame_set_footer_button(actor: *mut ComFrameActor, kind: i32, label: *const FixedBaseString64);

#[skyline::from_offset(offsets::com_frame_refresh_footer_layout())]
unsafe fn com_frame_refresh_footer(actor: *mut ComFrameActor);

#[skyline::from_offset(offsets::menu_background_set_background())]
unsafe fn com_bg_set_background(actor: *mut MenuBackgroundActor, index: i32);

#[repr(C)]
pub struct ShowFrameOptions {
    unk0: [u8; 4],
    pub category: i32,
    pub back_wanted: u8,
    unk1: [u8; 0x14 - 0x9],
    pub mode: FixedBaseString64,
    pub format: FixedBaseString64,
    pub scene: FixedBaseString64,
    unk2: [u8; 0x10f0 - 0xec],
    pub footer_button_kind: i32,
    unk3: [u8; 0x10f8 - 0x10f4],
    pub footer_button_label: FixedBaseString64,
    unk4: [u8; 0x1144 - 0x1140],
    pub footer_mode: i32,
    pub footer_label: FixedBaseString64,
    unk5: [u8; 0x2190 - 0x1190],
}

const _: () = {
    assert!(size_of::<ShowFrameOptions>() == 0x2190);
    assert!(std::mem::offset_of!(ShowFrameOptions, category) == 0x4);
    assert!(std::mem::offset_of!(ShowFrameOptions, back_wanted) == 0x8);
    assert!(std::mem::offset_of!(ShowFrameOptions, mode) == 0x14);
    assert!(std::mem::offset_of!(ShowFrameOptions, format) == 0x5c);
    assert!(std::mem::offset_of!(ShowFrameOptions, scene) == 0xa4);
    assert!(std::mem::offset_of!(ShowFrameOptions, footer_button_kind) == 0x10f0);
    assert!(std::mem::offset_of!(ShowFrameOptions, footer_mode) == 0x1144);
    assert!(std::mem::offset_of!(ShowFrameOptions, footer_label) == 0x1148);
};

#[repr(C)]
pub struct ComFrameActor {
    unk0: [u8; 0xb8],

    bar_layout: *mut u8,
    pub options: ShowFrameOptions,
    unk1: [u8; 0x2253 - 0x2250],
    pub back_shown: u8,
    unk2: [u8; 0x2257 - 0x2254],
    pub footer_ready: u8,
    unk3: u8,
    pub footer_shown: u8,
    unk4: [u8; 0x2268 - 0x225a],
    pub footer_selector: *mut ButtonSelector,
    pub decided_id: i32,
    unk5: [u8; 0x227c - 0x2274],
    pub frame_visible: u8,
    unk6: [u8; 3],
}

const _: () = {
    assert!(std::mem::offset_of!(ComFrameActor, bar_layout) == 0xb8);
    assert!(std::mem::offset_of!(ComFrameActor, options) == 0xc0);
    assert!(std::mem::offset_of!(ComFrameActor, back_shown) == 0x2253);
    assert!(std::mem::offset_of!(ComFrameActor, footer_ready) == 0x2257);
    assert!(std::mem::offset_of!(ComFrameActor, footer_shown) == 0x2259);
    assert!(std::mem::offset_of!(ComFrameActor, footer_selector) == 0x2268);
    assert!(std::mem::offset_of!(ComFrameActor, decided_id) == 0x2270);
    assert!(std::mem::offset_of!(ComFrameActor, frame_visible) == 0x227c);
};

#[repr(C)]
pub struct ComFrameManager {
    unk: [u8; 0x80],
    actor: *mut ComFrameActor,
}

#[repr(C)]
pub struct MenuBackgroundManager {
    unk: [u8; 0x80],
    actor: *mut MenuBackgroundActor,
}

#[repr(C)]
pub struct MenuBackgroundActor {
    unk: [u8; 8],
    index: i32,
}

#[repr(C)]
struct ComFrameOwner {
    unk: [u8; 0xb8],
    frame_manager: *mut ComFrameManager,
    bg_manager: *mut MenuBackgroundManager,
}

#[repr(C)]
struct ComFrameHolder {
    unk: u64,
    owner: *mut ComFrameOwner,
}

const _: () = {
    assert!(std::mem::offset_of!(ComFrameManager, actor) == 0x80);
    assert!(std::mem::offset_of!(MenuBackgroundManager, actor) == 0x80);
    assert!(std::mem::offset_of!(MenuBackgroundActor, index) == 8);
    assert!(std::mem::offset_of!(ComFrameOwner, frame_manager) == 0xb8);
    assert!(std::mem::offset_of!(ComFrameOwner, bg_manager) == 0xc0);
    assert!(std::mem::offset_of!(ComFrameHolder, owner) == 8);
};

#[derive(Clone, Copy)]
pub struct HeaderLabels {
    pub format: FixedBaseString64,
    pub mode: FixedBaseString64,
    pub scene: FixedBaseString64,
    pub background: i32,
}

pub struct ComFrame<'a> {
    manager: *mut ComFrameManager,
    background: *mut MenuBackgroundActor,
    _life: PhantomData<&'a mut ComFrameManager>,
}

impl ComFrame<'_> {
    pub unsafe fn get() -> Option<ComFrame<'static>> {
        let holder = *(patterns::offset_to_addr(offsets::g_com_frame_holder()) as *const *mut ComFrameHolder);
        let owner = holder.as_ref()?.owner;
        let owner = owner.as_ref()?;

        let manager = owner.frame_manager;
        if manager.is_null() || (*manager).actor.is_null() {
            return None;
        }

        let background = match owner.bg_manager.as_mut() {
            Some(bg) => bg.actor,
            None => ptr::null_mut(),
        };

        Some(ComFrame {
            manager,
            background,
            _life: PhantomData,
        })
    }

    fn actor(&self) -> *mut ComFrameActor {
        unsafe { (*self.manager).actor }
    }

    pub fn describe(&self) -> String {
        let actor = self.actor();
        unsafe {
            format!(
                "manager {:p} actor {:p} bar_layout {:p} footer_ready {} footer_shown {} visible {}",
                self.manager,
                actor,
                (*actor).bar_layout,
                (*actor).footer_ready,
                (*actor).footer_shown,
                (*actor).frame_visible
            )
        }
    }

    pub fn bar_ready(&self) -> bool {
        unsafe { !(*self.actor()).bar_layout.is_null() }
    }

    pub unsafe fn header_labels(&self) -> HeaderLabels {
        let options = addr_of_mut!((*self.actor()).options);
        HeaderLabels {
            format: (*options).format,
            mode: (*options).mode,
            scene: (*options).scene,
            background: self.background_index(),
        }
    }

    pub unsafe fn background_index(&self) -> i32 {
        match self.background.as_ref() {
            Some(actor) => actor.index,
            None => MENU_BG_NONE,
        }
    }

    pub fn set_header_labels(&self, labels: &HeaderLabels) {
        let actor = self.actor();
        unsafe {
            com_frame_set_title_format(actor, &labels.format);
            com_frame_set_title_text(actor, &labels.mode);
            com_frame_set_header_size(actor, HEADER_SIZE_SMALL);
            com_frame_set_scene_label(actor, &labels.scene);
        }
    }

    pub unsafe fn set_background(&self, background: MenuBackground) -> i32 {
        self.set_background_index(background as i32)
    }

    pub unsafe fn set_background_index(&self, index: i32) -> i32 {
        let Some(actor) = self.background.as_mut() else {
            warn!("No menu background actor, the header band stays as it is");
            return MENU_BG_NONE;
        };
        let previous = actor.index;
        if !(MENU_BG_NONE..=MENU_BG_LAST).contains(&previous) {
            warn!("Menu background actor at {:p} holds index {}, not touching it", actor, previous);
            return MENU_BG_NONE;
        }
        if previous != index {
            com_bg_set_background(actor, index);
        }
        previous
    }

    pub fn set_footer_label(&self, label: &FixedBaseString64) {
        unsafe { com_frame_set_footer_message(self.actor(), label) }
    }

    pub fn want_back_button(&self, category: HeaderCategory) {
        let actor = self.actor();
        unsafe {
            (*actor).options.category = category as i32;
            (*actor).options.back_wanted = 1;
            (*actor).back_shown = 0;
        }
    }

    pub fn colour_header(&self, category: HeaderCategory) {
        unsafe { com_frame_colour_header(self.actor(), category as i32) }
    }

    pub fn show(&self, visible: bool, mode: i32) {
        let actor = self.actor();
        let mut scratch: Box<MaybeUninit<ShowFrameOptions>> = Box::new_uninit();
        unsafe {
            com_frame_build_show_view(scratch.as_mut_ptr().cast(), addr_of_mut!((*actor).options));
            com_frame_show_frame(self.manager, scratch.as_mut_ptr().cast(), visible as u8, mode);
        }
    }

    pub fn footer_ready(&self) -> bool {
        unsafe { (*self.actor()).footer_ready != 0 }
    }

    pub fn visible(&self) -> bool {
        unsafe { (*self.actor()).frame_visible != 0 }
    }

    pub fn show_footer(&self, visible: bool, instant: bool) {
        self.show_footer_forced(visible, instant, false)
    }

    pub fn show_footer_forced(&self, visible: bool, instant: bool, force: bool) {
        let actor = self.actor();
        let mode = visible as i32;

        unsafe {
            let shown = (*actor).footer_shown != 0;
            if !force && (*actor).options.footer_mode == mode && shown == visible {
                return;
            }

            (*actor).options.footer_mode = mode;
            com_frame_play_footer_anim(actor, visible as u8, instant as u8);
            com_frame_refresh_footer(actor);
        }
    }

    pub fn set_footer_button(&self, kind: i32, label: &FixedBaseString64) {
        unsafe { com_frame_set_footer_button(self.actor(), kind, label) }
    }

    pub fn footer_button_kind(&self) -> i32 {
        unsafe { (*self.actor()).options.footer_button_kind }
    }

    pub fn footer_button_label(&self) -> FixedBaseString64 {
        unsafe { (*self.actor()).options.footer_button_label }
    }

    pub fn highlight_footer_button(&self, on: bool) {
        let actor = self.actor();
        let button = if self.footer_button_kind() > FooterButtonKind::Medium100 as i32 { FOOTER_SEL_LARGE } else { FOOTER_SEL_MEDIUM };

        unsafe {
            let slot = addr_of_mut!((*actor).footer_selector);
            set_selector_focus(*slot, on);
            select_button_in_slot(slot, if on { button } else { FOOTER_SEL_PARK }, 0);
        }
    }

    pub fn footer_decided(&self) -> bool {
        let id = unsafe { (*self.actor()).decided_id };
        id == FOOTER_SEL_MEDIUM || id == FOOTER_SEL_LARGE
    }

    pub fn forget_footer_label(&self) {
        unsafe { (*self.actor()).options.footer_label = FixedBaseString64::new("") }
    }
}
