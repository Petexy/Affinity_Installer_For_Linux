//! The GTK interface: a port of `AffinityInstallerWidget`.

use crate::install::{self, AppInfo, Privilege, Report};
use adw::prelude::*;
use futures_util::StreamExt;
use gtk::{gdk, gio, glib};
use linexin_plugin_api::{self as api, Host};
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::rc::Rc;

/// Widget id and translation domain.
pub const DOMAIN: &str = "affinity_installer";

pub(crate) const ICON: &str = "/usr/share/icons/github.petexy.affinityinstaller.png";

const CSS: &str = "
.screenshot-bg {
    background: linear-gradient(135deg, #4a1a8a, #0d021e, #3a1070);
    border-radius: 12px;
    padding: 24px;
    min-height: 300px;
}
";

pub fn tr(text: &str) -> String {
    api::tr(DOMAIN, text)
}

/// GTK calls back through C, where a panic must not unwind.
fn guard(what: &str, callback: impl FnOnce()) {
    if catch_unwind(AssertUnwindSafe(callback)).is_err() {
        eprintln!("Affinity Installer: {what} panicked");
    }
}

/// The original `_apply_css` (it added the provider for every new widget;
/// once per display is the same style).
fn apply_css() {
    thread_local! {
        static APPLIED: Cell<bool> = const { Cell::new(false) };
    }
    if APPLIED.get() {
        return;
    }
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    APPLIED.set(true);
}

/// The original `_load_screenshots`: `affinity*.png` of the screenshots
/// folder beside the installed library, sorted.
pub(crate) fn load_screenshots() -> Vec<PathBuf> {
    let dir = crate::widgets_dir().join("screenshots");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| {
                    name.len() >= "affinity.png".len()
                        && name.starts_with("affinity")
                        && name.ends_with(".png")
                })
                .map(|name| dir.join(name))
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// What a failed install says when its worker died instead of answering.
const STOPPED: &str = "The installation stopped unexpectedly.";

enum Message {
    Report(Report),
    Done(Result<(), String>),
}

struct Installer {
    host: Option<Host>,
    window: Option<glib::WeakRef<gtk::Window>>,
    root: glib::WeakRef<gtk::Box>,
    total_steps: Cell<u32>,
    current_screenshot: Cell<usize>,
    screenshots: Vec<PathBuf>,
    action_box: gtk::Box,
    content_stack: gtk::Stack,
    btn_prev: gtk::Button,
    btn_next: gtk::Button,
    screenshot_picture: gtk::Picture,
    step_label: gtk::Label,
    progress_bar: gtk::ProgressBar,
    btn_remove: RefCell<Option<gtk::Button>>,
    btn_launch: RefCell<Option<gtk::Button>>,
}

/// Build the widget (GTK main thread).
pub fn build(host: Option<Host>, window: Option<gtk::Window>, hide_sidebar: bool) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.set_margin_top(12);
    root.set_margin_bottom(48);
    root.set_margin_start(48);
    root.set_margin_end(48);

    let installer = Rc::new(Installer {
        host,
        window: window.map(|window| window.downgrade()),
        root: root.downgrade(),
        total_steps: Cell::new(6),
        current_screenshot: Cell::new(0),
        screenshots: load_screenshots(),
        action_box: gtk::Box::new(gtk::Orientation::Horizontal, 8),
        content_stack: gtk::Stack::new(),
        btn_prev: gtk::Button::from_icon_name("go-previous-symbolic"),
        btn_next: gtk::Button::from_icon_name("go-next-symbolic"),
        screenshot_picture: gtk::Picture::new(),
        step_label: gtk::Label::new(None),
        progress_bar: gtk::ProgressBar::new(),
        btn_remove: RefCell::default(),
        btn_launch: RefCell::default(),
    });

    apply_css();
    installer.build_header(&root);
    installer.build_content_stack(&root);

    if install::is_installed() {
        installer.set_state_post_install();
    } else {
        installer.set_state_pre_install();
    }

    if hide_sidebar {
        let weak = Rc::downgrade(&installer);
        glib::idle_add_local_once(move || {
            if let Some(installer) = weak.upgrade() {
                installer.resize_window();
            }
        });
    }

    // The root owns the state; everything else holds it weakly, so the widget
    // and its state go away together.
    let keep = RefCell::new(Some(installer));
    root.connect_destroy(move |_| {
        keep.take();
    });
    root
}

impl Installer {
    // Initialization helpers

