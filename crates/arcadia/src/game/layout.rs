use std::{
    arch::naked_asm,
    mem::offset_of,
    ptr::{self, addr_of_mut, NonNull},
};

use smash_arc::Hash40;

use super::{alloc, loading::LoadingList};
use crate::offsets;

#[derive(Clone, Copy)]
pub struct Buttons(pub u32);

impl Buttons {
    pub const UP: Buttons = Buttons(0x1);
    pub const RIGHT: Buttons = Buttons(0x2);
    pub const DOWN: Buttons = Buttons(0x4);
    pub const LEFT: Buttons = Buttons(0x8);
    pub const X: Buttons = Buttons(0x10);
    pub const A: Buttons = Buttons(0x20);
    pub const B: Buttons = Buttons(0x40);
    pub const Y: Buttons = Buttons(0x80);
    pub const L: Buttons = Buttons(0x100);
    pub const R: Buttons = Buttons(0x200);

    pub const SLIDE_UP: Buttons = Buttons(0x10_0000);
    pub const SLIDE_RIGHT: Buttons = Buttons(0x20_0000);
    pub const SLIDE_DOWN: Buttons = Buttons(0x40_0000);
    pub const SLIDE_LEFT: Buttons = Buttons(0x80_0000);

    pub const NAV_UP: Buttons = Buttons(Self::UP.0 | Self::SLIDE_UP.0);
    pub const NAV_DOWN: Buttons = Buttons(Self::DOWN.0 | Self::SLIDE_DOWN.0);
    pub const NAV_LEFT: Buttons = Buttons(Self::LEFT.0 | Self::SLIDE_LEFT.0);
    pub const NAV_RIGHT: Buttons = Buttons(Self::RIGHT.0 | Self::SLIDE_RIGHT.0);
}

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum VirtualButton {
    Up = 0,
    Down = 1,
    Left = 2,
    Right = 3,
    Decide = 4,
    Cancel = 5,
    Extra0 = 6,
    Extra1 = 7,
}

pub const ASSIGN_STANDARD: [(VirtualButton, Buttons); 6] = [
    (VirtualButton::Up, Buttons::NAV_UP),
    (VirtualButton::Down, Buttons::NAV_DOWN),
    (VirtualButton::Left, Buttons::NAV_LEFT),
    (VirtualButton::Right, Buttons::NAV_RIGHT),
    (VirtualButton::Decide, Buttons::A),
    (VirtualButton::Cancel, Buttons::B),
];

pub const ANIM_FRAME_STILL: f32 = 1.0;

pub static TEXT_FMT: &[u8] = b"%s\0";

pub static INT_FMT: &[u8] = b"%d\0";

pub fn with_nul(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    bytes
}

pub fn debug_name(bytes: &[u8]) -> &str {
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
    core::str::from_utf8(&bytes[..end]).unwrap_or("<unreadable>")
}

#[skyline::from_offset(offsets::layout_root_create())]
unsafe fn layout_root_create(cell: *mut *mut LayoutRootHandle);

#[skyline::from_offset(offsets::layout_root_build_from_layout_arc())]
unsafe fn build_from_layout_arc(cell_field: *mut *mut LayoutRootHandle, list: *mut super::loading::LoadingListData, path: *const u64, view_count: i32);

#[skyline::from_offset(offsets::layout_root_set_enable_input())]
unsafe fn layout_root_set_enable_input(root: *mut LayoutRoot, enable: bool);

#[skyline::from_offset(offsets::layout_root_activate())]
unsafe fn layout_root_activate(handle: *mut LayoutRootHandle, layer: i32, draw_order: i32);

#[skyline::from_offset(offsets::layout_root_destroy())]
unsafe fn layout_root_destroy(cell: *mut *mut LayoutRootHandle);

#[skyline::from_offset(offsets::text_box_set_text_string())]
unsafe fn set_text_string(text_box: *mut TextBox, text: *const u8);

#[skyline::from_offset(offsets::msbt_get_by_label())]
unsafe fn msbt_get_by_label(pane_handle: *mut PaneRaw, label: *const u8);

#[skyline::from_offset(offsets::pane_set_text_format())]
unsafe fn set_text_format_str(pane_handle: *mut PaneRaw, fmt: *const u8, arg: *const u8);

#[skyline::from_offset(offsets::pane_set_text_format())]
unsafe fn set_text_format_int(pane_handle: *mut PaneRaw, fmt: *const u8, arg: i32);

