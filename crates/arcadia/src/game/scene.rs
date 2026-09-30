use std::{
    ffi::CStr,
    mem::offset_of,
    ptr::{self, NonNull},
};

use super::{scene_vtable::SceneVtable, CppVector};
use crate::offsets;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FixedBaseString64 {
    hash: u32,
    length: u32,
    bytes: [u8; 64],
}

impl FixedBaseString64 {
    pub const EMPTY: FixedBaseString64 = FixedBaseString64 {
        hash: 0,
        length: 0,
        bytes: [0; 64],
    };

    pub fn new(text: &str) -> FixedBaseString64 {
        let length = text.len().min(63);
        let mut bytes = [0u8; 64];
        bytes[..length].copy_from_slice(&text.as_bytes()[..length]);

        FixedBaseString64 {
            hash: base_string_hash(&bytes[..length]),
            length: length as u32,
            bytes,
        }
    }

    pub fn as_str(&self) -> &str {
        CStr::from_bytes_until_nul(&self.bytes)
            .ok()
            .and_then(|name| name.to_str().ok())
            .unwrap_or("<unreadable>")
    }
}

fn base_string_hash(text: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in text {
        hash = hash.wrapping_mul(0x89) ^ *byte as u32;
    }
    hash
}

#[repr(C)]
pub struct Scene {
    vtable: &'static SceneVtable,
    pub name: FixedBaseString64,
}

impl Scene {
    pub fn new(vtable: &'static SceneVtable) -> Scene {
        Scene {
            vtable,
            name: FixedBaseString64::EMPTY,
        }
    }
}

#[repr(C)]
pub struct SequenceScene {
    vtable: *const (),
    list_next: *mut SequenceScene,
    list_prev: *mut SequenceScene,
    unk: u64,
    pub queue: *mut SceneQueue,
    pub holder: *mut SceneHolder,

    pub state: u32,
    unk2: u32,
    pub active_scene: *mut Scene,
    pub exit_code: u32,

    pub name: FixedBaseString64,
    pub exiting: u8,
    unk3: [u8; 3],
}

#[skyline::from_offset(offsets::sequence_scene_ctor())]
unsafe fn sequence_scene_ctor(driver: *mut SequenceScene);

#[skyline::from_offset(offsets::sequence_scene_dtor())]
unsafe fn sequence_scene_dtor(driver: *mut SequenceScene);

#[skyline::from_offset(offsets::register_scene_table())]
unsafe fn register_scene_table(driver: *mut SequenceScene, table: *const SceneTableEntry);

#[skyline::from_offset(offsets::unregister_scene_table())]
unsafe fn unregister_scene_table(driver: *mut SequenceScene, table: *const SceneTableEntry);

#[skyline::from_offset(offsets::exit_active_scene())]
unsafe fn exit_active_scene(driver: *mut SequenceScene, code: u32);

#[skyline::from_offset(offsets::scene_queue_push_front())]
unsafe fn scene_queue_push_front(
    queue: *mut SceneQueue,
    target: *const FixedBaseString64,
    leaving: *const FixedBaseString64,
    parameters: *const SceneParameter,
);

impl SequenceScene {
    pub unsafe fn construct(&mut self) {
        sequence_scene_ctor(self);

        self.vtable = patterns::offset_to_addr(offsets::g_vtable_sequence_scene_ui());
    }

    pub unsafe fn destruct(&mut self) {
        sequence_scene_dtor(self)
    }

    pub fn drained(&self) -> bool {
        let (Some(holder), Some(queue)) = (unsafe { self.holder.as_ref() }, unsafe { self.queue.as_ref() }) else {
            return false;
        };

        holder.current.is_null() && holder.stack.is_empty() && queue.count == 0
    }

    pub unsafe fn active_as<T>(&self, name: &str) -> Option<&mut T> {
        if self.active_name().as_str() != name {
            return None;
        }
        self.active_scene.cast::<T>().as_mut()
    }

    pub fn active_name(&self) -> FixedBaseString64 {
        match unsafe { self.active_scene.as_ref() } {
            Some(scene) => scene.name,
            None => FixedBaseString64::EMPTY,
        }
    }

    pub unsafe fn push_front(&mut self, target: &FixedBaseString64, parameters: Option<&SceneParameter>) {
        let leaving = self.active_name();
        let parameters = parameters.map_or(ptr::null(), |parameters| parameters as *const SceneParameter);

        scene_queue_push_front(self.queue, target, &leaving, parameters);
        self.state = 1;
    }

    pub unsafe fn exit_active_scene(&mut self, code: u32) {
        exit_active_scene(self, code)
    }

    pub unsafe fn register_table(&mut self, table: &'static [SceneTableEntry]) {
        register_scene_table(self, table.as_ptr())
    }