    fn resize_window(&self) {
        if let Some(window) = self.window.as_ref().and_then(|window| window.upgrade()) {
            window.set_default_size(800, 600);
        }
    }

    // UI construction

    fn build_header(self: &Rc<Self>, root: &gtk::Box) {
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.set_margin_bottom(12);

        let icon = gtk::Image::new();
        if std::path::Path::new(ICON).exists() {
            icon.set_from_file(Some(ICON));
        } else {
            icon.set_icon_name(Some("package-x-generic"));
        }
        icon.set_pixel_size(48);
        header.append(&icon);

        let text_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text_box.set_valign(gtk::Align::Center);
        text_box.set_hexpand(true);

        let title = gtk::Label::new(Some(&tr("Affinity Installer")));
        title.add_css_class("title-3");
        title.set_halign(gtk::Align::Start);
        text_box.append(&title);

        let subtitle = gtk::Label::new(Some(&tr(
            "Supports Affinity Photo 2, Designer 2, Publisher 2 and Affinity (2025)",
        )));
        subtitle.add_css_class("dim-label");
        subtitle.set_halign(gtk::Align::Start);
        subtitle.set_wrap(true);
        text_box.append(&subtitle);

        header.append(&text_box);

        self.action_box.set_valign(gtk::Align::Center);
        header.append(&self.action_box);

        root.append(&header);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    }

    fn build_content_stack(self: &Rc<Self>, root: &gtk::Box) {
        let stack = &self.content_stack;
        stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        stack.set_hexpand(true);
        stack.set_vexpand(true);

        self.build_carousel();
        self.build_progress();

        root.append(stack);
    }

