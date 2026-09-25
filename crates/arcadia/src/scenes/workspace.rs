use std::{
    ffi::CStr,
    ptr::{addr_of_mut, NonNull},
};

use crate::{
    data::workspaces::{self, Workspace},
    game::{
        keyboard,
        layout::{debug_name, play_animation, with_nul, write_text, Buttons, LayoutViewHandle, Pane, PaneHandle, PartsHandle, VirtualButton, ANIM_FRAME_STILL, ASSIGN_STANDARD},
        list::{RowBinder, RowBinderVtable, Scroller, ITEM_NONE},
        popup,
        scene::{FixedBaseString64, Scene, SceneChanger, SceneInitContext},
        scene_vtable::{SceneImpl, SceneVtable},
        selector::{ButtonSelectorCell, SelectorConfig},
        submenu::{self, SubMenu},
    },
    labels,
    scenes::{EXIT_BACK, EXIT_WORKSPACE_EDIT_BASE},
    screen::{ExitStep, HeaderStyle, Screen, ScreenEvent, UI_WAIT_LIMIT},
};

static VTABLE: SceneVtable = SceneVtable::of::<ArcadiaWorkspaceScene>();

static BINDER_VTABLE: RowBinderVtable = RowBinderVtable::new(bind_row);

const LAYOUT_PATH: &str = "ui/layout/menu/arcadia/arcadia_workspace/layout.arc";

const _: () = assert!(crate::game::hash40(LAYOUT_PATH) == 0x33_818a_1541);

static SCROLL_GROUP: &[u8] = b"scroll\0";

static HINT_GROUP: &[u8] = b"selector_0\0";
static HINT_PARTS: &[u8] = b"btn_keyhelp_delete\0";
const HINT_BUTTON: i32 = 0;

static HINT_WORD_PANE: &[u8] = b"set_txt_help\0";
static HINT_WORD: &str = "Edit";

static SUBMENU_PARTS: &[u8] = b"set_parts_submenu\0";

static CURSOR_PANE: &[u8] = b"cursor\0";

const CURSOR_TO_COLUMN: f32 = -20.0;

const SUBMENU_EDIT: i32 = 0;
const SUBMENU_RENAME: i32 = 1;
const SUBMENU_DELETE: i32 = 2;

static CAPSULE_PANE: &[u8] = b"txt_onamae\0";
static CAPSULE_TEXT: &str = "Workspaces";

static LOOK_CREATE: &[u8] = b"onamaeinput_on\0";
static LOOK_ACTIVE: &[u8] = b"onamaenashi\0";
static LOOK_NAMED: &[u8] = b"onamae\0";

static ROW_CREATE_ON_PANE: &[u8] = b"txt_new_on\0";
static ROW_CREATE_OFF_PANE: &[u8] = b"txt_new_off\0";
static ROW_ACTIVE_PANE: &[u8] = b"txt_noname\0";
static ROW_NAMED_PANE: &[u8] = b"set_txt_00\0";

static ROW_CREATE_TEXT: &str = "Create workspace";

static ROW_COUNT_PANE: &[u8] = b"txt_con_sel_00\0";
static COUNT_ON: &[u8] = b"controller_joy_dual_on\0";
static COUNT_OFF: &[u8] = b"controller_joy_dual_off\0";

static ICON_JOY_OFF: &[u8] = b"controller_joy_l_off\0";
static ICON_GC_OFF: &[u8] = b"controller_gc_off\0";

const MAX_NAME_LENGTH: usize = 10;

const FOOTER_BUBBLE_ROW: i32 = -2;

static ASSIGN_EXTRA: [(VirtualButton, Buttons); 1] = [(VirtualButton::Extra0, Buttons::X)];

