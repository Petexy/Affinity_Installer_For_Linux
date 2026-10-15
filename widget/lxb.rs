//! The LineXinBar page: the installer in LineXinBar's own language, driven by
//! controller, keyboard and pointer. The same steps as the GTK page; the
//! password for installing Wine is asked by the session's polkit agent (on
//! LineXinBar, the shell's own panel with its on-screen keyboard), the
//! installer file by the shell's file question, and the confirmations by its
//! dialogs.

use crate::install::{self, AppInfo, Privilege, Report};
use crate::ui::{ICON, tr};
use linexin_lxb::{
    Emblem, Flow, Metric, PickerSelection, Role, SidePanel, WidgetFrame, guard, icon_file,
    page_result,
};
use linexin_plugin_api::{LXB_PAGE_STAY, LxbContextV1, LxbFrameV1};
use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr;
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// How much of a landscape page the screenshots take beside the buttons
/// (less where the window is too short for pictures that wide).
const GALLERY_SHARE: f32 = 0.58;

/// How tall the progress bar is, against the 1080-pixel reference.
const BAR_HEIGHT: f32 = 8.0;

enum Message {
    Report(Report),
    Done(Result<(), String>),
}

enum Phase {
    /// Installed or not, with what the page offers for each.
    Idle,
    Installing {
        messages: Receiver<Message>,
        label: String,
        fraction: f64,
        total: u32,
    },
    Removing(Receiver<()>),
}

