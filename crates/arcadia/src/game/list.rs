use std::ptr::{self, addr_of_mut};

use super::{alloc, layout::{LayoutViewHandle, Pane}, vtable_call, StdFunction, VTABLE_SLOT_DELETING_DTOR, VTABLE_SLOT_SET_FOCUS};
use crate::offsets;

pub use super::INDEX_NONE as ITEM_NONE;

const EDGE_DOWN: i32 = 2;

pub const JUMP_IMMEDIATE: u32 = 1;

pub const CLAMP_TO_LIST: u32 = 1;

#[skyline::from_offset(offsets::list_scroller_setup())]
unsafe fn list_scroller_setup(holder: *mut *mut ListScroller, view_handle: *mut LayoutViewHandle, group_name: *const u8, params: *const ListScrollerParams, binder: *mut RowBinder);

#[skyline::from_offset(offsets::list_scroller_set_current_index())]
unsafe fn list_scroller_set_current_index(holder: *mut *mut ListScroller, index: i32, immediate: u32, validate: u32);

#[skyline::from_offset(offsets::list_scroller_refresh_visible_rows())]
unsafe fn list_scroller_refresh_visible_rows(scroller: *mut ListScroller, force: u64);

#[repr(C, align(16))]
pub struct ListScrollerParams {
    view: i32,
    item_count: i32,
    row_height: f32,
    unk0c: f32,
    unk10: f32,
    unk14: u8,
    cursor: u8,
    cursor_wrap: u8,
    unk17: [u8; 2],
    unk19: u8,
    unk1a: u8,
    unk1b: u8,
    no_wrap_top: u8,
    no_wrap_bottom: u8,
    unk1e: [u8; 3],
    stick: u8,
    unk22: [u8; 0x30 - 0x22],
    sounds: [StdFunction<SoundCallableVtable>; 5],
}

const _: () = assert!(size_of::<ListScrollerParams>() == 0x120);

impl ListScrollerParams {
    pub fn new(items: i32) -> ListScrollerParams {
        let mut params = ListScrollerParams::zeroed();
        params.item_count = items;
        params.row_height = 16.0;
        params.unk0c = 50.0;
        params.unk10 = 1.0;
        params.cursor = 1;
        params.cursor_wrap = 1;
        params.unk1a = 1;
        params.no_wrap_bottom = 1;
        params
    }

    pub fn text_scroll(items: i32, view: i32, stick: u8) -> ListScrollerParams {
        let mut params = ListScrollerParams::zeroed();
        params.view = view;
        params.item_count = items;
        params.row_height = 16.0;
        params.unk0c = 50.0;
        params.unk10 = 1.0;
        params.unk19 = 1;
        params.unk1b = 1;
        params.stick = stick;
        params
    }

    fn zeroed() -> ListScrollerParams {
        ListScrollerParams {
            view: 0,
            item_count: 0,
            row_height: 0.0,
            unk0c: 0.0,
            unk10: 0.0,
            unk14: 0,
            cursor: 0,
            cursor_wrap: 0,
            unk17: [0; 2],
            unk19: 0,
            unk1a: 0,
            unk1b: 0,
            no_wrap_top: 0,
            no_wrap_bottom: 0,
            unk1e: [0; 3],
            stick: 0,
            unk22: [0; 0x30 - 0x22],
            sounds: [StdFunction::zeroed(), StdFunction::zeroed(), StdFunction::zeroed(), StdFunction::zeroed(), StdFunction::zeroed()],
        }
    }

    pub fn wrap_at_bottom(&mut self) {
        self.no_wrap_bottom = 0;
    }

    unsafe fn point_at_self(&mut self) {
        for sound in &mut self.sounds {
            sound.point_at_self(&SOUND_CALLABLE_VTABLE);
        }
    }
}

