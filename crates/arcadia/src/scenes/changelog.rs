use std::{ffi::CStr, ptr::NonNull};

use crate::{
    data::changelog::{self, Line},
    game::{
        layout::{debug_name, play_animation, show_pane, with_nul, write_text, LayoutViewHandle, Pane, PaneHandle, PartsHandle, VirtualButton, ASSIGN_STANDARD},
        scene::{Scene, SceneChanger, SceneInitContext},
        scene_vtable::{SceneImpl, SceneVtable},
        selector::{ButtonSelectorCell, SelectorConfig, BUTTON_ID_NONE},
    },
    labels,
    scenes::EXIT_BACK,
    screen::{ExitStep, HeaderStyle, Screen, ScreenEvent},
};

static VTABLE: SceneVtable = SceneVtable::of::<ArcadiaChangelogScene>();

const LAYOUT_PATH: &str = "ui/layout/menu/arcadia/arcadia_changelog/layout.arc";

const DESC_SLOTS: usize = 4;
const ROW_SLOTS: usize = 40;

const ROW_TOP: f32 = -287.0;
const ROW_PITCH: f32 = 57.0;

const FOOTER_GAP: f32 = 90.0;

const FOOTER_DEPTH: f32 = 300.0;

const BOTTOM_Y: f32 = -520.0;

const SCROLL_STEP: f32 = 12.0;

const COLUMNS: usize = 92;

const DESCRIPTION_COLUMNS: usize = 50;

const BLINK_FRAME: f32 = 4.0;

static BTN_UPDATE: &[u8] = b"btn_update\0";
static BTN_LATER: &[u8] = b"btn_later\0";

static GUIDE_OK: &[u8] = b"mnu_guide_ok_guide\0";
static GUIDE_CLOSE: &[u8] = b"mnu_guide_close_guide\0";

static SELECTOR_GROUP: &[u8] = b"arcadia_changelog_btns\0";

const BUTTON_UPDATE: i32 = 0;
const BUTTON_LATER: i32 = 1;

const DOWNLOADING_TEXT: &str = "Downloading the update. The game will restart by itself.";

#[repr(C)]
pub struct ArcadiaChangelogScene {
    base: Scene,
    screen: Screen,
    lines: Vec<Line>,
    title: String,
    date: String,

    offer_update: bool,
    bail: bool,
    downloading: bool,
    content: Option<NonNull<Pane>>,

    scroll: f32,
    max_scroll: f32,

    selector: Option<ButtonSelectorCell>,
    last_focus: i32,

    desc_used: usize,
    built: bool,
}

unsafe impl SceneImpl for ArcadiaChangelogScene {
    const NAME: &'static CStr = c"ArcadiaChangelogScene";

    fn create() -> Box<ArcadiaChangelogScene> {
        let pending = crate::take_pending_notes();
        let mut screen = Screen::new("changelog", LAYOUT_PATH, HeaderStyle::HEADER_HELP, labels::HDR_SCENE_NOTES, false);

        screen.set_bare();

        let bail = pending.is_none();
        if bail {
            warn!("The update notes were opened with nothing to show, closing again");
        }

        let (lines, title, date, offer_update) = match pending {
            Some((notes, offer_update)) => (changelog::lines(&notes, COLUMNS), notes.title, notes.date, offer_update),
            None => (Vec::new(), String::new(), String::new(), false),
        };

        Box::new(ArcadiaChangelogScene {
            base: Scene::new(&VTABLE),
            screen,
            lines,
            title,
            date,
            offer_update,
            bail,
            downloading: false,
            content: None,
            scroll: 0.0,
            max_scroll: 0.0,
            selector: None,
            last_focus: BUTTON_ID_NONE,
            desc_used: 0,
            built: false,
        })
    }

    fn initialize(&mut self, ctx: &SceneInitContext) {
        self.screen.initialize(ctx);
    }

    fn update(&mut self, _changer: &mut SceneChanger) {
        if self.bail {
            self.screen.leave(EXIT_BACK);
            return;
        }

        match self.screen.poll() {
            ScreenEvent::LayoutReady => {
                self.build();
                self.screen.activate();
                self.screen.reveal();
            },
            ScreenEvent::GaveUp => self.screen.leave(EXIT_BACK),
            ScreenEvent::Nothing => {},
        }
        self.screen.tick();

        if self.built {
            self.poll_input();
        }
    }

    fn request_exit(&mut self) {
        self.screen.request_exit();
    }

    fn is_exit_finished(&mut self) -> bool {
        match self.screen.exit_step() {
            ExitStep::Waiting => false,
            ExitStep::Teardown => {
                self.selector = None;
                self.content = None;
                self.screen.teardown();
                true
            },
            ExitStep::Done => true,
        }
    }
}

