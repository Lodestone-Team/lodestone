//! The Installed view (`InstalledMods.tsx`, `ClientPackPanel.tsx`): the modpack panel
//! (see [`super::modpack`]), the client modpack of a Fabric server (a big one folded to
//! its first few mods), the toolbar (update all, check for updates, upload), and a row
//! per mod with its update, Modrinth link, switch and delete, 20 to a page.

use std::path::PathBuf;

use gpui::{
    AnyElement, AppContext as _, Context, ExternalPaths, IntoElement, ParentElement,
    PathPromptOptions, SharedString, Styled, div, img, prelude::*,
};
#[cfg(feature = "screenshot")]
use launcher_core::cores::admin::mods::InstalledModList;
use launcher_core::cores::admin::mods::{
    ClientRequirement, InstalledMod, InstalledModLoader, ModInstallRequest, ModInstallResult,
    modrinth_project_url,
};
use lodestone_ui::{
    components::{
        Button, ButtonSize, ButtonVariant, Checkbox, Icon, IconButton, LoadingDots, Toast, Toggle,
        settings::{card, tag},
        text_tooltip,
    },
    i18n::format,
    theme::{colors, ink, px, radius, text, weights},
};

use super::{ModsPage, Prompt, Target, muted};
use crate::{
    i18n::{InstanceMsg as M, InstancePlural as P, t, tr, trn},
    pages::pager,
    state::HostEvent,
};

/// The client pack's mods shown while it is folded: about three rows of chips at the
/// tab's width (a chip with a name and a version is 150 to 250 px wide).
const PACK_CHIPS_FOLDED: usize = 12;
/// The client pack folds only above this many mods, so that "Show all" always shows a
/// row or more that was hidden.
const PACK_FOLDS_ABOVE: usize = 16;

/// How many of the client pack's `count` mods it shows, and whether it has the
/// control that shows them all (`open`) or folds them again.
pub(super) fn pack_chips(count: usize, open: bool) -> (usize, bool) {
    if count <= PACK_FOLDS_ABOVE {
        (count, false)
    } else if open {
        (count, true)
    } else {
        (PACK_CHIPS_FOLDED, true)
    }
}

/// A project's icon from Modrinth's CDN, or a cube.
pub(super) fn mod_icon(url: Option<&str>, size: f32, cx: &mut Context<ModsPage>) -> AnyElement {
    let image = url.and_then(|url| crate::instance_icons::modrinth_image(url, cx));
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .overflow_hidden()
        .rounded(radius::MD)
        .bg(colors::GRAY_900)
        .map(|this| match image {
            Some(image) => this.child(img(image).size_full()),
            None => this.child(Icon::Cube.render(px(size * 0.4), ink::faint())),
        })
        .into_any_element()
}

/// Whether players need an installed mod (`ClientRequirementLabel`): optional mods
/// show nothing. With `marks` (Fabric, where the row has "For players"), the box says
/// "players need this" instead of a tag.
fn requirement_label(id: SharedString, m: &InstalledMod, marks: bool) -> Option<AnyElement> {
    match m.client_requirement {
        ClientRequirement::Required if marks => None,
        ClientRequirement::Required => {
            let label = tag(t(M::ModsPlayersNeed), colors::BLUE_200);
            Some(if m.required_by.is_empty() {
                label.into_any_element()
            } else {
                let tip = tr!(M::ModsNeededBy, mods = format::list(&m.required_by));
                div()
                    .id(id)
                    .child(label)
                    .tooltip(text_tooltip(tip))
                    .into_any_element()
            })
        }
        ClientRequirement::Unsupported => Some(
            div()
                .text_size(text::SMALL)
                .text_color(ink::faint())
                .child(t(M::ModsServerOnly))
                .into_any_element(),
        ),
        ClientRequirement::Unknown => Some(
            div()
                .id(id)
                .text_size(text::SMALL)
                .text_color(ink::faint())
                .child(t(M::ModsUnknown))
                .tooltip(text_tooltip(t(if m.modrinth.is_some() {
                    M::ModsUnknownModrinth
                } else {
                    M::ModsUnknownNotOnModrinth
                })))
                .into_any_element(),
        ),
        ClientRequirement::Optional => None,
    }
}

