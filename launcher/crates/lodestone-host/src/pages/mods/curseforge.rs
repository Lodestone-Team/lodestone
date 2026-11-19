//! CurseForge on the Mods tab. The core has no CurseForge client, so the Host asks
//! CurseForge itself (`launcher_core::cores::admin::curseforge`): which of the
//! installed jars are its files (by the fingerprints the core reports), which have a
//! newer file, and, to install one, it downloads the file here and uploads it to the
//! core as a jar. A build without Lodestone's key shows none of this.
//!
//! A file its author keeps on CurseForge's website goes to the Host's watch on the
//! Downloads folder (`crate::by_hand`); the card that lists such files is drawn here.

use std::collections::HashSet;

use gpui::{
    AnyElement, AppContext as _, Context, IntoElement, ParentElement, PathPromptOptions,
    SharedString, Styled, div, prelude::*,
};
use launcher_core::{
    content::{
        CurseForgeError,
        curseforge::{CfFile, CfMod},
    },
    cores::admin::{
        curseforge::{self as server, CfInstalled},
        mods::InstalledMod,
    },
};
use lodestone_ui::{
    components::{
        Button, ButtonSize, ButtonVariant, Icon, Kind, ObjectChip, PathText, StatusBadge, Toast,
        readable::object_name, settings::card,
    },
    theme::{Tone, colors, ink, px, radius, text, weights},
};

use super::ModsPage;
use crate::i18n::{InstanceMsg as M, t, tr};

impl ModsPage {
    /// The CurseForge file an installed jar is, where Modrinth doesn't know the jar:
    /// the core's own word (and its updates) is shown for the ones it does.
    pub(super) fn on_curseforge(&self, m: &InstalledMod) -> Option<&CfInstalled> {
        if m.modrinth.is_some() {
            return None;
        }
        self.curseforge.get(&m.file_name)
    }

    /// The name a row shows: Modrinth's title, else CurseForge's, else the jar's own.
    pub(super) fn mod_name(&self, m: &InstalledMod) -> String {
        match self.on_curseforge(m) {
            Some(curseforge) => curseforge.title.clone(),
            None => m.name().to_string(),
        }
    }