    pub unsafe fn unregister_table(&mut self, table: &'static [SceneTableEntry]) {
        unregister_scene_table(self, table.as_ptr())
    }
}

#[repr(C)]
pub struct SceneHolder {
    unk: [u8; 0x30],
    pub stack: CppVector<*mut Scene>,
    unk2: [u8; 0x18],
    pub current: *mut Scene,
}

#[repr(C)]
pub struct SceneQueue {
    pub back: *mut SceneQueueEntry,
    pub front: *mut SceneQueueEntry,
    pub count: u64,

    pub active: FixedBaseString64,

    pub previous: FixedBaseString64,
    pub parameters: *const SceneParameter,

    parameters_ref: *const (),
}

#[repr(C)]
pub struct SceneQueueEntry {
    pub prev: *mut SceneQueueEntry,
    pub next: *mut SceneQueueEntry,
    pub target: FixedBaseString64,
    pub leaving: FixedBaseString64,
    pub parameters: *const SceneParameter,
    parameters_ref: *const (),
}

#[repr(C)]
pub struct SceneTableEntry {
    pub name: *const u8,
    pub factory: *const FactoryHeader,
}

impl SceneTableEntry {
    pub const END: SceneTableEntry = SceneTableEntry {
        name: ptr::null(),
        factory: ptr::null(),
    };
}

#[repr(C)]
pub struct FactoryHeader {
    pub vtable: &'static FactoryVtable,
}

#[repr(C)]
pub struct FactoryVtable {
    pub dtor: unsafe extern "C" fn(*mut FactoryHeader),
    pub deleting_dtor: unsafe extern "C" fn(*mut FactoryHeader),
    pub create: unsafe extern "C" fn(*mut FactoryHeader) -> *mut Scene,
}

#[repr(C)]
pub struct SceneInitContext {
    vtable: *const (),
    pub owner: NonNull<SequenceScene>,
    pub previous_exit_code: u32,

    pub from_scene: FixedBaseString64,
    pub previous_scene: FixedBaseString64,
    unk: u32,
    parameters: *const SceneParameter,
}

impl SceneInitContext {
    pub fn init_code(&self) -> Option<u32> {
        unsafe { self.parameters.as_ref() }?.as_code()
    }
}

#[repr(C)]
pub struct SceneParameter {
    vtable: *const (),
    payload: u32,
    unk: u32,
}

impl SceneParameter {
    pub fn code(code: u32) -> SceneParameter {
        SceneParameter {
            vtable: patterns::offset_to_addr(offsets::g_vtable_scene_init_code_parameter()),
            payload: code,
            unk: 0,
        }
    }

    fn as_code(&self) -> Option<u32> {
        (self.vtable == patterns::offset_to_addr(offsets::g_vtable_scene_init_code_parameter())).then_some(self.payload)
    }
}

#[repr(C)]
pub struct SceneChanger {
    vtable: *const (),
    owner: *mut SequenceScene,
}

impl SceneChanger {
    pub fn owner(&mut self) -> Option<&mut SequenceScene> {
        unsafe { self.owner.as_mut() }
    }
}

#[repr(u32)]
pub enum ExitCode {
    Default = 0,
    Code1 = 1,
    Code2 = 2,
    Code3 = 3,
    Demo = 0xffff_fffe,
    NetworkError = 0xffff_ffff,
}

const _: () = {
    assert!(size_of::<FixedBaseString64>() == 0x48);
    assert!(size_of::<Scene>() == 0x50);
    assert!(size_of::<SequenceScene>() == 0x90);
    assert!(offset_of!(SequenceScene, queue) == 0x20);
    assert!(offset_of!(SequenceScene, active_scene) == 0x38);
    assert!(offset_of!(SequenceScene, name) == 0x44);
    assert!(offset_of!(SequenceScene, exiting) == 0x8c);
    assert!(size_of::<SceneHolder>() == 0x68);
    assert!(offset_of!(SceneHolder, stack) == 0x30);
    assert!(offset_of!(SceneHolder, current) == 0x60);
    assert!(size_of::<SceneQueue>() == 0xb8);
    assert!(offset_of!(SceneQueue, count) == 0x10);
    assert!(offset_of!(SceneQueue, previous) == 0x60);
    assert!(size_of::<SceneQueueEntry>() == 0xb0);
    assert!(size_of::<SceneTableEntry>() == 0x10);
    assert!(size_of::<SceneInitContext>() == 0xb0);
    assert!(offset_of!(SceneInitContext, owner) == 8);
    assert!(offset_of!(SceneInitContext, previous_exit_code) == 0x10);
    assert!(offset_of!(SceneInitContext, previous_scene) == 0x5c);
    assert!(offset_of!(SceneInitContext, parameters) == 0xa8);
    assert!(size_of::<SceneParameter>() == 0x10);
};
