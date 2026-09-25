use std::ffi::CStr;

use crate::{
    game::{
        layout::{debug_name, write_text, ASSIGN_STANDARD, LayoutRootCell, PartsHandle},
        scene::{Scene, SceneChanger, SceneInitContext},
        scene_vtable::{SceneImpl, SceneVtable},
        selector::BUTTON_ID_NONE,
    },
    labels,
    screen::{ExitStep, HeaderStyle, Screen, ScreenEvent},
    scenes::EXIT_BACK,
};

static VTABLE: SceneVtable = SceneVtable::of::<ArcadiaTopScene>();

const LAYOUT_PATH: &str = "ui/layout/menu/arcadia/arcadia_top/layout.arc";

const _: () = assert!(crate::game::hash40(LAYOUT_PATH) == 0x2d_5743_85ea);

static SELECTOR_GROUP: &[u8] = b"arcadia_top_btns\0";

static BUTTON_NAME_FMT: &[u8] = b"set_parts_btn%02d\0";

static BUTTON_TEXT_PANE: &[u8] = b"set_txt_val\0";

static ROWS: [&str; 3] = ["Mod Manager", "Workspace", "Configuration"];

const HEADER_SCENE: &str = "";

fn exit_code_for_row(row: i32) -> u32 {
    row as u32 + 1
}

#[repr(C)]
pub struct ArcadiaTopScene {
    base: Scene,

    selector: Option<crate::game::selector::ButtonSelectorCell>,
    screen: Screen,

    restore_row: i32,

    built: bool,

    exit_requested: bool,
}

unsafe impl SceneImpl for ArcadiaTopScene {
    const NAME: &'static CStr = c"ArcadiaTopScene";

    fn create() -> Box<ArcadiaTopScene> {
        Box::new(ArcadiaTopScene {
            base: Scene::new(&VTABLE),
            selector: None,
            screen: Screen::new("ArcadiaTopScene", LAYOUT_PATH, HeaderStyle::HEADER_HELP, HEADER_SCENE, true),
            restore_row: BUTTON_ID_NONE,
            built: false,
            exit_requested: false,
        })
    }

    fn initialize(&mut self, ctx: &SceneInitContext) {
        self.screen.initialize(ctx);

        self.restore_row = match ctx.init_code() {
            Some(code) if (code as usize) < ROWS.len() => code as i32,
            _ => BUTTON_ID_NONE,
        };

        debug!("Hub starting on row {}", self.restore_row.max(0));
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
            self.screen.footer_line(row, labels::hub_footer);
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

impl ArcadiaTopScene {
    fn build_widgets(&mut self) {
        let Some(cell) = self.screen.root_mut() else {
            return;
        };

        if let Some(view) = cell.view0() {
            for &(button, mask) in &ASSIGN_STANDARD {
                view.set_assign(button, mask);
            }
        }
        unsafe { cell.set_enable_input(true) };

        let mut config = crate::game::selector::SelectorConfig::new();
        unsafe { config.install_defaults() };

        let selector = self.selector.get_or_insert_with(crate::game::selector::ButtonSelectorCell::new);
        unsafe { selector.create(cell.view_handle(), SELECTOR_GROUP, &config) };

        if selector.is_empty() {
            warn!("Hub selector got nothing, group '{}' is probably not in the bflyt", debug_name(SELECTOR_GROUP));
            return;
        }

        for row in 0..ROWS.len() as i32 {
            unsafe { selector.setup_button(row, BUTTON_NAME_FMT, row) };
        }
        unsafe { selector.set_focus(true) };

        let row = self.restore_row.max(0);
        unsafe { selector.select_button(row, 1) };

        let count = unsafe { selector.button_count() };
        self.built = true;
        info!("Hub up with {} buttons, cursor on row {}", count, row);

        set_button_text(cell);
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
        let Some(selector) = self.selector.as_ref() else {
            return;
        };
        if selector.is_empty() {
            return;
        }

        let state = unsafe { selector.state() };

        if state.cancelled {
            self.screen.leave(EXIT_BACK);
            return;
        }

        if let Some(row) = state.decided {
            if (row as usize) < ROWS.len() {
                debug!("Hub row {} '{}' decided", row, ROWS[row as usize]);
                self.screen.leave(exit_code_for_row(row));
            }
        }
    }
}

fn set_button_text(cell: &LayoutRootCell) {
    let view = cell.view_handle();

    for (row, label) in ROWS.iter().enumerate() {
        let Some(mut parts) = (unsafe { PartsHandle::get_checked(view, BUTTON_NAME_FMT, row as u64) }) else {
            warn!("No button part for row {}, its label stays as shipped", row);
            continue;
        };

        unsafe { write_text(parts.view_handle(), BUTTON_TEXT_PANE, label) };
    }
}
