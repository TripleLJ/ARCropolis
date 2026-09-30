use std::mem::{self, offset_of};

use super::{
    scene::{Scene, SceneChanger, SceneInitContext, SequenceScene},
    scene_vtable::SceneVtable,
};
use crate::offsets;

#[repr(C, align(16))]
pub struct HybridScene {
    pub base: Scene,
    pub driver: SequenceScene,

    auto_exit_gate: u8,

    message_pending: u8,
    unk: [u8; 2],

    network_call_countdown: u32,

    pub menu_info: *mut MenuDriverInfo,
}

#[repr(C)]
pub struct MenuDriverInfo {
    pub driver: *mut SequenceScene,
    unk: [u8; 0xb8],
    program: [u32; MENU_PROGRAM_SLOTS],
    pc: u32,
    pub state: u32,
    pub frames_in_state: u32,
    unk2: [u8; 0x18],
}

pub const MENU_PROGRAM_SLOTS: usize = 0x13;

const _: () = assert!(offset_of!(HybridScene, menu_info) == 0xe8);
const _: () = assert!(offset_of!(MenuDriverInfo, program) == 0xc0);
const _: () = assert!(offset_of!(MenuDriverInfo, pc) == 0x10c);
const _: () = assert!(offset_of!(MenuDriverInfo, state) == 0x110);
const _: () = assert!(offset_of!(MenuDriverInfo, frames_in_state) == 0x114);
const _: () = assert!(mem::size_of::<MenuDriverInfo>() == 0x130);

pub const MENU_STATE_HOW_TO_PLAY: u32 = 5;

impl MenuDriverInfo {
    pub fn set_state(&mut self, state: u32) -> bool {
        let Some(slot) = self.program.get_mut(self.pc as usize) else {
            return false;
        };
        *slot = state;
        true
    }
}

#[repr(C)]
pub struct MainMenuScene {
    pub base: Scene,
    unk: u64,
    pub block: *mut MainMenuBlock,
}

#[repr(C)]
pub struct MainMenuBlock {
    unk: [u8; 0xc8],
    pub exit_code: u32,
}

const _: () = assert!(offset_of!(MainMenuScene, block) == 0x58);
const _: () = assert!(offset_of!(MainMenuBlock, exit_code) == 0xc8);

pub const MAIN_MENU_EXIT_TILE: u32 = 2;

impl HybridScene {
    pub fn new(vtable: &'static SceneVtable) -> HybridScene {
        HybridScene {
            base: Scene::new(vtable),
            driver: unsafe { mem::zeroed() },
            auto_exit_gate: 0,
            message_pending: 0,
            unk: [0; 2],
            network_call_countdown: 0,
            menu_info: std::ptr::null_mut(),
        }
    }

    pub unsafe fn construct(&mut self) {
        self.driver.construct()
    }

    pub unsafe fn destruct(&mut self) {
        self.driver.destruct()
    }
}

unsafe extern "C" fn uses_async_initialize(_this: *mut Scene) -> bool {
    false
}

pub fn vtable(
    dtor: unsafe extern "C" fn(*mut Scene),
    deleting_dtor: unsafe extern "C" fn(*mut Scene),
    on_enter: unsafe extern "C" fn(*mut Scene, *const SceneInitContext),
    on_exit: unsafe extern "C" fn(*mut Scene),
    tick: unsafe extern "C" fn(*mut Scene, *mut SceneChanger),
) -> SceneVtable {
    let base = unsafe { &*(patterns::offset_to_addr(offsets::g_vtable_scene()) as *const SceneVtable) };

    SceneVtable {
        dtor,
        deleting_dtor,
        uses_async_initialize,
        on_enter,
        on_exit,
        tick,
        ..*base
    }
}

const _: () = {
    assert!(size_of::<HybridScene>() == 0xf0);
    assert!(offset_of!(HybridScene, driver) == 0x50);
    assert!(offset_of!(HybridScene, auto_exit_gate) == 0xe0);
    assert!(offset_of!(HybridScene, message_pending) == 0xe1);
};
