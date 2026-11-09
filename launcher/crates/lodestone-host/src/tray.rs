//! The menu bar icon, as the Tauri Host's tray icon: it keeps the Host reachable while
//! its window is closed and the servers run in the background. Its menu shows the
//! window or quits (stopping the core and its servers).
//!
//! GPUI has no tray API, so on macOS this is an `NSStatusItem` made through AppKit.
//! On Windows and Linux there is no icon yet (work package D): closing the window
//! minimizes it there, and starting the Host again brings it back.
//!
//! Development builds also set the Dock icon with the DEV ribbon, as the Tauri Host
//! did, since the bundle's icon is the release one.

use futures::channel::mpsc::UnboundedSender;

/// What the menu asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    Show,
    Quit,
}

/// Keeps the icon on screen while it lives.
pub struct Tray {
    #[cfg(target_os = "macos")]
    _item: macos::Item,
}

/// Shows the icon, sending what its menu asks for to `tx`. `None` where there is no
/// icon (Windows and Linux, for now).
pub fn install(show_label: &str, quit_label: &str, tx: UnboundedSender<TrayCommand>) -> Option<Tray> {
    #[cfg(target_os = "macos")]
    {
        macos::install(show_label, quit_label, tx).map(|item| Tray { _item: item })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (show_label, quit_label, tx);
        None
    }
}

/// In a development build on macOS, the Dock shows the DEV icon.
pub fn set_dev_dock_icon() {
    #[cfg(target_os = "macos")]
    if crate::IS_DEV {
        macos::set_dock_icon(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/images/host-icon-dev.png"
        )));
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::{Mutex, OnceLock};

    use futures::channel::mpsc::UnboundedSender;
    use objc2::{
        AllocAnyThread as _, MainThreadMarker, MainThreadOnly, define_class, msg_send,
        rc::Retained, runtime::AnyObject, sel,
    };
    use objc2_app_kit::{
        NSApplication, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
        NSVariableStatusItemLength,
    };
    use objc2_foundation::{NSData, NSObject, NSSize, NSString};

    use super::TrayCommand;

    /// Where the menu's commands go.
    static COMMANDS: OnceLock<Mutex<UnboundedSender<TrayCommand>>> = OnceLock::new();

    fn send(command: TrayCommand) {
        if let Some(tx) = COMMANDS.get() {
            let _ = tx
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .unbounded_send(command);
        }
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, and the class implements no
        // Drop.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "LodestoneHostTrayTarget"]
        struct Target;

        impl Target {
            #[unsafe(method(showHost:))]
            fn show_host(&self, _sender: Option<&AnyObject>) {
                send(TrayCommand::Show);
            }

            #[unsafe(method(quitHost:))]
            fn quit_host(&self, _sender: Option<&AnyObject>) {
                send(TrayCommand::Quit);
            }
        }
    );

    /// The status item and what its menu needs, kept alive together.
    pub struct Item {
        item: Retained<NSStatusItem>,
        _menu: Retained<NSMenu>,
        _target: Retained<Target>,
    }

    impl Drop for Item {
        fn drop(&mut self) {
            NSStatusBar::systemStatusBar().removeStatusItem(&self.item);
        }
    }

    fn menu_item(
        mtm: MainThreadMarker,
        title: &str,
        action: objc2::runtime::Sel,
        target: &Target,
    ) -> Retained<NSMenuItem> {
        // SAFETY: the action is one of `Target`'s methods, which take the sender.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                Some(action),
                &NSString::from_str(""),
            )
        };
        // SAFETY: the target outlives the item: both are kept in `Item`.
        unsafe { item.setTarget(Some(target)) };
        item
    }

    pub fn install(show: &str, quit: &str, tx: UnboundedSender<TrayCommand>) -> Option<Item> {
        let mtm = MainThreadMarker::new()?;
        match COMMANDS.get() {
            Some(slot) => *slot.lock().unwrap_or_else(|e| e.into_inner()) = tx,
            None => {
                let _ = COMMANDS.set(Mutex::new(tx));
            }
        }
        // SAFETY: `new` takes no arguments and returns a retained `Target`.
        let target: Retained<Target> = unsafe { msg_send![Target::alloc(mtm), init] };
        let menu = NSMenu::new(mtm);
        menu.addItem(&menu_item(mtm, show, sel!(showHost:), &target));
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&menu_item(mtm, quit, sel!(quitHost:), &target));

        let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = item.button(mtm) {
            let image = image(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/images/host-tray@2x.png"
            )));
            match image {
                Some(image) => {
                    image.setSize(NSSize::new(18.0, 18.0));
                    // drawn in the menu bar's colour, light or dark
                    image.setTemplate(true);
                    button.setImage(Some(&image));
                }
                None => button.setTitle(&NSString::from_str("Lodestone")),
            }
        }
        item.setMenu(Some(&menu));
        Some(Item {
            item,
            _menu: menu,
            _target: target,
        })
    }

    fn image(bytes: &'static [u8]) -> Option<Retained<NSImage>> {
        let data = NSData::with_bytes(bytes);
        NSImage::initWithData(NSImage::alloc(), &data)
    }

    pub fn set_dock_icon(bytes: &'static [u8]) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        if let Some(image) = image(bytes) {
            // SAFETY: on the main thread, with an image we made.
            unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
        }
    }
}
