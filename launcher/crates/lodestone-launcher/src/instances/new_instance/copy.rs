//! "Copy with another version or loader": the New instance dialog's form for a new
//! instance made from an existing one, which stays as it is. The Minecraft version and
//! loader pickers are the dialog's own; under them, what to take along: the worlds
//! (only to the same or a newer version, and with a word that the game upgrades them
//! for good), the resource packs, shaders and options, and the mods, as each one's
//! build for the new version and loader, looked up on Modrinth as the choice changes.
//! The work is `Instances::copy_as`'s, a job in the sidebar.

use std::time::Duration;

use gpui::{AnyElement, Context, IntoElement, SharedString, Task, div, prelude::*};
use launcher_core::{
    content::{
        Target,
        copy_as::{self, Contents, CopyAsOptions, CopyAsRequest, ResolvedMods, WorldsRule},
    },
    instances::{Instance, Loader},
};

use super::NewInstanceDialog;
use crate::theme::px;
use crate::{
    components::{Checkbox, Icon, LoadingDots},
    i18n::{InstanceSettingsMsg as M, InstanceSettingsPlural as P, format, t, tr, trn},
    instances::content::Load,
    theme::{self, colors, leading, radius, text, weights},
};

/// How long the choice must stay before its mods are looked up, so clicking through
/// versions doesn't ask Modrinth about each.
const LOOKUP_DELAY: Duration = Duration::from_millis(350);

/// The mods' builds for one Minecraft version and loader.
type Lookup = ((String, Loader), Load<Option<ResolvedMods>>);

/// The copy form's own state.
pub(super) struct CopyForm {
    pub source: Instance,
    pub options: CopyAsOptions,
    /// What the instance has, read when the dialog opens.
    contents: Load<Contents>,
    /// The mods' builds for the version and loader chosen last: `None` inside when
    /// Modrinth can't be reached.
    lookup: Option<Lookup>,
    _contents_task: Option<Task<()>>,
    _lookup_task: Option<Task<()>>,
}

impl CopyForm {
    pub fn new(source: Instance) -> Self {
        Self {
            source,
            options: CopyAsOptions::default(),
            contents: Load::NotLoaded,
            lookup: None,
            _contents_task: None,
            _lookup_task: None,
        }
    }
}

impl NewInstanceDialog {
    fn form(&self) -> Option<&CopyForm> {
        self.copy.as_ref()
    }

    fn form_mut(&mut self) -> Option<&mut CopyForm> {
        self.copy.as_mut()
    }