/// A question of this page's own that is on screen.
enum Question {
    Remove,
    Choose(Vec<&'static AppInfo>),
    Failed,
}

struct State {
    /// The page, and in a landscape window the screenshots beside it.
    panel: SidePanel,
    installed: bool,
    phase: Phase,
    launching: Option<Receiver<()>>,
    asking: Option<Question>,
    picking: bool,
    /// The GTK page's screenshots, and the one shown.
    screenshots: Vec<PathBuf>,
    shown: usize,
}

pub unsafe extern "C" fn create(context: *const LxbContextV1) -> *mut c_void {
    guard(ptr::null_mut(), || {
        // SAFETY: the pointer the host passed to `lxb_create`.
        if unsafe { LxbContextV1::read(context) }.is_none() {
            return ptr::null_mut();
        }
        let state = State {
            panel: SidePanel::new(),
            installed: install::is_installed(),
            phase: Phase::Idle,
            launching: None,
            asking: None,
            picking: false,
            screenshots: crate::ui::load_screenshots(),
            shown: 0,
        };
        Box::into_raw(Box::new(state)).cast()
    })
}

pub unsafe extern "C" fn page(state: *mut c_void, frame: *const LxbFrameV1) -> u32 {
    guard(LXB_PAGE_STAY, || {
        // SAFETY: the state from `create`, alive for the call.
        let Some(state) = (unsafe { state.cast::<State>().as_mut() }) else {
            return LXB_PAGE_STAY;
        };
        // SAFETY: the host's frame, read for this call only.
        let Some(frame) = (unsafe { WidgetFrame::read(frame) }) else {
            return LXB_PAGE_STAY;
        };
        state.draw(&frame)
    })
}

pub unsafe extern "C" fn destroy(state: *mut c_void) {
    guard((), || {
        if !state.is_null() {
            // SAFETY: the state from `create`, released once. Work still
            // running finishes on its own thread; its answer is dropped.
            drop(unsafe { Box::from_raw(state.cast::<State>()) });
        }
    })
}

/// What pressing a row of the page asks for.
enum Press {
    Install(Option<PathBuf>),
    Pick,
    Launch,
    Remove,
}

impl State {
    fn draw(&mut self, frame: &WidgetFrame) -> u32 {
        let page = &frame.page;
        self.collect(page);
        if !matches!(self.phase, Phase::Idle) || self.launching.is_some() {
            page.redraw_within(0.2);
        }
        self.take_answer(page);
        if self.picking
            && let Some(path) = page.picked()
        {
            self.picking = false;
            self.start(Some(path));
        }

        // The GTK page's carousel: beside what can be done in a landscape
        // window, under it otherwise.
        let gallery = matches!(self.phase, Phase::Idle)
            && !self.screenshots.is_empty()
            && page.draws_pictures();
        let title = tr("Affinity Installer");
        let (main_rect, side_rect) = if gallery && frame.rect[2] > frame.rect[3] * 1.15 {
            // The pictures as wide as the window lets them stand, the page
            // (its buttons) taking the rest, the page's margin between them;
            // under the page's heading as tall as it is on the narrowest
            // page the pictures leave.
            let narrowest = frame.rect[2] * (1.0 - GALLERY_SHARE) - SidePanel::spacing(page);
            let pinned = Flow::head_height(page, &title, narrowest, true);
            let width = SidePanel::gallery_width(page, frame.rect, GALLERY_SHARE, pinned);
            SidePanel::split(page, frame.rect, width, page.scaled(360.0))
        } else {
            (frame.rect, None)
        };
        let wide = side_rect.is_some();
        let (main_input, side_input) = self.panel.input(page, &frame.actions, frame.focused, wide);
        let (main_lit, side_lit) = self.panel.lit(frame.focused, wide);
        let mut pressed = None;
        let launching = self.launching.is_some();
        let mut flow = self.panel.main.begin(page, main_rect, main_lit);
        let icon = icon_file(ICON);
        flow.head(
            Emblem::with_picture(icon.as_deref(), "category-graphics"),
            &title,
        );
        flow.note(&tr(
            "Supports Affinity Photo 2, Designer 2, Publisher 2 and Affinity (2025)",
        ));
        flow.gap();
        match &self.phase {
            Phase::Installing {
                label, fraction, ..
            } => {
                flow.heading(label);
                let height = page.scaled(BAR_HEIGHT).max(2.0);
                if let Some([x, y, width, height]) = flow.space(height) {
                    page.draw_rule([x, y, width, height], Role::GlassRaised);
                    let filled = width * fraction.clamp(0.0, 1.0) as f32;
                    if filled > 0.0 {
                        page.draw_rule([x, y, filled, height], Role::Accent);
                    }
                }
                flow.gap();
                flow.text(&tr("Installation in progress. Do NOT close the app."));
                flow.note(&tr("The app will show you a couple of windows."));
                flow.note(&tr(
                    "All of those components need to be installed manually using \
                     buttons on the screen in order for the app to work properly",
                ));
            }
            Phase::Removing(_) => {
                flow.entry("uninstall", &tr("Removing..."), None);
            }
            Phase::Idle if self.installed => {
                let launch = tr(if launching { "Launching..." } else { "Launch" });
                if flow.entry("launch", &launch, None) && !launching {
                    pressed = Some(Press::Launch);
                }
                if flow.entry("uninstall", &tr("Remove"), None) && !launching {
                    pressed = Some(Press::Remove);
                }
            }
            Phase::Idle => {
                if flow.entry("add", &tr("Download and Install"), None) {
                    pressed = Some(Press::Install(None));
                }
                if flow.entry("file-page", &tr("Select .exe file instead"), None) {
                    pressed = Some(Press::Pick);
                }
            }
        }
        if gallery && !wide {
            flow.gap();
            let height = (main_rect[3] * 0.45).min(main_rect[2] * 9.0 / 16.0);
            flow.gallery(&self.screenshots, &mut self.shown, height);
        }
        flow.finish();
        if let Some(side) = side_rect {
            // Level with the page's content, under its heading.
            let pinned = self.panel.main.pinned();
            let side = [
                side[0],
                side[1] + pinned,
                side[2],
                (side[3] - pinned).max(0.0),
            ];
            let mut flow = self.panel.side.begin(page, side, side_lit);
            let room = side[3] - flow.gallery_strip() - page.metric(Metric::Gap) * 4.0;
            let height = (side[2] * 9.0 / 16.0).min(room.max(page.scaled(160.0)));
            flow.gallery(&self.screenshots, &mut self.shown, height);
            flow.finish();
        }
        let leave = self.panel.settle(&main_input, &side_input, wide);

        match pressed {
            Some(Press::Install(exe)) => self.start(exe),
            Some(Press::Pick) => {
                let home = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_default();
                self.picking = page.pick(PickerSelection::File, home);
            }
            Some(Press::Launch) => self.launch(page),
            Some(Press::Remove) => {
                page.ask(
                    &tr("Are you sure?"),
                    &tr("This will remove Affinity and all its data from your system."),
                    &[&tr("Cancel"), &tr("Remove")],
                );
                self.asking = Some(Question::Remove);
            }
            None => {}
        }
        page_result(leave)
    }

    /// An answer to one of this page's questions, on the frame it is given.
    fn take_answer(&mut self, page: &linexin_lxb::Page) {
        if self.asking.is_none() {
            return;
        }
        let Some(answer) = page.answered() else {
            return;
        };
        match self.asking.take() {
            Some(Question::Remove) if answer == 1 => self.remove(),
            Some(Question::Choose(apps)) if answer > 0 => {
                if let Some(app) = apps.get(answer - 1) {
                    self.launch_app(app);
                }
            }
            _ => {}
        }
    }

