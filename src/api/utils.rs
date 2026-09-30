#[no_mangle]
pub extern "C" fn arcrop_show_mod_manager() {
    debug!("arcrop_show_mod_manager -> Function called");

    #[cfg(feature = "ui")]
    {
        arcadia::request(arcadia::Request::ModManager);
        // opens right away from the main menu, anywhere else the request waits for the next one
        arcadia::open_from_menu();
    }
}

#[no_mangle]
pub extern "C" fn arcrop_show_config_editor() {
    debug!("arcrop_show_config_editor -> Function called");

    #[cfg(feature = "ui")]
    {
        arcadia::request(arcadia::Request::Config);
        arcadia::open_from_menu();
    }
}

#[no_mangle]
pub extern "C" fn arcrop_show_main_menu() {
    debug!("arcrop_show_main_menu -> Function called");

    #[cfg(feature = "ui")]
    {
        arcadia::request(arcadia::Request::Hub);
        arcadia::open_from_menu();
    }
}