#[derive(Clone, Copy)]
enum NameFor {
    Create,
    Rename(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Waiting {
    Nothing,
    Popup,
    SubMenu,
}

#[repr(C)]
pub struct ArcadiaWorkspaceScene {
    base: Scene,

    bubble: Option<SubMenu>,

    hint: Option<ButtonSelectorCell>,

    scroller: Option<Scroller>,
    screen: Screen,

    binder: RowBinder,

    workspaces: Vec<Workspace>,

    restore_row: i32,
    revealed: bool,
    exit_requested: bool,

    last_index: i32,

    hint_shown: bool,

    hint_pane: Option<NonNull<Pane>>,

    waiting: Waiting,
    waiting_frames: u32,

    bubble_target: usize,
    bubble_opened: bool,

    popup_name: Vec<u16>,
    popup_target: usize,
    popup_opened: bool,
}

unsafe impl SceneImpl for ArcadiaWorkspaceScene {
    const NAME: &'static CStr = c"ArcadiaWorkspaceScene";

    fn create() -> Box<ArcadiaWorkspaceScene> {
        Box::new(ArcadiaWorkspaceScene {
            base: Scene::new(&VTABLE),
            bubble: None,
            hint: None,
            scroller: None,
            screen: Screen::new("workspace", LAYOUT_PATH, HeaderStyle::HEADER_LOCAL, labels::HDR_SCENE_WORKSPACE, true),
            binder: RowBinder::empty(),
            workspaces: Vec::new(),
            restore_row: 0,
            revealed: false,
            exit_requested: false,
            last_index: ITEM_NONE,
            hint_shown: false,
            hint_pane: None,
            waiting: Waiting::Nothing,
            waiting_frames: 0,
            bubble_target: 0,
            bubble_opened: false,
            popup_name: Vec::new(),
            popup_target: 0,
            popup_opened: false,
        })
    }

    fn initialize(&mut self, ctx: &SceneInitContext) {
        self.screen.initialize(ctx);
        self.load_list();

        self.restore_row = match ctx.init_code() {
            Some(code) => code as i32 + 1,
            None => 0,
        };
        debug!("Workspace starting on row {}", self.restore_row);
    }

    fn update(&mut self, _changer: &mut SceneChanger) {
        match self.screen.poll() {
            ScreenEvent::LayoutReady => {
                self.build_widgets();
                self.screen.activate();
                self.screen.reveal();
                self.revealed = true;
            },
            ScreenEvent::GaveUp => self.screen.leave(EXIT_BACK),
            ScreenEvent::Nothing => {},
        }

        if self.revealed {
            self.screen.tick();
            self.tick_bubble();
            self.poll_popup();
            self.poll_submenu();
            self.poll_input();

            let footer_row = if self.waiting == Waiting::SubMenu { FOOTER_BUBBLE_ROW } else { self.current_row() };
            self.screen.footer_line(footer_row, workspace_footer);
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
                self.bubble = None;
                self.hint = None;
                self.scroller = None;
                self.hint_pane = None;
                self.screen.teardown();
                true
            },
            ExitStep::Done => true,
        }
    }
}

impl ArcadiaWorkspaceScene {
    fn load_list(&mut self) {
        self.workspaces = match workspaces::list() {
            Ok(list) => list,
            Err(err) => {
                warn!("Workspace: could not read the list, {}", err);
                Vec::new()
            },
        };
    }

    fn build_widgets(&mut self) {
        let this = self as *mut ArcadiaWorkspaceScene;

        let Some(cell) = self.screen.root_mut() else {
            return;
        };
        if let Some(view) = cell.view0() {
            for &(button, mask) in ASSIGN_STANDARD.iter().chain(ASSIGN_EXTRA.iter()) {
                view.set_assign(button, mask);
            }
        }
        unsafe { cell.set_enable_input(true) };

        self.binder.vtable = &BINDER_VTABLE;
        self.binder.owner = this.cast();

        self.set_capsule_text();
        let cursor = self.restore_row;
        self.build_scroller(cursor);
        self.setup_hint();
        self.setup_bubble();
    }

    fn build_scroller(&mut self, cursor: i32) {
        let binder_ptr = addr_of_mut!(self.binder);
        let Some(cell) = self.screen.root_mut() else {
            return;
        };
        let view_handle = cell.view_handle();
        let items = self.workspaces.len() as i32 + 1;
        let cursor = cursor.clamp(0, items - 1);

        let scroller = self.scroller.get_or_insert_with(Scroller::new);
        if scroller.is_empty() {
            if !unsafe { scroller.setup_list(view_handle, SCROLL_GROUP, items, cursor, true, binder_ptr) } {
                warn!(
                    "Workspace scroller got nothing, group '{}' is probably not in the bflyt",
                    debug_name(SCROLL_GROUP)
                );
                return;
            }
        } else {
            unsafe { scroller.resize(items, cursor) };
        }

        self.last_index = ITEM_NONE;
        info!("Workspace up with {} rows, cursor on row {}", items, cursor);
    }