/// Wraps a control that is off for lack of the write permission, to say why.
pub(super) fn why_not(
    id: impl Into<SharedString>,
    can_write: bool,
    control: impl IntoElement,
) -> AnyElement {
    if can_write {
        return control.into_any_element();
    }
    div()
        .id(id.into())
        .child(control)
        .tooltip(text_tooltip(t(M::ModsNoWrite)))
        .into_any_element()
}

impl ModsPage {
    fn set_enabled(&mut self, m: &InstalledMod, enabled: bool, cx: &mut Context<Self>) {
        let file = m.file_name.clone();
        let name = m.name().to_string();
        if !self.busy_files.insert(file.clone()) {
            return;
        }
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = {
                let file = file.clone();
                cx.background_spawn(
                    async move { admin.set_mod_enabled(&uuid, &file, enabled).await },
                )
                .await
            };
            this.update(cx, |this, cx| {
                this.busy_files.remove(&file);
                if let Err(err) = result {
                    let message = if enabled {
                        tr!(M::ModsEnableFailed, name = name, error = err)
                    } else {
                        tr!(M::ModsDisableFailed, name = name, error = err)
                    };
                    this.toast(Toast::error(message), cx);
                }
                this.reload(cx);
            })
            .ok();
        });
        self.push(task);
    }

    /// Marks a mod as needed by players, or takes the mark off: the client pack gives
    /// it to them the next time they join.
    fn set_client_mark(&mut self, m: &InstalledMod, required: bool, cx: &mut Context<Self>) {
        let file = m.file_name.clone();
        let name = m.name().to_string();
        if !self.busy_files.insert(file.clone()) {
            return;
        }
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = {
                let file = file.clone();
                cx.background_spawn(
                    async move { admin.set_client_mark(&uuid, &file, required).await },
                )
                .await
            };
            this.update(cx, |this, cx| {
                this.busy_files.remove(&file);
                let toast = match result {
                    Ok(()) if required => Toast::success(tr!(M::ModsMarkedForPlayers, name = name)),
                    Ok(()) => Toast::success(tr!(M::ModsUnmarkedForPlayers, name = name)),
                    Err(err) => Toast::error(tr!(M::ModsMarkFailed, name = name, error = err)),
                };
                this.toast(toast.key("mods-client-mark"), cx);
                this.reload(cx);
            })
            .ok();
        });
        self.push(task);
    }

    fn update_one(&mut self, m: &InstalledMod, cx: &mut Context<Self>) {
        let Some(update) = m.modrinth.as_ref().and_then(|m| m.update.clone()) else {
            return;
        };
        let file = m.file_name.clone();
        let name = m.name().to_string();
        if !self.busy_files.insert(file.clone()) {
            return;
        }
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        // this tab says how the update went; the core's event needn't
        let claim = self
            .state
            .update(cx, |state, _| state.expect_own_file_toast(&uuid));
        let task = cx.spawn(async move |this, cx| {
            let request = ModInstallRequest {
                version_id: update.version_id.clone(),
                install_dependencies: true,
                force: false,
            };
            let result = cx
                .background_spawn(async move { admin.install_mod(&uuid, &request).await })
                .await;
            this.update(cx, |this, cx| {
                this.busy_files.remove(&file);
                let toast = match result {
                    Ok(result) => Self::result_toast(
                        tr!(M::ModsUpdated, name = name, version = update.version_number),
                        &result,
                    ),
                    Err(err) => {
                        // the core may not have sent an event to take the claim
                        this.state
                            .update(cx, |state, _| state.release_own_file_toast(claim));
                        Toast::error(tr!(M::ModsUpdateFailed, name = name, error = err))
                    }
                };
                this.toast(toast, cx);
                this.reload(cx);
            })
            .ok();
        });
        self.push(task);
    }

    /// Updates every mod with an update, one after another (`updateAll`).
    ///
    /// The batch outlives the page: switching tabs rebuilds the page, and must not
    /// stop the updates after the one in flight. So the task is detached, keeps its
    /// "updating" mark in the Host's state (where the next Mods tab sees it), says how
    /// it went through the Host's toasts, and has the Mods tab list the mods again.
    fn update_all(&mut self, cx: &mut Context<Self>) {
        let Some(list) = &self.list else {
            return;
        };
        let updates: Vec<(String, String)> = list
            .mods
            .iter()
            .filter_map(|m| {
                let update = m.modrinth.as_ref()?.update.as_ref()?;
                Some((m.name().to_string(), update.version_id.clone()))
            })
            .collect();
        if updates.is_empty() {
            return;
        }
        let uuid = self.uuid.clone();
        let started = self.state.update(cx, |state, cx| {
            let started = state.set_updating_mods(&uuid, true);
            cx.notify();
            started
        });
        if !started {
            return;
        }
        let admin = self.state.read(cx).admin.clone();
        let state = self.state.downgrade();
        cx.spawn(async move |_, cx| {
            let mut total = ModInstallResult::default();
            let mut failures = Vec::new();
            for (name, version_id) in updates {
                // one toast for them all, not one per mod from the core's events: each
                // event is claimed just before its update starts, and the claim given
                // up when the update fails, so none is left to hide another's toast
                let claim = state
                    .update(cx, |state, _| state.expect_own_file_toast(&uuid))
                    .ok();
                let request = ModInstallRequest {
                    version_id,
                    install_dependencies: true,
                    force: false,
                };
                let result = cx
                    .background_spawn({
                        let (admin, uuid) = (admin.clone(), uuid.clone());
                        async move { admin.install_mod(&uuid, &request).await }
                    })
                    .await;
                match result {
                    Ok(result) => total.extend(result),
                    Err(err) => {
                        if let Some(claim) = claim {
                            state
                                .update(cx, |state, _| state.release_own_file_toast(claim))
                                .ok();
                        }
                        failures.push(format!("{name}: {err}"));
                    }
                }
            }
            state
                .update(cx, |state, cx| {
                    state.set_updating_mods(&uuid, false);
                    if !total.installed.is_empty() || !total.removed.is_empty() {
                        state.toast(
                            Self::result_toast(t(M::ModsUpdatesInstalled).to_string(), &total),
                            cx,
                        );
                    }
                    if !failures.is_empty() {
                        state.toast(
                            Toast::error(tr!(
                                M::ModsSomeUpdatesFailed,
                                failures = failures.join("; ")
                            )),
                            cx,
                        );
                    }
                    cx.emit(HostEvent::ModsChanged(uuid.clone()));
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    /// Shows another page of the installed list, from its top.
    fn set_installed_page(&mut self, page: usize, cx: &mut Context<Self>) {
        self.installed_page = page;
        self.state
            .read(cx)
            .page_scroll
            .set_offset(gpui::Point::default());
        cx.notify();
    }

    fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        self.check_updates = true;
        self.reload(cx);
    }

    /// Upload: a file dialog for `.jar` files, then the upload.
    fn choose_and_upload(&mut self, cx: &mut Context<Self>) {
        let dialog = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(t(M::ModsUploadPrompt).into()),
        });
        let task = cx.spawn(async move |this, cx| {
            let paths = match dialog.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Err(_)) => {
                    this.update(cx, |this, cx| {
                        this.toast(
                            Toast::error(t(crate::i18n::CommonMsg::FileDialogUnavailable)),
                            cx,
                        )
                    })
                    .ok();
                    return;
                }
                _ => return,
            };
            this.update(cx, |this, cx| this.upload_paths(paths, cx))
                .ok();
        });
        self.push(task);
    }

    /// Uploads `.jar` files picked in the dialog or dropped on the tab; anything else
    /// is refused, by name, before a byte is sent.
    fn upload_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if paths.is_empty() || self.uploading {
            return;
        }
        let task = cx.spawn(async move |this, cx| {
            let not_jars: Vec<String> = paths
                .iter()
                .filter(|p| !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jar")))
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .collect();
            if !not_jars.is_empty() {
                this.update(cx, |this, cx| {
                    this.toast(
                        Toast::error(tr!(M::ModsOnlyJars, names = not_jars.join(", "))),
                        cx,
                    )
                })
                .ok();
                return;
            }
            let read = cx
                .background_spawn(async move {
                    paths
                        .iter()
                        .map(|path| {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            std::fs::read(path).map(|bytes| (name, bytes))
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .await;
            this.update(cx, |this, cx| match read {
                Ok(files) => this.upload(files, false, cx),
                Err(err) => this.toast(Toast::error(tr!(M::ModsUploadFailed, error = err)), cx),
            })
            .ok();
        });
        self.push(task);
    }

    pub(super) fn upload(
        &mut self,
        files: Vec<(String, Vec<u8>)>,
        overwrite: bool,
        cx: &mut Context<Self>,
    ) {
        self.uploading = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let (files, result) = cx
                .background_spawn(async move {
                    let result = admin.upload_mods(&uuid, &files, overwrite).await;
                    (files, result)
                })
                .await;
            this.update(cx, |this, cx| {
                this.uploading = false;
                match result {
                    Ok(uploaded) => {
                        let names: Vec<&str> = uploaded.iter().map(|m| m.name()).collect();
                        this.toast(
                            Toast::success(tr!(M::ModsUploaded, names = names.join(", "))),
                            cx,
                        );
                    }
                    // a file of the same name: ask before replacing it
                    Err(launcher_core::cores::CoreError::Server {
                        status: 409,
                        message,
                    }) if !overwrite => this.open_prompt(Prompt::Overwrite { files, message }, cx),
                    Err(err) => this.toast(Toast::error(tr!(M::ModsUploadFailed, error = err)), cx),
                }
                this.reload(cx);
            })
            .ok();
        });
        self.push(task);
    }

    pub(super) fn delete_mod(&mut self, file_name: String, cx: &mut Context<Self>) {
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = {
                let file = file_name.clone();
                cx.background_spawn(async move { admin.delete_mod(&uuid, &file).await })
                    .await
            };
            this.update(cx, |this, cx| {
                let toast = match result {
                    Ok(()) => Toast::success(tr!(M::ModsDeleted, name = file_name)),
                    Err(err) => Toast::error(tr!(M::ModsDeleteFailed, error = err)),
                };
                this.toast(toast, cx);
                this.reload(cx);
            })
            .ok();
        });
        self.push(task);
    }

    /// Builds the client modpack on the core, downloads it and saves it where the user
    /// says (the dashboard opens the download in the browser).
    fn download_pack(&mut self, cx: &mut Context<Self>) {
        if self.downloading_pack {
            return;
        }
        self.downloading_pack = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let fetched = cx
                .background_spawn(async move {
                    let pack = admin.client_pack(&uuid).await?;
                    let bytes = admin.download_file(&pack.key, 512 * 1024 * 1024).await?;
                    Ok::<_, launcher_core::cores::CoreError>((pack.file_name, bytes))
                })
                .await;
            let (name, bytes) = match fetched {
                Ok(fetched) => fetched,
                Err(err) => {
                    this.update(cx, |this, cx| {
                        this.downloading_pack = false;
                        this.toast(
                            Toast::error(tr!(M::ModsDownloadPackFailed, error = err)),
                            cx,
                        );
                    })
                    .ok();
                    return;
                }
            };
            let name = if name.is_empty() {
                "client.mrpack".to_string()
            } else {
                name
            };
            let downloads = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
            let Ok(dialog) =
                this.update(cx, |_, cx| cx.prompt_for_new_path(&downloads, Some(&name)))
            else {
                return;
            };
            let path = match dialog.await {
                Ok(Ok(Some(path))) => path,
                _ => {
                    this.update(cx, |this, cx| {
                        this.downloading_pack = false;
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            let written = {
                let path = path.clone();
                cx.background_spawn(async move { std::fs::write(&path, bytes) })
                    .await
            };
            this.update(cx, |this, cx| {
                this.downloading_pack = false;
                let file = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let toast = match written {
                    Ok(()) => Toast::success(tr!(M::ModsPackSaved, name = file)),
                    Err(err) => Toast::error(tr!(M::ModsDownloadPackFailed, error = err)),
                };
                this.toast(toast, cx);
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// "Players need N mods to join" on a Fabric server (`ClientPackPanel.tsx`).
    fn client_pack_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let list = self.list.as_ref()?;
        if list.loader != InstalledModLoader::Fabric || list.mods.is_empty() {
            return None;
        }
        let loader = match &list.loader_version {
            Some(version) => tr!(M::ModsLoader, version = version),
            None => t(M::ModsLoaderNoVersion).to_string(),
        };
        if list.client_required_count == 0 {
            return Some(muted(tr!(
                M::ModsPlainFabric,
                gameVersion = list.game_version,
                loader = loader
            )));
        }
        // disabled mods never go into the pack
        let needed: Vec<InstalledMod> = list
            .mods
            .iter()
            .filter(|m| m.enabled && m.client_requirement == ClientRequirement::Required)
            .cloned()
            .collect();
        let included_jars = needed.iter().filter(|m| m.modrinth.is_none()).count();
        let count = list.client_required_count;
        // a big pack shows its first few mods, until "Show all"
        let (shown, folds) = pack_chips(needed.len(), self.pack_open);
        let fold = folds.then(|| {
            let open = self.pack_open;
            div()
                .id("client-pack-fold")
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .cursor_pointer()
                .text_size(text::SMALL)
                .text_color(colors::BLUE_200)
                .hover(|style| style.underline())
                .child(
                    if open {
                        Icon::ChevronDown
                    } else {
                        Icon::ChevronRight
                    }
                    .render(px(8.), colors::BLUE_200),
                )
                .child(if open {
                    t(M::ModsPackShowFewer).to_string()
                } else {
                    trn!(P::ModsPackShowAll, needed.len())
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.pack_open = !this.pack_open;
                    cx.notify();
                }))
        });
        let chips: Vec<AnyElement> = needed
            .iter()
            .take(shown)
            .enumerate()
            .map(|(index, m)| {
                let chip = div()
                    .id(SharedString::from(format!("pack-chip-{index}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .py(px(4.))
                    .pl(px(4.))
                    .pr(px(8.))
                    .rounded(radius::MD)
                    .bg(colors::GRAY_900)
                    .text_size(text::SMALL)
                    .child(mod_icon(
                        m.modrinth.as_ref().and_then(|m| m.icon_url.as_deref()),
                        20.,
                        cx,
                    ))
                    .child(div().text_color(ink::text()).child(m.name().to_string()))
                    .when_some(m.version().map(str::to_string), |this, v| {
                        this.child(div().text_color(ink::muted()).child(v))
                    });
                if m.required_by.is_empty() {
                    chip.into_any_element()
                } else {
                    chip.tooltip(text_tooltip(tr!(
                        M::ModsNeededBy,
                        mods = format::list(&m.required_by)
                    )))
                    .into_any_element()
                }
            })
            .collect();
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .p(px(16.))
                .rounded(radius::XL)
                .border_1()
                .border_color(colors::BLUE_FADED.opacity(50))
                .bg(colors::GRAY_850)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_start()
                        .justify_between()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .text_size(text::H3)
                                        .font_weight(weights::BOLD.face())
                                        .text_color(ink::text())
                                        .child(trn!(P::ModsPackTitle, count)),
                                )
                                .child(muted(format!(
                                    "Minecraft {} · {loader}",
                                    list.game_version
                                ))),
                        )
                        .child(
                            Button::new("download-client-pack", t(M::ModsDownloadPack))
                                .variant(ButtonVariant::Primary)
                                .icon(Icon::Download)
                                .loading(self.downloading_pack)
                                .disabled(self.downloading_pack)
                                .on_click(cx.listener(|this, _, _, cx| this.download_pack(cx))),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap(px(8.))
                        .children(chips),
                )
                .children(fold)
                .child(div().text_size(text::SMALL).text_color(ink::muted()).child(
                    if included_jars > 0 {
                        trn!(P::ModsImportHintJars, included_jars)
                    } else {
                        t(M::ModsImportHint).to_string()
                    },
                ))
                .into_any_element(),
        )
    }

    fn mod_row(
        &self,
        index: usize,
        m: &InstalledMod,
        target: &Target,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let can_write = self.can_write(cx);
        let busy = self.busy_files.contains(&m.file_name);
        let update = m.modrinth.as_ref().and_then(|m| m.update.clone());
        let name = m.name().to_string();
        let id = |part: &str| SharedString::from(format!("mod-{index}-{part}"));
        let (toggled, updated, deleted, marked) = (m.clone(), m.clone(), m.clone(), m.clone());
        // client packs are Fabric's: there, the owner can mark a mod for players
        let marks = target.loader == InstalledModLoader::Fabric && !target.is_plugin();
        let required_anyway =
            m.client_requirement == ClientRequirement::Required && !m.client_marked;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .px(px(16.))
            .py(px(12.))
            .child(mod_icon(
                m.modrinth.as_ref().and_then(|m| m.icon_url.as_deref()),
                40.,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(weights::BOLD.face())
                                    .text_color(if m.enabled { ink::text() } else { ink::muted() })
                                    .child(name.clone()),
                            )
                            .when_some(m.version().map(str::to_string), |this, v| {
                                this.child(
                                    div()
                                        .flex_none()
                                        .text_size(text::SMALL)
                                        .text_color(ink::muted())
                                        .child(v),
                                )
                            })
                            // players never install plugins
                            .when(!target.is_plugin(), |this| {
                                this.children(requirement_label(id("requirement"), m, marks))
                            })
                            // by the name, so the controls on the right line up on every row
                            .when_some(m.modrinth.clone(), |this, modrinth| {
                                let url = modrinth_project_url(
                                    if target.is_plugin() { "plugin" } else { "mod" },
                                    &modrinth.slug,
                                );
                                this.child(
                                    // the button is taller than the line: don't push it apart
                                    div().flex_none().my(px(-6.)).child(
                                        IconButton::new(
                                            id("modrinth"),
                                            Icon::ArrowUpRightFromSquare,
                                        )
                                        .label(t(M::ModsViewOnModrinth))
                                        .on_click(move |_, _, cx| cx.open_url(&url)),
                                    ),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(text::SMALL)
                                    .text_color(ink::faint())
                                    .child(format!("{} · {}", m.file_name, format::bytes(m.size))),
                            )
                            // the modpack version this file came with
                            .when_some(self.blame_label(&m.file_name), |this, (label, tip)| {
                                this.child(
                                    div()
                                        .id(id("blame"))
                                        .flex_none()
                                        .text_size(text::SMALL)
                                        .text_color(ink::muted())
                                        .child(label)
                                        .tooltip(text_tooltip(tip)),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .items_center()
                    .gap(px(8.))
                    .when_some(update, |this, update| {
                        this.child(why_not(
                            id("update-why"),
                            can_write,
                            Button::new(
                                id("update"),
                                tr!(M::ModsUpdateTo, version = update.version_number),
                            )
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Slim)
                            .icon(Icon::CircleUp)
                            .loading(busy)
                            .disabled(!can_write || busy)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.update_one(&updated, cx)),
                            ),
                        ))
                    })
                    .when(marks, |this| {
                        let checkbox = Checkbox::new(
                            id("players"),
                            m.client_marked || required_anyway,
                            t(M::ModsForPlayers),
                        )
                        .disabled(!can_write || busy || required_anyway)
                        .on_change(cx.listener(
                            move |this, on: &bool, _, cx| this.set_client_mark(&marked, *on, cx),
                        ));
                        let tip = if required_anyway && !m.required_by.is_empty() {
                            tr!(M::ModsNeededBy, mods = format::list(&m.required_by))
                        } else if required_anyway {
                            t(M::ModsForPlayersAlready).to_string()
                        } else {
                            t(M::ModsForPlayersHint).to_string()
                        };
                        this.child(if can_write {
                            div()
                                .id(id("players-tip"))
                                .child(checkbox)
                                .tooltip(text_tooltip(tip))
                                .into_any_element()
                        } else {
                            why_not(id("players-why"), can_write, checkbox)
                        })
                    })
                    .child(why_not(
                        id("toggle-why"),
                        can_write,
                        Toggle::new(id("toggle"), m.enabled)
                            .label(name.clone())
                            .disabled(!can_write || busy)
                            .on_change(cx.listener(move |this, on: &bool, _, cx| {
                                this.set_enabled(&toggled, *on, cx)
                            })),
                    ))
                    .child(why_not(
                        id("delete-why"),
                        can_write,
                        IconButton::new(id("delete"), Icon::TrashCan)
                            .label(tr!(M::ModsDeleteLabel, name = name))
                            .disabled(!can_write)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_prompt(
                                    Prompt::Delete {
                                        file_name: deleted.file_name.clone(),
                                        name: deleted.name().to_string(),
                                    },
                                    cx,
                                )
                            })),
                    )),
            )
            .into_any_element()
    }

    pub(super) fn render_installed(&self, target: &Target, cx: &mut Context<Self>) -> AnyElement {
        let can_write = self.can_write(cx);
        let updating_all = self.state.read(cx).updating_mods(&self.uuid);
        let plugin = target.is_plugin();
        let first_load = self.list.is_none() && self.list_error.is_none();
        let updates = self
            .list
            .as_ref()
            .map(|l| {
                l.mods
                    .iter()
                    .filter(|m| m.modrinth.as_ref().is_some_and(|m| m.update.is_some()))
                    .count()
            })
            .unwrap_or(0);
        let count = self.list.as_ref().map(|list| {
            if plugin {
                trn!(
                    P::ModsCountPlugin,
                    list.mods.len(),
                    directory = list.directory
                )
            } else {
                trn!(P::ModsCountMod, list.mods.len(), directory = list.directory)
            }
        });
        let toolbar = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .child(muted(count.unwrap_or_default()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .when(updates > 0, |this| {
                        this.child(why_not(
                            "update-all-why",
                            can_write,
                            Button::new("update-all", tr!(M::ModsUpdateAll, count = updates))
                                .variant(ButtonVariant::Primary)
                                .icon(Icon::CircleUp)
                                .loading(updating_all)
                                .disabled(!can_write || updating_all)
                                .on_click(cx.listener(|this, _, _, cx| this.update_all(cx))),
                        ))
                    })
                    .child(
                        Button::new("check-updates", t(M::ModsCheckUpdates))
                            .loading(self.check_updates && self.loading)
                            .disabled(self.list.is_none() || (self.check_updates && self.loading))
                            .on_click(cx.listener(|this, _, _, cx| this.check_for_updates(cx))),
                    )
                    .child(why_not(
                        "upload-why",
                        can_write,
                        Button::new("upload-mods", t(M::ModsUpload))
                            .icon(Icon::Upload)
                            .loading(self.uploading)
                            .disabled(!can_write || self.list.is_none() || self.uploading)
                            .on_click(cx.listener(|this, _, _, cx| this.choose_and_upload(cx))),
                    )),
            );
        let list: AnyElement = if let Some(error) = &self.list_error {
            div()
                .p(px(16.))
                .rounded(radius::LG)
                .border_1()
                .border_color(ink::border())
                .bg(colors::GRAY_850)
                .text_color(colors::RED_200)
                .child(if plugin {
                    tr!(M::ModsLoadFailedPlugin, error = error)
                } else {
                    tr!(M::ModsLoadFailedMod, error = error)
                })
                .into_any_element()
        } else if let Some(list) = &self.list {
            if list.mods.is_empty() {
                div()
                    .p(px(24.))
                    .rounded(radius::LG)
                    .border_1()
                    .border_color(ink::border())
                    .bg(colors::GRAY_850)
                    .flex()
                    .justify_center()
                    .text_color(ink::muted())
                    .child(t(if plugin {
                        M::ModsEmptyPlugin
                    } else {
                        M::ModsEmptyMod
                    }))
                    .into_any_element()
            } else {
                // a page of the mods, and the pager under it when there are more
                let len = list.mods.len();
                let page = pager::clamp(self.installed_page, len);
                let range = pager::range(page, len);
                let first = range.start;
                let mods = list.mods[range].to_vec();
                let rows: Vec<AnyElement> = mods
                    .iter()
                    .enumerate()
                    .map(|(index, m)| self.mod_row(first + index, m, target, cx))
                    .collect();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(card(rows))
                    .when(pager::paged(len), |this| {
                        this.child(pager::pager(
                            page,
                            pager::page_count(len),
                            |this: &mut Self, page, cx| this.set_installed_page(page, cx),
                            cx,
                        ))
                    })
                    .into_any_element()
            }
        } else {
            card([div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(96.))
                .child(LoadingDots)
                .into_any_element()])
            .into_any_element()
        };
        let can_drop = can_write && self.list.is_some() && !self.uploading;
        div()
            .id("mods-installed")
            .min_h(px(360.))
            // while files are dragged over the view, its edge and a tint say they can be
            // dropped (as the launcher's Content tab does)
            .rounded(radius::LG)
            .border_2()
            .border_color(colors::TRANSPARENT)
            .flex()
            .flex_col()
            .gap(px(16.))
            .children(self.modpack_panel(cx))
            .when(!first_load, |this| {
                this.children(self.client_pack_panel(cx))
            })
            .child(toolbar)
            .child(list)
            .when(can_drop, |this| {
                this.drag_over::<ExternalPaths>(|style, _, _, _| {
                    style
                        .border_color(lodestone_ui::theme::accent::light().opacity(60))
                        .bg(lodestone_ui::theme::accent::base().opacity(8))
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                    this.upload_paths(paths.paths().to_vec(), cx)
                }))
            })
            .into_any_element()
    }
}

impl ModsPage {
    /// Sample mods on the Installed view, for the development screenshots' `--mods
    /// <scene>`: `many` (158 mods on 8 pages, 40 of them in the folded client pack),
    /// `last` (their last page), `pack` (the client pack unfolded) or `few` (a handful,
    /// with no pager and no fold); with `-end` (`many-end`), scrolled to the bottom.
    #[cfg(feature = "screenshot")]
    pub fn demo_installed(&mut self, scene: &str, cx: &mut Context<Self>) {
        let (scene, end) = match scene.strip_suffix("-end") {
            Some(scene) => (scene, true),
            None => (scene, false),
        };
        const NAMES: [&str; 20] = [
            "Sodium",
            "Lithium",
            "Fabric API",
            "Mod Menu",
            "Iris Shaders",
            "Entity Culling",
            "FerriteCore",
            "Krypton",
            "Indium",
            "Cloth Config API",
            "Xaero's Minimap",
            "Jade",
            "AppleSkin",
            "Continuity",
            "Lambda Dynamic Lights",
            "Better Mount HUD",
            "ImmediatelyFast",
            "Zoomify",
            "Clumps",
            "Ksyxis",
        ];
        let (count, needed) = if scene == "few" { (6, 3) } else { (158, 40) };
        let mods: Vec<serde_json::Value> = (0..count)
            .map(|index| {
                let base = NAMES[index % NAMES.len()];
                let name = match index / NAMES.len() {
                    0 => base.to_string(),
                    n => format!("{base} Addon {n}"),
                };
                let slug = name.to_lowercase().replace([' ', '\''], "-");
                let version = format!("{}.{}.{}", 1 + index % 4, index % 10, index % 7);
                // every fourth not on Modrinth, and a few with an update
                let modrinth = (index % 4 != 3).then(|| {
                    serde_json::json!({
                        "project_id": format!("p{index}"), "version_id": format!("v{index}"),
                        "slug": slug, "title": name, "icon_url": null,
                        "version_number": version,
                        "update": (index % 9 == 1).then(|| serde_json::json!({
                            "version_id": format!("u{index}"),
                            "version_number": format!("{}.{}.{}", 2 + index % 4, 0, 1),
                        })),
                        "client_side": "required", "server_side": "required",
                    })
                });
                serde_json::json!({
                    "file_name": format!("{slug}-{version}.jar"),
                    "enabled": index % 13 != 5,
                    "size": 200_000 + index * 37_000,
                    "metadata": { "name": name, "version": version, "authors": [] },
                    "modrinth": modrinth,
                    "client_requirement": if index % 13 == 5 || index * needed / count
                        != (index + 1) * needed / count { "required" } else { "optional" },
                    "required_by": [],
                })
            })
            .collect();
        let mut list: InstalledModList = serde_json::from_value(serde_json::json!({
            "kind": "mod", "loader": "fabric", "game_version": "1.21.4",
            "directory": "mods", "loader_version": "0.16.10", "mods": mods,
        }))
        .expect("the sample mods read");
        list.client_required_count = list
            .mods
            .iter()
            .filter(|m| m.enabled && m.client_requirement == ClientRequirement::Required)
            .count() as u32;
        // fetches still under way are for a server the core doesn't have
        self.list_serial += 1;
        self.history_serial += 1;
        self.modpack_check = super::modpack::ModpackCheck::Newest;
        self.modpack_history = None;
        self.list = Some(list);
        self.list_error = None;
        self.loading = false;
        self.pack_open = scene == "pack";
        let last = pager::page_count(count) - 1;
        self.set_installed_page(if scene == "last" { last } else { 0 }, cx);
        if end {
            // the page clamps it to the bottom when it's laid out
            self.state
                .read(cx)
                .page_scroll
                .set_offset(gpui::point(px(0.), px(-100_000.)));
        }
    }
}
