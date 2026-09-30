use std::ptr::NonNull;

use smash_arc::Hash40;

use crate::{
    game::{
        fade::Fade,
        frame::{ComFrame, HeaderCategory, HeaderLabels, MenuBackground, SHOW_ANIMATED, SHOW_INSTANT},
        hash40,
        layout::LayoutRootCell,
        loading::LoadingList,
        scene::{FixedBaseString64, SceneInitContext, SequenceScene},
    },
    labels,
};

const LOAD_WAIT_LIMIT: u32 = 600;

pub const UI_WAIT_LIMIT: u32 = 120;

const BAR_WAIT_WARN_FRAMES: u32 = 60;

const NO_FOOTER_ROW: i32 = -1;

const LAYOUT_DRAW_ORDER: i32 = 0x1000;

#[derive(Clone, Copy)]
pub struct HeaderStyle {
    pub category: HeaderCategory,

    pub background: MenuBackground,

    pub format_label: &'static str,
}

impl HeaderStyle {
    pub const HEADER_HELP: HeaderStyle = HeaderStyle {
        category: HeaderCategory::Help,
        background: MenuBackground::Help,
        format_label: "mnu_header_help",
    };

    pub const HEADER_OPTION: HeaderStyle = HeaderStyle {
        category: HeaderCategory::Options,
        background: MenuBackground::Option,
        format_label: "mnu_header_option",
    };

    pub const HEADER_LOCAL: HeaderStyle = HeaderStyle {
        category: HeaderCategory::Online,
        background: MenuBackground::Local,
        format_label: "mnu_header_local",
    };
}

pub enum ScreenEvent {
    Nothing,

    LayoutReady,

    GaveUp,
}

pub enum ExitStep {
    Waiting,

    Teardown,

    Done,
}

pub struct Screen {
    name: &'static str,
    layout_hash: Hash40,

    view_count: i32,
    style: HeaderStyle,
    scene_label: &'static str,
    footer: bool,

    owner: Option<NonNull<SequenceScene>>,

    layout: Option<LayoutRootCell>,
    loading: Option<LoadingList>,

    wait_frames: u32,
    load_done: bool,

    fade_in_asked: bool,
    fade_out_asked: bool,
    fade_wait_done: bool,
    fade_wait_frames: u32,

    header_previous: Option<HeaderLabels>,

    header_shown: bool,
    bar_wait: u32,

    bare: bool,

    footer_row: i32,
    footer_set: bool,

    exit_asked: bool,
    torn_down: bool,
}

impl Screen {
    pub fn new(name: &'static str, layout_path: &'static str, style: HeaderStyle, scene_label: &'static str, footer: bool) -> Screen {
        Screen {
            name,
            layout_hash: Hash40(hash40(layout_path)),
            view_count: 1,
            style,
            scene_label,
            footer,
            owner: None,
            layout: None,
            loading: None,
            wait_frames: 0,
            load_done: false,
            fade_in_asked: false,
            fade_out_asked: false,
            fade_wait_done: false,
            fade_wait_frames: 0,
            header_previous: None,
            header_shown: false,
            bar_wait: 0,
            bare: false,
            footer_row: NO_FOOTER_ROW,
            footer_set: false,
            exit_asked: false,
            torn_down: false,
        }
    }

    pub fn set_view_count(&mut self, views: i32) {
        self.view_count = views;
    }

    pub fn set_bare(&mut self) {
        self.bare = true;
    }

    pub fn initialize(&mut self, ctx: &SceneInitContext) {
        self.owner = Some(ctx.owner);

        unsafe {
            let Some(list) = LoadingList::new() else {
                warn!("{}: no memory for the loading list, no layout this run", self.name);
                return;
            };
            list.queue(&self.layout_hash);
            self.loading = Some(list);
        }

        debug!("{}: queued layout {:#x}", self.name, self.layout_hash.0);
    }

    pub fn poll(&mut self) -> ScreenEvent {
        if self.load_done {
            return ScreenEvent::Nothing;
        }
        if self.loading.is_none() {
            return ScreenEvent::Nothing;
        }

        self.wait_frames += 1;

        let ready = unsafe { self.loading.as_ref().is_some_and(|list| list.is_ready()) };
        if !ready {
            if self.wait_frames >= LOAD_WAIT_LIMIT {
                self.load_done = true;
                warn!("{}: the layout never loaded after {} frames, giving up", self.name, self.wait_frames);
                self.fade_in();
                return ScreenEvent::GaveUp;
            }
            return ScreenEvent::Nothing;
        }

        self.load_done = true;
        self.build();
        ScreenEvent::LayoutReady
    }

    fn build(&mut self) {
        unsafe {
            let Some(mut cell) = LayoutRootCell::create() else {
                warn!("{}: no memory for the layout cell", self.name);
                return;
            };

            if let Some(list) = self.loading.as_ref() {
                cell.build(list, &self.layout_hash, self.view_count);
            }

            if cell.handle().is_null() {
                warn!("{}: the layout build left the cell empty, nothing to show", self.name);
            }

            self.layout = Some(cell);
        }

        debug!("{}: layout built after {} frames", self.name, self.wait_frames);
    }

    pub fn root_mut(&mut self) -> Option<&mut LayoutRootCell> {
        self.layout.as_mut()
    }

