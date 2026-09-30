use std::ptr::{self, NonNull};

use smash_arc::Hash40;

use super::{alloc, vtable_call, VTABLE_SLOT_DELETING_DTOR};
use crate::offsets;

#[repr(C)]
struct TreeRoot {
    begin: *mut u8,
    root: *mut u8,
    size: usize,
}

#[repr(C, align(16))]
pub struct LoadingListData {
    vtable: *const usize,
    trees: [TreeRoot; 4],
    priority: u32,
    unk: u32,
    tag: u64,
    tail: [u8; 8],
}

const _: () = {
    assert!(size_of::<LoadingListData>() == 0x80);
    assert!(std::mem::offset_of!(LoadingListData, priority) == 0x68);
    assert!(std::mem::offset_of!(LoadingListData, tag) == 0x70);
};

#[skyline::from_offset(offsets::loading_list_queue())]
unsafe fn queue_file_to_loading_list(list: *mut LoadingListData, path: *const u64, request_tag: u64, priority: u32, unused: i32);

#[skyline::from_offset(offsets::loading_list_wait())]
unsafe fn wait_for_threaded_loading(list: *mut LoadingListData) -> u64;

#[skyline::from_offset(offsets::loading_list_free())]
unsafe fn free_loading_list(list: *mut LoadingListData);

pub struct LoadingList {
    data: NonNull<LoadingListData>,
}

impl LoadingList {
    pub unsafe fn new() -> Option<LoadingList> {
        let block = alloc::je_aligned_alloc(16, size_of::<LoadingListData>()).cast::<LoadingListData>();
        let data = NonNull::new(block)?;
        let list = data.as_ptr();
        list.write_bytes(0, 1);

        (*list).vtable = patterns::offset_to_addr(offsets::g_vtable_threaded_loading_list()) as *const usize;

        for tree in &mut (*list).trees {
            tree.begin = ptr::addr_of_mut!(tree.root) as *mut u8;
        }

        (*list).priority = 2;
        Some(LoadingList { data })
    }

    pub fn as_ptr(&self) -> *mut LoadingListData {
        self.data.as_ptr()
    }

    pub unsafe fn queue(&self, hash: &Hash40) {
        let list = self.data.as_ptr();
        let tag = (*list).tag;
        let priority = (*list).priority;
        queue_file_to_loading_list(list, &hash.0 as *const u64, tag, priority, 0);
    }

    pub unsafe fn is_ready(&self) -> bool {
        wait_for_threaded_loading(self.data.as_ptr()) & 1 != 0
    }
}

impl Drop for LoadingList {
    fn drop(&mut self) {
        unsafe {
            let list = self.data.as_ptr();
            free_loading_list(list);

            let destruct_and_free: unsafe extern "C" fn(*mut LoadingListData) = vtable_call(list.cast(), VTABLE_SLOT_DELETING_DTOR);
            destruct_and_free(list);
        }
    }
}
