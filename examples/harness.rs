//! Development harness: loads the built widget library through the C ABI with a
//! host whose translation services are the Center's own (dictionaries, `{}`
//! patterns and `translate_dialog` from linexin-localization), shows it in an
//! `AdwApplicationWindow`, runs the scenario of `examples/python_reference.py`
//! and renders PNGs.
//!
//! ```sh
//! cargo build --lib --examples
//! target/debug/examples/harness --library DIR/d-affinity_installer.so --lang pl_PL --output out.png
//! ```
//! The library must sit beside `screenshots/`, as installed (examples/compare.sh
//! arranges that). Run it inside a headless compositor, with the fake commands
//! of compare.sh first on `PATH`: an install runs `curl`, the sudo wrapper,
//! the configuration script and `rum`. The host accepts only the password in
//! `$FAKE_PASSWORD`, and its sudo wrapper is `$FAKE_WRAPPER`.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::glib::translate::{FromGlibPtrBorrow, FromGlibPtrFull};
use gtk::{gio, glib, graphene};
use linexin_localization::{Localization, widgets};
use linexin_plugin_api::{
    ABI_VERSION, CreateContextV1, ENTRY_SYMBOL, EntryV1, Header, HostApiV1, WidgetDescriptorV1,
};

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
const SETTLE: Duration = Duration::from_millis(1500);
const PATIENCE: Duration = Duration::from_secs(30);

struct Options {
    library: PathBuf,
    localization: PathBuf,
    /// The Center's stylesheet; its own copy (`CENTER_STYLE_CSS`) without one.
    css: Option<PathBuf>,
    language: CString,
    dark: bool,
    width: i32,
    height: i32,
    hide_sidebar: bool,
    output: Option<PathBuf>,
    clicks: Vec<String>,
    after_click: Option<PathBuf>,
    dialog_output: Option<PathBuf>,
    choose: Option<PathBuf>,
    password: Option<String>,
    respond: String,
    progress_at: Option<String>,
    progress_output: Option<PathBuf>,
    wait_finish: bool,
    final_output: Option<PathBuf>,
    final_dialog_output: Option<PathBuf>,
}

static OPTIONS: OnceLock<Options> = OnceLock::new();

fn options() -> &'static Options {
    OPTIONS
        .get()
        .expect("options are set before the host is used")
}

