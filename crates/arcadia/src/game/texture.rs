use std::ptr;

use super::{
    alloc,
    layout::Picture,
};
use crate::offsets;

const BNTX_ALIGNMENT: usize = 0x1000;

const NO_FILE_PATH_IDX: u32 = 0xffffff;

#[repr(C)]
struct TextureWrapper {
    vtable: *const u8,
    record: *mut TextureRecord,
}

#[repr(C)]
struct TextureRecord {
    unk0: [u8; 0x08],
    buffer: *mut u8,
    size: u64,
    unk18: [u8; 0x38 - 0x18],
    texture_info: [u8; 0x50 - 0x38],
    resfile: *mut u8,
    unk58: [u8; 0x5c - 0x58],
    slot_registered: u8,
    unk5d: [u8; 0x68 - 0x5d],
}

const _: () = {
    assert!(size_of::<TextureRecord>() == 0x68);
    assert!(std::mem::offset_of!(TextureRecord, buffer) == 0x08);
    assert!(std::mem::offset_of!(TextureRecord, texture_info) == 0x38);
    assert!(std::mem::offset_of!(TextureRecord, resfile) == 0x50);
    assert!(std::mem::offset_of!(TextureRecord, slot_registered) == 0x5c);
};

#[skyline::from_offset(offsets::texture_resource_construct())]
unsafe fn texture_resource_construct(wrapper: *mut TextureWrapper, file_path_idx: *const u32);

#[skyline::from_offset(offsets::res_texture_file_cast())]
unsafe fn res_texture_file_cast(buffer: *mut u8) -> *mut u8;

#[skyline::from_offset(offsets::texture_resource_setup_gpu())]
unsafe fn texture_resource_setup_gpu(record: *mut TextureRecord);

#[skyline::from_offset(offsets::texture_resource_reset())]
unsafe fn texture_resource_reset(record: *mut TextureRecord);

#[skyline::from_offset(offsets::picture_set_texture_info())]
unsafe fn picture_set_texture_info(picture: *mut Picture, texture_info: *const u8, texmap_idx: i32);

pub struct MemoryTexture {
    wrapper: *mut TextureWrapper,
    record: *mut TextureRecord,
}

unsafe impl Send for MemoryTexture {}

impl MemoryTexture {
    pub unsafe fn create(bntx: &[u8]) -> Option<MemoryTexture> {
        let buffer = alloc::je_aligned_alloc(BNTX_ALIGNMENT, bntx.len());
        if buffer.is_null() {
            return None;
        }
        ptr::copy_nonoverlapping(bntx.as_ptr(), buffer, bntx.len());

        let wrapper = alloc::je_aligned_alloc(16, size_of::<TextureWrapper>()).cast::<TextureWrapper>();
        if wrapper.is_null() {
            alloc::free_default(buffer);
            return None;
        }

        texture_resource_construct(wrapper, &NO_FILE_PATH_IDX);
        let record = (*wrapper).record;
        if record.is_null() {
            alloc::free_default(buffer);
            alloc::free_default(wrapper.cast::<u8>());
            return None;
        }

        (*record).buffer = buffer;
        (*record).size = bntx.len() as u64;
        (*record).resfile = res_texture_file_cast(buffer);

        texture_resource_setup_gpu(record);

        Some(MemoryTexture { wrapper, record })
    }

    pub unsafe fn slot_registered(&self) -> bool {
        (*self.record).slot_registered != 0
    }

    pub fn texture_info(&self) -> *const u8 {
        unsafe { ptr::addr_of!((*self.record).texture_info) as *const u8 }
    }

    pub unsafe fn bind(&self, picture: *mut Picture) {
        picture_set_texture_info(picture, self.texture_info(), 0);
    }
}

impl Drop for MemoryTexture {
    fn drop(&mut self) {
        unsafe {
            texture_resource_reset(self.record);
            alloc::free_default(self.record.cast::<u8>());
            alloc::free_default(self.wrapper.cast::<u8>());
        }
    }
}
