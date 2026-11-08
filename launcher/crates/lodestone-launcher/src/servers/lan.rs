//! "On your network": worlds opened to LAN, heard while the Servers page is shown
//! ([`launcher_core::cores::lan`]). Join starts a chosen instance straight into the
//! world, with the same join arguments as a server.
//!
//! macOS asks once whether the launcher may use the local network. Until it is
//! allowed, nothing arrives, so after a few quiet seconds the section says where to
//! allow it.

use std::{
    net::IpAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, IntoElement, SharedString, Task, div,
    prelude::*, px,
};
use launcher_core::{
    cores::lan::{self, LanListener, LanWorld, LanWorlds},
    game::launch::JoinServer,
};

use super::{Servers, ServersEvent, extras::note};
use crate::{
    components::{Button, ButtonSize, ButtonVariant, Dialog, Icon, LoadingDots},
    theme::{self, colors, fonts, radius, text, weights},
};

/// How often the list is checked for changes and expired worlds.
const TICK: Duration = Duration::from_millis(500);
/// After this long with nothing heard, the section explains why that may be.
pub const HINT_AFTER: Duration = Duration::from_secs(8);

#[derive(Default)]
pub struct LanState {
    listener: Option<LanListener>,
    worlds: Arc<Mutex<LanWorlds>>,
    /// Set by the listener thread when the list changed in a way people see.
    dirty: Arc<AtomicBool>,
    started: Option<Instant>,
    error: Option<SharedString>,
    /// The worlds as last drawn.
    shown: Vec<LanWorld>,
    heard_any: bool,
    _ticker: Option<Task<()>>,
    preview_quiet: bool,
}

/// "Join with…": the world, while an instance is chosen.
pub struct LanPicker {
    pub world: LanWorld,
    focus: FocusHandle,
}

impl Servers {
    /// Starts listening when the page is shown, and stops when it is hidden.
    pub(super) fn lan_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        // the offscreen screenshots fill the list themselves
        if self.preview {
            return;
        }
        let state = &mut self.extras.lan;
        if !visible {
            state.listener = None;
            state._ticker = None;
            state.started = None;
            state.shown.clear();
            state.heard_any = false;
            if let Ok(mut worlds) = state.worlds.lock() {
                worlds.clear();
            }
            return;
        }
        if state.listener.is_some() {
            return;
        }
        state.started = Some(Instant::now());
        state.error = None;
        let worlds = state.worlds.clone();
        let dirty = state.dirty.clone();
        let started = lan::lan_socket().and_then(|socket| {
            lan::listen(socket, move |host, announcement| {
                if let Ok(mut worlds) = worlds.lock() {
                    let heard_before = worlds.heard_any;
                    if worlds.record(host, announcement, Instant::now()) || !heard_before {
                        dirty.store(true, Ordering::Relaxed);
                    }
                }
                true
            })
        });
        match started {
            Ok(listener) => state.listener = Some(listener),
            Err(err) => {
                state.error = Some(format!("Couldn't listen for LAN worlds: {err}").into());
                return;
            }
        }
        state._ticker = Some(cx.spawn(async move |this, cx| {
            let mut hinted = false;
            loop {
                cx.background_executor().timer(TICK).await;
                let alive = this
                    .update(cx, |this, cx| {
                        let state = &mut this.extras.lan;
                        let mut changed = state.dirty.swap(false, Ordering::Relaxed);
                        if let Ok(mut worlds) = state.worlds.lock() {
                            changed |= worlds.expire(Instant::now());
                            if changed {
                                state.shown = worlds.list();
                                state.heard_any = worlds.heard_any;
                            }
                        }
                        let quiet = !state.heard_any
                            && state.started.is_some_and(|s| s.elapsed() >= HINT_AFTER);
                        if quiet && !hinted {
                            hinted = true;
                            changed = true;
                        }
                        if changed {
                            cx.notify();
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        }));
    }

    pub(super) fn open_lan_join(&mut self, world: LanWorld, cx: &mut Context<Self>) {
        self.extras.lan_picker = Some(LanPicker {
            world,
            focus: cx.focus_handle(),
        });
        cx.notify();
    }

    fn join_lan(&mut self, instance_id: &str, cx: &mut Context<Self>) {
        let Some(picker) = self.extras.lan_picker.take() else {
            return;
        };
        let host = match picker.world.host {
            IpAddr::V4(ip) => ip.to_string(),
            IpAddr::V6(ip) => ip.to_string(),
        };
        let target = JoinServer {
            host,
            port: picker.world.port,
        };
        let id = instance_id.to_string();
        if self.instances.read(cx).is_active(&id) {
            cx.emit(ServersEvent::OpenInstance(id));
            return;
        }
        self.instances.update(cx, |instances, cx| {
            instances.play_joining(&id, Some(target), cx)
        });
        cx.emit(ServersEvent::OpenInstance(id));
        cx.notify();
    }

    /// The "On your network" section of the page.
    pub(super) fn render_lan(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.extras.lan;
        let quiet = state.preview_quiet
            || (!state.heard_any && state.started.is_some_and(|s| s.elapsed() >= HINT_AFTER));
        let body: AnyElement = if let Some(error) = &state.error {
            note(error.clone())
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(colors::FADE_700)
                .text_color(colors::YELLOW)
                .into_any_element()
        } else if state.shown.is_empty() {
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap_2()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(colors::FADE_700)
                .when(!quiet, |this| this.child(LoadingDots))
                .child(note(if quiet {
                    "No LAN worlds found. In Minecraft, open a world and choose Open to LAN. If macOS asked whether Lodestone Launcher may find devices on your local network, allow it in System Settings → Privacy & Security → Local Network, then reopen this page."
                } else {
                    "Looking for worlds opened to LAN…"
                }))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .children(state.shown.iter().map(|world| {
                    let id = format!("lan-{}", world.address());
                    let picked = world.clone();
                    div()
                        .id(SharedString::from(id.clone()))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .px_4()
                        .py_2p5()
                        .border_t_1()
                        .border_color(colors::FADE_700)
                        .child(
                            div()
                                .flex_none()
                                .size(px(8.))
                                .rounded_full()
                                .bg(colors::GREEN_200),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .truncate()
                                        .font_weight(weights::BOLD.into())
                                        .text_color(colors::GRAY_300)
                                        .child(world.title()),
                                )
                                .child(note(world.address()).font_family(fonts::MONO)),
                        )
                        .child(
                            Button::new(SharedString::from(format!("join-{id}")), "Join")
                                .size(ButtonSize::Slim)
                                .variant(ButtonVariant::Primary)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_lan_join(picked.clone(), cx)
                                })),
                        )
                        .into_any_element()
                }))
                .into_any_element()
        };
        div()
            .id("lan-worlds")
            .flex()
            .flex_col()
            .w_full()
            .rounded(radius::XL)
            .border_1()
            .border_color(colors::FADE_700)
            .bg(colors::GRAY_850)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .child(Icon::Cube.render(px(16.), colors::GRAY_400))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(
                                div()
                                    .font_family(fonts::HEADING)
                                    .text_size(theme::text_size(text::H3))
                                    .font_weight(weights::BOLD.into())
                                    .text_color(colors::GRAY_300)
                                    .child("On your network"),
                            )
                            .child(note("Worlds opened to LAN on this network.")),
                    ),
            )
            .child(body)
            .into_any_element()
    }

    /// The `index`th world shown, for the offscreen screenshots.
    pub fn preview_lan_world(&self, index: usize) -> Option<LanWorld> {
        self.extras.lan.shown.get(index).cloned()
    }

    /// Opens "Join with…", for the offscreen screenshots.
    pub fn open_lan_join_preview(&mut self, world: LanWorld, cx: &mut Context<Self>) {
        self.open_lan_join(world, cx);
    }

    /// Fills "On your network", for the offscreen screenshots.
    pub fn preview_lan(&mut self, worlds: &[(&str, &str, u16)], quiet: bool) {
        let now = Instant::now();
        let mut list = LanWorlds::default();
        for (motd, host, port) in worlds {
            list.record(
                host.parse().expect("a valid address"),
                lan::Announcement {
                    motd: Some(motd.to_string()),
                    port: *port,
                },
                now,
            );
        }
        let state = &mut self.extras.lan;
        state.shown = list.list();
        state.heard_any = !worlds.is_empty();
        state.preview_quiet = quiet;
    }
}

