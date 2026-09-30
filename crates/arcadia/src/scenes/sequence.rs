use std::{ffi::CStr, sync::LazyLock};

use crate::{
    game::hybrid::MENU_STATE_HOW_TO_PLAY,
    game::{
        fade::Fade,
        hybrid::{self, HybridScene},
        scene::{ExitCode, FixedBaseString64, Scene, SceneChanger, SceneInitContext, SceneParameter},
        scene_vtable::{SceneImpl, SceneVtable},
    },
    scenes::{
        self, changelog::ArcadiaChangelogScene, config::ArcadiaConfigScene, log_level::ArcadiaLogLevelScene, mods::ArcadiaScene, top::ArcadiaTopScene,
        workspace::ArcadiaWorkspaceScene, EXIT_BACK, EXIT_CONFIG, EXIT_LOG_LEVEL, EXIT_MOD_MANAGER, EXIT_WORKSPACE, EXIT_WORKSPACE_EDIT_BASE,
    },
    Request,
};

static VTABLE: LazyLock<SceneVtable> = LazyLock::new(|| hybrid::vtable(dtor, deleting_dtor, on_enter, on_exit, tick));

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HubState {
    Start,
    Hub,
    ModManager,

    Workspace,

    WorkspaceMods,
    Config,
    LogLevel,

    Changelog,

    Leaving,
    Done,
}

#[repr(C)]
pub struct ArcadiaSequenceScene {
    pub base: HybridScene,
    pub state: HubState,

    pub state_frames: u32,

    pub pending_row: i32,

    pub pending_config_row: i32,

    pub pending_workspace: i32,
    pub table_registered: bool,

    pub direct_exit: bool,
}

const _: () = assert!(std::mem::offset_of!(ArcadiaSequenceScene, base) == 0);

unsafe impl SceneImpl for ArcadiaSequenceScene {
    const NAME: &'static CStr = c"ArcadiaSequenceScene";

    fn create() -> Box<ArcadiaSequenceScene> {
        let mut scene = Box::new(ArcadiaSequenceScene {
            base: HybridScene::new(&VTABLE),
            state: HubState::Start,
            state_frames: 0,
            pending_row: -1,
            pending_config_row: -1,
            pending_workspace: -1,
            table_registered: false,
            direct_exit: false,
        });

        unsafe { scene.base.construct() };
        scene
    }

    fn initialize(&mut self, _ctx: &SceneInitContext) {}

    fn update(&mut self, _changer: &mut SceneChanger) {}

    fn request_exit(&mut self) {}

    fn is_exit_finished(&mut self) -> bool {
        true
    }
}

impl ArcadiaSequenceScene {
    fn enter(&mut self, state: HubState) {
        self.state = state;
        self.state_frames = 0;
    }

    unsafe fn push(&mut self, target: &CStr, state: HubState, code: Option<u32>) {
        let name = FixedBaseString64::new(target.to_str().unwrap_or_default());
        let parameter = code.map(SceneParameter::code);

        self.base.driver.push_front(&name, parameter.as_ref());
        self.enter(state);

        debug!("Opening '{}'", name.as_str());
    }

    unsafe fn open_hub(&mut self) {
        let row = self.pending_row.max(0) as u32;
        self.push(ArcadiaTopScene::NAME, HubState::Hub, Some(row))
    }

    unsafe fn return_up(&mut self) {
        if self.direct_exit {
            self.enter(HubState::Leaving)
        } else {
            self.open_hub()
        }
    }

    unsafe fn open_mod_manager(&mut self, workspace: i32, state: HubState) {
        self.push(ArcadiaScene::NAME, state, Some((workspace + 1) as u32))
    }

    unsafe fn open_config(&mut self) {
        self.push(ArcadiaConfigScene::NAME, HubState::Config, Some(self.pending_config_row.max(0) as u32))
    }

    unsafe fn open_log_level(&mut self) {
        self.push(ArcadiaLogLevelScene::NAME, HubState::LogLevel, None)
    }

    unsafe fn open_workspace(&mut self) {
        self.push(ArcadiaWorkspaceScene::NAME, HubState::Workspace, Some(self.pending_workspace.max(0) as u32))
    }

    unsafe fn open_changelog(&mut self) {
        self.push(ArcadiaChangelogScene::NAME, HubState::Changelog, None)
    }

    fn finished_child(&mut self) -> Option<u32> {
        if self.state_frames <= 1 || !self.base.driver.drained() {
            return None;
        }

        Some(self.base.driver.exit_code)
    }