    pub fn activate(&self) {
        if let Some(cell) = self.layout.as_ref() {
            unsafe { cell.activate(0, LAYOUT_DRAW_ORDER) }
        }
    }

    pub fn reveal(&mut self) {
        self.fade_in();
        self.tick();
    }

    pub fn tick(&mut self) {
        if self.header_shown {
            return;
        }

        unsafe {
            let Some(frame) = ComFrame::get() else {
                self.bar_wait += 1;
                if self.bar_wait == BAR_WAIT_WARN_FRAMES {
                    warn!("{}: no shared bar reachable, no header or footer this run", self.name);
                }
                return;
            };

            if !frame.bar_ready() {
                self.bar_wait += 1;
                if self.bar_wait == BAR_WAIT_WARN_FRAMES {
                    warn!("{}: the shared bar is not built, no header or footer this run ({})", self.name, frame.describe());
                }
                return;
            }

            if self.bare {
                frame.show(false, SHOW_INSTANT);
                self.header_shown = true;
                debug!("{}: shared bar hidden", self.name);
                return;
            }

            let previous = frame.header_labels();
            let wanted = HeaderLabels {
                format: FixedBaseString64::new(self.style.format_label),
                mode: FixedBaseString64::new(labels::HDR_MODE),
                scene: FixedBaseString64::new(self.scene_label),
                background: self.style.background as i32,
            };

            let hidden = !frame.visible();
            frame.set_background(self.style.background);
            frame.want_back_button(self.style.category);
            frame.set_header_labels(&wanted);
            frame.show_footer_forced(self.footer, false, hidden);
            frame.show(true, if hidden { SHOW_ANIMATED } else { SHOW_INSTANT });
            frame.colour_header(self.style.category);
            debug!("{}: shared bar shown, it was {}", self.name, if hidden { "hidden" } else { "already up" });

            self.header_previous = Some(previous);
            self.header_shown = true;
            self.footer_set = self.footer;
            self.footer_row = NO_FOOTER_ROW;

            debug!(
                "{}: header up, mode '{}' scene '{}', category {}",
                self.name,
                wanted.mode.as_str(),
                wanted.scene.as_str(),
                self.style.category as i32
            );
        }
    }

    pub fn footer_line(&mut self, row: i32, label_for: fn(i32) -> &'static str) {
        if !self.footer_set || row == self.footer_row {
            return;
        }
        self.footer_row = row;

        unsafe {
            if let Some(frame) = ComFrame::get() {
                let label = FixedBaseString64::new(label_for(row));
                frame.set_footer_label(&label);
            }
        }
    }

    pub fn leave(&mut self, code: u32) {
        if self.exit_asked {
            return;
        }
        self.exit_asked = true;

        let Some(mut owner) = self.owner else {
            warn!("{}: no driver to leave through", self.name);
            return;
        };

        debug!("{}: leaving with code {}", self.name, code);
        unsafe { owner.as_mut().exit_active_scene(code) }
    }

    pub fn request_exit(&mut self) {
        self.fade_out();
        self.restore_header();
    }

    pub fn exit_step(&mut self) -> ExitStep {
        if self.torn_down {
            return ExitStep::Done;
        }
        if !self.fade_out_finished() {
            return ExitStep::Waiting;
        }
        ExitStep::Teardown
    }

    pub fn teardown(&mut self) {
        if self.torn_down {
            return;
        }
        self.torn_down = true;

        self.layout = None;
        self.loading = None;

        debug!("{}: torn down", self.name);
    }

    fn restore_header(&mut self) {
        if self.bare {
            if self.header_shown {
                self.header_shown = false;
                unsafe {
                    if let Some(frame) = ComFrame::get() {
                        frame.show(true, SHOW_INSTANT);
                    }
                }
            }
            return;
        }
        let Some(previous) = self.header_previous.take() else {
            return;
        };
        self.footer_set = false;

        unsafe {
            let Some(frame) = ComFrame::get() else {
                return;
            };
            frame.set_header_labels(&previous);
            frame.set_background_index(previous.background);
            frame.forget_footer_label();
            frame.show_footer(false, false);
            frame.show(true, SHOW_INSTANT);
        }
    }

    fn fade_in(&mut self) {
        if self.fade_in_asked {
            return;
        }
        self.fade_in_asked = true;

        if let Some(fade) = Fade::get() {
            fade.fade_in();
        }
    }

    fn fade_out(&mut self) {
        if self.fade_out_asked {
            return;
        }
        self.fade_out_asked = true;

        if let Some(fade) = Fade::get() {
            fade.fade_out();
        }
    }

    fn fade_out_finished(&mut self) -> bool {
        if self.fade_wait_done || !self.fade_out_asked {
            return true;
        }

        if self.owner.is_none() {
            self.fade_wait_done = true;
            return true;
        }

        match Fade::get() {
            Some(fade) if fade.finished() => {
                self.fade_wait_done = true;
                true
            },
            _ => {
                self.fade_wait_frames += 1;
                if self.fade_wait_frames >= UI_WAIT_LIMIT {
                    self.fade_wait_done = true;
                    warn!("{}: fade out still going after {} frames, leaving anyway", self.name, self.fade_wait_frames);
                    true
                } else {
                    false
                }
            },
        }
    }
}