/// "Join with…": the instances to start, most recently played first.
pub fn render_picker(servers: &Entity<Servers>, cx: &mut App) -> AnyElement {
    let this = servers.read(cx);
    let Some(picker) = &this.extras.lan_picker else {
        return div().into_any_element();
    };
    let mut instances: Vec<(String, String, String, Option<u64>)> = this
        .instances
        .read(cx)
        .list()
        .iter()
        .map(|i| {
            (
                i.id.clone(),
                i.name.clone(),
                match i.loader.kind() {
                    Some(kind) => format!("{} · {}", i.game_version, kind.label()),
                    None => i.game_version.clone(),
                },
                i.last_played,
            )
        })
        .collect();
    instances.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| a.1.cmp(&b.1)));
    let close = servers.clone();
    let world = picker.world.clone();
    Dialog::new(
        "lan-join",
        format!("Join {}", world.title()),
        picker.focus.clone(),
    )
    .on_close(move |_, cx| {
        close.update(cx, |this, cx| {
            this.extras.lan_picker = None;
            cx.notify();
        })
    })
    .child(
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(note(format!(
                "Pick the instance to play with. It needs the same Minecraft version and mods as the world at {}.",
                world.address()
            )))
            .when(instances.is_empty(), |this| {
                this.child(note("You have no instances yet. Create one first."))
            })
            .children(instances.into_iter().map(|(id, name, detail, _)| {
                let servers = servers.clone();
                let pick = id.clone();
                div()
                    .id(SharedString::from(format!("lan-pick-{id}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded(radius::LG)
                    .cursor_pointer()
                    .bg(colors::GRAY_800)
                    .hover(|s| s.bg(colors::GRAY_700))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(weights::BOLD.into())
                            .text_color(colors::GRAY_300)
                            .child(name),
                    )
                    .child(note(detail))
                    .on_click(move |_, _, cx| {
                        servers.update(cx, |this, cx| this.join_lan(&pick, cx))
                    })
            })),
    )
    .footer({
        let cancel = servers.clone();
        Button::new("lan-join-cancel", "Cancel").on_click(move |_, _, cx| {
            cancel.update(cx, |this, cx| {
                this.extras.lan_picker = None;
                cx.notify();
            })
        })
    })
    .into_any_element()
}
