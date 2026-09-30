use std::{ffi::CStr, ptr::addr_of_mut};

use crate::{
    data::settings::{flag, level_label, logging_level_index, set_flag, set_logging_level, FLAG_ROWS, LEVELS},
    game::{
        frame::{ComFrame, FooterButtonKind},
        layout::{debug_name, play_animation, with_nul, ANIM_FRAME_STILL, ASSIGN_STANDARD, Buttons, LayoutViewHandle, PaneHandle, PartsHandle, TEXT_FMT, VirtualButton},
        list::{RowBinder, RowBinderVtable, Scroller, CLAMP_TO_LIST, ITEM_NONE, JUMP_IMMEDIATE},
        scene::{FixedBaseString64, Scene, SceneChanger, SceneInitContext},
        scene_vtable::{SceneImpl, SceneVtable},
    },
    labels,
    screen::{ExitStep, HeaderStyle, Screen, ScreenEvent},
    scenes::{EXIT_BACK, EXIT_LOG_LEVEL},
};

static VTABLE: SceneVtable = SceneVtable::of::<ArcadiaConfigScene>();

static BINDER_VTABLE: RowBinderVtable = RowBinderVtable::new(bind_row);

const LAYOUT_PATH: &str = "ui/layout/menu/arcadia/arcadia_config/layout.arc";

const _: () = assert!(crate::game::hash40(LAYOUT_PATH) == 0x30_7c8d_e710);

static SCROLL_GROUP: &[u8] = b"scroll\0";

const ROW_LOGGING_LEVEL: usize = 0;
const ROW_COUNT: usize = 1 + FLAG_ROWS.len();

static ROW_LABEL_PANE: &[u8] = b"set_txt_title\0";

static ROW_VALUE_PART: &[u8] = b"set_parts_val_%02d_cursor\0";

static PILL_LEFT_PART: &[u8] = b"set_parts_sub_btn_00\0";
static PILL_RIGHT_PART: &[u8] = b"set_parts_sub_btn_01\0";

static PILL_TEXT_PANE: &[u8] = b"set_txt_00\0";

static PILL_ON: &[u8] = b"BlinkOn\0";
static PILL_OFF: &[u8] = b"BlinkOff\0";

const PILL_LEFT_X: f32 = 254.0;
const PILL_RIGHT_X: f32 = 518.0;
const PILL_CENTRE_X: f32 = 386.0;
const PILL_Y: f32 = 0.0;

static FOOTER_BUTTON_LABEL: &str = "mnu_footer_ok";

static ASSIGN: [(VirtualButton, Buttons); 8] = [
    ASSIGN_STANDARD[0],
    ASSIGN_STANDARD[1],
    (VirtualButton::Left, Buttons(0)),
    (VirtualButton::Right, Buttons(0)),
    ASSIGN_STANDARD[4],
    ASSIGN_STANDARD[5],
    (VirtualButton::Extra0, Buttons::NAV_LEFT),
    (VirtualButton::Extra1, Buttons::NAV_RIGHT),
];

#[repr(C)]
pub struct ArcadiaConfigScene {
    base: Scene,

    scroller: Option<Scroller>,
    screen: Screen,

    binder: RowBinder,

    restore_row: i32,
    revealed: bool,
    exit_requested: bool,

    last_index: i32,

    footer_button_lit: bool,

    footer_button_set: bool,

    footer_button_previous: Option<(i32, FixedBaseString64)>,
}

unsafe impl SceneImpl for ArcadiaConfigScene {
    const NAME: &'static CStr = c"ArcadiaConfigScene";

    fn create() -> Box<ArcadiaConfigScene> {
        Box::new(ArcadiaConfigScene {
            base: Scene::new(&VTABLE),
            scroller: None,
            screen: Screen::new("config", LAYOUT_PATH, HeaderStyle::HEADER_OPTION, labels::HDR_SCENE_CONFIG, true),
            binder: RowBinder::empty(),
            restore_row: ITEM_NONE,
            revealed: false,
            exit_requested: false,
            last_index: ITEM_NONE,
            footer_button_lit: false,
            footer_button_set: false,
            footer_button_previous: None,
        })
    }

    fn initialize(&mut self, ctx: &SceneInitContext) {
        self.screen.initialize(ctx);

        self.restore_row = match ctx.init_code() {
            Some(code) if (code as usize) < ROW_COUNT => code as i32,
            _ => ITEM_NONE,
        };
        debug!("Config starting on row {}", self.restore_row.max(0));
    }

    fn update(&mut self, _changer: &mut SceneChanger) {
        match self.screen.poll() {
            ScreenEvent::LayoutReady => {
                self.build_widgets();
                self.screen.activate();

                self.ensure_footer_button();
                self.screen.reveal();
                self.revealed = true;
            },
            ScreenEvent::GaveUp => self.screen.leave(EXIT_BACK),
            ScreenEvent::Nothing => {},
        }

        if self.revealed {
            self.ensure_footer_button();
            self.screen.tick();
            let row = if self.footer_button_lit { ITEM_NONE } else { self.current_row() };
            self.screen.footer_line(row, labels::config_footer);
            self.poll_input();
        }
    }