#[repr(C)]
pub struct SoundCallableVtable {
    dtor: unsafe extern "C" fn(*mut u8),
    deleting_dtor: unsafe extern "C" fn(*mut u8),
    clone: unsafe extern "C" fn(*mut u8) -> *mut u8,
    clone_into: unsafe extern "C" fn(*mut u8, *mut u8),
    destroy: unsafe extern "C" fn(*mut u8),
    destroy_deallocate: unsafe extern "C" fn(*mut u8),
    call: unsafe extern "C" fn(*mut u8, *mut u8, *mut u8, *mut u8) -> u64,
}

const _: () = assert!(size_of::<SoundCallableVtable>() == 7 * 8);

static SOUND_CALLABLE_VTABLE: SoundCallableVtable = SoundCallableVtable {
    dtor: sound_destroy,
    deleting_dtor: sound_destroy,
    clone: sound_clone,
    clone_into: sound_clone_into,
    destroy: sound_destroy,
    destroy_deallocate: sound_destroy_deallocate,
    call: sound_call,
};

unsafe extern "C" fn sound_destroy(_this: *mut u8) {}

unsafe extern "C" fn sound_clone(_this: *mut u8) -> *mut u8 {
    let copy = alloc::je_aligned_alloc(16, 8);
    if !copy.is_null() {
        *(copy as *mut usize) = &SOUND_CALLABLE_VTABLE as *const SoundCallableVtable as usize;
    }
    copy
}

unsafe extern "C" fn sound_clone_into(_this: *mut u8, dest: *mut u8) {
    *(dest as *mut usize) = &SOUND_CALLABLE_VTABLE as *const SoundCallableVtable as usize;
}

unsafe extern "C" fn sound_destroy_deallocate(this: *mut u8) {
    alloc::free_default(this);
}

unsafe extern "C" fn sound_call(_this: *mut u8, _a: *mut u8, _b: *mut u8, _c: *mut u8) -> u64 {
    0
}

#[repr(C)]
pub struct RowBinder {
    pub vtable: *const RowBinderVtable,
    pub owner: *mut (),
}

pub type BindRowFn = unsafe extern "C" fn(*mut RowBinder, i32, *mut LayoutViewHandle, *mut u8);

#[repr(C)]
pub struct RowBinderVtable {
    pub dtor: unsafe extern "C" fn(*mut RowBinder),
    pub deleting_dtor: unsafe extern "C" fn(*mut RowBinder),
    pub bind_row: BindRowFn,
    pub on_refresh_begin: unsafe extern "C" fn(*mut RowBinder),
    pub on_refresh_end: unsafe extern "C" fn(*mut RowBinder),
    pub adjust_index: unsafe extern "C" fn(*mut RowBinder, i32) -> i32,
}

const _: () = {
    assert!(size_of::<RowBinder>() == 0x10);
    assert!(size_of::<RowBinderVtable>() == 6 * 8);
};

unsafe extern "C" fn binder_dtor(_this: *mut RowBinder) {}
unsafe extern "C" fn binder_refresh_begin(_this: *mut RowBinder) {}
unsafe extern "C" fn binder_refresh_end(_this: *mut RowBinder) {}
unsafe extern "C" fn binder_adjust_index(_this: *mut RowBinder, index: i32) -> i32 {
    index
}

impl RowBinder {
    pub const fn empty() -> RowBinder {
        RowBinder {
            vtable: ptr::null(),
            owner: ptr::null_mut(),
        }
    }
}

impl RowBinderVtable {
    pub const fn new(bind_row: BindRowFn) -> RowBinderVtable {
        RowBinderVtable {
            dtor: binder_dtor,
            deleting_dtor: binder_dtor,
            bind_row,
            on_refresh_begin: binder_refresh_begin,
            on_refresh_end: binder_refresh_end,
            adjust_index: binder_adjust_index,
        }
    }
}

#[repr(C)]
pub struct ScrollerRow {
    unk0: u64,
    view: LayoutViewHandle,
    extra: u64,
    pane_holder: *mut *mut Pane,
    unk28: [u8; 0x30 - 0x28],
    item_index: i32,
}

