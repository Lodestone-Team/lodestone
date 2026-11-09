//! What the window shows, for the debug traces: recorded each time the shell draws,
//! with an event for each change. See [`launcher_core::trace`].

use std::time::Instant;

use gpui::{Context, Window};
use launcher_core::trace;
use serde_json::{Value, json};

use super::{InstanceTab, LauncherShell, Modal, Page, settings_modal::SettingsPage};

/// The last values recorded, to emit only changes.
#[derive(Default)]
pub(super) struct TraceState {
    last: Option<Value>,
    last_frame: Option<Instant>,
}

impl Modal {
    /// The dialog's name in traces.
    fn trace_name(&self) -> &'static str {
        match self {
            Modal::NewInstance { .. } => "new_instance",
            Modal::Import { .. } => "import",
            Modal::Share { .. } => "share",
            Modal::Crash { .. } => "crash",
            Modal::LaunchFailed { .. } => "launch_failed",
            Modal::DeleteInstance { .. } => "delete_instance",
            Modal::SignIn { .. } => "sign_in",
            Modal::Offline { .. } => "offline_account",
            Modal::RemoveAccount { .. } => "remove_account",
            Modal::LoaderChange { .. } => "loader_change",
            Modal::DeleteWorld { .. } => "delete_world",
            Modal::ReplaceWorld { .. } => "replace_world",
            Modal::ModpackUpdate { .. } => "modpack_update",
            Modal::EnableLauncherIdentity { .. } => "enable_launcher_identity",
            Modal::DisableLauncherIdentity { .. } => "disable_launcher_identity",
            Modal::Unsaved { .. } => "unsaved_changes",
            Modal::StorageClean { .. } => "storage_clean",
        }
    }
}

fn page_name(page: Page) -> &'static str {
    match page {
        Page::Instance => "instance",
        Page::Servers => "servers",
    }
}

fn tab_name(tab: InstanceTab) -> &'static str {
    match tab {
        InstanceTab::Play => "play",
        InstanceTab::Content => "content",
        InstanceTab::Worlds => "worlds",
        InstanceTab::Settings => "settings",
        InstanceTab::Logs => "logs",
    }
}

fn settings_page_name(page: SettingsPage) -> &'static str {
    match page {
        SettingsPage::Defaults => "instance_defaults",
        SettingsPage::Shared => "shared_game_settings",
        SettingsPage::General => "general",
        SettingsPage::Accounts => "accounts",
        SettingsPage::Storage => "storage",
        SettingsPage::Developer => "developer",
        SettingsPage::Skin => "skin",
    }
}

impl LauncherShell {
    /// Records the window's state, and emits `nav.*`, `ui.modal`, `ui.settings`,
    /// `ui.window` and `ui.zoom` for what changed since the last frame. Cheap: a few
    /// comparisons when nothing changed.
    pub(super) fn trace_frame(&mut self, window: &Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        if let Some(last) = self.trace.last_frame.replace(now) {
            let gap = now.duration_since(last).as_secs_f64() * 1000.0;
            trace::emit(
                "perf.frame",
                || json!({ "since_last_ms": (gap * 10.0).round() / 10.0 }),
            );
        }
        let size = window.viewport_size();
        let instance = self
            .instances
            .read(cx)
            .selected()
            .map(|i| (i.id.clone(), i.name.clone()));
        let state = json!({
            "page": page_name(self.page),
            "tab": tab_name(self.tab),
            "instance": instance.as_ref().map(|(id, name)| json!({ "id": id, "name": name })),
            "modal": self.modal.current().map(|m| m.trace_name()),
            "settings": self.settings_modal.current().map(|_| settings_page_name(self.settings_page)),
            "window": { "width": f32::from(size.width).round(), "height": f32::from(size.height).round() },
            "zoom": crate::theme::zoom::get(),
            "account": self.accounts.read(cx).active().map(|a| a.username.clone()),
        });
        if self.trace.last.as_ref() == Some(&state) {
            return;
        }
        let before = self
            .trace
            .last
            .replace(state.clone())
            .unwrap_or(Value::Null);
        let changed = |key: &str| before.get(key) != state.get(key);
        for (key, topic) in [
            ("page", "nav.page"),
            ("tab", "nav.tab"),
            ("instance", "nav.instance"),
            ("window", "ui.window"),
            ("zoom", "ui.zoom"),
            ("account", "nav.account"),
        ] {
            if changed(key) {
                trace::emit(
                    topic,
                    || json!({ "from": before.get(key), "to": state[key] }),
                );
            }
        }
        if changed("modal") {
            if let Some(closed) = before.get("modal").filter(|m| !m.is_null()) {
                trace::emit("ui.modal", || json!({ "action": "close", "modal": closed }));
            }
            if !state["modal"].is_null() {
                trace::emit(
                    "ui.modal",
                    || json!({ "action": "open", "modal": state["modal"] }),
                );
            }
        }
        if changed("settings") {
            trace::emit(
                "ui.settings",
                || json!({ "from": before.get("settings"), "to": state["settings"] }),
            );
        }
        if let Value::Object(fields) = state {
            for (key, value) in fields {
                trace::set_state(&key, value);
            }
        }
    }
}