    /// Reads what the instance has, once.
    pub(super) fn read_copy_contents(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.form() else {
            return;
        };
        if !matches!(form.contents, Load::NotLoaded) {
            return;
        }
        let id = form.source.id.clone();
        let task = self
            .instances
            .update(cx, |instances, cx| instances.copy_contents(&id, cx));
        let read = cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if let Some(form) = this.form_mut() {
                    form.contents = match result {
                        Ok(contents) => Load::Ready(contents),
                        Err(err) => Load::Failed(err.into()),
                    };
                }
                cx.notify();
            })
            .ok();
        });
        if let Some(form) = self.form_mut() {
            form.contents = Load::Loading;
            form._contents_task = Some(read);
        }
    }

    /// The Minecraft version and loader chosen now.
    fn copy_target(&self, cx: &gpui::App) -> Option<(String, Loader)> {
        let version = self.selected.clone()?;
        Some((version, Loader::from_kind(self.loader.read(cx).kind())))
    }

    /// Looks the mods up for the version and loader chosen, after a moment, unless they
    /// are known already (or not wanted).
    fn look_up_mods(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.copy_target(cx) else {
            return;
        };
        let Some(form) = self.form() else {
            return;
        };
        let Load::Ready(contents) = &form.contents else {
            return;
        };
        let wanted = form.options.mods && target.1 != Loader::None && !contents.mods.is_empty();
        let known = form.lookup.as_ref().is_some_and(|(at, _)| *at == target);
        if !wanted || known {
            return;
        }
        let mods = contents.mods.clone();
        let instances = self.instances.clone();
        let at = target.clone();
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(LOOKUP_DELAY).await;
            let lookup = instances.update(cx, |instances, cx| {
                instances.resolve_copy_mods(mods, Target::new(at.0.clone(), at.1), cx)
            });
            let result = match lookup {
                Some(task) => task.await.map(Some),
                None => Ok(None),
            };
            this.update(cx, |this, cx| {
                if let Some(form) = this.form_mut()
                    && let Some((key, state)) = &mut form.lookup
                    && *key == at
                {
                    *state = match result {
                        Ok(found) => Load::Ready(found),
                        Err(err) => Load::Failed(err.into()),
                    };
                }
                cx.notify();
            })
            .ok();
        });
        if let Some(form) = self.form_mut() {
            form.lookup = Some((target, Load::Loading));
            form._lookup_task = Some(task);
        }
    }

    /// Shows made-up builds for the mods, for the offscreen screenshots.
    pub fn preview_copy_lookup(
        &mut self,
        options: CopyAsOptions,
        build: impl FnMut(&copy_as::SourceMod) -> Option<lodestone_minecraft::modrinth::RawVersion>,
        cx: &mut Context<Self>,
    ) {
        let target = self.copy_target(cx);
        if let Some(form) = self.form_mut() {
            form.options = options;
            if let (Load::Ready(contents), Some(target)) = (&form.contents, target) {
                let resolved = ResolvedMods::sort(&contents.mods, build);
                form.lookup = Some((target, Load::Ready(Some(resolved))));
                form._lookup_task = None;
            }
        }
        cx.notify();
    }

    /// Shows `contents` as the instance's, for the offscreen screenshots.
    pub fn preview_copy_contents(&mut self, contents: Contents, cx: &mut Context<Self>) {
        if let Some(form) = self.form_mut() {
            form.contents = Load::Ready(contents);
            form._contents_task = None;
        }
        cx.notify();
    }

    /// Whether worlds can go to the version chosen.
    fn copy_worlds_rule(&self, cx: &gpui::App) -> WorldsRule {
        match (self.form(), &self.selected) {
            (Some(form), Some(version)) => self
                .instances
                .read(cx)
                .worlds_rule(&form.source.game_version, version),
            _ => WorldsRule::Unknown,
        }
    }

    /// The mods' builds for the version and loader chosen, when they have been looked
    /// up; `Some(None)` when Modrinth can't be reached.
    fn copy_resolved(&self, cx: &gpui::App) -> Option<Option<&ResolvedMods>> {
        let target = self.copy_target(cx)?;
        match self.form()?.lookup.as_ref()? {
            (at, Load::Ready(found)) if *at == target => Some(found.as_ref()),
            _ => None,
        }
    }

    /// Whether the copy can start: the instance has been read, and its mods (when they
    /// go) have been looked up.
    pub(super) fn copy_ready(&self, cx: &gpui::App) -> bool {
        let (Some(form), Some((_, loader))) = (self.form(), self.copy_target(cx)) else {
            return false;
        };
        let Load::Ready(contents) = &form.contents else {
            return false;
        };
        let mods_wanted = form.options.mods && loader != Loader::None && !contents.mods.is_empty();
        !mods_wanted || self.copy_resolved(cx).is_some()
    }

    /// Starts the copy: a job in the sidebar, and the dialog closes.
    pub(super) fn start_copy(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let Some((version, loader_now)) = self.copy_target(cx) else {
            return Err(t(M::ChooseGameVersion).into());
        };
        let loader = self.loader.read(cx).choice()?;
        let rule = self.copy_worlds_rule(cx);
        let resolved = self.copy_resolved(cx).flatten().cloned();
        let Some(form) = self.form() else {
            return Ok(());
        };
        let Load::Ready(contents) = &form.contents else {
            return Ok(());
        };
        let plan = copy_as::plan(
            contents,
            &version,
            loader_now,
            form.options,
            rule,
            resolved.as_ref(),
        );
        let typed = self.name.read(cx).text().trim().to_string();
        let request = CopyAsRequest {
            name: (!typed.is_empty()).then_some(typed),
            loader,
            plan,
        };
        let id = form.source.id.clone();
        self.instances
            .update(cx, |instances, cx| instances.copy_as(&id, request, cx))
            .map(|_| ())
    }

    /// "From Survival (1.20.1 · Fabric 0.16.9). It stays as it is."
    pub(super) fn copy_intro(&self) -> Option<AnyElement> {
        let form = self.form()?;
        let from = match form.source.loader.kind() {
            Some(_) => format!(
                "{} · {}",
                form.source.game_version,
                form.source.loader_label()
            ),
            None => form.source.game_version.clone(),
        };
        Some(
            div()
                .text_color(crate::theme::ink::muted())
                .whitespace_normal()
                .child(tr!(M::CopyFrom, name = form.source.name, version = from))
                .into_any_element(),
        )
    }

    /// What to take along: worlds, packs and options, mods.
    pub(super) fn copy_options(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.read_copy_contents(cx);
        self.look_up_mods(cx);
        let rule = self.copy_worlds_rule(cx);
        let resolved = self.copy_resolved(cx).map(|found| found.cloned());
        let (version, loader) = self.copy_target(cx).unwrap_or_default();
        let form = self.form()?;
        let options = form.options;
        let source = &form.source;
        let target_label = match loader.kind() {
            Some(kind) => format!("{version} {}", kind.label()),
            None => version.clone(),
        };
        let contents = match &form.contents {
            Load::Ready(contents) => Some(contents),
            _ => None,
        };
        let entity = cx.entity();
        let set = move |change: fn(&mut CopyAsOptions, bool)| {
            let entity = entity.clone();
            move |value: &bool, _: &mut gpui::Window, cx: &mut gpui::App| {
                let value = *value;
                entity.update(cx, |this, cx| {
                    if let Some(form) = this.form_mut() {
                        change(&mut form.options, value);
                    }
                    cx.notify();
                });
            }
        };

        // Worlds: only to the same or a newer version.
        let worlds = contents.map_or(0, |c| c.worlds.len());
        let worlds_allowed = rule.allowed() && worlds > 0;
        let worlds_note = match rule {
            _ if contents.is_some() && worlds == 0 => Some(note(t(M::CopyNoWorlds).into())),
            WorldsRule::Older => Some(note(t(M::CopyWorldsOlder).into())),
            WorldsRule::Unknown => Some(note(tr!(M::CopyWorldsUnknown, version = version).into())),
            WorldsRule::Newer if options.worlds => Some(warning(tr!(
                M::CopyWorldsUpgraded,
                version = version,
                name = source.name
            ))),
            _ => None,
        };
        let worlds_row = option_row(
            Checkbox::new(
                "copy-worlds",
                options.worlds && worlds_allowed,
                if worlds > 0 {
                    trn!(P::CopyWorldsCount, worlds)
                } else {
                    t(M::CopyWorlds).to_string()
                },
            )
            .disabled(!worlds_allowed)
            .on_change(set(|o, v| o.worlds = v)),
            worlds_note,
        );

        let packs_row = option_row(
            Checkbox::new("copy-packs", options.packs, t(M::CopyPacks))
                .on_change(set(|o, v| o.packs = v)),
            None,
        );

        // Mods: each one's build for the new version and loader.
        let has_mods = contents.is_some_and(|c| !c.mods.is_empty() || !c.other_mods.is_empty());
        let mods_possible = loader != Loader::None && has_mods;
        let mods_note: Option<AnyElement> = match contents {
            None => Some(match &form.contents {
                Load::Failed(err) => error_note(tr!(M::CopyModsFailed, error = err)),
                _ => loading(t(M::CopyReading).into()),
            }),
            Some(_) if !has_mods => Some(note(t(M::CopyNoMods).into())),
            Some(_) if loader == Loader::None => Some(note(t(M::CopyModsVanilla).into())),
            Some(_) if !options.mods => None,
            Some(contents) => Some(mods_status(contents, resolved, &form.lookup, &target_label)),
        };
        let mods_row = option_row(
            Checkbox::new("copy-mods", options.mods && mods_possible, t(M::CopyMods))
                .disabled(!mods_possible)
                .on_change(set(|o, v| o.mods = v)),
            mods_note,
        );

        Some(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .font_weight(weights::MEDIUM.into())
                        .text_color(crate::theme::ink::secondary())
                        .child(t(M::CopyWhat)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .px_3()
                        .py_3()
                        .rounded(radius::LG)
                        .bg(colors::GRAY_900)
                        .child(worlds_row)
                        .child(packs_row)
                        .child(mods_row),
                )
                .into_any_element(),
        )
    }
}

