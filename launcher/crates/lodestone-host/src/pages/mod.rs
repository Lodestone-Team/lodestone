//! The pages, one file each, made by [`open`] for a [`Route`].
//!
//! Each page is a view made with `new(state, …, cx)` that reads and follows
//! [`HostState`] and fetches what only it shows through `state.read(cx).admin`. Pages
//! that are still stubs say so with [`not_built`]; the work package that owns a page
//! replaces its stub (see `launcher/docs/host-parity.md`):
//!
//! | Page | File | Package |
//! |---|---|---|
//! | Home (no instance selected) | [`home`] | foundation |
//! | Create instance (dialog) | [`create_instance`] | A |
//! | Overview | [`overview`] | A |
//! | Console | [`console`] | B |
//! | Files | [`files`] | B |
//! | Settings (instance) | [`instance_settings`] | C |
//! | Mods / Plugins | [`mods`] | C |
//! | Event logs | [`event_logs`] | C |
//! | Playit.gg | [`playit`] | C |
//! | Macros (needs Deno: out of scope) | [`macros`] | — |
//! | Settings › General | [`core_settings`] | D |
//! | Settings › Users | [`users`] | D |
//! | Settings › Profile | [`profile`] | D |

pub mod account_dialogs;
pub mod api_keys;
pub mod console;
pub mod core_settings;
pub mod cores;
pub mod create_instance;
pub mod eula_prompt;
pub mod event_logs;
pub mod files;
pub mod home;
pub mod instance_settings;
pub mod macros;
pub mod minecraft_links;
pub mod mods;
pub mod overview;
pub mod parts;
pub mod permissions;
pub mod playit;
pub mod profile;
pub mod public_listing;
pub mod updates;
pub mod users;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, Entity, IntoElement, ParentElement, Styled, div,
    prelude::FluentBuilder as _,
};
use lodestone_ui::theme::{ink, px, text};

use crate::{
    i18n::{ShellMsg, t},
    routes::{InstanceTab, Route, SettingsPage},
    state::HostState,
};

/// Registers the pages' key bindings. Call once, after `lodestone_ui::init`.
pub fn init(cx: &mut App) {
    console::init(cx);
    files::init(cx);
}

/// The page for `route`.
pub fn open(route: &Route, state: Entity<HostState>, cx: &mut App) -> AnyView {
    match route {
        Route::Home => cx.new(|cx| home::HomePage::new(state, cx)).into(),
        Route::Instance { uuid, tab } => {
            let uuid = uuid.clone();
            match tab {
                InstanceTab::Overview => cx
                    .new(|cx| overview::OverviewPage::new(state, uuid, cx))
                    .into(),
                InstanceTab::Settings => cx
                    .new(|cx| instance_settings::InstanceSettingsPage::new(state, uuid, cx))
                    .into(),
                InstanceTab::Console => cx
                    .new(|cx| console::ConsolePage::new(state, uuid, cx))
                    .into(),
                InstanceTab::Files => cx.new(|cx| files::FilesPage::new(state, uuid, cx)).into(),
                InstanceTab::Mods => cx.new(|cx| mods::ModsPage::new(state, uuid, cx)).into(),
                InstanceTab::Macros => cx.new(|cx| macros::MacrosPage::new(state, uuid, cx)).into(),
                InstanceTab::EventLogs => cx
                    .new(|cx| event_logs::EventLogsPage::new(state, uuid, cx))
                    .into(),
                InstanceTab::Playit => cx.new(|cx| playit::PlayitPage::new(state, uuid, cx)).into(),
            }
        }
        Route::Settings(page) => match page {
            SettingsPage::General => cx
                .new(|cx| core_settings::CoreSettingsPage::new(state, cx))
                .into(),
            SettingsPage::Users => cx.new(|cx| users::UsersPage::new(state, cx)).into(),
            SettingsPage::Profile => cx.new(|cx| profile::ProfilePage::new(state, cx)).into(),
            SettingsPage::Updates => cx.new(|cx| updates::UpdatesPage::new(state, cx)).into(),
        },
    }
}

/// Fills the page with sample data (console lines, …), for the development
/// screenshots' `--demo`.
#[cfg(feature = "screenshot")]
pub fn demo(view: &AnyView, cx: &mut App) {
    if let Ok(page) = view.clone().downcast::<console::ConsolePage>() {
        page.update(cx, |page, cx| page.demo(cx));
    }
    if let Ok(page) = view.clone().downcast::<files::FilesPage>() {
        page.update(cx, |page, cx| page.demo(cx));
    }
    if let Ok(page) = view.clone().downcast::<overview::OverviewPage>() {
        page.update(cx, |page, cx| page.demo(cx));
    }
}

/// Stops sharing the server whose Overview `view` is, as its Stop sharing dialog's
/// confirm button does (development screenshots' end-to-end run).
pub fn stop_share_for_screenshot(view: &AnyView, cx: &mut App) {
    if let Ok(page) = view.clone().downcast::<overview::OverviewPage>() {
        page.update(cx, |page, cx| page.confirm_stop_share(cx));
    }
}

/// What a page still to be built shows, naming its work package.
pub fn not_built(package: &'static str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .p(px(16.))
        .text_size(text::MEDIUM)
        .text_color(ink::muted())
        .child(t(ShellMsg::NotBuilt))
        .when(crate::IS_DEV, |this| {
            this.child(
                div()
                    .text_size(text::SMALL)
                    .text_color(ink::faint())
                    .child(format!(
                        "Work package {package}: launcher/docs/host-parity.md"
                    )),
            )
        })
        .into_any_element()
}

#[cfg(all(test, feature = "screenshot"))]
mod tests {
    /// Every page's key bindings parse (a bad one panics at startup).
    #[gpui::test]
    fn the_pages_key_bindings_load(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            lodestone_ui::components::init(cx);
            super::init(cx);
        });
    }
}