    fn set_capsule_text(&mut self) {
        let Some(cell) = self.screen.root_mut() else {
            return;
        };
        let pane = unsafe { PaneHandle::get(cell.view_handle(), CAPSULE_PANE) };
        if pane.is_empty() || unsafe { pane.text_box() }.is_null() {
            warn!("Workspace: no '{}' text box, the capsule keeps its shipped word", debug_name(CAPSULE_PANE));
            return;
        }
        unsafe { pane.set_text(&with_nul(CAPSULE_TEXT)) };
    }

    fn setup_hint(&mut self) {
        let Some(cell) = self.screen.root_mut() else {
            return;
        };
        let view = cell.view_handle();

        match unsafe { PartsHandle::get_checked(view, HINT_PARTS, 0) } {
            Some(parts) => self.hint_pane = NonNull::new(unsafe { parts.root_pane() }),
            None => {
                self.hint_pane = None;
                warn!("Workspace: no '{}' part, no delete hint this run", debug_name(HINT_PARTS));
            },
        }

        let mut config = SelectorConfig::new();
        config.use_only_shortcut();
        unsafe { config.install_defaults() };

        let hint = self.hint.get_or_insert_with(ButtonSelectorCell::new);
        unsafe { hint.create(view, HINT_GROUP, &config) };
        if hint.is_empty() {
            warn!("Workspace: hint selector empty for group '{}', X will do nothing", debug_name(HINT_GROUP));
            return;
        }
        unsafe {
            hint.setup_button(HINT_BUTTON, HINT_PARTS, 0);
            hint.set_shortcut(HINT_BUTTON, VirtualButton::Extra0 as i32, false);
        }

        self.set_hint_word();

        self.show_hint(false);
    }

    fn set_hint_word(&mut self) {
        let Some(cell) = self.screen.root_mut() else {
            return;
        };
        let Some(mut parts) = (unsafe { PartsHandle::get_checked(cell.view_handle(), HINT_PARTS, 0) }) else {
            return;
        };
        unsafe { write_text(parts.view_handle(), HINT_WORD_PANE, HINT_WORD) };
    }

    fn setup_bubble(&mut self) {
        let items = [
            FixedBaseString64::new(labels::CTX_EDIT),
            FixedBaseString64::new(labels::CTX_RENAME),
            FixedBaseString64::new(labels::CTX_DELETE),
        ];

        let Some(cell) = self.screen.root_mut() else {
            return;
        };
        let bubble = self.bubble.get_or_insert_with(SubMenu::new);
        unsafe {
            bubble.create();
            bubble.attach(cell, SUBMENU_PARTS, &items, submenu::ANCHOR_RIGHT, submenu::SHIFT_NONE);
            bubble.set_focus(false);
        }

        unsafe { cell.set_enable_input(true) };
    }

    fn show_hint(&mut self, on: bool) {
        if let Some(mut pane) = self.hint_pane {
            unsafe { pane.as_mut().set_visible(on) };
        }
        if let Some(hint) = self.hint.as_ref() {
            unsafe { hint.set_enable(on) };
        }
        self.hint_shown = on;
    }

    fn current_row(&self) -> i32 {
        match self.scroller.as_ref() {
            Some(scroller) if !scroller.is_empty() => unsafe { scroller.current_index() },
            _ => ITEM_NONE,
        }
    }

    fn workspace_index(&self, row: i32) -> Option<usize> {
        if row < 1 {
            return None;
        }
        let index = row as usize - 1;
        (index < self.workspaces.len()).then_some(index)
    }

    fn tick_bubble(&mut self) {
        if let Some(bubble) = self.bubble.as_mut() {
            unsafe { bubble.update() };
        }
    }