fn parse_options() -> Result<Options, String> {
    let manifest = Path::new(MANIFEST_DIR);
    let mut options = Options {
        library: beside_the_examples("libaffinity_installer.so"),
        localization: manifest.join("data/usr/share/linexin/widgets/localization"),
        css: None,
        language: c"en_US".into(),
        dark: false,
        width: 1300,
        height: 820,
        hide_sidebar: false,
        output: None,
        clicks: Vec::new(),
        after_click: None,
        dialog_output: None,
        choose: None,
        password: None,
        respond: "cancel".into(),
        progress_at: None,
        progress_output: None,
        wait_finish: false,
        final_output: None,
        final_dialog_output: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--library" => options.library = value()?.into(),
            "--localization" => options.localization = value()?.into(),
            "--css" => options.css = Some(value()?.into()),
            "--lang" => options.language = CString::new(value()?).map_err(|e| e.to_string())?,
            "--dark" => options.dark = true,
            "--width" => options.width = value()?.parse().map_err(|_| "bad --width")?,
            "--height" => options.height = value()?.parse().map_err(|_| "bad --height")?,
            "--hide-sidebar" => options.hide_sidebar = true,
            "--output" => options.output = Some(value()?.into()),
            "--click" => options.clicks.push(value()?),
            "--after-click" => options.after_click = Some(value()?.into()),
            "--dialog-output" => options.dialog_output = Some(value()?.into()),
            "--choose" => options.choose = Some(value()?.into()),
            "--password" => options.password = Some(value()?),
            "--respond" => options.respond = value()?,
            "--progress-at" => options.progress_at = Some(value()?),
            "--progress-output" => options.progress_output = Some(value()?.into()),
            "--wait-finish" => options.wait_finish = true,
            "--final-output" => options.final_output = Some(value()?.into()),
            "--final-dialog-output" => options.final_dialog_output = Some(value()?.into()),
            "--widget" => {
                value()?;
            }
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    Ok(options)
}

fn install_localization() {
    let options = options();
    let localization = Localization::load(
        std::slice::from_ref(&options.localization),
        &options.language.to_string_lossy(),
        None,
    );
    for diagnostic in localization.diagnostics() {
        eprintln!("localization: {diagnostic}");
    }
    linexin_localization::install(localization);
}

/// A line in `$FAKE_LOG` (or `$FAKE_LOCK_LOG`), beside the fake commands'.
fn log(variable: &str, line: &str) {
    if let Some(path) = std::env::var_os(variable)
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

unsafe extern "C" fn language() -> *const c_char {
    options().language.as_ptr()
}

unsafe extern "C" fn translate(domain: *const c_char, text: *const c_char) -> *mut c_char {
    if domain.is_null() || text.is_null() {
        return std::ptr::null_mut();
    }
    let (domain, text) = unsafe { (CStr::from_ptr(domain), CStr::from_ptr(text)) };
    let translated = linexin_localization::global()
        .translate(&text.to_string_lossy(), &domain.to_string_lossy());
    CString::new(translated).map_or(std::ptr::null_mut(), CString::into_raw)
}

unsafe extern "C" fn free_string(string: *mut c_char) {
    if !string.is_null() {
        drop(unsafe { CString::from_raw(string) });
    }
}

unsafe extern "C" fn translate_widget(domain: *const c_char, widget: *mut c_void) {
    if domain.is_null() || widget.is_null() {
        return;
    }
    let domain = unsafe { CStr::from_ptr(domain) }.to_string_lossy();
    let widget = unsafe { gtk::Widget::from_glib_borrow(widget.cast::<gtk::ffi::GtkWidget>()) };
    widgets::translate_dialog(&widget, &domain);
}

unsafe extern "C" fn no_op() {}
unsafe extern "C" fn lock_ui() {
    log("FAKE_LOCK_LOG", "lock");
}
unsafe extern "C" fn unlock_ui() {
    log("FAKE_LOCK_LOG", "unlock");
}
unsafe extern "C" fn validate_password(password: *const c_char) -> bool {
    log("FAKE_LOG", "sudo validate_password");
    let expected = std::env::var("FAKE_PASSWORD").unwrap_or_default();
    !password.is_null()
        && !expected.is_empty()
        && unsafe { CStr::from_ptr(password) }.to_bytes() == expected.as_bytes()
}
unsafe extern "C" fn set_password(_: *const c_char) {
    log("FAKE_LOG", "sudo set_password");
}
unsafe extern "C" fn no_password() -> bool {
    false
}
unsafe extern "C" fn forget_password() {
    log("FAKE_LOG", "sudo forget_password");
}
unsafe extern "C" fn wrapper_path() -> *const c_char {
    static WRAPPER: OnceLock<CString> = OnceLock::new();
    WRAPPER
        .get_or_init(|| {
            CString::new(std::env::var("FAKE_WRAPPER").unwrap_or_else(|_| "/bin/false".into()))
                .unwrap_or_default()
        })
        .as_ptr()
}
unsafe extern "C" fn start_session() {
    log("FAKE_LOG", "sudo start_privileged_session");
}
unsafe extern "C" fn stop_session() {
    log("FAKE_LOG", "sudo stop_privileged_session");
}
unsafe extern "C" fn run_privileged(_: *const *const c_char) -> c_int {
    -1
}

unsafe extern "C" fn askpass_path() -> *const c_char {
    c"/bin/false".as_ptr()
}

unsafe extern "C" fn not_compact(_: *mut c_void) -> bool {
    false
}

static HOST: HostApiV1 = HostApiV1 {
    abi_version: ABI_VERSION,
    struct_size: size_of::<HostApiV1>() as u32,
    language,
    translate,
    free_string,
    translate_widget,
    lock_ui,
    unlock_ui,
    sudo_validate_password: validate_password,
    sudo_set_password: set_password,
    sudo_has_password: no_password,
    sudo_forget_password: forget_password,
    sudo_wrapper_path: wrapper_path,
    sudo_start_session: start_session,
    sudo_stop_session: stop_session,
    sudo_run_privileged: run_privileged,
    refresh_widgets: no_op,
    sudo_askpass_path: askpass_path,
    window_compact_mode: not_compact,
};

fn load_descriptor(path: &Path) -> Result<&'static WidgetDescriptorV1, String> {
    // SAFETY: a trusted development build of the widget.
    let library = unsafe { libloading::Library::new(path) }.map_err(|e| e.to_string())?;
    // Like the Center, never unload: GTK keeps pointers into the library.
    let library = Box::leak(Box::new(library));
    let entry = unsafe { library.get::<EntryV1>(ENTRY_SYMBOL) }.map_err(|e| e.to_string())?;
    let descriptor = unsafe { entry(&HOST) };
    if !unsafe { Header::is_compatible(descriptor.cast(), size_of::<WidgetDescriptorV1>()) } {
        return Err("incompatible descriptor".into());
    }
    Ok(unsafe { &*descriptor })
}

fn find_all<T: IsA<gtk::Widget>>(widget: &gtk::Widget, found: &mut Vec<T>) {
    if let Ok(widget) = widget.clone().downcast::<T>() {
        found.push(widget);
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        find_all(&current, found);
        child = current.next_sibling();
    }
}

fn render(widget: &impl IsA<gtk::Widget>, path: &Path) -> Result<(), String> {
    let widget = widget.as_ref();
    let (width, height) = (widget.width(), widget.height());
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(widget)).snapshot(&snapshot, width.into(), height.into());
    let node = snapshot.to_node().ok_or("nothing was drawn")?;
    let renderer = widget
        .native()
        .and_then(|native| native.renderer())
        .ok_or("no renderer")?;
    let viewport = graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    renderer
        .render_texture(&node, Some(&viewport))
        .save_to_png(path)
        .map_err(|e| e.to_string())?;
    println!("rendered {width}x{height} to {}", path.display());
    Ok(())
}