#[skyline::from_offset(offsets::view_play_animation())]
unsafe fn view_play_animation(view_payload: *mut ViewPayload, tag: *const u8, frame: f32);

#[skyline::from_offset(offsets::pane_restore_original_texture())]
unsafe fn restore_original_texture_raw(pane_payload: *mut PanePayload);

#[unsafe(naked)]
unsafe extern "C" fn get_pane_shim(_out: *mut PaneRaw, _view: *mut LayoutViewHandle, _name: *const u8, _target: *const ()) {
    naked_asm!("mov x8, x0", "mov x0, x1", "mov x1, x2", "br x3")
}

#[unsafe(naked)]
unsafe extern "C" fn get_parts_shim(_out: *mut PartsRaw, _view: *mut LayoutViewHandle, _name_fmt: *const u8, _arg: u64, _target: *const ()) {
    naked_asm!("mov x8, x0", "mov x0, x1", "mov x1, x2", "mov x2, x3", "br x4")
}

const ASSIGN_MASK: u32 = 0xbfff_ffff;

const BUTTON_PRESSED: u32 = 2;
const BUTTON_HELD: u32 = 0x10;

#[repr(C)]
pub struct LayoutView {
    assign: [u32; 51],
    unk0: [u8; 0x198 - 0xcc],

    pressed: [[u32; 51]; 3],
    unk1: [u8; 0x4aa - 0x3fc],

    input_enabled: u8,
}

impl LayoutView {
    pub fn set_assign(&mut self, button: VirtualButton, mask: Buttons) {
        self.assign[button as usize] = mask.0 & ASSIGN_MASK;
    }

    pub fn pressed(&self, button: VirtualButton) -> bool {
        self.pressed[0][button as usize] & BUTTON_PRESSED != 0
    }

    pub fn held(&self, button: VirtualButton) -> bool {
        self.pressed[0][button as usize] & BUTTON_HELD != 0
    }

    pub fn input_enabled(&self) -> bool {
        self.input_enabled != 0
    }

    pub fn clear_assign(&mut self) {
        unsafe { ptr::write_bytes(self as *mut LayoutView as *mut u8, 0, offset_of!(LayoutView, pressed)) }
    }
}

const _: () = {
    assert!(std::mem::offset_of!(LayoutView, pressed) == 0x198);
    assert!(std::mem::offset_of!(LayoutView, input_enabled) == 0x4aa);
};

#[repr(C)]
pub struct LayoutViewHandle {
    unk: *mut u8,
    payload: *mut ViewPayload,
}

#[repr(C)]
pub struct ViewPayload {
    _private: [u8; 0],
}

impl LayoutViewHandle {
    pub fn payload(&self) -> *mut ViewPayload {
        self.payload
    }
}

#[repr(C)]
pub struct LayoutRoot {
    unk0: [u8; 0x458],
    layout: *mut u8,
    input_handlers: super::CppVector<*mut u8>,
    unk1: [u8; 0x600 - 0x478],
    view0: *mut LayoutView,
    extra_views: super::CppVector<LayoutViewHandle>,
    unk2: [u8; 0x648 - 0x620],
    layer: i32,
    draw_order: i32,
    view_mode: i32,
    unk3: [u8; 0x660 - 0x654],
}

const _: () = {
    assert!(size_of::<LayoutRoot>() == 0x660);
    assert!(std::mem::offset_of!(LayoutRoot, view0) == 0x600);
    assert!(std::mem::offset_of!(LayoutRoot, layer) == 0x648);
};

#[repr(C)]
pub struct LayoutRootHandle {
    root: *mut LayoutRoot,
    view: LayoutViewHandle,
}

pub struct LayoutRootCell {
    cell: NonNull<*mut LayoutRootHandle>,
}

impl LayoutRootCell {
    pub unsafe fn create() -> Option<LayoutRootCell> {
        let slot = alloc::je_aligned_alloc(16, 8).cast::<*mut LayoutRootHandle>();
        let cell = NonNull::new(slot)?;

        *cell.as_ptr() = ptr::null_mut();
        layout_root_create(cell.as_ptr());

        Some(LayoutRootCell { cell })
    }

    pub fn handle(&self) -> *mut LayoutRootHandle {
        unsafe { *self.cell.as_ptr() }
    }

    pub fn cell_slot(&self) -> *const *mut LayoutRootHandle {
        std::ptr::addr_of!(self.cell) as *const *mut LayoutRootHandle
    }

    fn handle_ref(&self) -> Option<&mut LayoutRootHandle> {
        unsafe { self.handle().as_mut() }
    }