    fn poll_input(&mut self) {
        if self.waiting != Waiting::Nothing {
            return;
        }

        let (current, decided, cancel) = {
            let Some(view) = self.screen.root_mut().and_then(|cell| cell.view0()) else {
                return;
            };
            let Some(scroller) = self.scroller.as_ref() else {
                return;
            };
            if scroller.is_empty() {
                return;
            }
            unsafe { (scroller.current_index(), scroller.decided_index(), view.pressed(VirtualButton::Cancel)) }
        };

        if current != self.last_index {
            self.last_index = current;
            self.show_hint(self.workspace_index(current).is_some());
        }

        let x_pressed = self.hint.as_ref().is_some_and(|hint| unsafe { hint.state() }.decided.is_some());
        if x_pressed && self.workspace_index(current).is_some() {
            self.open_bubble(current);
            return;
        }

        if decided != ITEM_NONE {
            match self.workspace_index(decided) {
                Some(index) => self.make_active(index),
                None => self.open_keyboard(NameFor::Create),
            }
            return;
        }

        if cancel {
            self.screen.leave(EXIT_BACK);
        }
    }

    fn make_active(&mut self, index: usize) {
        let Some(entry) = self.workspaces.get(index) else {
            return;
        };
        if entry.active {
            debug!("Workspace '{}' is already active", entry.name);
            return;
        }
        let target = entry.name.clone();

        match workspaces::set_active(&target) {
            Ok(()) => {
                info!("Workspace active -> '{}'", target);

                self.reload_to_name(&target);
            },
            Err(err) => warn!("Workspace: could not make '{}' active, {}", target, err),
        }
    }

    fn open_bubble(&mut self, row: i32) {
        let Some(index) = self.workspace_index(row) else {
            return;
        };

        let reserved = index == 0;

        self.point_bubble_at_row();

        let Some(bubble) = self.bubble.as_mut() else {
            return;
        };
        if unsafe { bubble.is_empty() } {
            warn!("Workspace: no bubble to open, X does nothing");
            return;
        }
        unsafe {
            bubble.clear_result();
            bubble.open(SUBMENU_EDIT);
            bubble.set_focus(true);
            bubble.set_item_enabled(SUBMENU_EDIT, true);
            bubble.set_item_enabled(SUBMENU_RENAME, !reserved);
            bubble.set_item_enabled(SUBMENU_DELETE, !reserved);
        }

        if let Some(cell) = self.screen.root_mut() {
            unsafe { cell.set_enable_input(true) };
        }

        self.bubble_target = index;
        self.bubble_opened = false;
        self.enter_waiting(Waiting::SubMenu);
        info!(
            "Workspace: bubble opened over '{}'{}",
            self.workspaces[index].name,
            if reserved { ", rename and delete greyed" } else { "" }
        );
    }

    fn point_bubble_at_row(&mut self) {
        let Some(cell) = self.screen.root_mut() else {
            return;
        };
        let view = cell.view_handle();

        let cursor = unsafe { PaneHandle::get(view, CURSOR_PANE) };
        let Some(cursor_y) = (unsafe { cursor.pane().as_ref() }).map(|pane| pane.translate().1) else {
            return;
        };

        let part = unsafe { PaneHandle::get(view, SUBMENU_PARTS) };
        if let Some(pane) = unsafe { part.pane().as_mut() } {
            let (part_x, _) = pane.translate();
            pane.set_translate(part_x, cursor_y + CURSOR_TO_COLUMN);
        }
    }

    fn poll_submenu(&mut self) {
        if self.waiting != Waiting::SubMenu {
            return;
        }
        self.waiting_frames += 1;

        let Some(answer) = self.bubble.as_ref().map(|bubble| unsafe { bubble.result() }) else {
            self.leave_waiting();
            return;
        };

        if answer == submenu::RESULT_PENDING {
            if self.bubble.as_ref().is_some_and(|bubble| unsafe { bubble.is_busy() }) {
                self.bubble_opened = true;
                return;
            }

            if self.bubble_opened {
                debug!("Workspace: the bubble closed with nothing picked");
                self.leave_waiting();
            } else if self.waiting_frames >= UI_WAIT_LIMIT {
                warn!("Workspace: the bubble never opened, giving up");
                self.leave_waiting();
            }
            return;
        }

        if let Some(bubble) = self.bubble.as_mut() {
            unsafe {
                bubble.close();
                bubble.clear_result();
            }
        }
        let index = self.bubble_target;
        self.leave_waiting();

        match answer {
            SUBMENU_EDIT => {
                debug!("Workspace: editing the mods of workspace {}", index);
                self.screen.leave(EXIT_WORKSPACE_EDIT_BASE + index as u32);
            },
            SUBMENU_RENAME => self.open_keyboard(NameFor::Rename(index)),
            SUBMENU_DELETE => self.ask_delete(index),
            _ => {},
        }
    }