async fn sleep(duration: Duration) {
    glib::timeout_future(duration).await;
}

/// Wait (up to [`PATIENCE`]) until `ready` holds.
async fn until(what: &str, ready: impl Fn() -> bool) -> Result<(), String> {
    let start = Instant::now();
    while !ready() {
        if start.elapsed() > PATIENCE {
            return Err(format!("timed out waiting for {what}"));
        }
        sleep(Duration::from_millis(50)).await;
    }
    Ok(())
}

#[allow(deprecated)] // Adw.MessageDialog, as the Python widget uses
fn message_dialogs() -> Vec<adw::MessageDialog> {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|window| window.downcast::<adw::MessageDialog>().ok())
        .filter(|dialog| dialog.is_visible())
        .collect()
}

#[allow(deprecated)]
fn file_choosers() -> Vec<gtk::FileChooserDialog> {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|window| window.downcast::<gtk::FileChooserDialog>().ok())
        .filter(|dialog| dialog.is_visible())
        .collect()
}

#[allow(deprecated)]
fn print_dialogs() {
    for dialog in message_dialogs() {
        println!(
            "dialog heading={:?} body={:?}",
            dialog.heading().unwrap_or_default(),
            dialog.body()
        );
    }
    for chooser in file_choosers() {
        println!(
            "chooser title={:?} filter={:?} modal={}",
            chooser.title().unwrap_or_default(),
            chooser
                .filter()
                .and_then(|filter| filter.name())
                .unwrap_or_default(),
            chooser.is_modal()
        );
    }
}

/// root → [header → [icon, text, actions], separator, stack]
fn action_box(widget: &gtk::Widget) -> Option<gtk::Widget> {
    widget.first_child()?.last_child()
}

fn stack(widget: &gtk::Widget) -> Option<gtk::Stack> {
    widget.last_child()?.downcast::<gtk::Stack>().ok()
}

fn describe(widget: &gtk::Widget) -> String {
    let mut actions = Vec::new();
    let mut child = action_box(widget).and_then(|actions| actions.first_child());
    while let Some(current) = child {
        let classes: Vec<_> = ["suggested-action", "destructive-action"]
            .into_iter()
            .filter(|class| current.has_css_class(class))
            .collect();
        if let Ok(split) = current.clone().downcast::<adw::SplitButton>() {
            actions.push(format!(
                "split {:?} sensitive={} {classes:?}",
                split.label().unwrap_or_default(),
                split.is_sensitive()
            ));
        } else if let Ok(button) = current.clone().downcast::<gtk::Button>() {
            actions.push(format!(
                "button {:?} sensitive={} {classes:?}",
                button.label().unwrap_or_default(),
                button.is_sensitive()
            ));
        }
        child = current.next_sibling();
    }
    let page = stack(widget)
        .and_then(|stack| stack.visible_child_name())
        .unwrap_or_default();
    let mut line = format!("state page={page} actions=[{}]", actions.join(", "));
    if page == "progress" {
        let mut labels = Vec::<gtk::Label>::new();
        let mut bars = Vec::<gtk::ProgressBar>::new();
        if let Some(stack) = stack(widget) {
            find_all(stack.upcast_ref(), &mut labels);
            find_all(stack.upcast_ref(), &mut bars);
        }
        let step = labels
            .iter()
            .find(|label| label.has_css_class("title-4"))
            .map(|label| label.label().to_string())
            .unwrap_or_default();
        let fraction = bars.first().map_or(-1.0, |bar| bar.fraction());
        line.push_str(&format!(" step={step:?} fraction={fraction:.4}"));
    }
    line
}