impl ArcadiaChangelogScene {
    fn build(&mut self) {
        let Some(cell) = self.screen.root_mut() else {
            return;
        };
        if let Some(view) = cell.view0() {
            for &(button, mask) in &ASSIGN_STANDARD {
                view.set_assign(button, mask);
            }
        }
        unsafe { cell.set_enable_input(true) };
        let view = cell.view_handle();

        unsafe {
            write_text(view, b"band_title\0", &self.title);
            write_text(view, b"band_date\0", &self.date);
            self.content = NonNull::new(PaneHandle::get(view, b"content\0").pane());
        }

        let mut at = 0;
        let mut used_desc = 0;
        while let Some(Line::Text(text)) = self.lines.get(at) {
            for wrapped in changelog::wrap(text, DESCRIPTION_COLUMNS) {
                if used_desc < DESC_SLOTS {
                    unsafe { show_text(view, &with_nul(&format!("desc_{used_desc:02}")), &wrapped) };
                    used_desc += 1;
                }
            }
            at += 1;
        }
        self.desc_used = used_desc;

        let credits_from = self.lines.len().saturating_sub(changelog::CREDIT_LINES);
        let mut used_rows = 0;
        while at < credits_from {
            if used_rows >= ROW_SLOTS {
                warn!("Changelog: the notes have more lines than the layout has rows, the rest are cut");
                break;
            }
            unsafe { fill_row(view, used_rows, &self.lines[at]) };
            used_rows += 1;
            at += 1;
        }

        let last_row_y = ROW_TOP - ROW_PITCH * used_rows.saturating_sub(1) as f32;
        let y_after = last_row_y - FOOTER_GAP;
        unsafe {
            if let Some(footer) = PaneHandle::get(view, b"footer\0").pane().as_mut() {
                footer.set_translate(0.0, y_after);
            }
        }

        unsafe {
            for (n, line) in self.lines[credits_from..].iter().enumerate() {
                if let Line::Text(text) = line {
                    show_text(view, &with_nul(&format!("credit_{n:02}")), text);
                }
            }
            show_pane(view, b"credits_rule\0", true);
        }
        self.write_buttons(view);
        self.setup_selector(view);

        self.max_scroll = (BOTTOM_Y - (y_after - FOOTER_DEPTH)).max(0.0);
        self.scroll = 0.0;

        self.built = true;
        info!("Changelog up with {used_desc} description lines, {used_rows} rows, scrolls {}, update {}", self.max_scroll > 0.0, self.offer_update);
    }

    fn write_buttons(&self, view: *mut LayoutViewHandle) {
        unsafe {
            if self.offer_update {
                write_button_text(view, BTN_UPDATE, "Update");
                write_button_text(view, BTN_LATER, "Later");
                write_button_guide(view, BTN_UPDATE, GUIDE_OK);
                write_button_guide(view, BTN_LATER, GUIDE_CLOSE);
            } else {
                show_pane(view, BTN_UPDATE, false);
                if let Some(pane) = PaneHandle::get(view, BTN_LATER).pane().as_mut() {
                    pane.set_translate(0.0, 0.0);
                }
                write_button_text(view, BTN_LATER, "OK");
                write_button_guide(view, BTN_LATER, GUIDE_OK);
            }
        }
    }

    fn setup_selector(&mut self, view: *mut LayoutViewHandle) {
        let mut config = SelectorConfig::new();
        unsafe { config.install_defaults() };

        let selector = self.selector.get_or_insert_with(ButtonSelectorCell::new);
        unsafe { selector.create(view, SELECTOR_GROUP, &config) };

        if selector.is_empty() {
            warn!("Changelog selector got nothing, group '{}' is probably not in the bflyt", debug_name(SELECTOR_GROUP));
            return;
        }

        unsafe {
            if self.offer_update {
                selector.setup_button(BUTTON_UPDATE, BTN_UPDATE, 0);
            }
            selector.setup_button(BUTTON_LATER, BTN_LATER, 0);
            selector.set_focus(true);
        }

        let initial = if self.offer_update { BUTTON_UPDATE } else { BUTTON_LATER };
        unsafe { selector.select_button(initial, 1) };

        self.last_focus = initial;
        self.update_focus_look(view, initial);
    }

    fn update_focus_look(&self, view: *mut LayoutViewHandle, focus: i32) {
        unsafe {
            if self.offer_update {
                play_blink(view, BTN_UPDATE, focus == BUTTON_UPDATE);
                play_blink(view, BTN_LATER, focus == BUTTON_LATER);
            } else {
                play_blink(view, BTN_LATER, true);
            }
        }
    }

