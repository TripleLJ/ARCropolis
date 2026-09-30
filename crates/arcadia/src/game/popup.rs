use std::ptr;

use super::alloc;
use crate::offsets;

pub const POPUP_DELETE_CONFIRM: u64 = 0x58ff_ff17_f737_7bb7;

pub const RESULT_NONE: i32 = 0;
pub const RESULT_YES: i32 = 2;

#[repr(C)]
struct ArgVector {
    begin: *const *const u16,
    end: *const *const u16,
    capacity: *const *const u16,
}

#[repr(C)]
struct PopupHolder {
    unk0: *mut u8,
    manager: *mut PopupManager,
    params: *mut PopupParams,
}

const LABEL_SIZE: usize = 72;

#[repr(C)]
struct PopupParams {
    style: u32,
    unk04: [u8; 0x24 - 0x04],
    body: [u8; LABEL_SIZE],
    button_1: [u8; LABEL_SIZE],
    button_2: [u8; LABEL_SIZE],
    button_3: [u8; 0x140 - 0xfc],
    args: ArgVector,
    unk158: [u8; 0x1a0 - 0x158],
}

#[repr(C)]
struct PopupManager {
    unk0: [u8; 0xf8],
    state: i32,
    unkfc: [u8; 0x118 - 0xfc],
    result: i32,
    unk11c: [u8; 0x142 - 0x11c],
    popup_open: u8,
}

const _: () = {
    assert!(std::mem::offset_of!(PopupHolder, manager) == 8);
    assert!(std::mem::offset_of!(PopupHolder, params) == 0x10);
    assert!(std::mem::offset_of!(PopupParams, body) == 0x24);
    assert!(std::mem::offset_of!(PopupParams, button_1) == 0x6c);
    assert!(std::mem::offset_of!(PopupParams, button_2) == 0xb4);
    assert!(std::mem::offset_of!(PopupParams, button_3) == 0xfc);
    assert!(std::mem::offset_of!(PopupParams, args) == 0x140);
    assert!(size_of::<PopupParams>() == 0x1a0);
    assert!(std::mem::offset_of!(PopupManager, state) == 0xf8);
    assert!(std::mem::offset_of!(PopupManager, result) == 0x118);
    assert!(std::mem::offset_of!(PopupManager, popup_open) == 0x142);
};

#[skyline::from_offset(offsets::popup_open())]
unsafe fn app_popup_manager_open_popup(holder: *mut PopupHolder, id: u64, args: *const ArgVector);

#[skyline::from_offset(offsets::popup_populate_params())]
unsafe fn populate_parameters_for_hash40(params: *mut PopupParams, id: u64);

#[skyline::from_offset(offsets::popup_open_from_params())]
unsafe fn app_popup_manager_open_from_params(manager: *mut PopupManager, params: *const PopupParams);

unsafe fn holder() -> *mut PopupHolder {
    *(patterns::offset_to_addr(offsets::g_popup_holder()) as *const *mut PopupHolder)
}

unsafe fn manager() -> *mut PopupManager {
    match holder().as_ref() {
        Some(holder) => holder.manager,
        None => ptr::null_mut(),
    }
}

pub unsafe fn open(id: u64, argument: *const u16) -> bool {
    let holder = holder();
    if holder.is_null() {
        return false;
    }

    let arguments = [argument];
    let vector = ArgVector {
        begin: arguments.as_ptr(),
        end: arguments.as_ptr().add(1),
        capacity: arguments.as_ptr().add(1),
    };

    app_popup_manager_open_popup(holder, id, &vector);
    true
}

pub unsafe fn open_with_body(base: u64, body_label: &str, argument: *const u16) -> bool {
    let Some(holder) = holder().as_mut() else {
        return false;
    };
    let (Some(manager), Some(params)) = (holder.manager.as_mut(), holder.params.as_mut()) else {
        return false;
    };

    populate_parameters_for_hash40(params, base);

    let label = body_label.as_bytes();
    if label.len() >= LABEL_SIZE {
        return false;
    }
    params.body = [0; LABEL_SIZE];
    params.body[..label.len()].copy_from_slice(label);

    let args = &mut params.args;
    if args.begin.is_null() || args.capacity.offset_from(args.begin) < 1 {
        if !args.begin.is_null() {
            alloc::free_default(args.begin as *mut u8);
        }
        let block = alloc::je_aligned_alloc(0x10, size_of::<*const u16>()) as *mut *const u16;
        if block.is_null() {
            return false;
        }
        args.begin = block;
        args.capacity = block.add(1);
    }
    *(args.begin as *mut *const u16) = argument;
    args.end = args.begin.add(1);

    app_popup_manager_open_from_params(manager, params);
    true
}

pub unsafe fn is_open() -> bool {
    match manager().as_ref() {
        Some(manager) => manager.state != 0 || manager.popup_open != 0,
        None => false,
    }
}

pub unsafe fn result() -> i32 {
    match manager().as_ref() {
        Some(manager) => manager.result,
        None => RESULT_NONE,
    }
}
