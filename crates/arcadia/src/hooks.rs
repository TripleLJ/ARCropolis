use std::{
    ptr::{self, NonNull},
    sync::atomic::{AtomicPtr, Ordering},
};

use crate::{
    game::{
        hybrid::{HybridScene, MainMenuScene, MenuDriverInfo, MAIN_MENU_EXIT_TILE, MENU_STATE_HOW_TO_PLAY},
        scene::{FixedBaseString64, SceneChanger, SceneInitContext, SceneParameter, SceneQueue},
        scene_vtable::SceneImpl,
    },
    offsets,
    scenes::{self, sequence::ArcadiaSequenceScene},
};

static MENU_SCENE: AtomicPtr<HybridScene> = AtomicPtr::new(ptr::null_mut());

const MAIN_MENU_SCENE: &str = "MainMenuScene";

const HOW_TO_PLAY_SCENE: &str = "HowToPlayScene";

const TITLE_SCENE: &str = "TitleScene";

fn menu_scene() -> Option<NonNull<HybridScene>> {
    NonNull::new(MENU_SCENE.load(Ordering::Acquire))
}

pub fn leave_main_menu() -> bool {
    let block = unsafe {
        menu_scene()
            .and_then(|menu| (*menu.as_ptr()).driver.active_as::<MainMenuScene>(MAIN_MENU_SCENE))
            .and_then(|scene| scene.block.as_mut())
    };

    let Some(block) = block else {
        debug!("The main menu is not up, the menus open the next time it is");
        return false;
    };

    block.exit_code = MAIN_MENU_EXIT_TILE;
    debug!("Asked the main menu to leave");
    true
}

pub fn menu_state() -> Option<u32> {
    let menu = unsafe { menu_scene()?.as_ref() };
    let info = unsafe { menu.menu_info.as_ref() }?;
    Some(info.state)
}

#[skyline::hook(offset = offsets::menu_sequence_scene_push_main_menu())]
unsafe fn menu_sequence_scene_push_main_menu(mut info: Option<&mut MenuDriverInfo>, changer: *mut SceneChanger) {
    if let Some(info) = info.as_deref_mut() {
        let drained = info.driver.as_ref().is_some_and(|driver| driver.drained());
        if info.frames_in_state != 0 && drained && crate::requested() && info.set_state(MENU_STATE_HOW_TO_PLAY) {
            debug!("Routing the main menu's exit to the ARCropolis menus");
            return;
        }
    }

    call_original!(info, changer)
}

fn takes_over(target: &FixedBaseString64) -> bool {
    match target.as_str() {
        HOW_TO_PLAY_SCENE => crate::requested() || config::GLOBAL_CONFIG.lock().unwrap().get_flag("arcadia_help_entry"),
        TITLE_SCENE => crate::requested(),
        _ => false,
    }
}

#[skyline::hook(offset = offsets::scene_queue_push_front())]
unsafe fn scene_queue_push_front(
    queue: *mut SceneQueue,
    target: Option<&FixedBaseString64>,
    leaving: Option<&FixedBaseString64>,
    parameters: *const SceneParameter,
) {
    if let Some(name) = target.filter(|name| takes_over(name)) {
        let swapped = FixedBaseString64::new(ArcadiaSequenceScene::NAME.to_str().unwrap_or_default());

        if name.as_str() == TITLE_SCENE {
            debug!("Opening the ARCropolis menus before '{}'", name.as_str());

            call_original!(queue, target, leaving, parameters);
            return call_original!(queue, Some(&swapped), leaving, ptr::null());
        }

        debug!("Opening the ARCropolis menus instead of '{}'", name.as_str());

        return call_original!(queue, Some(&swapped), leaving, parameters);
    }

    call_original!(queue, target, leaving, parameters)
}

#[skyline::hook(offset = offsets::menu_sequence_scene_on_enter())]
unsafe fn menu_sequence_scene_on_enter(mut this: Option<&mut HybridScene>, ctx: *const SceneInitContext) {
    call_original!(this.as_deref_mut(), ctx);

    let Some(menu) = this else {
        return;
    };

    menu.driver.register_table(scenes::MENU_TABLE.rows());
    MENU_SCENE.store(menu as *mut HybridScene, Ordering::Relaxed);

    debug!("The ARCropolis menus are on the main menu's table");
}
