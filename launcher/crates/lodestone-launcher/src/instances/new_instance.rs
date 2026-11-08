//! "New instance": a name, a Minecraft version from Mojang's version list (with a
//! releases / snapshots / old versions filter and search), and a mod loader with its
//! version.

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, SharedString,
    Subscription, UniformListScrollHandle, Window, div, prelude::*, px, uniform_list,
};
use lodestone_minecraft::versions::mojang::{MojangVersion, MojangVersionType};

use super::{Instances, Versions, loader_picker::LoaderPicker};
use crate::{
    components::{Button, ButtonVariant, Dialog, LoadingDots, TextInput, Toggle},
    theme::{self, colors, leading, radius, text, weights},
};

/// Which kinds of versions the list shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VersionFilter {
    pub releases: bool,
    pub snapshots: bool,
    pub old: bool,
}

impl Default for VersionFilter {
    fn default() -> Self {
        Self {
            releases: true,
            snapshots: false,
            old: false,
        }
    }
}

impl VersionFilter {
    pub fn shows(&self, kind: MojangVersionType) -> bool {
        match kind {
            MojangVersionType::Release => self.releases,
            MojangVersionType::Snapshot | MojangVersionType::Other => self.snapshots,
            MojangVersionType::OldBeta | MojangVersionType::OldAlpha => self.old,
        }
    }
}

/// The versions matching the filter and search, in the manifest's order.
pub fn filter_versions<'a>(
    versions: &'a [MojangVersion],
    filter: VersionFilter,
    search: &str,
) -> Vec<&'a MojangVersion> {
    let search = search.trim().to_ascii_lowercase();
    versions
        .iter()
        .filter(|v| filter.shows(v.kind))
        .filter(|v| search.is_empty() || v.id.to_ascii_lowercase().contains(&search))
        .collect()
}

pub struct NewInstanceDismissed;

pub struct NewInstanceDialog {
    instances: Entity<Instances>,
    name: Entity<TextInput>,
    search: Entity<TextInput>,
    filter: VersionFilter,
    selected: Option<String>,
    loader: Entity<LoaderPicker>,
    error: Option<SharedString>,
    scroll: UniformListScrollHandle,
    _observe: Vec<Subscription>,
}

impl EventEmitter<NewInstanceDismissed> for NewInstanceDialog {}

impl Focusable for NewInstanceDialog {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.name.focus_handle(cx)
    }
}

