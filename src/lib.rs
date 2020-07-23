#![feature(proc_macro_hygiene)]

use std::fs;
use skyline::{nn, hook, install_hooks};
use skyline::logging::hex_dump_ptr;

mod replacement_files;
use replacement_files::ARC_FILES;

mod hashes;
mod resource;
use resource::*;

macro_rules! log {
    ($($arg:tt)*) => {
        // Uncomment to enable logging
        //println!($($arg)*);
    }
}

#[allow(unused_variables)]
fn print_table1idx_info(table1_idx: u32) {
    let loaded_tables = LoadedTables::get_instance();
    let table1entry = match loaded_tables.table_1().get(table1_idx as usize) {
        Some(entry) => entry,
        None => {
            log!("Could not fetch table1entry.");
            return;
        }
    };

    let table2entry = match loaded_tables.table_2().get(table1entry.table2_index as usize) {
            Some(entry) => entry,
            None => {
                log!("Could not fetch the table2entry.");
                return;
            }
        };

    let hash = loaded_tables.get_hash_from_t1_index(table1_idx);

    // if !hashes::get(hash.as_u64()).unwrap_or(&"none").ends_with("nutexb") {
    //     return;
    // }

    log!(
        "Filename: {}, State: {}, Flags: {}, RefCount: {:x?}, Data loaded: {}",
        hashes::get(hash.as_u64()).unwrap_or(&"none"),
        table2entry.state,
        table2entry.flags,
        table2entry.ref_count,
        !table2entry.data.is_null()
    );
}

#[hook(offset = 0x32545a0)]
unsafe fn idk(res_state: *const u64, table1_idx: u32, flag_related: u32) {
    original!()(res_state, table1_idx, flag_related);

    let loaded_tables = LoadedTables::get_instance();
    let mutex = loaded_tables.mutex;
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();

    if let Some(path) = ARC_FILES.get_from_hash(hash) {
        log!("--- [Idk] ---");
        log!("File hash matching, path: {}", path.display());

        let mut table2entry = loaded_tables.get_t2_mut(table1_idx).unwrap();

        if table2entry.state == FileState::Loaded {
            return;
        }

        log!("Replacing...");

        nn::os::LockMutex(mutex);

        let data = fs::read(path).unwrap().into_boxed_slice();
        let data = Box::leak(data);
        table2entry.data = data.as_ptr();
        table2entry.state = FileState::Loaded;
        table2entry.flags = 45;

        nn::os::UnlockMutex(mutex);

        print_table1idx_info(table1_idx);
    }
}

#[hook(offset = 0x324e9f0)]
unsafe fn add_idx_to_table1_and_table2(loaded_table: *const LoadedTables, table1_idx: u32) {
    original!()(loaded_table, table1_idx);

    let loaded_tables = LoadedTables::get_instance();
    let mutex = loaded_tables.mutex;
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();

    if let Some(path) = ARC_FILES.get_from_hash(hash) {
        log!("--- [AddIdx] ---");
        log!("File hash matching, path: {}", path.display());

        let mut table2entry = loaded_tables.get_t2_mut(table1_idx).unwrap();

        if table2entry.state == FileState::Loaded || table2entry.state == FileState::Unloaded && !table2entry.data.is_null() {
            return;
        }

        log!("Replacing...");

        nn::os::LockMutex(mutex);

        let data = fs::read(path).unwrap().into_boxed_slice();
        let data = Box::leak(data);
        table2entry.data = data.as_ptr();
        table2entry.state = FileState::Loaded;
        table2entry.flags = 43;

        nn::os::UnlockMutex(mutex);

        print_table1idx_info(table1_idx);
    }
}

#[skyline::main(name = "replace")]
pub fn main() {
    lazy_static::initialize(&ARC_FILES);
    hashes::init();

    install_hooks!(idk, add_idx_to_table1_and_table2);
    log!("File replacement mod installed.");
}
