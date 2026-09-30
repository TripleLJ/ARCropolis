use std::ffi::CStr;

use crate::{
    data::settings::{level_label, logging_level_index, set_logging_level, LEVELS},
    game::{
        layout::{debug_name, play_animation, write_text, ASSIGN_STANDARD, Buttons, LayoutRootCell, PartsHandle, VirtualButton},
        scene::{Scene, SceneChanger, SceneInitContext},
        scene_vtable::{SceneImpl, SceneVtable},
        selector::{ButtonSelectorCell, SelectorConfig, BUTTON_ID_NONE},
    },
    labels,
    screen::{ExitStep, HeaderStyle, Screen, ScreenEvent},
    scenes::EXIT_BACK,
};

static VTABLE: SceneVtable = SceneVtable::of::<ArcadiaLogLevelScene>();

const LAYOUT_PATH: &str = "ui/layout/menu/arcadia/arcadia_loglevel/layout.arc";

const _: () = assert!(crate::game::hash40(LAYOUT_PATH) == 0x32_e0dc_f13c);

static SELECTOR_GROUP: &[u8] = b"arcadia_lvl_btns\0";

static BUTTON_NAME_FMT: &[u8] = b"set_btn_%02d\0";

static BUTTON_TEXT_PANE: &[u8] = b"set_txt_00\0";

static CHECK_ANIM: &[u8] = b"Check\0";
const CHECK_ON: f32 = 1.0;
const CHECK_OFF: f32 = 0.0;

const EXIT_LOG_LEVEL_PICKED: u32 = 1;

static ASSIGN: [(VirtualButton, Buttons); 4] = [ASSIGN_STANDARD[0], ASSIGN_STANDARD[1], ASSIGN_STANDARD[4], ASSIGN_STANDARD[5]];

#[repr(C)]
pub struct ArcadiaLogLevelScene {
    base: Scene,

    selector: Option<ButtonSelectorCell>,
    screen: Screen,

    checked: usize,
    built: bool,
    exit_requested: bool,
}

unsafe impl SceneImpl for ArcadiaLogLevelScene {
    const NAME: &'static CStr = c"ArcadiaLogLevelScene";

    fn create() -> Box<ArcadiaLogLevelScene> {
        Box::new(ArcadiaLogLevelScene {
            base: Scene::new(&VTABLE),
            selector: None,
            screen: Screen::new("log level", LAYOUT_PATH, HeaderStyle::HEADER_OPTION, labels::HDR_SCENE_LEVEL, true),
            checked: 0,
            built: false,
            exit_requested: false,
        })
    }

    fn initialize(&mut self, ctx: &SceneInitContext) {
        self.screen.initialize(ctx);

        self.checked = logging_level_index();
        debug!("Log level starting on '{}'", level_label(self.checked));
    }

    fn update(&mut self, _changer: &mut SceneChanger) {
        match self.screen.poll() {
            ScreenEvent::LayoutReady => {
                self.build_widgets();
                self.screen.activate();
                self.screen.reveal();
            },
            ScreenEvent::GaveUp => self.screen.leave(EXIT_BACK),
            ScreenEvent::Nothing => {},
        }

        if self.built {
            self.screen.tick();
            let row = self.current_row();
            self.screen.footer_line(row, labels::level_footer);
            self.poll_input();
        }
    }

    fn request_exit(&mut self) {
        self.screen.request_exit();
        self.exit_requested = true;
    }

    fn is_exit_finished(&mut self) -> bool {
        if !self.exit_requested {
            return false;
        }

        match self.screen.exit_step() {
            ExitStep::Waiting => false,
            ExitStep::Teardown => {
                self.selector = None;
                self.screen.teardown();
                true
            },
            ExitStep::Done => true,
        }
    }
}

impl ArcadiaLogLevelScene {
    fn build_widgets(&mut self) {
        let Some(cell) = self.screen.root_mut() else {
            return;
        };

        if let Some(view) = cell.view0() {
            for &(button, mask) in &ASSIGN {
                view.set_assign(button, mask);
            }
        }
        unsafe { cell.set_enable_input(true) };

        let mut config = SelectorConfig::new();
        unsafe { config.install_defaults() };

        let selector = self.selector.get_or_insert_with(ButtonSelectorCell::new);
        unsafe { selector.create(cell.view_handle(), SELECTOR_GROUP, &config) };

        if selector.is_empty() {
            warn!("Log level selector got nothing, group '{}' is probably not in the bflyt", debug_name(SELECTOR_GROUP));
            return;
        }

        for row in 0..LEVELS.len() as i32 {
            unsafe { selector.setup_button(row, BUTTON_NAME_FMT, row) };
        }
        unsafe { selector.set_focus(true) };
        unsafe { selector.select_button(self.checked as i32, 1) };

        let count = unsafe { selector.button_count() };
        self.built = true;
        info!("Log level up with {} buttons, cursor on level {} '{}'", count, self.checked, level_label(self.checked));

        set_row_text(cell);

        for row in 0..LEVELS.len() {
            show_check(cell, row, row == self.checked);
        }
    }

    fn current_row(&self) -> i32 {
        let Some(selector) = self.selector.as_ref() else {
            return BUTTON_ID_NONE;
        };
        if selector.is_empty() {
            return BUTTON_ID_NONE;
        }

        let state = unsafe { selector.state() };
        if state.selected >= 0 {
            state.selected
        } else {
            state.focus
        }
    }

    fn poll_input(&mut self) {
        let state = {
            let Some(selector) = self.selector.as_ref() else {
                return;
            };
            if selector.is_empty() {
                return;
            }
            unsafe { selector.state() }
        };

        if state.cancelled {
            self.screen.leave(EXIT_BACK);
            return;
        }

        if let Some(row) = state.decided {
            self.pick_level(row);
        }
    }

    fn pick_level(&mut self, level: i32) {
        if level < 0 || level as usize >= LEVELS.len() {
            return;
        }
        let level = level as usize;

        if let Err(err) = set_logging_level(level) {
            warn!("Log level: could not write logging_level, {}", err);
        }

        self.move_check(level);
        info!("Log level picked '{}'", level_label(level));
        self.screen.leave(EXIT_LOG_LEVEL_PICKED);
    }

    fn move_check(&mut self, level: usize) {
        let was = self.checked;
        self.checked = level;

        let Some(cell) = self.screen.root_mut() else {
            return;
        };
        if was != level {
            show_check(cell, was, false);
        }
        show_check(cell, level, true);
    }
}

fn set_row_text(cell: &LayoutRootCell) {
    let view = cell.view_handle();

    for row in 0..LEVELS.len() {
        let Some(mut parts) = (unsafe { PartsHandle::get_checked(view, BUTTON_NAME_FMT, row as u64) }) else {
            warn!("No row part for level {}, its label stays as shipped", row);
            continue;
        };

        unsafe { write_text(parts.view_handle(), BUTTON_TEXT_PANE, level_label(row)) };
    }
}

fn show_check(cell: &LayoutRootCell, row: usize, on: bool) {
    let Some(mut parts) = (unsafe { PartsHandle::get_checked(cell.view_handle(), BUTTON_NAME_FMT, row as u64) }) else {
        warn!("No row part for level {}, no tick", row);
        return;
    };

    let frame = if on { CHECK_ON } else { CHECK_OFF };
    let payload = unsafe { parts.view_handle().as_ref() }.map_or(std::ptr::null_mut(), |view| view.payload());
    unsafe { play_animation(payload, CHECK_ANIM, frame) };
}
