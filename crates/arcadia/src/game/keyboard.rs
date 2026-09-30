use std::{
    alloc::{alloc_zeroed, dealloc, Layout},
    mem::offset_of,
    ptr,
};

use crate::offsets;

const CONFIG_SIZE: usize = 0x4d0;
const ARG_SIZE: usize = 0x510;

const PRESET_DEFAULT: u32 = 0;

const MODE_LANGUAGE_SET_2: u32 = 8;

const INPUT_MODE_ANY: u32 = 0;

const RESULT_CANCELLED: u32 = 0x29f;

const STRING_UNITS: usize = 1002;

const WORK_BUFFER_ALIGN: usize = 0x1000;

#[skyline::from_offset(offsets::swkbd_make_preset())]
unsafe fn make_preset(config: *mut u8, preset: u32);

#[skyline::from_offset(offsets::swkbd_set_header_text())]
unsafe fn set_header_text(config: *mut u8, text: *const u16);

#[skyline::from_offset(offsets::swkbd_set_guide_text())]
unsafe fn set_guide_text(config: *mut u8, text: *const u16);

#[skyline::from_offset(offsets::swkbd_set_initial_text())]
unsafe fn set_initial_text(arg: *mut ShowKeyboardArg, text: *const u16);

#[skyline::from_offset(offsets::swkbd_get_required_work_buffer_size())]
unsafe fn get_required_work_buffer_size(use_dictionary: bool) -> usize;

#[skyline::from_offset(offsets::swkbd_show_keyboard())]
unsafe fn show_keyboard(out: *mut SwkbdString, arg: *const ShowKeyboardArg) -> u32;

#[repr(C)]
struct SwkbdString {
    buffer: *mut u16,
    units: usize,
}

#[repr(C)]
struct SwkbdConfig {
    mode: u32,
    unk04: [u8; 0x1a - 0x04],
    unk1a: u8,
    unk1b: [u8; 0x3ac - 0x1b],
    max_length: u32,
    min_length: u32,
    unk3b4: [u8; 0x3b8 - 0x3b4],
    input_mode: u32,
    cancel_disabled: u8,
    unk3bd: [u8; CONFIG_SIZE - 0x3bd],
}

const _: () = {
    assert!(size_of::<SwkbdConfig>() == CONFIG_SIZE);
    assert!(offset_of!(SwkbdConfig, mode) == 0x00);
    assert!(offset_of!(SwkbdConfig, unk1a) == 0x1a);
    assert!(offset_of!(SwkbdConfig, max_length) == 0x3ac);
    assert!(offset_of!(SwkbdConfig, min_length) == 0x3b0);
    assert!(offset_of!(SwkbdConfig, input_mode) == 0x3b8);
    assert!(offset_of!(SwkbdConfig, cancel_disabled) == 0x3bc);
};

#[repr(C, align(16))]
struct ShowKeyboardArg {
    config: SwkbdConfig,
    work_buffer: *mut u8,
    work_buffer_size: usize,
    text_buffer: *const u8,
    text_buffer_size: usize,
    dictionary: *const u8,
    dictionary_size: usize,
    text_check: [usize; 2],
}

const _: () = assert!(size_of::<ShowKeyboardArg>() == ARG_SIZE);

fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub unsafe fn ask(header: &str, guide: &str, max_length: u32, initial: Option<&str>) -> Option<String> {
    let work_size = get_required_work_buffer_size(false).max(WORK_BUFFER_ALIGN);
    let work_layout = Layout::from_size_align(work_size, WORK_BUFFER_ALIGN).ok()?;

    let work_buffer = alloc_zeroed(work_layout);
    if work_buffer.is_null() {
        return None;
    }

    let mut arg = Box::new(ShowKeyboardArg {
        config: std::mem::zeroed(),
        work_buffer,
        work_buffer_size: work_size,
        text_buffer: ptr::null(),
        text_buffer_size: 0,
        dictionary: ptr::null(),
        dictionary_size: 0,
        text_check: [0; 2],
    });

    let config = ptr::addr_of_mut!(arg.config).cast::<u8>();
    make_preset(config, PRESET_DEFAULT);

    arg.config.mode = MODE_LANGUAGE_SET_2;
    arg.config.max_length = max_length;
    arg.config.min_length = 1;
    arg.config.input_mode = INPUT_MODE_ANY;
    arg.config.cancel_disabled = 0;
    arg.config.unk1a = 0;

    let header = utf16(header);
    let guide = utf16(guide);
    set_header_text(config, header.as_ptr());
    set_guide_text(config, guide.as_ptr());

    if let Some(initial) = initial {
        let initial = utf16(initial);
        set_initial_text(&mut *arg, initial.as_ptr());
    }

    let mut buffer = vec![0u16; STRING_UNITS];
    let mut out = SwkbdString {
        buffer: buffer.as_mut_ptr(),
        units: STRING_UNITS,
    };

    let result = show_keyboard(&mut out, &*arg);
    dealloc(work_buffer, work_layout);

    if result == RESULT_CANCELLED {
        return None;
    }

    let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(0);
    Some(String::from_utf16_lossy(&buffer[..end]))
}