    fn open_keyboard(&mut self, what: NameFor) {
        let initial = match what {
            NameFor::Create => None,
            NameFor::Rename(index) => self.workspaces.get(index).map(|entry| entry.name.clone()),
        };

        let answer = unsafe { keyboard::ask("Workspace name", "Workspace name", MAX_NAME_LENGTH as u32, initial.as_deref()) };

        let Some(name) = answer else {
            debug!("Workspace: keyboard cancelled, nothing written");
            return;
        };

        match what {
            NameFor::Create => self.create_workspace(&name),
            NameFor::Rename(index) => self.rename_workspace(index, &name),
        }
    }

    fn create_workspace(&mut self, typed: &str) {
        let valid = match self.check_name(typed, None) {
            Ok(name) => name,
            Err(problem) => {
                warn!("Workspace: name rejected, {}", problem);
                return;
            },
        };

        match workspaces::create(&valid) {
            Ok(()) => {
                info!("Workspace created '{}'", valid);
                self.reload_to_name(&valid);
            },
            Err(err) => warn!("Workspace: could not create '{}', {}", valid, err),
        }
    }

    fn rename_workspace(&mut self, index: usize, typed: &str) {
        if index == 0 {
            warn!("Workspace: Default cannot be renamed");
            return;
        }
        let Some(from) = self.workspaces.get(index).map(|entry| entry.name.clone()) else {
            return;
        };

        let valid = match self.check_name(typed, Some(index)) {
            Ok(name) => name,
            Err(problem) => {
                warn!("Workspace: name rejected, {}", problem);
                return;
            },
        };

        match workspaces::rename(&from, &valid) {
            Ok(()) => {
                info!("Workspace renamed '{}' -> '{}'", from, valid);
                self.reload_to_name(&valid);
            },
            Err(err) => warn!("Workspace: could not rename '{}', {}", from, err),
        }
    }

    fn check_name(&self, name: &str, skip: Option<usize>) -> Result<String, &'static str> {
        let name = name.trim();

        if name.is_empty() {
            return Err("a workspace needs a name");
        }
        if name.chars().count() > MAX_NAME_LENGTH {
            return Err("that name is too long");
        }

        if name.chars().any(|c| c == '"' || c == '\\' || c.is_control()) {
            return Err("that name has a character the store cannot hold");
        }
        if self
            .workspaces
            .iter()
            .enumerate()
            .any(|(row, entry)| Some(row) != skip && entry.name == name)
        {
            return Err("there is already a workspace with that name");
        }

        Ok(name.to_string())
    }

    fn ask_delete(&mut self, index: usize) {
        if index == 0 {
            warn!("Workspace: Default cannot be deleted");
            return;
        }
        let Some(entry) = self.workspaces.get(index) else {
            return;
        };
        let name = entry.name.clone();

        self.popup_name = name.encode_utf16().chain(std::iter::once(0)).collect();
        self.popup_target = index;
        self.popup_opened = false;

        if !unsafe { popup::open(popup::POPUP_DELETE_CONFIRM, self.popup_name.as_ptr()) } {
            warn!("Workspace: no popup manager, delete of '{}' does nothing", name);
            return;
        }

        self.enter_waiting(Waiting::Popup);
        info!("Workspace: delete popup opened for '{}'", name);
    }