/// What the mod lookup found, or what it is doing.
fn mods_status(
    contents: &Contents,
    resolved: Option<Option<ResolvedMods>>,
    lookup: &Option<Lookup>,
    target: &str,
) -> AnyElement {
    if let Some((_, Load::Failed(err))) = lookup {
        return error_note(tr!(M::CopyModsFailed, error = err));
    }
    let not_on_modrinth = (!contents.other_mods.is_empty()).then(|| {
        tr!(
            M::CopyModsNotOnModrinth,
            names = format::list(&contents.other_mods)
        )
    });
    let resolved = match resolved {
        None if contents.mods.is_empty() => ResolvedMods::default(),
        None => return loading(trn!(P::CopyCheckingMods, contents.mods.len())),
        Some(None) => {
            return note(t(M::CopyModsOffline).into());
        }
        Some(Some(resolved)) => resolved,
    };
    let total = contents.mods.len() + contents.other_mods.len();
    let mut lines: Vec<AnyElement> = vec![
        div()
            .child(trn!(
                P::CopyModsFound,
                total,
                found = resolved.found.len(),
                target = target
            ))
            .into_any_element(),
    ];
    if !resolved.missing.is_empty() {
        let names: Vec<String> = resolved.missing.iter().map(|m| m.title.clone()).collect();
        lines.push(
            div()
                .child(tr!(
                    M::CopyModsNoBuild,
                    target = target,
                    names = format::list(&names)
                ))
                .into_any_element(),
        );
    }
    if let Some(line) = not_on_modrinth {
        lines.push(div().child(line).into_any_element());
    }
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .text_color(crate::theme::ink::muted())
        .whitespace_normal()
        .children(lines)
        .into_any_element()
}

/// An option with what it says under it.
fn option_row(check: Checkbox, under: Option<AnyElement>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(check)
        .when_some(under, |this, under| {
            this.child(div().pl(px(24.)).child(under))
        })
}

fn note(text_: SharedString) -> AnyElement {
    div()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .text_color(crate::theme::ink::muted())
        .whitespace_normal()
        .child(text_)
        .into_any_element()
}

fn error_note(text_: String) -> AnyElement {
    div()
        .text_size(theme::text_size(text::SMALL))
        .text_color(colors::RED_200)
        .whitespace_normal()
        .child(text_)
        .into_any_element()
}

fn loading(text_: String) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .text_size(theme::text_size(text::SMALL))
        .text_color(crate::theme::ink::muted())
        .child(LoadingDots)
        .child(text_)
        .into_any_element()
}

/// A one-line warning with its sign.
fn warning(text_: String) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap_2()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .text_color(crate::theme::ink::text())
        .child(
            div()
                .pt(px(2.))
                .child(Icon::TriangleExclamation.render(px(12.), colors::YELLOW_300)),
        )
        .child(div().flex_1().min_w_0().whitespace_normal().child(text_))
        .into_any_element()
}