fn step_label(widget: &gtk::Widget) -> String {
    describe(widget)
        .split(" step=")
        .nth(1)
        .and_then(|rest| rest.split(" fraction=").next())
        .map(|quoted| quoted.trim_matches('"').to_owned())
        .unwrap_or_default()
}

/// The files the scenario touches, as `ls` would list them.
fn print_files() {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let temporary = PathBuf::from(std::env::var_os("TMPDIR").unwrap_or_default());
    let list = |dir: PathBuf| {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    };
    println!(
        "files apps={:?} applications={:?} shortcuts={:?} desktop={:?} tmp={:?}",
        list(home.join(".WineAffinity/drive_c/Program Files/Affinity")),
        list(home.join(".local/share/applications")),
        list(home.join(".local/share/linexin/shortcuts")),
        list(home.join("Desktop")),
        list(temporary),
    );
}

#[allow(deprecated)]
async fn click(widget: &gtk::Widget, what: &str) -> Result<(), String> {
    match what {
        "next" | "prev" => {
            let icon = if what == "next" {
                "go-next-symbolic"
            } else {
                "go-previous-symbolic"
            };
            let mut buttons = Vec::<gtk::Button>::new();
            find_all(widget, &mut buttons);
            let button = buttons
                .into_iter()
                .find(|button| button.icon_name().as_deref() == Some(icon))
                .ok_or(format!("no {what} button"))?;
            button.emit_clicked();
        }
        "download" | "menu" => {
            let mut splits = Vec::<adw::SplitButton>::new();
            find_all(widget, &mut splits);
            let split = splits.first().ok_or("no split button")?;
            if what == "download" {
                split.emit_clicked();
            } else {
                split.popup();
            }
        }
        "select-exe" => {
            widget
                .activate_action("install.select_exe", None)
                .map_err(|e| e.to_string())?;
        }
        label => {
            let mut buttons = Vec::<gtk::Button>::new();
            find_all(&action_box(widget).ok_or("no action box")?, &mut buttons);
            let button = buttons
                .into_iter()
                .find(|button| button.label().as_deref() == Some(label))
                .ok_or(format!("no button {label:?}"))?;
            button.emit_clicked();
        }
    }
    println!("clicked {what}");
    Ok(())
}

#[allow(deprecated)]
async fn scenario(window: adw::ApplicationWindow, widget: gtk::Widget) -> Result<(), String> {
    let options = options();
    sleep(SETTLE).await;
    println!("{}", describe(&widget));
    if let Some(path) = &options.output {
        render(&window, path)?;
    }
    if options.clicks.is_empty() {
        print_files();
        return Ok(());
    }
    for what in &options.clicks {
        click(&widget, what).await?;
        sleep(Duration::from_millis(300)).await;
    }
    sleep(SETTLE).await;
    println!("{}", describe(&widget));
    print_dialogs();
    if let Some(path) = &options.after_click {
        render(&window, path)?;
    }
    if let Some(path) = &options.dialog_output {
        if let Some(dialog) = message_dialogs().first() {
            render(dialog, path)?;
        } else if let Some(chooser) = file_choosers().first() {
            render(chooser, path)?;
        } else {
            let mut popovers = Vec::<gtk::PopoverMenu>::new();
            find_all(&widget, &mut popovers);
            if let Some(popover) = popovers.iter().find(|popover| popover.is_visible()) {
                render(popover, path)?;
            }
        }
    }
    if let Some(path) = &options.choose {
        let chooser = file_choosers()
            .into_iter()
            .next()
            .ok_or("no file chooser")?;
        chooser
            .set_file(&gio::File::for_path(path))
            .map_err(|e| e.to_string())?;
        sleep(Duration::from_millis(500)).await;
        chooser.response(gtk::ResponseType::Ok);
        sleep(SETTLE).await;
        println!("chose {}", path.display());
        print_dialogs();
    }
    let dialogs = message_dialogs();
    if let Some(password) = &options.password {
        let dialog = dialogs.first().ok_or("no password dialog")?;
        let mut entries = Vec::<gtk::PasswordEntry>::new();
        find_all(dialog.upcast_ref(), &mut entries);
        entries
            .first()
            .ok_or("no password entry")?
            .set_text(password);
        dialog.response("unlock");
        println!("responded unlock");
    } else {
        for dialog in &dialogs {
            dialog.response(&options.respond);
        }
        for chooser in file_choosers() {
            chooser.response(gtk::ResponseType::Cancel);
        }
        println!("responded {}", options.respond);
    }
    if let Some(step) = &options.progress_at {
        until(step, || step_label(&widget) == *step).await?;
        sleep(Duration::from_millis(500)).await;
        println!("{}", describe(&widget));
        if let Some(path) = &options.progress_output {
            render(&window, path)?;
        }
    }
    if options.wait_finish {
        until("the end", || {
            let line = describe(&widget);
            !line.contains("page=progress") && !line.contains("sensitive=false")
        })
        .await?;
    }
    sleep(SETTLE).await;
    println!("{}", describe(&widget));
    print_dialogs();
    if let Some(path) = &options.final_dialog_output
        && let Some(dialog) = message_dialogs().first()
    {
        render(dialog, path)?;
    }
    if let Some(path) = &options.final_output {
        render(&window, path)?;
    }
    for dialog in message_dialogs() {
        dialog.response("ok");
    }
    print_files();
    Ok(())
}

