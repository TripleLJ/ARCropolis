use super::{layout::LayoutViewHandle, vtable_call, CppVector, RbTree, VTABLE_SLOT_DELETING_DTOR, VTABLE_SLOT_SET_FOCUS};
use crate::offsets;

pub use super::INDEX_NONE as BUTTON_ID_NONE;

const VTABLE_SLOT_SET_ENABLE: usize = 0x60 / 8;

#[skyline::from_offset(offsets::button_selector_create())]
unsafe fn create_button_selector(holder: *mut *mut ButtonSelector, view_handle: *mut LayoutViewHandle, group_name: *const u8, config: *const SelectorConfig);

#[skyline::from_offset(offsets::button_selector_setup_button())]
unsafe fn setup_button(holder: *mut *mut ButtonSelector, id: i32, name_fmt: *const u8, arg: i32);

#[skyline::from_offset(offsets::button_selector_select_button())]
unsafe fn select_button(holder: *mut *mut ButtonSelector, id: i32, immediate: u32);

#[skyline::from_offset(offsets::button_selector_set_shortcut())]
unsafe fn set_shortcut(selector: *mut ButtonSelector, id: i32, virtual_button: i32, repeat: bool);

#[skyline::from_offset(offsets::button_selector_config_set_default_callbacks())]
unsafe fn set_default_callbacks(config: *mut SelectorConfig);

#[repr(C)]
pub struct Button {
    _private: [u8; 0],
}

#[repr(C)]
pub struct ButtonSelector {
    vtable: *const usize,
    unk0: [u8; 0x180 - 8],
    buttons: CppVector<*mut Button>,
    unk1: [u8; 0x1b8 - 0x198],
    button_map: RbTree<i32, *mut Button>,
    unk2: [u8; 0x250 - 0x1d0],
    focus: i32,
    selected: i32,
    unk3: u32,
    decided: i32,
    unk4: [u8; 0x280 - 0x260],
    cancelled: u8,
}

const _: () = {
    assert!(std::mem::offset_of!(ButtonSelector, buttons) == 0x180);
    assert!(std::mem::offset_of!(ButtonSelector, button_map) == 0x1b8);
    assert!(std::mem::offset_of!(ButtonSelector, focus) == 0x250);
    assert!(std::mem::offset_of!(ButtonSelector, selected) == 0x254);
    assert!(std::mem::offset_of!(ButtonSelector, decided) == 0x25c);
    assert!(std::mem::offset_of!(ButtonSelector, cancelled) == 0x280);
};

impl ButtonSelector {
    pub unsafe fn button(&self, id: i32) -> Option<*mut Button> {
        self.button_map.lower_bound(id).filter(|button| !button.is_null())
    }
}

pub struct SelectorState {
    pub focus: i32,
    pub selected: i32,
    pub decided: Option<i32>,
    pub cancelled: bool,
}

#[repr(C, align(16))]
pub struct SelectorConfig {
    bytes: [u8; 0x130],
}

impl Default for SelectorConfig {
    fn default() -> SelectorConfig {
        SelectorConfig::new()
    }
}

impl SelectorConfig {
    pub fn new() -> SelectorConfig {
        let mut config = SelectorConfig { bytes: [0; 0x130] };

        config.bytes[0x08] = 1;
        config.bytes[0x09] = 1;
        config.bytes[0x10] = 1;
        config.bytes[0x14] = 1;
        config.bytes[0x16] = 1;

        config.bytes[0x1c..0x20].copy_from_slice(&30.0f32.to_le_bytes());

        config
    }

    pub fn use_only_shortcut(&mut self) {
        self.bytes[0x0a] = 1;
    }

    pub unsafe fn install_defaults(&mut self) {
        set_default_callbacks(self);
    }
}

pub struct ButtonSelectorCell {
    holder: Box<*mut ButtonSelector>,
}

impl Default for ButtonSelectorCell {
    fn default() -> ButtonSelectorCell {
        ButtonSelectorCell::new()
    }
}

impl ButtonSelectorCell {
    pub fn new() -> ButtonSelectorCell {
        ButtonSelectorCell { holder: Box::new(std::ptr::null_mut()) }
    }

    pub unsafe fn create(&mut self, view_handle: *mut LayoutViewHandle, group_name: &[u8], config: &SelectorConfig) {
        create_button_selector(&mut *self.holder, view_handle, group_name.as_ptr(), config);
    }

    pub unsafe fn setup_button(&mut self, id: i32, name_fmt: &[u8], arg: i32) {
        setup_button(&mut *self.holder, id, name_fmt.as_ptr(), arg);
    }

    pub unsafe fn select_button(&mut self, id: i32, immediate: u32) {
        select_button(&mut *self.holder, id, immediate);
    }

    pub unsafe fn set_shortcut(&mut self, id: i32, virtual_button: i32, repeat: bool) {
        let selector = self.as_ptr();
        if !selector.is_null() {
            set_shortcut(selector, id, virtual_button, repeat);
        }
    }

    pub unsafe fn set_enable(&self, on: bool) {
        call_bool_slot(self.as_ptr(), VTABLE_SLOT_SET_ENABLE, on);
    }

    pub unsafe fn set_focus(&self, on: bool) {
        call_bool_slot(self.as_ptr(), VTABLE_SLOT_SET_FOCUS, on);
    }

    pub fn as_ptr(&self) -> *mut ButtonSelector {
        *self.holder
    }

    pub fn is_empty(&self) -> bool {
        self.as_ptr().is_null()
    }

    pub unsafe fn button_count(&self) -> usize {
        match self.as_ptr().as_ref() {
            Some(selector) => selector.buttons.len(),
            None => 0,
        }
    }

    pub unsafe fn state(&self) -> SelectorState {
        let Some(selector) = self.as_ptr().as_ref() else {
            return SelectorState {
                focus: BUTTON_ID_NONE,
                selected: BUTTON_ID_NONE,
                decided: None,
                cancelled: false,
            };
        };

        SelectorState {
            focus: selector.focus,
            selected: selector.selected,
            decided: (selector.decided >= 0).then_some(selector.decided),
            cancelled: selector.cancelled != 0,
        }
    }
}

impl Drop for ButtonSelectorCell {
    fn drop(&mut self) {
        let selector = self.as_ptr();
        if selector.is_null() {
            return;
        }

        unsafe {
            let destruct_and_free: unsafe extern "C" fn(*mut ButtonSelector) = vtable_call(selector.cast(), VTABLE_SLOT_DELETING_DTOR);
            destruct_and_free(selector);
        }
    }
}

unsafe fn call_bool_slot(selector: *mut ButtonSelector, slot: usize, on: bool) {
    if selector.is_null() {
        return;
    }
    let call: unsafe extern "C" fn(*mut ButtonSelector, bool) = vtable_call(selector.cast(), slot);
    call(selector, on);
}

pub unsafe fn set_selector_focus(selector: *mut ButtonSelector, on: bool) {
    call_bool_slot(selector, VTABLE_SLOT_SET_FOCUS, on);
}

pub unsafe fn select_button_in_slot(slot: *mut *mut ButtonSelector, id: i32, immediate: u32) {
    if slot.is_null() || (*slot).is_null() {
        return;
    }
    select_button(slot, id, immediate);
}