    fn build_carousel(self: &Rc<Self>) {
        let page = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        page.set_valign(gtk::Align::Fill);
        page.set_vexpand(true);
        page.set_margin_top(24);
        page.set_margin_bottom(16);

        for (button, delta) in [(&self.btn_prev, -1), (&self.btn_next, 1)] {
            button.add_css_class("flat");
            button.add_css_class("circular");
            button.set_valign(gtk::Align::Center);
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(installer) = weak.upgrade() {
                    guard("navigate", || installer.navigate_screenshot(delta));
                }
            });
        }
        page.append(&self.btn_prev);

        let background = gtk::Box::new(gtk::Orientation::Vertical, 0);
        background.add_css_class("screenshot-bg");
        background.set_hexpand(true);
        background.set_vexpand(true);
        background.set_valign(gtk::Align::Fill);

        let picture = &self.screenshot_picture;
        picture.set_can_shrink(true);
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.set_hexpand(true);
        picture.set_vexpand(true);
        background.append(picture);

        page.append(&background);
        page.append(&self.btn_next);

        self.show_screenshot(0);
        self.content_stack.add_named(&page, Some("carousel"));
    }

    fn build_progress(&self) {
        let page = gtk::Box::new(gtk::Orientation::Vertical, 16);
        page.set_valign(gtk::Align::Center);
        page.set_halign(gtk::Align::Center);
        page.set_vexpand(true);
        page.set_hexpand(true);
        page.set_margin_start(60);
        page.set_margin_end(60);
        page.set_margin_top(40);
        page.set_margin_bottom(40);

        self.step_label.add_css_class("title-4");
        page.append(&self.step_label);

        self.progress_bar.set_hexpand(true);
        page.append(&self.progress_bar);

        let warning = gtk::Label::new(Some(&tr("Installation in progress. Do NOT close the app.")));
        warning.add_css_class("title-3");
        page.append(&warning);

        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_vexpand(true);
        page.append(&spacer);

        let note = gtk::Label::new(None);
        note.set_markup(&format!(
            "{}\n{}",
            tr("The app will show you a couple of windows."),
            tr(
                "All of those components need to be installed manually using \
                buttons on the screen in order for the app to work properly"
            )
        ));
        note.set_justify(gtk::Justification::Center);
        note.set_wrap(true);
        note.add_css_class("dim-label");
        page.append(&note);

        self.content_stack.add_named(&page, Some("progress"));
    }

    // Screenshot carousel

    fn show_screenshot(&self, index: i64) {
        if self.screenshots.is_empty() {
            self.btn_prev.set_visible(false);
            self.btn_next.set_visible(false);
            return;
        }
        let current = index.rem_euclid(self.screenshots.len() as i64) as usize;
        self.current_screenshot.set(current);
        self.screenshot_picture
            .set_filename(Some(&self.screenshots[current]));
        let has_many = self.screenshots.len() > 1;
        self.btn_prev.set_sensitive(has_many);
        self.btn_next.set_sensitive(has_many);
    }

    fn navigate_screenshot(&self, delta: i64) {
        self.show_screenshot(self.current_screenshot.get() as i64 + delta);
    }

    // State management

    fn clear_actions(&self) {
        while let Some(child) = self.action_box.first_child() {
            self.action_box.remove(&child);
        }
    }

    fn set_state_pre_install(self: &Rc<Self>) {
        self.clear_actions();

        let menu = gio::Menu::new();
        menu.append(
            Some(&tr("Select .exe file instead")),
            Some("install.select_exe"),
        );

        let split = adw::SplitButton::new();
        split.set_label(&tr("Download and Install"));
        split.set_menu_model(Some(&menu));
        split.add_css_class("suggested-action");
        let weak = Rc::downgrade(self);
        split.connect_clicked(move |_| {
            if let Some(installer) = weak.upgrade() {
                guard("download and install", || installer.prompt_password(None));
            }
        });

        let group = gio::SimpleActionGroup::new();
        let action = gio::SimpleAction::new("select_exe", None);
        let weak = Rc::downgrade(self);
        action.connect_activate(move |_, _| {
            if let Some(installer) = weak.upgrade() {
                guard("select exe", || installer.on_select_exe());
            }
        });
        group.add_action(&action);
        if let Some(root) = self.root.upgrade() {
            root.insert_action_group("install", Some(&group));
        }

        self.action_box.append(&split);
        self.content_stack.set_visible_child_name("carousel");
    }

    fn set_state_installing(&self) {
        self.clear_actions();
        let button = gtk::Button::with_label(&tr("Installing..."));
        button.set_sensitive(false);
        self.action_box.append(&button);
        self.content_stack.set_visible_child_name("progress");
    }

    fn set_state_post_install(self: &Rc<Self>) {
        self.clear_actions();

        let remove = gtk::Button::with_label(&tr("Remove"));
        remove.add_css_class("destructive-action");
        let weak = Rc::downgrade(self);
        remove.connect_clicked(move |_| {
            if let Some(installer) = weak.upgrade() {
                guard("remove", || installer.on_remove());
            }
        });
        self.action_box.append(&remove);

        let launch = gtk::Button::with_label(&tr("Launch"));
        launch.add_css_class("suggested-action");
        let weak = Rc::downgrade(self);
        launch.connect_clicked(move |_| {
            if let Some(installer) = weak.upgrade() {
                guard("launch", || installer.on_launch());
            }
        });
        self.action_box.append(&launch);

        self.btn_remove.replace(Some(remove));
        self.btn_launch.replace(Some(launch));
        self.content_stack.set_visible_child_name("carousel");
    }

    // Action handlers

    /// `self.get_root() or self.window`.
    fn parent_window(&self) -> Option<gtk::Window> {
        self.root
            .upgrade()
            .and_then(|root| root.root())
            .and_then(|root| root.downcast::<gtk::Window>().ok())
            .or_else(|| self.window.as_ref().and_then(|window| window.upgrade()))
    }

    /// The injected `translate_dialog` of the original.
    fn translate_dialog(&self, dialog: &adw::MessageDialog) {
        if let Some(host) = self.host {
            // SAFETY: a live GtkWidget on the GTK thread.
            unsafe {
                host.translate_widget(DOMAIN, dialog.upcast_ref::<gtk::Widget>().as_ptr().cast())
            };
        }
    }

    fn on_select_exe(self: &Rc<Self>) {
        let parent = self
            .root
            .upgrade()
            .and_then(|root| root.root())
            .and_then(|root| root.downcast::<gtk::Window>().ok());
        #[allow(deprecated)] // Gtk.FileChooserDialog, as the original uses
        let dialog = {
            let dialog = glib::Object::builder::<gtk::FileChooserDialog>()
                .property("title", tr("Select Affinity .exe file"))
                .property("transient-for", parent)
                .property("modal", true)
                .property("action", gtk::FileChooserAction::Open)
                .build();
            dialog.add_button(&tr("Cancel"), gtk::ResponseType::Cancel);
            dialog.add_button(&tr("Open"), gtk::ResponseType::Ok);

            let filter = gtk::FileFilter::new();
            filter.set_name(Some(&tr("Affinity Installer (*.exe)")));
            filter.add_pattern("*.exe");
            dialog.add_filter(&filter);
            dialog
        };

        let weak = Rc::downgrade(self);
        #[allow(deprecated)]
        dialog.connect_response(move |dialog, response| {
            if response == gtk::ResponseType::Ok
                && let Some(file) = dialog.file()
                && let Some(installer) = weak.upgrade()
            {
                guard("chosen exe", || installer.prompt_password(file.path()));
            }
            dialog.destroy();
        });
        dialog.present();
    }

    fn on_remove(self: &Rc<Self>) {
        let dialog = adw::MessageDialog::new(
            self.parent_window().as_ref(),
            Some(&tr("Are you sure?")),
            Some(&tr(
                "This will remove Affinity and all its data from your system.",
            )),
        );
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("remove", &tr("Remove"));
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);

        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |dialog, response| {
            dialog.close();
            if response == "remove"
                && let Some(installer) = weak.upgrade()
            {
                guard("removal", || installer.perform_removal());
            }
        });
        self.translate_dialog(&dialog);
        dialog.present();
    }

    fn on_launch(self: &Rc<Self>) {
        let apps = install::installed_apps();
        match apps.as_slice() {
            [] => return,
            [app] => {
                self.launch_app(app);
                return;
            }
            _ => {}
        }

        let dialog = adw::MessageDialog::new(
            self.parent_window().as_ref(),
            Some(&tr("Choose Application")),
            Some(&tr("Select which Affinity app to launch:")),
        );
        dialog.add_response("cancel", &tr("Cancel"));
        for app in &apps {
            dialog.add_response(app.name, app.name);
            dialog.set_response_appearance(app.name, adw::ResponseAppearance::Suggested);
        }

        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |dialog, response| {
            dialog.close();
            if response != "cancel"
                && let Some(app) = apps.iter().find(|app| app.name == response)
                && let Some(installer) = weak.upgrade()
            {
                guard("launch", || installer.launch_app(app));
            }
        });
        self.translate_dialog(&dialog);
        dialog.present();
    }

    fn set_buttons_launching(&self) {
        if let Some(launch) = &*self.btn_launch.borrow() {
            launch.set_label(&tr("Launching..."));
            launch.set_sensitive(false);
        }
        if let Some(remove) = &*self.btn_remove.borrow() {
            remove.set_sensitive(false);
        }
    }

    fn set_buttons_idle(&self) {
        if let Some(launch) = &*self.btn_launch.borrow() {
            launch.set_label(&tr("Launch"));
            launch.set_sensitive(true);
        }
        if let Some(remove) = &*self.btn_remove.borrow() {
            remove.set_sensitive(true);
        }
    }

    // Launch / Remove helpers

    /// The original `_launch_app`: the buttons wait for the app to exit.
    fn launch_app(self: &Rc<Self>, app: &'static AppInfo) {
        self.set_buttons_launching();
        let (sender, exited) = futures_channel::oneshot::channel::<()>();
        let worker = std::thread::Builder::new()
            .name("affinity-launch".into())
            .spawn(move || {
                match install::launch(app) {
                    Ok(mut child) => {
                        let _ = child.wait();
                    }
                    // The original's thread died here and left the buttons
                    // waiting; they come back instead.
                    Err(error) => eprintln!("Could not launch {}: {error}", app.name),
                }
                let _ = sender.send(());
            });
        if let Err(error) = worker {
            eprintln!("Could not launch {}: {error}", app.name);
        }
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let _ = exited.await;
            if let Some(installer) = weak.upgrade() {
                installer.set_buttons_idle();
            }
        });
    }

    fn perform_removal(self: &Rc<Self>) {
        self.clear_actions();
        let button = gtk::Button::with_label(&tr("Removing..."));
        button.set_sensitive(false);
        self.action_box.append(&button);

        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let _ = gio::spawn_blocking(install::remove).await;
            if let Some(installer) = weak.upgrade() {
                guard("removed", || installer.set_state_pre_install());
            }
        });
    }

    // Password prompt

    /// The original `_prompt_password`; `exe` is the chosen installer, or
    /// `None` to download it.
    fn prompt_password(self: &Rc<Self>, exe: Option<PathBuf>) {
        let dialog = adw::MessageDialog::new(
            self.parent_window().as_ref(),
            Some(&tr("Authentication Required")),
            Some(&tr(
                "Please enter your password to proceed with the installation.",
            )),
        );
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("unlock", &tr("Unlock"));
        dialog.set_response_appearance("unlock", adw::ResponseAppearance::Suggested);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let entry = gtk::PasswordEntry::new();
        entry.set_property("placeholder-text", tr("Password"));
        content.append(&entry);
        dialog.set_extra_child(Some(&content));

        let weak = Rc::downgrade(self);
        let password = entry.clone();
        dialog.connect_response(None, move |dialog, response| {
            dialog.close();
            if response == "unlock"
                && let Some(installer) = weak.upgrade()
            {
                let typed = password.text().to_string();
                if !typed.is_empty() {
                    guard("unlock", || installer.check_password(typed, exe.clone()));
                }
            }
        });

        let dialog_weak = dialog.downgrade();
        entry.connect_activate(move |_| {
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.response("unlock");
            }
        });
        self.translate_dialog(&dialog);
        dialog.present();
    }

    /// Validate off the GTK thread (the original blocked it), then go on as
    /// the original's response handler did.
    fn check_password(self: &Rc<Self>, password: String, exe: Option<PathBuf>) {
        let Some(host) = self.host else {
            // Without the Center's services the original installed anyway.
            self.begin_install(exe);
            return;
        };
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let checked = password.clone();
            let valid = gio::spawn_blocking(move || host.sudo_validate_password(&checked))
                .await
                .unwrap_or(false);
            let Some(installer) = weak.upgrade() else {
                return;
            };
            if valid {
                host.sudo_set_password(&password);
                guard("install", || installer.begin_install(exe));
            } else {
                installer.show_auth_error();
            }
        });
    }

    fn show_auth_error(&self) {
        let dialog = adw::MessageDialog::new(
            self.parent_window().as_ref(),
            Some(&tr("Authentication Failed")),
            Some(&tr("Incorrect password.")),
        );
        dialog.add_response("ok", &tr("OK"));
        self.translate_dialog(&dialog);
        dialog.present();
    }

    // Installation flow

    fn begin_install(self: &Rc<Self>, exe: Option<PathBuf>) {
        let downloading = exe.is_none();
        self.total_steps.set(install::total_steps(downloading));

        self.set_state_installing();
        self.update_step(0, &tr("Preparing..."));

        let host = self.host;
        let (sender, mut messages) = futures_channel::mpsc::unbounded::<Message>();
        let worker = std::thread::Builder::new()
            .name("affinity-install".into())
            .spawn({
                let sender = sender.clone();
                move || {
                    let report = |report| {
                        let _ = sender.unbounded_send(Message::Report(report));
                    };
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        install::run(exe, Privilege::Sudo(host), &report)
                    }))
                    .unwrap_or_else(|_| Err(STOPPED.into()));
                    let _ = sender.unbounded_send(Message::Done(result));
                }
            });
        if let Err(error) = worker {
            let _ = sender.unbounded_send(Message::Done(Err(error.to_string())));
        }
        drop(sender);

        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let mut result = Err(STOPPED.into());
            while let Some(message) = messages.next().await {
                match message {
                    Message::Report(report) => {
                        if let Some(installer) = weak.upgrade() {
                            installer.apply(report);
                        }
                    }
                    Message::Done(done) => {
                        result = done;
                        break;
                    }
                }
            }
            // The privileged session ends even when the page is gone.
            if let Some(host) = host {
                host.sudo_stop_session();
                host.sudo_forget_password();
            }
            if let Some(installer) = weak.upgrade() {
                guard("finish", || installer.finish_install(result.err()));
            }
        });
    }

    fn update_step(&self, step: u32, label: &str) {
        self.step_label.set_label(label);
        self.progress_bar
            .set_fraction(f64::from(step) / f64::from(self.total_steps.get()));
    }

    fn apply(&self, report: Report) {
        match report {
            Report::Step { step, label } => self.update_step(step, &label),
            Report::Fraction(fraction) => self.progress_bar.set_fraction(fraction),
        }
    }

    /// The original `_finish_install` (the sudo session is over already).
    fn finish_install(self: &Rc<Self>, error: Option<String>) {
        match error.filter(|message| !message.is_empty()) {
            Some(message) => {
                let dialog = adw::MessageDialog::new(
                    self.parent_window().as_ref(),
                    Some(&tr("Installation failed")),
                    Some(&message),
                );
                dialog.add_response("ok", &tr("OK"));
                self.translate_dialog(&dialog);
                dialog.present();
                self.set_state_pre_install();
            }
            None => self.set_state_post_install(),
        }
    }
}