const _: () = {
    assert!(std::mem::offset_of!(ScrollerRow, view) == 0x08);
    assert!(std::mem::offset_of!(ScrollerRow, extra) == 0x18);
    assert!(std::mem::offset_of!(ScrollerRow, pane_holder) == 0x20);
    assert!(std::mem::offset_of!(ScrollerRow, item_index) == 0x30);
};

#[repr(C)]
pub struct ListScroller {
    vtable: *const usize,
    unk08: [u8; 0x14 - 8],
    enabled: u8,
    active: u8,
    focused: u8,
    unk17: [u8; 0x34 - 0x17],
    params_item_count: i32,
    unk38: [u8; 0x160 - 0x38],
    binder: *mut RowBinder,
    row_pool: *mut *mut ScrollerRow,
    unk170: [u8; 0x288 - 0x170],
    item_count: i32,
    column_count: i32,
    page_count: i32,
    unk294: [u8; 0x29c - 0x294],
    top_index: i32,
    unk2a0: [u8; 0x2a4 - 0x2a0],
    current_index: i32,
    unk2a8: [u8; 0x2b4 - 0x2a8],
    decided_index: i32,
    edge_direction: i32,
    edge_pressed: u8,
    unk2bd: [u8; 0x2c0 - 0x2bd],
    pool_size: i32,
    row_count: i32,
    unk2c8: [u8; 0x315 - 0x2c8],
    can_scroll: u8,
}

const _: () = {
    assert!(std::mem::offset_of!(ListScroller, row_pool) == 0x168);
    assert!(std::mem::offset_of!(ListScroller, params_item_count) == 0x34);
    assert!(std::mem::offset_of!(ListScroller, item_count) == 0x288);
    assert!(std::mem::offset_of!(ListScroller, page_count) == 0x290);
    assert!(std::mem::offset_of!(ListScroller, top_index) == 0x29c);
    assert!(std::mem::offset_of!(ListScroller, pool_size) == 0x2c0);
    assert!(std::mem::offset_of!(ListScroller, column_count) == 0x28c);
    assert!(std::mem::offset_of!(ListScroller, current_index) == 0x2a4);
    assert!(std::mem::offset_of!(ListScroller, edge_direction) == 0x2b8);
    assert!(std::mem::offset_of!(ListScroller, edge_pressed) == 0x2bc);
    assert!(std::mem::offset_of!(ListScroller, decided_index) == 0x2b4);
    assert!(std::mem::offset_of!(ListScroller, row_count) == 0x2c4);
    assert!(std::mem::offset_of!(ListScroller, can_scroll) == 0x315);
};

pub struct Scroller {
    holder: Box<*mut ListScroller>,
}

impl Default for Scroller {
    fn default() -> Scroller {
        Scroller::new()
    }
}

impl Scroller {
    pub fn new() -> Scroller {
        Scroller { holder: Box::new(std::ptr::null_mut()) }
    }

    pub unsafe fn setup(&mut self, view_handle: *mut LayoutViewHandle, group_name: &[u8], params: &mut ListScrollerParams, binder: *mut RowBinder) {
        params.point_at_self();
        list_scroller_setup(&mut *self.holder, view_handle, group_name.as_ptr(), params, binder);
    }

    pub unsafe fn resize(&mut self, items: i32, cursor: i32) {
        let Some(scroller) = self.as_ptr().as_mut() else {
            return;
        };
        let rows = items.min(scroller.pool_size);

        for slot in rows.max(0)..scroller.row_count {
            let row = *scroller.row_pool.add(slot as usize);
            if row.is_null() {
                continue;
            }
            if let Some(pane) = (*row).pane_holder.as_ref().and_then(|holder| holder.as_mut()) {
                pane.set_visible(false);
            }
            (*row).item_index = ITEM_NONE;
        }

        scroller.item_count = items;
        scroller.params_item_count = items;
        let columns = scroller.column_count;
        scroller.page_count = if columns > 1 { (items + columns - 1) / columns } else { items };
        scroller.row_count = rows;
        scroller.top_index = 0;

        if items > 0 {
            self.set_current_index(cursor, JUMP_IMMEDIATE, CLAMP_TO_LIST);
        }
        self.refresh_rows();
    }