    unsafe fn unregister_table(&mut self) {
        if !self.table_registered {
            return;
        }

        self.table_registered = false;
        self.base.driver.unregister_table(scenes::HUB_TABLE.rows());
    }
}

unsafe fn scene_of(this: *mut Scene) -> Option<&'static mut ArcadiaSequenceScene> {
    this.cast::<ArcadiaSequenceScene>().as_mut()
}

unsafe extern "C" fn dtor(_this: *mut Scene) {}

unsafe extern "C" fn deleting_dtor(this: *mut Scene) {
    if this.is_null() {
        return;
    }

    let mut scene = Box::from_raw(this.cast::<ArcadiaSequenceScene>());
    scene.unregister_table();
    scene.base.destruct();

    debug!("The ARCropolis menus are closed");
}

unsafe extern "C" fn on_enter(this: *mut Scene, _ctx: *const SceneInitContext) {
    let Some(scene) = scene_of(this) else {
        return;
    };

    scene.base.driver.register_table(scenes::HUB_TABLE.rows());
    scene.table_registered = true;

    match crate::take_request() {
        None | Some(Request::Hub) => {},
        Some(Request::ModManager) => {
            scene.direct_exit = true;
            scene.open_mod_manager(-1, HubState::ModManager)
        },
        Some(Request::Config) => {
            scene.direct_exit = true;
            scene.open_config()
        },
        Some(Request::Changelog) => {
            scene.direct_exit = true;

            if crate::has_pending_notes() {
                scene.open_changelog()
            } else {
                warn!("The update notes were asked for but nothing was left for them, closing again");
                scene.enter(HubState::Leaving)
            }
        },
    }

    info!("ARCropolis menus opened");
}

unsafe extern "C" fn on_exit(this: *mut Scene) {
    if let Some(scene) = scene_of(this) {
        scene.unregister_table()
    }
}

unsafe extern "C" fn tick(this: *mut Scene, changer: *mut SceneChanger) {
    let Some(scene) = scene_of(this) else {
        return;
    };

    scene.state_frames += 1;

    match scene.state {
        HubState::Start => {
            if let Some(fade) = Fade::get() {
                fade.fade_out_instant()
            }

            scene.open_hub()
        },

        HubState::Hub => {
            let Some(code) = scene.finished_child() else {
                return;
            };

            scene.pending_row = code as i32 - 1;
            match code {
                EXIT_BACK => scene.enter(HubState::Leaving),
                EXIT_MOD_MANAGER => scene.open_mod_manager(-1, HubState::ModManager),
                EXIT_WORKSPACE => {
                    scene.pending_workspace = 0;
                    scene.open_workspace()
                },
                EXIT_CONFIG => {
                    scene.pending_config_row = 0;
                    scene.open_config()
                },
                other => {
                    warn!("The hub left with {}, taking it as back", other);
                    scene.enter(HubState::Leaving)
                },
            }
        },

        HubState::ModManager => {
            if scene.finished_child().is_some() {
                scene.return_up()
            }
        },

        HubState::Config => {
            if let Some(code) = scene.finished_child() {
                match code {
                    EXIT_LOG_LEVEL => scene.open_log_level(),
                    _ => scene.return_up(),
                }
            }
        },

        HubState::LogLevel => {
            if scene.finished_child().is_some() {
                scene.open_config()
            }
        },

        HubState::Workspace => {
            if let Some(code) = scene.finished_child() {
                if code == EXIT_BACK {
                    scene.return_up()
                } else {
                    scene.pending_workspace = (code - EXIT_WORKSPACE_EDIT_BASE) as i32;
                    scene.open_mod_manager(scene.pending_workspace, HubState::WorkspaceMods)
                }
            }
        },

        HubState::Changelog => {
            if scene.finished_child().is_some() {
                scene.return_up()
            }
        },

        HubState::WorkspaceMods => {
            if scene.finished_child().is_some() {
                scene.open_workspace()
            }
        },

        HubState::Leaving => {
            scene.enter(HubState::Done);

            let Some(outer) = changer.as_mut().and_then(|changer| changer.owner()) else {
                warn!("No driver on the scene changer, the menus cannot close");
                return;
            };

            let code = if crate::hooks::menu_state() == Some(MENU_STATE_HOW_TO_PLAY) { ExitCode::Code2 as u32 } else { ExitCode::Default as u32 };
            debug!("Handing the main menu back with exit code {code}");
            outer.exit_active_scene(code)
        },

        HubState::Done => {},
    }
}