    /// Asks CurseForge which of the listed jars are its files and, with `check`, which
    /// have a newer one for the server. `serial` is the list's fetch this belongs to.
    pub(super) fn load_curseforge(&mut self, check: bool, serial: u64, cx: &mut Context<Self>) {
        let (Some(curseforge), Some(list)) = (crate::curseforge::client(cx), &self.list) else {
            self.curseforge.clear();
            self.curseforge_loading = false;
            return;
        };
        let mods = list.mods.clone();
        let target = server::ServerTarget {
            kind: list.kind,
            loader: list.loader,
            game_version: list.game_version.clone(),
        };
        // what is known stays for the jars still there, until CurseForge answers
        let names: HashSet<&str> = list.mods.iter().map(|m| m.file_name.as_str()).collect();
        self.curseforge
            .retain(|name, _| names.contains(name.as_str()));
        self.curseforge_loading = true;
        let task = cx.spawn(async move |this, cx| {
            let known = cx
                .background_spawn(launcher_core::http::in_http_runtime(async move {
                    let mut known = server::identify(&curseforge, &mods).await?;
                    if check {
                        server::find_updates(&curseforge, &target, &mods, &mut known).await?;
                    }
                    Ok::<_, CurseForgeError>(known)
                }))
                .await;
            this.update(cx, |this, cx| {
                if serial != this.list_serial {
                    return;
                }
                this.curseforge_loading = false;
                match known {
                    Ok(known) => this.curseforge = known,
                    Err(err) => {
                        log::warn!("Could not look up mods on CurseForge: {err}");
                        // a plain listing just shows the jars as they are
                        if check {
                            this.toast(Toast::error(tr!(M::ModsCurseForgeFailed, error = err)), cx);
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// Installs a CurseForge file on the server: its required dependencies with it when
    /// asked, older files of the same projects replaced. The toast is about `object`
    /// (a name and a version) and says it was installed, or updated with `update`.
    /// `busy` is the installed file whose row waits for it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn install_curseforge(
        &mut self,
        file: CfFile,
        project: Option<CfMod>,
        with_dependencies: bool,
        force: bool,
        object: (String, String),
        update: bool,
        busy: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let (Some(curseforge), Some(downloader), Some(list)) = (
            crate::curseforge::client(cx),
            crate::curseforge::downloader(cx),
            self.list.as_ref(),
        ) else {
            if let Some(view) = &mut self.project {
                view.installing = None;
            }
            if let Some(file) = &busy {
                self.busy_files.remove(file);
            }
            return self.toast(Toast::error(t(M::ModsCurseForgeUnavailable)), cx);
        };
        let target = server::ServerTarget {
            kind: list.kind,
            loader: list.loader,
            game_version: list.game_version.clone(),
        };
        let (mods, known) = (list.mods.clone(), self.curseforge.clone());
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        // a file the website keeps to itself may be on Modrinth all the same
        let modrinth = crate::curseforge::modrinth(cx);
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(launcher_core::http::in_http_runtime(async move {
                    server::install_file(
                        &curseforge,
                        modrinth.as_deref(),
                        &downloader,
                        &admin,
                        &uuid,
                        &target,
                        &mods,
                        &known,
                        file,
                        project,
                        with_dependencies,
                        force,
                    )
                    .await
                }))
                .await;
            this.update(cx, |this, cx| {
                if let Some(view) = &mut this.project {
                    view.installing = None;
                }
                if let Some(file) = &busy {
                    this.busy_files.remove(file);
                }
                match result {
                    Ok(outcome) => {
                        // nothing was put on the server when the file itself is one
                        // the user has to fetch
                        let changed = !outcome.result.installed.is_empty()
                            || !outcome.result.removed.is_empty();
                        if changed || outcome.blocked.is_empty() {
                            let said = t(if update {
                                M::ModsUpdated
                            } else {
                                M::ModsInstalledVersion
                            });
                            this.toast(
                                Self::result_toast(Some(object), said.to_string(), &outcome.result),
                                cx,
                            );
                        }
                        // what only CurseForge's website has: watched for in the
                        // Downloads folder, and listed on the tab
                        if !outcome.blocked.is_empty() {
                            let uuid = this.uuid.clone();
                            this.state.update(cx, |state, cx| {
                                state.add_by_hand(&uuid, outcome.blocked, cx)
                            });
                        }
                    }
                    Err(err) => {
                        let text = if update {
                            tr!(M::ModsUpdateFailed, error = err)
                        } else {
                            tr!(M::ModsInstallFailed, error = err)
                        };
                        this.toast(
                            Toast::error(text).object(ObjectChip::new(object.0).kind(Kind::Mod)),
                            cx,
                        );
                    }
                }
                this.reload(cx);
            })
            .ok();
        });
        self.push(task);
    }
}

impl ModsPage {
    /// "Watch another folder…": a folder dialog for where the browser saves downloads.
    fn choose_by_hand_folder(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t(M::ModsByHandFolderPrompt).into()),
        });
        let task = cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = picked.await else {
                return;
            };
            let Some(folder) = paths.pop() else {
                return;
            };
            this.update(cx, |this, cx| {
                this.state
                    .update(cx, |state, cx| state.set_by_hand_folder(folder, cx));
            })
            .ok();
        });
        self.push(task);
    }