    fn poll_input(&mut self) {
        let Some(view) = self.screen.root_mut().map(|cell| cell.view_handle()) else {
            return;
        };

        if self.downloading {
            if crate::update_progress() == crate::UpdateProgress::Failed {
                self.screen.leave(EXIT_BACK);
            }
            return;
        }

        let (up, down) = {
            let Some(input) = self.screen.root_mut().and_then(|cell| cell.view0()) else {
                return;
            };
            (input.held(VirtualButton::Up), input.held(VirtualButton::Down))
        };

        if down {
            self.scroll = (self.scroll + SCROLL_STEP).min(self.max_scroll);
        }
        if up {
            self.scroll = (self.scroll - SCROLL_STEP).max(0.0);
        }
        self.scroll_content();

        let Some(selector) = self.selector.as_ref() else {
            return;
        };

        if selector.is_empty() {
            let Some(input) = self.screen.root_mut().and_then(|cell| cell.view0()) else {
                return;
            };
            if input.pressed(VirtualButton::Cancel) {
                self.decline();
            } else if input.pressed(VirtualButton::Decide) {
                if self.offer_update {
                    self.accept(view)
                } else {
                    self.decline()
                }
            }
            return;
        }

        let state = unsafe { selector.state() };

        if state.focus != self.last_focus {
            self.last_focus = state.focus;
            self.update_focus_look(view, state.focus);
        }

        if state.cancelled {
            self.decline();
            return;
        }

        match state.decided {
            Some(BUTTON_UPDATE) => self.accept(view),
            Some(_) => self.decline(),
            None => {},
        }
    }

    fn scroll_content(&mut self) {
        if let Some(mut content) = self.content {
            unsafe { content.as_mut().set_translate(0.0, self.scroll) };
        }
    }

    fn decline(&mut self) {
        crate::set_changelog_choice(false);
        self.screen.leave(EXIT_BACK);
    }

    fn accept(&mut self, view: *mut LayoutViewHandle) {
        crate::set_changelog_choice(true);
        self.downloading = true;

        let mut slot = self.desc_used.min(DESC_SLOTS.saturating_sub(2));
        for line in changelog::wrap(DOWNLOADING_TEXT, DESCRIPTION_COLUMNS) {
            if slot >= DESC_SLOTS {
                break;
            }
            unsafe { show_text(view, &with_nul(&format!("desc_{slot:02}")), &line) };
            slot += 1;
        }

        unsafe {
            show_pane(view, BTN_UPDATE, false);
            show_pane(view, BTN_LATER, false);
        }

        self.scroll = 0.0;
        self.scroll_content();

        info!("The update was accepted, the page waits on the download");
    }
}

unsafe fn show_text(view: *mut LayoutViewHandle, pane: &[u8], text: &str) {
    show_pane(view, pane, true);
    write_text(view, pane, text);
}

unsafe fn fill_row(view: *mut LayoutViewHandle, n: usize, line: &Line) {
    match line {
        Line::Blank => {},
        Line::Header(text) => {
            for part in ["hplate", "hslant", "hrule", "hbar"] {
                show_pane(view, &with_nul(&format!("{part}_{n:02}")), true);
            }
            show_text(view, &with_nul(&format!("htext_{n:02}")), text);
        },
        Line::Bullet(text) => {
            show_text(view, &with_nul(&format!("dot_{n:02}")), "\u{2022}");
            show_text(view, &with_nul(&format!("line_{n:02}")), text);
        },
        Line::Text(text) | Line::Continued(text) => show_text(view, &with_nul(&format!("line_{n:02}")), text),
        Line::Contributor(person) => {
            let text = match &person.name {
                Some(name) => format!("{} ({})", person.login, name),
                None => person.login.clone(),
            };
            show_text(view, &with_nul(&format!("line_{n:02}")), &text);
        },
    }
}

unsafe fn parts_for_button(view: *mut LayoutViewHandle, button: &[u8]) -> Option<PartsHandle> {
    PartsHandle::get_checked(view, button, 0)
}

unsafe fn write_button_text(view: *mut LayoutViewHandle, button: &[u8], text: &str) {
    let Some(mut parts) = parts_for_button(view, button) else {
        return;
    };
    write_text(parts.view_handle(), b"set_txt_00\0", text);
}

unsafe fn write_button_guide(view: *mut LayoutViewHandle, button: &[u8], label: &[u8]) {
    let Some(mut parts) = parts_for_button(view, button) else {
        return;
    };
    let mut pane = PaneHandle::get(parts.view_handle(), b"set_txt_guide\0");
    if pane.is_empty() || pane.text_box().is_null() {
        return;
    }
    pane.set_text_label(label);
}

unsafe fn play_blink(view: *mut LayoutViewHandle, button: &[u8], on: bool) {
    let Some(mut parts) = parts_for_button(view, button) else {
        return;
    };
    let payload = (*parts.view_handle()).payload();
    let tag: &[u8] = if on { b"BlinkOn\0" } else { b"BlinkOff\0" };
    play_animation(payload, tag, BLINK_FRAME);
}
