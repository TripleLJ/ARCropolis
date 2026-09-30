pub mod alloc;
pub mod fade;
pub mod frame;
pub mod hybrid;
pub mod keyboard;
pub mod layout;
pub mod list;
pub mod loading;
pub mod popup;
pub mod scene;
pub mod scene_vtable;
pub mod selector;
pub mod submenu;
pub mod texture;

use std::{
    marker::{PhantomData, PhantomPinned},
    mem::{self, offset_of},
    slice,
};

const fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xffff_ffff;
    let mut at = 0;
    while at < bytes.len() {
        crc ^= bytes[at] as u32;
        let mut bit = 0;
        while bit < 8 {
            let low = crc & 1;
            crc >>= 1;
            if low != 0 {
                crc ^= 0xedb8_8320;
            }
            bit += 1;
        }
        at += 1;
    }
    !crc
}

pub const fn hash40(text: &str) -> u64 {
    let bytes = text.as_bytes();
    ((bytes.len() as u64) << 32) + crc32(bytes) as u64
}

pub const INDEX_NONE: i32 = -1;

pub const VTABLE_SLOT_DELETING_DTOR: usize = 1;

pub const VTABLE_SLOT_SET_FOCUS: usize = 0x70 / 8;

pub unsafe fn vtable_call<F: Copy>(object: *const (), slot: usize) -> F {
    let vtable = *(object as *const *const usize);
    let entry = *vtable.add(slot);
    mem::transmute_copy(&entry)
}

#[repr(C)]
pub struct CppVector<T> {
    start: *mut T,
    end: *mut T,
    eos: *mut T,
}

impl<T> CppVector<T> {
    pub fn len(&self) -> usize {
        if self.start.is_null() {
            return 0;
        }
        unsafe { self.end.offset_from(self.start) as usize }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_slice(&self) -> &[T] {
        if self.start.is_null() {
            return &[];
        }
        unsafe { slice::from_raw_parts(self.start, self.len()) }
    }
}

const _: () = assert!(size_of::<CppVector<u8>>() == 0x18);

#[repr(C)]
pub struct RbNode {
    left: *mut RbNode,
    right: *mut RbNode,
    parent: *mut RbNode,
    colour: u8,
    _pad: [u8; 7],
}

const _: () = assert!(size_of::<RbNode>() == 0x20);

#[repr(C)]
pub struct RbTree<K, V> {
    begin: *mut RbNode,
    root: *mut RbNode,
    size: usize,
    _marker: PhantomData<fn() -> (K, V)>,
}

#[repr(C)]
struct RbNodeCore<K, V> {
    node: RbNode,
    key: K,
    value: V,
}

impl<K: Copy + PartialOrd, V: Copy> RbTree<K, V> {
    const LAYOUT: () = {
        assert!(offset_of!(RbNodeCore<K, V>, key) == 0x20);
        assert!(offset_of!(RbNodeCore<K, V>, value) == 0x28);
    };

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    unsafe fn core_of(node: *const RbNode) -> *const RbNodeCore<K, V> {
        let () = Self::LAYOUT;
        node.cast::<RbNodeCore<K, V>>()
    }

    pub unsafe fn lower_bound(&self, key: K) -> Option<V> {
        let base = self as *const Self as *const RbNode;
        let mut node = self.begin;
        let mut found = base;

        while !node.is_null() {
            let node_key = (*Self::core_of(node)).key;
            if key <= node_key {
                found = node;
            }
            node = if node_key < key { (*node).right } else { (*node).left };
        }

        if found == base || (*Self::core_of(found)).key > key {
            return None;
        }
        Some((*Self::core_of(found)).value)
    }
}

#[repr(C, align(8))]
pub struct StdFunction<V: 'static> {
    vtable: *const V,
    inline: [u8; 0x18],
    target: *mut (),
    tail: [u8; 8],
    _pin: PhantomPinned,
}

const _: () = assert!(size_of::<StdFunction<u8>>() == 0x30);

impl<V: 'static> StdFunction<V> {
    pub const fn zeroed() -> StdFunction<V> {
        StdFunction {
            vtable: std::ptr::null(),
            inline: [0; 0x18],
            target: std::ptr::null_mut(),
            tail: [0; 8],
            _pin: PhantomPinned,
        }
    }

    pub unsafe fn point_at_self(&mut self, vtable: &'static V) {
        self.vtable = vtable as *const V;
        self.target = self as *mut Self as *mut ();
    }
}
