use std::{mem::offset_of, ptr::NonNull};

use crate::offsets;

#[repr(C)]
struct FadeHolder {
    unk: u64,
    manager: *mut FadeManager,
}

#[repr(C)]
struct FadeManager {
    unk: [u8; 0x6a],
    done: u8,
    busy: u8,
}

#[skyline::from_offset(offsets::fade_in())]
unsafe fn fade_in(manager: *mut FadeManager);

#[skyline::from_offset(offsets::fade_out())]
unsafe fn fade_out(holder: *mut FadeHolder);

#[skyline::from_offset(offsets::fade_out_instant())]
unsafe fn fade_out_instant(manager: *mut FadeManager);

pub struct Fade {
    holder: NonNull<FadeHolder>,
    manager: NonNull<FadeManager>,
}

impl Fade {
    pub fn get() -> Option<Fade> {
        let global = patterns::offset_to_addr(offsets::g_fade_manager_holder()) as *const *mut FadeHolder;
        let holder = NonNull::new(unsafe { *global })?;
        let manager = NonNull::new(unsafe { holder.as_ref().manager })?;

        Some(Fade { holder, manager })
    }

    pub fn fade_in(&self) {
        unsafe { fade_in(self.manager.as_ptr()) }
    }

    pub fn fade_out(&self) {
        unsafe { fade_out(self.holder.as_ptr()) }
    }

    pub fn fade_out_instant(&self) {
        unsafe { fade_out_instant(self.manager.as_ptr()) }
    }

    pub fn finished(&self) -> bool {
        let manager = unsafe { self.manager.as_ref() };
        manager.done != 0 && manager.busy == 0
    }
}

const _: () = {
    assert!(offset_of!(FadeManager, done) == 0x6a);
    assert!(offset_of!(FadeManager, busy) == 0x6b);
};