    pub unsafe fn set_current_index(&mut self, index: i32, immediate: u32, validate: u32) {
        list_scroller_set_current_index(&mut *self.holder, index, immediate, validate);
    }

    pub unsafe fn setup_list(&mut self, view_handle: *mut LayoutViewHandle, group_name: &[u8], items: i32, start: i32, wrap_bottom: bool, binder: *mut RowBinder) -> bool {
        let mut params = ListScrollerParams::new(items);
        if wrap_bottom {
            params.wrap_at_bottom();
        }

        self.setup(view_handle, group_name, &mut params, binder);
        if self.is_empty() {
            return false;
        }

        if items > 0 {
            self.set_current_index(start, JUMP_IMMEDIATE, CLAMP_TO_LIST);
            self.set_focus(true);
            self.refresh_rows();
        }
        true
    }

    pub unsafe fn set_focus(&self, on: bool) {
        let scroller = self.as_ptr();
        if scroller.is_null() {
            return;
        }
        let set_focus: unsafe extern "C" fn(*mut ListScroller, bool) = vtable_call(scroller.cast(), VTABLE_SLOT_SET_FOCUS);
        set_focus(scroller, on);
    }

    pub unsafe fn refresh_rows(&self) {
        let scroller = self.as_ptr();
        if !scroller.is_null() {
            list_scroller_refresh_visible_rows(scroller, 1);
        }
    }

    pub fn as_ptr(&self) -> *mut ListScroller {
        *self.holder
    }

    pub fn is_empty(&self) -> bool {
        self.as_ptr().is_null()
    }

    pub unsafe fn current_index(&self) -> i32 {
        match self.as_ptr().as_ref() {
            Some(scroller) => scroller.current_index,
            None => ITEM_NONE,
        }
    }

    pub unsafe fn decided_index(&self) -> i32 {
        match self.as_ptr().as_ref() {
            Some(scroller) => scroller.decided_index,
            None => ITEM_NONE,
        }
    }

    pub unsafe fn pushed_past_bottom(&self) -> bool {
        match self.as_ptr().as_ref() {
            Some(scroller) => scroller.edge_pressed != 0 && scroller.edge_direction == EDGE_DOWN,
            None => false,
        }
    }

    pub unsafe fn can_scroll(&self) -> bool {
        match self.as_ptr().as_ref() {
            Some(scroller) => scroller.can_scroll != 0,
            None => false,
        }
    }

    pub unsafe fn row_for_item(&self, item: i32) -> Option<(*mut LayoutViewHandle, *mut u8)> {
        let scroller = self.as_ptr().as_ref()?;
        if scroller.row_pool.is_null() || scroller.row_count <= 0 {
            return None;
        }

        for slot in 0..scroller.row_count as usize {
            let row = *scroller.row_pool.add(slot);
            if row.is_null() || (*row).item_index != item {
                continue;
            }
            return Some((addr_of_mut!((*row).view), addr_of_mut!((*row).extra) as *mut u8));
        }
        None
    }

    pub unsafe fn slot_for_view(&self, row_view: *mut LayoutViewHandle) -> Option<usize> {
        let scroller = self.as_ptr().as_ref()?;
        if scroller.row_pool.is_null() || scroller.row_count <= 0 {
            return None;
        }

        (0..scroller.row_count as usize).find(|&slot| {
            let row = *scroller.row_pool.add(slot);
            !row.is_null() && addr_of_mut!((*row).view) == row_view
        })
    }
}

impl Drop for Scroller {
    fn drop(&mut self) {
        let scroller = self.as_ptr();
        if scroller.is_null() {
            return;
        }
        unsafe {
            let destruct_and_free: unsafe extern "C" fn(*mut ListScroller) = vtable_call(scroller.cast(), VTABLE_SLOT_DELETING_DTOR);
            destruct_and_free(scroller);
        }
    }
}