    fn request_exit(&mut self) {
        if self.footer_button_set {
            if let Some(frame) = unsafe { ComFrame::get() } {
                frame.highlight_footer_button(false);
                match self.footer_button_previous.take() {
                    Some((kind, label)) if kind != FooterButtonKind::None as i32 => frame.set_footer_button(kind, &label),
                    _ => frame.set_footer_button(FooterButtonKind::None as i32, &FixedBaseString64::new("")),
                }
            }
            self.footer_button_lit = false;
        }

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
                self.scroller = None;
                self.screen.teardown();
                true
            },
            ExitStep::Done => true,
        }
    }
}

impl ArcadiaConfigScene {
    fn build_widgets(&mut self) {
        let this = self as *mut ArcadiaConfigScene;

        let Some(cell) = self.screen.root_mut() else {
            return;
        };

        if let Some(view) = cell.view0() {
            for &(button, mask) in &ASSIGN {
                view.set_assign(button, mask);
            }
        }
        unsafe { cell.set_enable_input(true) };
        let view_handle = cell.view_handle();

        self.binder.vtable = &BINDER_VTABLE;
        self.binder.owner = this.cast();
        let binder_ptr = addr_of_mut!(self.binder);

        let row = self.restore_row.max(0);
        let scroller = self.scroller.get_or_insert_with(Scroller::new);
        if !unsafe { scroller.setup_list(view_handle, SCROLL_GROUP, ROW_COUNT as i32, row, false, binder_ptr) } {
            warn!("Config scroller got nothing, group '{}' is probably not in the bflyt", debug_name(SCROLL_GROUP));
            return;
        }

        info!("Config up with {} rows, cursor on row {}", ROW_COUNT, row);
    }

    fn ensure_footer_button(&mut self) {
        if self.footer_button_set {
            return;
        }

        let Some(frame) = (unsafe { ComFrame::get() }) else {
            return;
        };
        if !frame.bar_ready() {
            return;
        }

        self.footer_button_previous = Some((frame.footer_button_kind(), frame.footer_button_label()));
        frame.set_footer_button(FooterButtonKind::Large100 as i32, &FixedBaseString64::new(FOOTER_BUTTON_LABEL));
        self.footer_button_set = true;
        debug!("Config OK button placed on the bar");
    }

    fn current_row(&self) -> i32 {
        match self.scroller.as_ref() {
            Some(scroller) if !scroller.is_empty() => unsafe { scroller.current_index() },
            _ => ITEM_NONE,
        }
    }

    fn poll_input(&mut self) {
        let (cancel, up, down, left, right) = {
            let Some(view) = self.screen.root_mut().and_then(|cell| cell.view0()) else {
                return;
            };
            (
                view.pressed(VirtualButton::Cancel),
                view.pressed(VirtualButton::Up),
                view.pressed(VirtualButton::Down),
                view.pressed(VirtualButton::Extra0),
                view.pressed(VirtualButton::Extra1),
            )
        };

        let (current, decided, past_bottom) = {
            let Some(scroller) = self.scroller.as_ref() else {
                return;
            };
            if scroller.is_empty() {
                return;
            }
            unsafe { (scroller.current_index(), scroller.decided_index(), scroller.pushed_past_bottom()) }
        };

        if cancel {
            self.screen.leave(EXIT_BACK);
            return;
        }

        if unsafe { ComFrame::get() }.is_some_and(|frame| frame.footer_decided()) {
            self.screen.leave(EXIT_BACK);
            return;
        }

        let last_row = ROW_COUNT as i32 - 1;

        if self.footer_button_lit {
            if up || down {
                let target = if up { last_row } else { 0 };
                if let Some(scroller) = self.scroller.as_mut() {
                    unsafe { scroller.set_current_index(target, JUMP_IMMEDIATE, CLAMP_TO_LIST) };
                }
                self.last_index = target;
                self.focus_footer_button(false);
            }
            return;
        }

        let down_at_end = down && self.last_index == last_row && (current == last_row || current == 0);
        let up_at_top = up && self.last_index == 0 && (current == 0 || current == last_row);
        if past_bottom || down_at_end || up_at_top {
            if current != last_row {
                if let Some(scroller) = self.scroller.as_mut() {
                    unsafe { scroller.set_current_index(last_row, JUMP_IMMEDIATE, CLAMP_TO_LIST) };
                }
            }
            self.last_index = last_row;
            self.focus_footer_button(true);
            return;
        }

        if current != self.last_index {
            self.last_index = current;
            debug!("Config row {} of {}", current, ROW_COUNT);
        }

        if decided != ITEM_NONE {
            if decided as usize == ROW_LOGGING_LEVEL {
                self.screen.leave(EXIT_LOG_LEVEL);
            } else {
                self.toggle_row(decided);
            }
            return;
        }

        if left {
            self.step_value(current, -1);
        } else if right {
            self.step_value(current, 1);
        }
    }