impl NewInstanceDialog {
    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        instances.update(cx, |instances, cx| instances.load_versions(cx));
        let name = cx.new(|cx| TextInput::new(cx).with_placeholder("My instance"));
        let search = cx.new(|cx| TextInput::new(cx).with_placeholder("Search versions"));
        let observe = vec![
            cx.observe(&instances, |this, _, cx| {
                this.pick_default(cx);
                cx.notify();
            }),
            cx.observe(&search, |_, _, cx| cx.notify()),
            cx.observe(&name, |_, _, cx| cx.notify()),
        ];
        let loader = {
            let instances = instances.clone();
            cx.new(|cx| LoaderPicker::new(instances, None, None, 220., cx))
        };
        let mut this = Self {
            instances,
            name,
            search,
            filter: VersionFilter::default(),
            selected: None,
            loader,
            error: None,
            scroll: UniformListScrollHandle::new(),
            _observe: observe,
        };
        this.pick_default(cx);
        this
    }

    /// Fills in the fields, for the offscreen screenshots.
    pub fn with_fields(self, name: &str, search: &str, cx: &mut Context<Self>) -> Self {
        self.name
            .update(cx, |input, cx| input.set_text(name.to_string(), cx));
        self.search
            .update(cx, |input, cx| input.set_text(search.to_string(), cx));
        self
    }

    /// Selects the newest release once the list has loaded.
    fn pick_default(&mut self, cx: &mut Context<Self>) {
        if self.selected.is_some() {
            return;
        }
        if let Versions::Ready(versions) = self.instances.read(cx).versions() {
            self.selected = versions
                .iter()
                .find(|v| v.kind == MojangVersionType::Release)
                .map(|v| v.id.clone());
            self.sync_loader(cx);
        }
    }

    fn sync_loader(&mut self, cx: &mut Context<Self>) {
        let version = self.selected.clone();
        self.loader
            .update(cx, |picker, cx| picker.set_game_version(version, cx));
    }

    /// Chooses a loader, as clicking it would.
    pub fn choose_loader(
        &mut self,
        kind: launcher_core::game::loaders::LoaderKind,
        cx: &mut Context<Self>,
    ) {
        self.loader
            .update(cx, |picker, cx| picker.set_kind(Some(kind), cx));
    }

    /// The loader picker, for the offscreen screenshots.
    pub fn loader_picker(&self) -> &Entity<LoaderPicker> {
        &self.loader
    }

    fn set_filter(&mut self, filter: VersionFilter, cx: &mut Context<Self>) {
        self.filter = filter;
        cx.notify();
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let Some(version) = self.selected.clone() else {
            self.error = Some("Choose a Minecraft version.".into());
            cx.notify();
            return;
        };
        let loader = match self.loader.read(cx).choice() {
            Ok(loader) => loader,
            Err(err) => {
                self.error = Some(err.into());
                cx.notify();
                return;
            }
        };
        let typed = self.name.read(cx).text().trim().to_string();
        let name = if typed.is_empty() {
            match &loader {
                Some(loader) => format!("{version} {}", loader.kind.label()),
                None => version.clone(),
            }
        } else {
            typed
        };
        match self.instances.update(cx, |instances, cx| {
            instances.create(&name, &version, loader.as_ref(), cx)
        }) {
            Ok(_) => cx.emit(NewInstanceDismissed),
            Err(err) => {
                self.error = Some(err.into());
                cx.notify();
            }
        }
    }

    fn version_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let versions = self.instances.read(cx).versions().clone();
        let body = match versions {
            Versions::NotLoaded | Versions::Loading => div()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap_2()
                .size_full()
                .text_color(colors::GRAY_500)
                .child(LoadingDots)
                .child("Loading Mojang's version list")
                .into_any_element(),
            Versions::Failed(error) => div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .p_4()
                .text_center()
                .text_color(colors::RED_200)
                .text_size(theme::text_size(text::SMALL))
                .child(format!("Couldn't load the versions: {error}"))
                .into_any_element(),
            Versions::Ready(all) => {
                let search = self.search.read(cx).text().to_string();
                let count = filter_versions(&all, self.filter, &search).len();
                if count == 0 {
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_full()
                        .text_color(colors::GRAY_500)
                        .child("No versions match.")
                        .into_any_element()
                } else {
                    uniform_list(
                        "versions",
                        count,
                        cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                            let search = this.search.read(cx).text().to_string();
                            let shown = filter_versions(&all, this.filter, &search);
                            range
                                .filter_map(|index| shown.get(index).map(|v| (index, *v)))
                                .map(|(index, version)| {
                                    let selected = this.selected.as_deref() == Some(&version.id);
                                    let id = version.id.clone();
                                    version_row(index, version, selected).on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.selected = Some(id.clone());
                                            this.error = None;
                                            this.sync_loader(cx);
                                            cx.notify();
                                        },
                                    ))
                                })
                                .collect()
                        }),
                    )
                    .track_scroll(&self.scroll)
                    .size_full()
                    .into_any_element()
                }
            }
        };
        div()
            .h(px(220.))
            .w_full()
            .p_1()
            .rounded(radius::LG)
            .border_1()
            .border_color(colors::GRAY_FADED.opacity(30))
            .bg(colors::GRAY_900)
            .child(body)
    }
}

fn kind_label(kind: MojangVersionType) -> &'static str {
    match kind {
        MojangVersionType::Release => "Release",
        MojangVersionType::Snapshot => "Snapshot",
        MojangVersionType::OldBeta => "Beta",
        MojangVersionType::OldAlpha => "Alpha",
        MojangVersionType::Other => "Other",
    }
}