    /// The GTK page's Download and Install (or a chosen installer): the steps
    /// on a worker thread, Wine installed through pkexec.
    fn start(&mut self, exe: Option<PathBuf>) {
        if !matches!(self.phase, Phase::Idle) {
            return;
        }
        let total = install::total_steps(exe.is_none());
        let (sender, messages) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("affinity-install".into())
            .spawn({
                let sender = sender.clone();
                move || {
                    let report = |report| {
                        let _ = sender.send(Message::Report(report));
                    };
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        install::run(exe, Privilege::Polkit, &report)
                    }))
                    .unwrap_or_else(|_| Err("The installation stopped unexpectedly.".into()));
                    let _ = sender.send(Message::Done(result));
                }
            });
        if let Err(error) = worker {
            let _ = sender.send(Message::Done(Err(error.to_string())));
        }
        self.phase = Phase::Installing {
            messages,
            label: tr("Preparing..."),
            fraction: 0.0,
            total,
        };
        self.panel.main.focus(0);
    }

    /// Progress, and the ends of what runs on other threads.
    fn collect(&mut self, page: &linexin_lxb::Page) {
        if let Some(exited) = &self.launching
            && !matches!(exited.try_recv(), Err(TryRecvError::Empty))
        {
            self.launching = None;
        }
        let mut finished = None;
        match &mut self.phase {
            Phase::Installing {
                messages,
                label,
                fraction,
                total,
            } => loop {
                match messages.try_recv() {
                    Ok(Message::Report(Report::Step { step, label: text })) => {
                        *label = text;
                        *fraction = f64::from(step) / f64::from(*total);
                    }
                    Ok(Message::Report(Report::Fraction(value))) => *fraction = value,
                    Ok(Message::Done(result)) => {
                        finished = Some(result);
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        finished = Some(Err("The installation stopped unexpectedly.".into()));
                        break;
                    }
                }
            },
            Phase::Removing(done) => {
                if !matches!(done.try_recv(), Err(TryRecvError::Empty)) {
                    self.phase = Phase::Idle;
                    self.installed = false;
                    self.panel.main.focus(0);
                }
            }
            Phase::Idle => {}
        }
        if let Some(result) = finished {
            self.phase = Phase::Idle;
            self.panel.main.focus(0);
            match result {
                Ok(()) => self.installed = true,
                Err(message) => {
                    self.installed = false;
                    if !message.is_empty() {
                        page.ask(&tr("Installation failed"), &message, &[&tr("OK")]);
                        self.asking = Some(Question::Failed);
                    } else {
                        // An empty message counted as success in the original.
                        self.installed = true;
                    }
                }
            }
        }
    }

    /// The GTK page's Launch: one app straight away, several by question.
    fn launch(&mut self, page: &linexin_lxb::Page) {
        let apps = install::installed_apps();
        match apps.as_slice() {
            [] => {}
            [app] => self.launch_app(app),
            _ => {
                let cancel = tr("Cancel");
                let mut answers: Vec<&str> = vec![&cancel];
                answers.extend(apps.iter().map(|app| app.name));
                page.ask(
                    &tr("Choose Application"),
                    &tr("Select which Affinity app to launch:"),
                    &answers,
                );
                self.asking = Some(Question::Choose(apps));
            }
        }
    }

    fn launch_app(&mut self, app: &'static AppInfo) {
        let (sender, exited) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("affinity-launch".into())
            .spawn(move || {
                match install::launch(app) {
                    Ok(mut child) => {
                        let _ = child.wait();
                    }
                    Err(error) => eprintln!("Could not launch {}: {error}", app.name),
                }
                let _ = sender.send(());
            });
        match worker {
            Ok(_) => self.launching = Some(exited),
            Err(error) => eprintln!("Could not launch {}: {error}", app.name),
        }
    }

    fn remove(&mut self) {
        let (sender, done) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("affinity-remove".into())
            .spawn(move || {
                install::remove();
                let _ = sender.send(());
            });
        match worker {
            Ok(_) => {
                self.phase = Phase::Removing(done);
                self.panel.main.focus(0);
            }
            Err(error) => eprintln!("Could not remove Affinity: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use linexin_lxb::{App, Overlay, Panes, host_frame};

    /// Draws the page while installing, half-way through Step 2, to
    /// `$AFFINITY_LXB_SHOT_DIR/installing.png` (headless; needs
    /// `liblxb_app.so`). Skipped without the variable.
    #[test]
    fn installing_page_shot() {
        let Some(dir) = std::env::var_os("AFFINITY_LXB_SHOT_DIR") else {
            return;
        };
        let (_sender, messages) = mpsc::channel();
        let mut state = State {
            panel: SidePanel::new(),
            installed: false,
            phase: Phase::Installing {
                messages,
                label: tr("Step {}: Installing Wine...").replacen("{}", "2", 1),
                fraction: 2.5 / 6.0,
                total: 6,
            },
            launching: None,
            asking: None,
            picking: false,
            screenshots: Vec::new(),
            shown: 0,
        };
        let mut app = App::new("affinity-test", "Affinity Installer")
            .expect("liblxb_app.so")
            .plain()
            .driven();
        let path = PathBuf::from(dir).join("installing.png");
        app.shot(&path, 1600, 1000, 0.5, |page| {
            let panes = Panes::whole(page);
            page.draw_pane(panes.page, Overlay::Dialog);
            let frame = host_frame(page, panes.content, true, &[]);
            // SAFETY: a frame built for this call.
            let frame = unsafe { WidgetFrame::read(&frame) }.expect("frame");
            state.draw(&frame);
        })
        .expect("shot");
        assert!(path.exists());
    }
}
