use std::ptr;

use super::{
    alloc,
    layout::{debug_name, LayoutInner, LayoutRootCell, LayoutRootHandle},
    scene::FixedBaseString64,
    selector::{set_selector_focus, ButtonSelector},
    vtable_call, VTABLE_SLOT_DELETING_DTOR,
};
use crate::offsets;

pub const ANCHOR_RIGHT: u32 = 1;

pub const SHIFT_NONE: u32 = 0;

pub const MAX_ITEMS: usize = 3;

pub const RESULT_PENDING: i32 = -2;

pub const SUBMENU_CANCELLED: i32 = -1;

const BUTTON_VTABLE_SET_ENABLE: usize = 0x1f8 / 8;
const BUTTON_VTABLE_SET_DISABLED: usize = 0x388 / 8;

#[repr(C)]
struct SubMenuCalls {
    open: unsafe extern "C" fn(*mut SubMenuHolder, i32),
    close: unsafe extern "C" fn(*mut SubMenuHolder),
    clear_result: unsafe extern "C" fn(*mut SubMenuHolder),
    update: unsafe extern "C" fn(*mut SubMenuHolder),
}

#[repr(C)]
struct SubMenuHolder {
    calls: *const SubMenuCalls,
    object: *mut SubMenuObject,
}

#[skyline::from_offset(offsets::sub_menu_ctor())]
unsafe fn submenu_construct(holder: *mut SubMenuHolder);

#[skyline::from_offset(offsets::sub_menu_attach())]
unsafe fn submenu_attach(object: *mut SubMenuObject, cell_slot: *const *mut LayoutRootHandle, parts_name: *const FixedBaseString64, items: *const LabelVector, anchor: u32, shift: u32, own_callbacks: bool);

#[repr(C)]
struct SubMenuObject {
    unk0: [u8; 0xb8],
    state: i32,
    unkbc: [u8; 0xd8 - 0xbc],
    view_payload: *mut u8,
    selector: *mut ButtonSelector,
    unke8: [u8; 0xf0 - 0xe8],
    result: i32,
    unkf4: [u8; 0xf8 - 0xf4],
}

const _: () = {
    assert!(size_of::<SubMenuObject>() == 0xf8);
    assert!(std::mem::offset_of!(SubMenuObject, state) == 0xb8);
    assert!(std::mem::offset_of!(SubMenuObject, view_payload) == 0xd8);
    assert!(std::mem::offset_of!(SubMenuObject, selector) == 0xe0);
    assert!(std::mem::offset_of!(SubMenuObject, result) == 0xf0);
};

#[repr(C)]
struct LabelVector {
    begin: *const FixedBaseString64,
    end: *const FixedBaseString64,
    capacity: *const FixedBaseString64,
}

impl LabelVector {
    fn over(items: &[FixedBaseString64]) -> LabelVector {
        let begin = items.as_ptr();
        let end = unsafe { begin.add(items.len().min(MAX_ITEMS)) };
        LabelVector { begin, end, capacity: end }
    }
}

pub struct SubMenu {
    holder: Box<SubMenuHolder>,
}

impl Default for SubMenu {
    fn default() -> SubMenu {
        SubMenu::new()
    }
}

impl SubMenu {
    pub fn new() -> SubMenu {
        SubMenu { holder: Box::new(SubMenuHolder { calls: ptr::null(), object: ptr::null_mut() }) }
    }

    pub unsafe fn create(&mut self) {
        if !self.object().is_null() {
            return;
        }
        submenu_construct(&mut *self.holder);
    }

    pub unsafe fn attach(&mut self, cell: &LayoutRootCell, parts_name: &[u8], items: &[FixedBaseString64], anchor: u32, shift: u32) {
        let object = self.object();
        if object.is_null() {
            return;
        }
        let name = FixedBaseString64::new(debug_name(parts_name));
        let vector = LabelVector::over(items);
        submenu_attach(object, cell.cell_slot(), &name, &vector, anchor, shift, false);
    }

    pub unsafe fn open(&mut self, start: i32) {
        if let Some(calls) = self.calls() {
            (calls.open)(&mut *self.holder, start);
        }
    }

    pub unsafe fn close(&mut self) {
        if let Some(calls) = self.calls() {
            (calls.close)(&mut *self.holder);
        }
    }

    pub unsafe fn clear_result(&mut self) {
        if let Some(calls) = self.calls() {
            (calls.clear_result)(&mut *self.holder);
        }
    }

    pub unsafe fn update(&mut self) {
        if let Some(calls) = self.calls() {
            (calls.update)(&mut *self.holder);
        }
    }

    pub unsafe fn is_busy(&self) -> bool {
        match self.object().as_ref() {
            Some(object) => object.state > 0,
            None => false,
        }
    }

    pub unsafe fn result(&self) -> i32 {
        match self.object().as_ref() {
            Some(object) => object.result,
            None => RESULT_PENDING,
        }
    }

    pub unsafe fn set_item_enabled(&self, index: i32, on: bool) {
        let Some(button) = self.button(index) else {
            return;
        };
        let set_enable: unsafe extern "C" fn(*mut u8, bool) = vtable_call(button.cast(), BUTTON_VTABLE_SET_ENABLE);
        set_enable(button, on);

        let set_disabled: unsafe extern "C" fn(*mut u8, bool, bool) = vtable_call(button.cast(), BUTTON_VTABLE_SET_DISABLED);
        set_disabled(button, !on, false);
    }

    pub unsafe fn set_focus(&self, on: bool) {
        let selector = self.selector();
        if !selector.is_null() {
            set_selector_focus(selector, on);
        }
    }

    unsafe fn button(&self, index: i32) -> Option<*mut u8> {
        let selector = self.selector();
        let selector = selector.as_ref()?;
        selector.button(index).map(|button| button as *mut u8)
    }

    unsafe fn selector(&self) -> *mut ButtonSelector {
        match self.object().as_ref() {
            Some(object) => object.selector,
            None => ptr::null_mut(),
        }
    }

    fn calls(&self) -> Option<&SubMenuCalls> {
        unsafe { self.holder.calls.as_ref() }
    }

    fn object(&self) -> *mut SubMenuObject {
        self.holder.object
    }

    pub unsafe fn is_empty(&self) -> bool {
        self.object().is_null() || self.selector().is_null()
    }
}

impl Drop for SubMenu {
    fn drop(&mut self) {
        let object = self.object();
        if object.is_null() {
            return;
        }

        unsafe {
            let selector = (*object).selector;
            if !selector.is_null() {
                (*object).selector = ptr::null_mut();
                let destruct_and_free: unsafe extern "C" fn(*mut ButtonSelector) = vtable_call(selector.cast(), VTABLE_SLOT_DELETING_DTOR);
                destruct_and_free(selector);
            }

            let payload = (*object).view_payload;
            if let Some(layout) = (payload as *mut LayoutInner).as_mut() {
                (*object).view_payload = ptr::null_mut();
                let inner = layout.pane;
                layout.pane = ptr::null_mut();
                alloc::free_default(inner.cast::<u8>());
                alloc::free_default(payload);
            }

            alloc::free_default(object.cast::<u8>());
        }

        self.holder.calls = ptr::null();
        self.holder.object = ptr::null_mut();
    }
}