    fn focus_footer_button(&mut self, on: bool) {
        if self.footer_button_lit == on || !self.footer_button_set {
            return;
        }

        if let Some(scroller) = self.scroller.as_ref() {
            unsafe { scroller.set_focus(!on) };
        }
        self.footer_button_lit = on;
        if let Some(frame) = unsafe { ComFrame::get() } {
            frame.highlight_footer_button(on);
        }
        debug!("Config OK button lit {}", on);
    }

    fn step_value(&mut self, row: i32, step: i32) {
        if row == ROW_LOGGING_LEVEL as i32 {
            self.cycle_level(step);
        } else {
            self.toggle_row(row);
        }
    }

    fn cycle_level(&mut self, step: i32) {
        let levels = LEVELS.len() as i32;
        let next = ((logging_level_index() as i32 + step + levels) % levels) as usize;

        if let Err(err) = set_logging_level(next) {
            warn!("Config: could not write logging_level, {}", err);
        }
        debug!("Config logging level -> '{}'", level_label(next));

        self.rebind_row(ROW_LOGGING_LEVEL as i32);
    }

    fn toggle_row(&mut self, row: i32) {
        if row <= ROW_LOGGING_LEVEL as i32 || row as usize >= ROW_COUNT {
            return;
        }

        let key = FLAG_ROWS[row as usize - 1].0;
        let now = !flag(key);
        if let Err(err) = set_flag(key, now) {
            warn!("Config: could not write '{}', {}", key, err);
        }
        debug!("Config '{}' -> {}", key, if now { "ON" } else { "OFF" });

        self.rebind_row(row);
    }

    fn rebind_row(&mut self, row: i32) {
        let binder = addr_of_mut!(self.binder);
        let Some(scroller) = self.scroller.as_ref() else {
            return;
        };
        if let Some((row_view, extra)) = unsafe { scroller.row_for_item(row) } {
            unsafe { bind_row(binder, row, row_view, extra) };
        }
    }
}

unsafe extern "C" fn bind_row(binder: *mut RowBinder, item: i32, row_view: *mut LayoutViewHandle, _extra: *mut u8) {
    if binder.is_null() || row_view.is_null() {
        return;
    }

    let Some(this) = ((*binder).owner as *mut ArcadiaConfigScene).as_mut() else {
        return;
    };
    if item < 0 || item as usize >= ROW_COUNT {
        return;
    }
    let row = item as usize;

    {
        let mut label = PaneHandle::get(row_view, ROW_LABEL_PANE);
        if !label.is_empty() {
            label.set_text_format_str(TEXT_FMT, &with_nul(row_label(row)));
        }
    }

    let slot = {
        let Some(scroller) = this.scroller.as_ref() else {
            return;
        };
        let Some(slot) = scroller.slot_for_view(row_view) else {
            return;
        };
        slot
    };

    bind_value(this, row, slot);
}

unsafe fn bind_value(this: &mut ArcadiaConfigScene, row: usize, slot: usize) {
    let Some(cell) = this.screen.root_mut() else {
        return;
    };

    let Some(mut holder) = PartsHandle::get_checked(cell.view_handle(), ROW_VALUE_PART, slot as u64) else {
        return;
    };
    let holder_view = holder.view_handle();

    let single = row == ROW_LOGGING_LEVEL;
    let on = !single && flag(FLAG_ROWS[row - 1].0);

    let word = if single { level_label(logging_level_index()) } else { "ON" };
    write_pill(holder_view, PILL_LEFT_PART, if single { PILL_CENTRE_X } else { PILL_LEFT_X }, true, word, single || on);

    write_pill(holder_view, PILL_RIGHT_PART, PILL_RIGHT_X, !single, "OFF", !on);
}

unsafe fn write_pill(holder_view: *mut LayoutViewHandle, pill: &[u8], x: f32, visible: bool, word: &str, lit: bool) {
    {
        let handle = PaneHandle::get(holder_view, pill);
        if let Some(pane) = handle.pane().as_mut() {
            pane.set_visible(visible);
            pane.set_translate(x, PILL_Y);
        }
    }

    if !visible {
        return;
    }

    let Some(mut button) = PartsHandle::get_checked(holder_view, pill, 0) else {
        return;
    };

    {
        let mut text = PaneHandle::get(button.view_handle(), PILL_TEXT_PANE);
        if !text.is_empty() && !text.text_box().is_null() {
            text.set_text_format_str(TEXT_FMT, &with_nul(word));
        }
    }

    let payload = button.view_handle().as_ref().map_or(std::ptr::null_mut(), |view| view.payload());
    play_animation(payload, if lit { PILL_ON } else { PILL_OFF }, ANIM_FRAME_STILL);
}

fn row_label(row: usize) -> &'static str {
    if row == ROW_LOGGING_LEVEL {
        "Logging level"
    } else {
        FLAG_ROWS[row - 1].1
    }
}