    /// The card for the files the server waits for from CurseForge's website: each
    /// with its download page, and "Added" once the Host found it in the Downloads
    /// folder and put it on the server.
    pub(super) fn render_by_hand(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (files, added, error, one_by_one, own_window, folder) = {
            let state = self.state.read(cx);
            let waiting = state.by_hand(&self.uuid)?;
            (
                waiting.files.clone(),
                waiting.added.clone(),
                waiting.error.clone(),
                waiting.one_by_one,
                waiting.own_window,
                state.by_hand_folder().map(|f| f.display().to_string()),
            )
        };
        let left = added.iter().filter(|added| !**added).count();
        let rows: Vec<AnyElement> = files
            .iter()
            .zip(&added)
            .enumerate()
            .map(|(index, (file, added))| {
                let id = |part: &str| SharedString::from(format!("by-hand-{index}-{part}"));
                let status: AnyElement = if *added {
                    StatusBadge::new(t(M::ModsByHandAdded), Tone::Success).into_any_element()
                } else {
                    let uuid = self.uuid.clone();
                    Button::new(id("open"), t(M::ModsByHandOpen))
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Slim)
                        .icon(Icon::ArrowUpRightFromSquare)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state
                                .update(cx, |state, cx| state.open_by_hand_page(&uuid, index, cx));
                        }))
                        .into_any_element()
                };
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .px(px(16.))
                    .py(px(8.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(object_name(file.title()))
                            .child(
                                div()
                                    .text_size(text::SMALL)
                                    .text_color(ink::muted())
                                    .truncate()
                                    .child(if *added {
                                        file.file.file_name.clone()
                                    } else {
                                        tr!(M::ModsByHandWaiting, file = file.file.file_name)
                                    }),
                            ),
                    )
                    .child(status)
                    .into_any_element()
            })
            .collect();
        let lead: AnyElement = match folder {
            Some(folder) => {
                // the sentence, then the folder as the path it is
                let (before, after) = {
                    let sentence = tr!(M::ModsByHandLead, folder = "\u{1}");
                    match sentence.split_once('\u{1}') {
                        Some((before, after)) => (before.to_string(), after.to_string()),
                        None => (sentence, String::new()),
                    }
                };
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_baseline()
                    .gap_x(px(4.))
                    .child(before.trim_end().to_string())
                    // the folder with what follows it, so no space comes before a full stop
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .child(PathText::file(folder))
                            .child(after.trim_start().to_string()),
                    )
                    .into_any_element()
            }
            None => div().child(t(M::ModsByHandNoFolder)).into_any_element(),
        };
        let uuid = self.uuid.clone();
        let skip = self.uuid.clone();
        Some(
            div()
                .id("mods-by-hand")
                .flex()
                .flex_col()
                .gap(px(12.))
                .p(px(16.))
                .rounded(radius::XL)
                .border_1()
                .border_color(colors::YELLOW.opacity(40))
                .bg(colors::GRAY_850)
                .child(
                    div()
                        .text_size(text::H3)
                        .font_weight(weights::BOLD.face())
                        .text_color(ink::text())
                        .child(t(M::ModsByHandTitle)),
                )
                .child(
                    div()
                        .text_size(text::SMALL)
                        .text_color(ink::secondary())
                        .child(lead),
                )
                .child(card(rows))
                .when_some(error, |this, error| {
                    this.child(
                        div()
                            .text_size(text::SMALL)
                            .text_color(colors::RED_200)
                            .child(error),
                    )
                })
                .when(one_by_one && left > 0, |this| {
                    this.child(
                        div()
                            .text_size(text::SMALL)
                            .text_color(ink::muted())
                            .child(t(if own_window {
                                M::ModsByHandOwnWindow
                            } else {
                                M::ModsByHandOneByOne
                            })),
                    )
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap(px(8.))
                        .when(left > 1, |this| {
                            this.child(
                                Button::new("by-hand-open-all", t(M::ModsByHandOpenAll))
                                    .size(ButtonSize::Slim)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.state.update(cx, |state, cx| {
                                            state.open_by_hand_pages(&uuid, cx)
                                        });
                                    })),
                            )
                        })
                        .child(
                            Button::new("by-hand-folder", t(M::ModsByHandChangeFolder))
                                .size(ButtonSize::Slim)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.choose_by_hand_folder(cx)),
                                ),
                        )
                        .child(
                            Button::new("by-hand-skip", t(M::ModsByHandSkip))
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.state
                                        .update(cx, |state, cx| state.dismiss_by_hand(&skip, cx));
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}