fn version_row(index: usize, version: &MojangVersion, selected: bool) -> gpui::Stateful<gpui::Div> {
    let date: SharedString = version
        .release_time
        .as_deref()
        .and_then(|time| time.get(..10))
        .unwrap_or_default()
        .to_string()
        .into();
    div()
        .id(("version", index))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .h(px(30.))
        .px_2()
        .rounded(radius::MD)
        .cursor_pointer()
        .map(|this| {
            if selected {
                this.bg(colors::BLUE.opacity(35)).text_color(colors::WHITE)
            } else {
                this.text_color(colors::GRAY_300)
                    .hover(|style| style.bg(colors::GRAY_800))
            }
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_weight(weights::BOLD.into())
                .child(version.id.clone()),
        )
        .child(
            div()
                .w(px(64.))
                .text_size(theme::text_size(text::SMALL))
                .text_color(colors::GRAY_500)
                .child(kind_label(version.kind)),
        )
        .child(
            div()
                .w(px(76.))
                .text_size(theme::text_size(text::SMALL))
                .text_color(colors::GRAY_500)
                .child(date),
        )
}

fn label(text_: &'static str) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .font_weight(weights::BOLD.into())
        .text_color(colors::GRAY_400)
        .child(text_)
}

fn filter_toggle(
    id: &'static str,
    caption: &'static str,
    value: bool,
    on_change: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(Toggle::new(id, value).on_change(on_change))
        .child(
            div()
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .text_color(colors::GRAY_400)
                .child(caption),
        )
}

impl Render for NewInstanceDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let filter = self.filter;
        let toggle = |field: fn(&mut VersionFilter) -> &mut bool| {
            let entity = entity.clone();
            move |value: &bool, _: &mut Window, cx: &mut App| {
                let mut filter = filter;
                *field(&mut filter) = *value;
                entity.update(cx, |this, cx| this.set_filter(filter, cx));
            }
        };
        let selected: SharedString = self
            .selected
            .clone()
            .map(|id| match self.loader.read(cx).choice() {
                Ok(Some(loader)) => format!(
                    "Minecraft {id} with {} {}",
                    loader.kind.label(),
                    loader.version
                ),
                _ => format!("Minecraft {id}"),
            })
            .unwrap_or_else(|| "No version selected".into())
            .into();
        Dialog::new("new-instance", "New instance", self.name.focus_handle(cx))
            .width(px(820.))
            .on_close({
                let entity = entity.clone();
                move |_, cx| entity.update(cx, |_, cx| cx.emit(NewInstanceDismissed))
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .text_size(theme::text_size(text::MEDIUM))
                    .child(label("NAME"))
                    .child(self.name.clone())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_6()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().pt_1().child(label("MINECRAFT VERSION")))
                                    .child(self.search.clone())
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap_4()
                                            .child(filter_toggle(
                                                "filter-releases",
                                                "Releases",
                                                filter.releases,
                                                toggle(|f| &mut f.releases),
                                            ))
                                            .child(filter_toggle(
                                                "filter-snapshots",
                                                "Snapshots",
                                                filter.snapshots,
                                                toggle(|f| &mut f.snapshots),
                                            ))
                                            .child(filter_toggle(
                                                "filter-old",
                                                "Old versions",
                                                filter.old,
                                                toggle(|f| &mut f.old),
                                            )),
                                    )
                                    .child(self.version_list(cx)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().pt_1().child(label("MOD LOADER")))
                                    .child(self.loader.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(theme::text_size(text::SMALL))
                            .font_weight(weights::BOLD.into())
                            .map(|this| match &self.error {
                                Some(error) => {
                                    this.text_color(colors::RED_200).child(error.clone())
                                }
                                None => this.text_color(colors::GRAY_500).child(selected),
                            }),
                    ),
            )
            .footer(
                Button::new("create-instance", "Create instance")
                    .variant(ButtonVariant::Primary)
                    .disabled(self.selected.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.create(cx))),
            )
            .footer(
                Button::new("cancel-new-instance", "Cancel")
                    .variant(ButtonVariant::Secondary)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(NewInstanceDismissed))),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn versions() -> Vec<MojangVersion> {
        let manifest: lodestone_minecraft::versions::mojang::MojangVersionManifest =
            serde_json::from_str(include_str!(
                "../../../launcher-core/testdata/version_manifest_v2.json"
            ))
            .unwrap();
        manifest.versions
    }

    #[test]
    fn filters_and_searches() {
        let versions = versions();
        let ids = |filter, search| -> Vec<String> {
            filter_versions(&versions, filter, search)
                .into_iter()
                .map(|v| v.id.clone())
                .collect()
        };
        let releases = ids(VersionFilter::default(), "");
        assert_eq!(releases[0], "26.3");
        assert!(releases.contains(&"1.8.9".to_string()));
        assert!(
            !releases
                .iter()
                .any(|id| id.contains("snapshot") || id.starts_with('b'))
        );
        let snapshots = VersionFilter {
            releases: false,
            snapshots: true,
            old: false,
        };
        assert!(
            ids(snapshots, "")
                .iter()
                .all(|id| id.contains("snapshot") || id.contains("rc"))
        );
        let old = VersionFilter {
            releases: false,
            snapshots: false,
            old: true,
        };
        assert!(ids(old, "").contains(&"b1.7.3".to_string()));
        assert_eq!(ids(VersionFilter::default(), " 1.20 "), ["1.20.1"]);
        assert!(ids(VersionFilter::default(), "nope").is_empty());
    }
}