    fn root(&self) -> *mut LayoutRoot {
        match self.handle_ref() {
            Some(handle) => handle.root,
            None => ptr::null_mut(),
        }
    }

    pub fn view_handle(&self) -> *mut LayoutViewHandle {
        match self.handle_ref() {
            Some(handle) => addr_of_mut!(handle.view),
            None => ptr::null_mut(),
        }
    }

    pub fn view_payload(&self) -> *mut ViewPayload {
        match self.handle_ref() {
            Some(handle) => handle.view.payload,
            None => ptr::null_mut(),
        }
    }

    pub unsafe fn build(&mut self, list: &LoadingList, hash: &Hash40, views: i32) {
        let cell_field = addr_of_mut!(self.cell) as *mut *mut LayoutRootHandle;
        build_from_layout_arc(cell_field, list.as_ptr(), &hash.0 as *const u64, views);
    }

    pub unsafe fn activate(&self, layer: i32, draw_order: i32) {
        layout_root_activate(self.handle(), layer, draw_order);
    }

    pub unsafe fn set_enable_input(&self, enable: bool) {
        layout_root_set_enable_input(self.root(), enable);
    }

    pub fn view0(&mut self) -> Option<&mut LayoutView> {
        let root = self.root();
        if root.is_null() {
            return None;
        }
        unsafe { (*root).view0.as_mut() }
    }

    pub fn view_at(&mut self, index: usize) -> Option<&mut LayoutView> {
        if index == 0 {
            return self.view0();
        }

        let root = self.root();
        if root.is_null() {
            return None;
        }

        unsafe {
            let handle = (*root).extra_views.as_slice().get(index - 1)?;
            (handle.payload() as *mut LayoutView).as_mut()
        }
    }
}

impl Drop for LayoutRootCell {
    fn drop(&mut self) {
        unsafe {
            layout_root_destroy(self.cell.as_ptr());
            alloc::free_default(self.cell.as_ptr().cast::<u8>());
        }
    }
}

const PANE_TRANSLATE: usize = 0x30;

const PANE_FLAGS: usize = 0x58;
const PANE_FLAG_DIRTY: u8 = 0x10;

#[repr(C)]
pub struct Pane {
    unk0: [u8; PANE_TRANSLATE],
    translate: [f32; 3],
    rotate: [f32; 3],
    scale: [f32; 2],
    size: [f32; 2],
    flags: u8,
}

const _: () = assert!(std::mem::offset_of!(Pane, flags) == PANE_FLAGS);

impl Pane {
    pub fn set_visible(&mut self, visible: bool) {
        self.flags = (self.flags & 0xfe) | visible as u8;
    }

    pub fn set_translate(&mut self, x: f32, y: f32) {
        self.translate[0] = x;
        self.translate[1] = y;
        self.flags |= PANE_FLAG_DIRTY;
    }

    pub fn translate(&self) -> (f32, f32) {
        (self.translate[0], self.translate[1])
    }
}

const _: () = {
    assert!(std::mem::offset_of!(Pane, translate) == 0x30);
    assert!(std::mem::offset_of!(Pane, flags) == 0x58);
};

#[repr(C)]
pub struct PanePayload {
    pane: *mut Pane,
    picture: *mut Picture,
    text_box: *mut TextBox,
}

#[repr(C)]
pub struct Picture {
    _private: [u8; 0],
}

#[repr(C)]
pub struct TextBox {
    _private: [u8; 0],
}

#[repr(C)]
pub struct PaneRaw {
    unk: *mut u8,
    payload: *mut PanePayload,
}

pub struct PaneHandle {
    raw: PaneRaw,
}

impl PaneHandle {
    pub unsafe fn get(view: *mut LayoutViewHandle, name: &[u8]) -> PaneHandle {
        let mut handle = PaneHandle {
            raw: PaneRaw {
                unk: ptr::null_mut(),
                payload: ptr::null_mut(),
            },
        };
        get_pane_shim(&mut handle.raw, view, name.as_ptr(), patterns::offset_to_addr(offsets::view_get_pane()));
        handle
    }

    fn payload(&self) -> *mut PanePayload {
        self.raw.payload
    }

    pub fn is_empty(&self) -> bool {
        self.payload().is_null()
    }

    pub unsafe fn pane(&self) -> *mut Pane {
        match self.payload().as_ref() {
            Some(payload) => payload.pane,
            None => ptr::null_mut(),
        }
    }

