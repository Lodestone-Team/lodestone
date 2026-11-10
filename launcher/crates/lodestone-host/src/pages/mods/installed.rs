//! The Installed view (`InstalledMods.tsx`, `ClientPackPanel.tsx`): the client modpack
//! of a Fabric server, the toolbar (update all, check for updates, upload), and a row
//! per mod with its update, Modrinth link, switch and delete.

use std::path::PathBuf;

use gpui::{
    AnyElement, AppContext as _, Context, ExternalPaths, IntoElement, ParentElement,
    PathPromptOptions, SharedString, Styled, div, img, prelude::*,
};
use launcher_core::cores::admin::mods::{
    ClientRequirement, InstalledMod, InstalledModLoader, ModInstallRequest, ModInstallResult,
    modrinth_project_url,
};
use lodestone_ui::{
    components::{
        Button, ButtonSize, ButtonVariant, Icon, IconButton, LoadingDots, Toast, Toggle,
        settings::{card, tag},
        text_tooltip,
    },
    i18n::format,
    theme::{colors, ink, px, radius, text, weights},
};

use super::{ModsPage, Prompt, Target, muted};
use crate::i18n::{InstanceMsg as M, InstancePlural as P, t, tr, trn};

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
/// show nothing.
fn requirement_label(id: SharedString, m: &InstalledMod) -> Option<AnyElement> {
    match m.client_requirement {
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
                    Err(err) => Toast::error(tr!(M::ModsUpdateFailed, name = name, error = err)),
                };
                this.toast(toast, cx);
                this.reload(cx);
            })
            .ok();
        });
        self.push(task);
    }

    /// Updates every mod with an update, one after another (`updateAll`).
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
        if updates.is_empty() || self.updating_all {
            return;
        }
        self.updating_all = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let (total, failures) = cx
                .background_spawn(async move {
                    let mut total = ModInstallResult::default();
                    let mut failures = Vec::new();
                    for (name, version_id) in updates {
                        let request = ModInstallRequest {
                            version_id,
                            install_dependencies: true,
                            force: false,
                        };
                        match admin.install_mod(&uuid, &request).await {
                            Ok(result) => total.extend(result),
                            Err(err) => failures.push(format!("{name}: {err}")),
                        }
                    }
                    (total, failures)
                })
                .await;
            this.update(cx, |this, cx| {
                this.updating_all = false;
                if !total.installed.is_empty() || !total.removed.is_empty() {
                    this.toast(
                        Self::result_toast(t(M::ModsUpdatesInstalled).to_string(), &total),
                        cx,
                    );
                }
                if !failures.is_empty() {
                    this.toast(
                        Toast::error(tr!(
                            M::ModsSomeUpdatesFailed,
                            failures = failures.join("; ")
                        )),
                        cx,
                    );
                }
                this.reload(cx);
            })
            .ok();
        });
        self.push(task);
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
        let chips: Vec<AnyElement> = needed
            .iter()
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
        let (toggled, updated, deleted) = (m.clone(), m.clone(), m.clone());
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
                                this.children(requirement_label(id("requirement"), m))
                            }),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(text::SMALL)
                            .text_color(ink::faint())
                            .child(format!("{} · {}", m.file_name, format::bytes(m.size))),
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
                    .when_some(m.modrinth.clone(), |this, modrinth| {
                        let url = modrinth_project_url(
                            if target.is_plugin() { "plugin" } else { "mod" },
                            &modrinth.slug,
                        );
                        this.child(
                            IconButton::new(id("modrinth"), Icon::ArrowUpRightFromSquare)
                                .label(t(M::ModsViewOnModrinth))
                                .on_click(move |_, _, cx| cx.open_url(&url)),
                        )
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
                                .loading(self.updating_all)
                                .disabled(!can_write || self.updating_all)
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
                let mods = list.mods.clone();
                card(
                    mods.iter()
                        .enumerate()
                        .map(|(index, m)| self.mod_row(index, m, target, cx)),
                )
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