fn activate(app: &adw::Application) -> Result<(), String> {
    let options = options();
    adw::StyleManager::default().set_color_scheme(if options.dark {
        adw::ColorScheme::ForceDark
    } else {
        adw::ColorScheme::ForceLight
    });
    // The Center's application style sheet (.dim-label, ...).
    let css = gtk::CssProvider::new();
    css.load_from_string(&match &options.css {
        Some(path) => {
            std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => linexin_plugin_api::CENTER_STYLE_CSS.to_owned(),
    });
    let display = gtk::gdk::Display::default().ok_or("no display")?;
    gtk::style_context_add_provider_for_display(
        &display,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let descriptor = load_descriptor(&options.library)?;
    let text = |pointer| {
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned()
    };
    println!(
        "widgetname={:?} icon={} id={}",
        text(descriptor.name),
        text(descriptor.icon),
        text(descriptor.widget_id)
    );

    let window = adw::ApplicationWindow::new(app);
    window.set_default_size(options.width, options.height);
    let context = CreateContextV1 {
        abi_version: ABI_VERSION,
        struct_size: size_of::<CreateContextV1>() as u32,
        host: &HOST,
        window: window.upcast_ref::<gtk::Window>().as_ptr().cast(),
        hide_sidebar: options.hide_sidebar,
        voice_autostart: false,
        conversation_id: std::ptr::null(),
    };
    let create = descriptor.create.ok_or("descriptor without create")?;
    let pointer = unsafe { create(&context) };
    if pointer.is_null() {
        return Err("create returned null".into());
    }
    let widget = unsafe { gtk::Widget::from_glib_full(pointer.cast::<gtk::ffi::GtkWidget>()) };
    window.set_content(Some(&widget));
    window.present();

    let app = app.clone();
    glib::MainContext::default().spawn_local(async move {
        if let Err(error) = scenario(window, widget).await {
            eprintln!("harness: {error}");
            FAILED.with(|failed| failed.replace(true));
        }
        app.quit();
    });
    Ok(())
}

thread_local! {
    static FAILED: RefCell<bool> = const { RefCell::new(false) };
}

fn main() -> ExitCode {
    // The stylesheet, for the Python reference to load the same one.
    if std::env::args().nth(1).as_deref() == Some("--print-css") {
        print!("{}", linexin_plugin_api::CENTER_STYLE_CSS);
        return ExitCode::SUCCESS;
    }
    match parse_options() {
        Ok(options) => {
            let _ = OPTIONS.set(options);
            install_localization();
        }
        Err(error) => {
            eprintln!("harness: {error}");
            return ExitCode::FAILURE;
        }
    }
    let app = adw::Application::builder()
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(|app| {
        if let Err(error) = activate(app) {
            eprintln!("harness: {error}");
            FAILED.with(|failed| failed.replace(true));
            app.quit();
        }
    });
    let status = app.run_with_args::<&str>(&[]);
    if status == glib::ExitCode::SUCCESS && !FAILED.with(|failed| *failed.borrow()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// `name` where cargo built this harness's library: `<target>/<profile>/`,
/// beside the `examples/` holding the harness (wherever CARGO_TARGET_DIR put it).
fn beside_the_examples(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|harness| Some(harness.parent()?.parent()?.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}