    pub unsafe fn text_box(&self) -> *mut TextBox {
        match self.payload().as_ref() {
            Some(payload) => payload.text_box,
            None => ptr::null_mut(),
        }
    }

    pub unsafe fn picture(&self) -> *mut Picture {
        match self.payload().as_ref() {
            Some(payload) => payload.picture,
            None => ptr::null_mut(),
        }
    }

    pub unsafe fn set_text(&self, text: &[u8]) {
        let text_box = self.text_box();
        if !text_box.is_null() {
            set_text_string(text_box, text.as_ptr());
        }
    }

    pub unsafe fn set_text_label(&mut self, label: &[u8]) {
        msbt_get_by_label(&mut self.raw, label.as_ptr());
    }

    pub unsafe fn set_text_format_str(&mut self, fmt: &[u8], arg: &[u8]) {
        set_text_format_str(&mut self.raw, fmt.as_ptr(), arg.as_ptr());
    }

    pub unsafe fn set_text_format_int(&mut self, fmt: &[u8], arg: i32) {
        set_text_format_int(&mut self.raw, fmt.as_ptr(), arg);
    }
}

impl Drop for PaneHandle {
    fn drop(&mut self) {
        unsafe { alloc::free_default(self.payload().cast::<u8>()) }
    }
}

#[repr(C)]
pub struct LayoutInner {
    unk: [u8; 0x18],
    pub pane: *mut Pane,
}

const _: () = assert!(offset_of!(LayoutInner, pane) == 0x18);

#[repr(C)]
pub struct PartsPayload {
    unk: *mut u8,
    layout: *mut u8,
    unk2: *mut u8,
    inner: *mut u8,
}

#[repr(C)]
pub struct PartsRaw {
    unk: *mut u8,
    payload: *mut PartsPayload,
}

pub struct PartsHandle {
    raw: PartsRaw,
}

impl PartsHandle {
    pub unsafe fn get(view: *mut LayoutViewHandle, name_fmt: &[u8], arg: u64) -> PartsHandle {
        let mut handle = PartsHandle {
            raw: PartsRaw {
                unk: ptr::null_mut(),
                payload: ptr::null_mut(),
            },
        };
        get_parts_shim(&mut handle.raw, view, name_fmt.as_ptr(), arg, patterns::offset_to_addr(offsets::view_get_parts()));
        handle
    }

    fn payload(&self) -> *mut PartsPayload {
        self.raw.payload
    }

    pub unsafe fn get_checked(view: *mut LayoutViewHandle, name_fmt: &[u8], arg: u64) -> Option<PartsHandle> {
        let handle = PartsHandle::get(view, name_fmt, arg);
        (!handle.root_pane().is_null()).then_some(handle)
    }

    pub unsafe fn root_pane(&self) -> *mut Pane {
        let payload = self.payload();
        if payload.is_null() {
            return ptr::null_mut();
        }
        match ((*payload).layout as *mut LayoutInner).as_ref() {
            Some(layout) => layout.pane,
            None => ptr::null_mut(),
        }
    }

    pub fn view_handle(&mut self) -> *mut LayoutViewHandle {
        &mut self.raw as *mut PartsRaw as *mut LayoutViewHandle
    }
}

impl Drop for PartsHandle {
    fn drop(&mut self) {
        unsafe {
            let payload = self.payload();
            if payload.is_null() {
                return;
            }
            let inner = (*payload).inner;
            (*payload).inner = ptr::null_mut();
            alloc::free_default(inner);
            alloc::free_default(payload.cast::<u8>());
        }
    }
}

pub unsafe fn write_text(view: *mut LayoutViewHandle, pane: &[u8], text: &str) {
    let handle = PaneHandle::get(view, pane);
    if handle.text_box().is_null() {
        return;
    }
    handle.set_text(&with_nul(text));
}

pub unsafe fn show_pane(view: *mut LayoutViewHandle, pane: &[u8], visible: bool) {
    let handle = PaneHandle::get(view, pane);
    if let Some(pane) = handle.pane().as_mut() {
        pane.set_visible(visible);
    }
}

pub unsafe fn play_animation(view_payload: *mut ViewPayload, tag: &[u8], frame: f32) {
    if view_payload.is_null() {
        return;
    }
    view_play_animation(view_payload, tag.as_ptr(), frame);
}

pub unsafe fn restore_original_texture(pane_payload: *mut PanePayload) {
    if !pane_payload.is_null() {
        restore_original_texture_raw(pane_payload);
    }
}
