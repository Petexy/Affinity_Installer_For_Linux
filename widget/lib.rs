//! Native port of the Affinity Installer widget (`d-affinity_installer.py`),
//! installed as `/usr/share/linexin/widgets/d-affinity_installer.so`.
//!
//! Two front ends share the install steps (`install`): the GTK page (`ui`,
//! identical to the Python widget) and the LineXinBar page (`lxb`).

mod install;
mod lxb;
mod ui;

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::ptr;

use gtk::glib::translate::{FromGlibPtrNone, IntoGlibPtr};
use gtk::prelude::*;
use linexin_plugin_api as api;

static WIDGET: api::WidgetDescriptorV1 = api::WidgetDescriptorV1::new(
    c"affinity_installer",
    c"Affinity Installer",
    c"/usr/share/icons/github.petexy.affinityinstaller.png",
    Some(create),
)
.with_lxb_page(c"category-graphics", lxb::create, lxb::page, lxb::destroy);
api::export_widget!(WIDGET);

unsafe extern "C" fn create(context: *const api::CreateContextV1) -> *mut c_void {
    let widget = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: `context` is the pointer the host passed to `create`.
        let context = unsafe { api::CreateContextV1::read(context) }?;
        if let Err(error) = adw::init() {
            eprintln!("Affinity Installer: could not initialize GTK: {error}");
            return None;
        }
        let window = (!context.window.is_null())
            .then(|| {
                // SAFETY: the host passes a live GtkWindow*; a new reference is taken.
                let object = unsafe {
                    gtk::glib::Object::from_glib_none(
                        context.window.cast::<gtk::glib::gobject_ffi::GObject>(),
                    )
                };
                object.downcast::<gtk::Window>().ok()
            })
            .flatten();
        let root = ui::build(api::host(), window, context.hide_sidebar).upcast::<gtk::Widget>();
        let widget: *mut gtk::ffi::GtkWidget = root.into_glib_ptr();
        Some(widget.cast::<c_void>())
    }));
    match widget {
        Ok(Some(widget)) => widget,
        Ok(None) => ptr::null_mut(),
        Err(_) => {
            eprintln!("Affinity Installer: widget creation panicked");
            ptr::null_mut()
        }
    }
}

/// The folder the installed library is in (the Python widget's
/// `/usr/share/linexin/widgets`), where `screenshots/` lies.
fn widgets_dir() -> PathBuf {
    let mut info: libc::Dl_info = unsafe { std::mem::zeroed() };
    // SAFETY: dladdr only reads the address of a function in this library.
    let found = unsafe { libc::dladdr(widgets_dir as *const c_void, &mut info) } != 0;
    let library = (found && !info.dli_fname.is_null()).then(|| {
        // SAFETY: dladdr filled a NUL-terminated path.
        PathBuf::from(
            unsafe { std::ffi::CStr::from_ptr(info.dli_fname) }
                .to_string_lossy()
                .into_owned(),
        )
    });
    library
        .and_then(|path| path.parent().map(PathBuf::from))
        .filter(|dir| dir.join("screenshots").is_dir())
        .unwrap_or_else(|| PathBuf::from("/usr/share/linexin/widgets"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn descriptor_matches_the_python_widget() {
        let text = |pointer| unsafe { CStr::from_ptr(pointer) }.to_str().unwrap();
        assert_eq!(text(WIDGET.widget_id), ui::DOMAIN);
        assert_eq!(text(WIDGET.name), "Affinity Installer");
        assert_eq!(
            text(WIDGET.icon),
            "/usr/share/icons/github.petexy.affinityinstaller.png"
        );
        assert_eq!(text(WIDGET.lxb_mark), "category-graphics");
        assert!(WIDGET.create.is_some() && WIDGET.lxb_page.is_some());
    }

    #[test]
    fn entry_point_rejects_a_missing_host() {
        assert!(unsafe { linexin_widget_v1(ptr::null()) }.is_null());
        assert!(unsafe { create(ptr::null()) }.is_null());
    }
}