    fn poll_popup(&mut self) {
        if self.waiting != Waiting::Popup {
            return;
        }
        self.waiting_frames += 1;

        if !self.popup_opened {
            if unsafe { popup::is_open() } {
                self.popup_opened = true;
                return;
            }

            if self.waiting_frames >= UI_WAIT_LIMIT {
                warn!("Workspace: the delete popup never opened, giving up");
                self.leave_waiting();
            }
            return;
        }

        if unsafe { popup::is_open() } {
            return;
        }

        let answer = unsafe { popup::result() };
        self.leave_waiting();

        if answer != popup::RESULT_YES {
            debug!("Workspace: delete popup answered {}, nothing deleted", answer);
            return;
        }

        let index = self.popup_target;
        let Some(name) = self.workspaces.get(index).map(|entry| entry.name.clone()) else {
            return;
        };

        match workspaces::delete(&name) {
            Ok(()) => {
                info!("Workspace deleted '{}'", name);

                self.reload_to_row(1);
            },
            Err(err) => warn!("Workspace: could not delete '{}', {}", name, err),
        }
    }

    fn reload_to_name(&mut self, target: &str) {
        self.load_list();
        let row = self
            .workspaces
            .iter()
            .position(|entry| entry.name == target)
            .map_or(1, |index| index as i32 + 1);
        self.build_scroller(row);
    }

    fn reload_to_row(&mut self, row: i32) {
        self.load_list();
        self.build_scroller(row);
    }

    fn enter_waiting(&mut self, waiting: Waiting) {
        self.waiting = waiting;
        self.waiting_frames = 0;

        if waiting == Waiting::SubMenu {
            self.set_list_answers(false);
            return;
        }
        if let Some(cell) = self.screen.root_mut() {
            unsafe { cell.set_enable_input(false) };
        }
    }

    fn leave_waiting(&mut self) {
        let was = self.waiting;
        self.waiting = Waiting::Nothing;
        self.waiting_frames = 0;

        if let Some(cell) = self.screen.root_mut() {
            unsafe { cell.set_enable_input(true) };
        }

        if was == Waiting::SubMenu {
            self.set_list_answers(true);
        }
    }

    fn set_list_answers(&mut self, on: bool) {
        if let Some(scroller) = self.scroller.as_ref() {
            unsafe { scroller.set_focus(on) };
        }
        if let Some(hint) = self.hint.as_ref() {
            unsafe { hint.set_enable(on && self.hint_shown) };
        }
    }
}

unsafe extern "C" fn bind_row(binder: *mut RowBinder, item: i32, row_view: *mut LayoutViewHandle, _extra: *mut u8) {
    if binder.is_null() || row_view.is_null() {
        return;
    }
    let this = (*binder).owner as *mut ArcadiaWorkspaceScene;
    if this.is_null() {
        return;
    }

    let scene = &*this;

    let payload = (*row_view).payload();

    let Some(index) = scene.workspace_index(item) else {
        write_text(row_view, ROW_CREATE_ON_PANE, ROW_CREATE_TEXT);
        write_text(row_view, ROW_CREATE_OFF_PANE, ROW_CREATE_TEXT);
        play_animation(payload, LOOK_CREATE, ANIM_FRAME_STILL);
        play_animation(payload, COUNT_OFF, ANIM_FRAME_STILL);
        play_animation(payload, ICON_JOY_OFF, ANIM_FRAME_STILL);
        play_animation(payload, ICON_GC_OFF, ANIM_FRAME_STILL);
        return;
    };

    let Some(entry) = scene.workspaces.get(index) else {
        return;
    };

    let (look, pane) = if entry.active { (LOOK_ACTIVE, ROW_ACTIVE_PANE) } else { (LOOK_NAMED, ROW_NAMED_PANE) };

    let name = entry.name.clone();
    let count = format!("{} {}", entry.mod_count, if entry.mod_count == 1 { "mod" } else { "mods" });

    write_text(row_view, pane, &name);
    write_text(row_view, ROW_COUNT_PANE, &count);

    play_animation(payload, look, ANIM_FRAME_STILL);
    play_animation(payload, COUNT_ON, ANIM_FRAME_STILL);
    play_animation(payload, ICON_JOY_OFF, ANIM_FRAME_STILL);
    play_animation(payload, ICON_GC_OFF, ANIM_FRAME_STILL);
}

fn workspace_footer(row: i32) -> &'static str {
    match row {
        FOOTER_BUBBLE_ROW => labels::WS_FOOTER_BUBBLE,
        0 => labels::WS_FOOTER_CREATE,
        1 => labels::WS_FOOTER_DEFAULT,
        row if row >= 2 => labels::WS_FOOTER_NAMED,
        _ => "",
    }
}
