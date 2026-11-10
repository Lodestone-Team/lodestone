//! A Modrinth search box with pages of results, through the core (`ModrinthSearch.tsx`).
//! Text that looks like a Modrinth link (`modrinth.com/`) is resolved instead, and the
//! linked project is picked as soon as it resolves. Typing waits for a pause (300 ms)
//! before it searches; a pasted link resolves at once.
//!
//! Made for the modpack flow; the Mods tab uses it too (`project_type`, and the options
//! below: one box for the server's game version and loader, the "Installed" tag, and
//! whether players need a mod).

use std::{collections::HashSet, time::Duration};

use gpui::{
    AnyElement, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, SharedString,
    Styled, Task, Window, div, img, prelude::*,
};
use launcher_core::cores::admin::{
    AdminClient,
    mods::{
        ModrinthProjectType, ModrinthSearch, ModrinthSearchHit, ModrinthSearchIndex,
        ModrinthSearchResponse, ModrinthUrlTarget, looks_like_modrinth_link,
    },
};
use lodestone_ui::{
    components::{
        Accessible as _, Checkbox, Icon, IconButton, KeyboardFocus as _, LoadingDots, TextInput,
        settings::{popover, popover_panel, select_button},
    },
    theme::{colors, ink, px, radius, text, weights},
};

use super::Load;
use crate::{
    i18n::{InstancesMsg as M, t, tr},
    instance_icons,
};

const PAGE_SIZE: u32 = 20;
const PAUSE: Duration = Duration::from_millis(300);

/// A project picked from the results, or from a link.
#[derive(Clone, Debug)]
pub struct ModrinthPick {
    pub project_id: String,
    pub version_id: Option<String>,
    pub project_type: String,
    /// The result it was picked from, shown until the project loads.
    pub hit: Option<ModrinthSearchHit>,
}

impl ModrinthPick {
    fn from_target(target: &ModrinthUrlTarget) -> Self {
        Self {
            project_id: target.project_id.clone(),
            version_id: target.version_id.clone(),
            project_type: target.project_type.clone(),
            hit: None,
        }
    }
}

pub enum ModrinthSearchEvent {
    Picked(ModrinthPick),
}

/// The search: what is typed, the filters, and the page of results.
pub struct ModrinthSearchView {
    admin: AdminClient,
    pub input: Entity<TextInput>,
    project_type: ModrinthProjectType,
    game_version: Option<String>,
    /// The loader the "only …" box filters by, and whether it is ticked.
    loader: Option<(String, &'static str, bool)>,
    /// The Mods tab's box: the server's game version and loader, its label, and
    /// whether it is ticked.
    compat: Option<(String, String, String, bool)>,
    /// Projects installed on the server, tagged "Installed".
    installed: HashSet<String>,
    /// Mods: whether players need a result too (its `client_side`).
    client_side: bool,
    index: ModrinthSearchIndex,
    sort_open: bool,
    /// The last text that wasn't a link: what the results are for.
    query: String,
    page: u32,
    results: Load<ModrinthSearchResponse>,
    /// The link being resolved, and what it resolved to.
    link: Option<(String, Load<ModrinthUrlTarget>)>,
    picked_link: Option<String>,
    last_text: String,
    _debounce: Option<Task<()>>,
    _search: Option<Task<()>>,
    _resolve: Option<Task<()>>,
    _observe: gpui::Subscription,
}

impl EventEmitter<ModrinthSearchEvent> for ModrinthSearchView {}

impl ModrinthSearchView {
    pub fn new(
        admin: AdminClient,
        project_type: ModrinthProjectType,
        placeholder: &'static str,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(cx)
                .with_leading_icon(Icon::MagnifyingGlass)
                .with_clear_button()
                .with_placeholder(placeholder)
        });
        let observe = cx.observe(&input, |this, _, cx| this.text_changed(cx));
        let mut this = Self {
            admin,
            input,
            project_type,
            game_version: None,
            loader: None,
            compat: None,
            installed: HashSet::new(),
            client_side: false,
            index: ModrinthSearchIndex::Relevance,
            sort_open: false,
            query: String::new(),
            page: 0,
            results: Load::Loading,
            link: None,
            picked_link: None,
            last_text: String::new(),
            _debounce: None,
            _search: None,
            _resolve: None,
            _observe: observe,
        };
        this.search(cx);
        this
    }

    /// A box that limits the results to `loader` ("Only show Fabric modpacks"), ticked.
    pub fn with_loader_filter(
        mut self,
        loader: &str,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> Self {
        self.loader = Some((loader.to_string(), label, true));
        self.search(cx);
        self
    }

    /// Only results for this game version.
    pub fn with_game_version(mut self, version: Option<String>, cx: &mut Context<Self>) -> Self {
        self.game_version = version;
        self.search(cx);
        self
    }

    /// A box that limits the results to a server's game version and loader ("Only show
    /// versions for 1.21.1 / fabric"), ticked: the Mods tab's.
    pub fn with_compat_filter(
        mut self,
        game_version: String,
        loader: String,
        label: String,
        cx: &mut Context<Self>,
    ) -> Self {
        self.compat = Some((game_version, loader, label, true));
        self.search(cx);
        self
    }

    /// Shows whether players need each result too (mods).
    pub fn with_client_side(mut self) -> Self {
        self.client_side = true;
        self
    }

    /// The projects installed on the server, tagged "Installed" in the results.
    pub fn set_installed(&mut self, installed: HashSet<String>, cx: &mut Context<Self>) {
        if self.installed != installed {
            self.installed = installed;
            cx.notify();
        }
    }

    fn params(&self) -> ModrinthSearch {
        let compat = self.compat.as_ref().filter(|(.., on)| *on);
        ModrinthSearch {
            query: self.query.clone(),
            project_type: self.project_type,
            game_version: compat
                .map(|(version, ..)| version.clone())
                .or_else(|| self.game_version.clone()),
            loader: compat.map(|(_, loader, ..)| loader.clone()).or_else(|| {
                self.loader
                    .as_ref()
                    .filter(|(_, _, on)| *on)
                    .map(|(loader, _, _)| loader.clone())
            }),
            index: self.index,
            offset: self.page * PAGE_SIZE,
            limit: PAGE_SIZE,
        }
    }

    fn text_changed(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text().trim().to_string();
        if text == self.last_text {
            return;
        }
        // a paste adds many characters at once: resolve a pasted link at once
        let pasted = text.len() > self.last_text.len() + 8;
        self.last_text = text.clone();
        if looks_like_modrinth_link(&text) {
            self._debounce = None;
            if pasted {
                self.resolve(text, cx);
            } else {
                self.link = Some((text.clone(), Load::Loading));
                self._debounce = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(PAUSE).await;
                    this.update(cx, |this, cx| this.resolve(text, cx)).ok();
                }));
            }
        } else {
            self.link = None;
            self._resolve = None;
            self._debounce = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(PAUSE).await;
                this.update(cx, |this, cx| {
                    if this.query != text {
                        this.query = text;
                        this.page = 0;
                        this.search(cx);
                    }
                })
                .ok();
            }));
        }
        cx.notify();
    }

    fn resolve(&mut self, link: String, cx: &mut Context<Self>) {
        self.link = Some((link.clone(), Load::Loading));
        let admin = self.admin.clone();
        self._resolve = Some(cx.spawn(async move |this, cx| {
            let wanted = link.clone();
            let target = cx
                .background_spawn(async move { admin.modrinth_resolve(&wanted).await })
                .await;
            this.update(cx, |this, cx| {
                if this.link.as_ref().map(|(l, _)| l) != Some(&link) {
                    return;
                }
                match target {
                    Ok(target) => {
                        // pick a resolved link once, so going back doesn't reopen it
                        if this.picked_link.as_ref() != Some(&link) {
                            this.picked_link = Some(link.clone());
                            cx.emit(ModrinthSearchEvent::Picked(ModrinthPick::from_target(
                                &target,
                            )));
                        }
                        this.link = Some((link, Load::Ready(target)));
                    }
                    Err(err) => this.link = Some((link, Load::Failed(err.to_string()))),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn search(&mut self, cx: &mut Context<Self>) {
        let params = self.params();
        let admin = self.admin.clone();
        // the last results stay while the next page loads, so nothing jumps
        if matches!(self.results, Load::Failed(_)) {
            self.results = Load::Loading;
        }
        self._search = Some(cx.spawn(async move |this, cx| {
            let wanted = params.clone();
            let results = cx
                .background_spawn(async move { admin.modrinth_search(&wanted).await })
                .await;
            this.update(cx, |this, cx| {
                if this.params() != params {
                    return;
                }
                this.results = match results {
                    Ok(results) => Load::Ready(results),
                    Err(err) => Load::Failed(err.to_string()),
                };
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn set_page(&mut self, page: u32, cx: &mut Context<Self>) {
        self.page = page;
        self.search(cx);
    }

    fn render_sort(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let name = |index: ModrinthSearchIndex| {
            t(match index {
                ModrinthSearchIndex::Relevance => M::SortRelevance,
                ModrinthSearchIndex::Downloads => M::SortDownloads,
                ModrinthSearchIndex::Follows => M::SortFollows,
                ModrinthSearchIndex::Newest => M::SortNewest,
                ModrinthSearchIndex::Updated => M::SortUpdated,
            })
        };
        let current = self.index;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .text_size(text::MEDIUM)
                    .text_color(ink::muted())
                    .child(t(M::SortLabel)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        select_button("modrinth-sort", div().child(name(current)))
                            .aria_label(t(M::SortLabel))
                            .min_w(px(176.))
                            .justify_between()
                            .on_press(cx.listener(|this, _, _, cx| {
                                this.sort_open = !this.sort_open;
                                cx.notify();
                            })),
                    )
                    .when(self.sort_open, |this| {
                        let close = cx.listener(|this, _: &(), _, cx| {
                            this.sort_open = false;
                            cx.notify();
                        });
                        let options = ModrinthSearchIndex::ALL.map(|index| {
                            div()
                                .id(SharedString::from(format!("sort-{}", index.as_str())))
                                .accessible(
                                    gpui::Role::ListItem,
                                    format!("sort-{}", index.as_str()),
                                    name(index),
                                )
                                .keyboard_focus()
                                .px(px(10.))
                                .py(px(6.))
                                .rounded(radius::MD)
                                .cursor_pointer()
                                .text_size(text::MEDIUM)
                                .text_color(ink::text())
                                .when(index == current, |this| this.bg(colors::GRAY_700))
                                .hover(|style| style.bg(colors::GRAY_750))
                                .child(name(index))
                                .on_press(cx.listener(move |this, _, _, cx| {
                                    this.index = index;
                                    this.sort_open = false;
                                    this.page = 0;
                                    this.search(cx);
                                }))
                        });
                        this.child(
                            div().child(popover(
                                "modrinth-sort-popover",
                                popover_panel("modrinth-sort-list")
                                    .min_w(px(176.))
                                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                        this.sort_open = false;
                                        cx.notify();
                                    }))
                                    .children(options),
                                move |window, cx| close(&(), window, cx),
                            )),
                        )
                    }),
            )
            .into_any_element()
    }

    fn hit_card(&self, hit: &ModrinthSearchHit, cx: &mut Context<Self>) -> AnyElement {
        let icon = hit
            .icon_url
            .as_deref()
            .and_then(|url| instance_icons::modrinth_image(url, cx));
        let pick = ModrinthPick {
            project_id: hit.project_id.clone(),
            version_id: None,
            project_type: hit.project_type.clone(),
            hit: Some(hit.clone()),
        };
        div()
            .id(SharedString::from(format!("hit-{}", hit.project_id)))
            .accessible(
                gpui::Role::Button,
                format!("hit-{}", hit.project_id),
                hit.title.clone(),
            )
            .keyboard_focus_outer()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(12.))
            .p(px(12.))
            .rounded(radius::LG)
            .border_1()
            .border_color(colors::GRAY_FADED.opacity(30))
            .bg(colors::GRAY_800)
            .cursor_pointer()
            .hover(|style| {
                style
                    .bg(colors::GRAY_750)
                    .border_color(colors::GRAY_FADED.opacity(50))
            })
            .child(project_icon(icon, px(48.)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap(px(6.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(text::H3)
                                    .font_weight(weights::BOLD.face())
                                    .text_color(ink::text())
                                    .child(hit.title.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(text::SMALL)
                                    .text_color(ink::muted())
                                    .child(tr!(M::ByAuthor, author = hit.author)),
                            )
                            .when(self.installed.contains(&hit.project_id), |this| {
                                this.child(lodestone_ui::components::settings::tag(
                                    t(crate::i18n::InstanceMsg::ModsInstalledTag),
                                    colors::GREEN,
                                ))
                            })
                            .when(self.client_side, |this| {
                                this.children(client_side_label(&hit.client_side))
                            }),
                    )
                    .child(
                        div()
                            .text_size(text::MEDIUM)
                            .text_color(ink::secondary())
                            .line_clamp(2)
                            .child(hit.description.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.))
                            .text_size(text::SMALL)
                            .text_color(ink::muted())
                            .child(Icon::Download.render(px(10.), ink::muted()))
                            .child(lodestone_ui::i18n::format::compact(hit.downloads)),
                    ),
            )
            .on_press(
                cx.listener(move |_, _, _, cx| cx.emit(ModrinthSearchEvent::Picked(pick.clone()))),
            )
            .into_any_element()
    }
}

/// Whether players need a Modrinth project too, from its `client_side`: "Players need
/// this too" when required, a quiet "Server only" when unsupported, else nothing
/// (`ClientSideLabel`).
pub fn client_side_label(client_side: &str) -> Option<AnyElement> {
    use crate::i18n::InstanceMsg;
    match client_side {
        "required" => Some(
            lodestone_ui::components::settings::tag(
                t(InstanceMsg::ModsPlayersNeedToo),
                colors::BLUE_200,
            )
            .into_any_element(),
        ),
        "unsupported" => Some(
            div()
                .flex_none()
                .text_size(text::SMALL)
                .text_color(ink::faint())
                .child(t(InstanceMsg::ModsServerOnly))
                .into_any_element(),
        ),
        _ => None,
    }
}

/// A project's icon, or a grey square while it loads or when there is none.
pub fn project_icon(
    image: Option<std::sync::Arc<gpui::RenderImage>>,
    size: gpui::Pixels,
) -> AnyElement {
    match image {
        Some(image) => img(image)
            .flex_none()
            .size(size)
            .rounded(radius::MD)
            .into_any_element(),
        None => div()
            .flex_none()
            .size(size)
            .rounded(radius::MD)
            .bg(colors::GRAY_700)
            .flex()
            .items_center()
            .justify_center()
            .child(Icon::Cube.render(size * 0.45, ink::faint()))
            .into_any_element(),
    }
}

/// A result card's shape while the results load.
fn skeleton_card() -> AnyElement {
    let bar = |w: f32, h: f32| {
        div()
            .w(gpui::relative(w))
            .h(px(h))
            .rounded(radius::DEFAULT)
            .bg(colors::GRAY_700)
    };
    div()
        .flex()
        .flex_row()
        .gap(px(12.))
        .p(px(12.))
        .h(px(84.))
        .rounded(radius::LG)
        .border_1()
        .border_color(colors::GRAY_FADED.opacity(30))
        .bg(colors::GRAY_800)
        .child(
            div()
                .flex_none()
                .size(px(48.))
                .rounded(radius::MD)
                .bg(colors::GRAY_700),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(8.))
                .pt(px(4.))
                .child(bar(0.33, 14.))
                .child(bar(0.75, 10.))
                .child(bar(0.5, 10.)),
        )
        .into_any_element()
}

impl Render for ModrinthSearchView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let opening = matches!(self.link, Some((_, Load::Loading)));
        let link_status: Option<AnyElement> = match &self.link {
            Some((_, Load::Failed(error))) => Some(
                div()
                    .text_size(text::MEDIUM)
                    .text_color(colors::RED_200)
                    .child(tr!(M::SearchLinkFailed, error = error))
                    .into_any_element(),
            ),
            Some((_, Load::Ready(target))) => {
                let pick = ModrinthPick::from_target(target);
                Some(
                    div()
                        .id("open-link")
                        .text_size(text::MEDIUM)
                        .text_color(colors::BLUE_200)
                        .cursor_pointer()
                        .hover(|style| style.underline())
                        .child(tr!(M::SearchOpenLink, slug = target.slug))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(ModrinthSearchEvent::Picked(pick.clone()))
                        }))
                        .into_any_element(),
                )
            }
            _ => None,
        };
        let compat = self.compat.as_ref().map(|(.., label, on)| {
            Checkbox::new("modrinth-compat-filter", *on, label.clone()).on_change(cx.listener(
                |this, value: &bool, _, cx| {
                    if let Some((.., on)) = this.compat.as_mut() {
                        *on = *value;
                    }
                    this.page = 0;
                    this.search(cx);
                },
            ))
        });
        let filter = self.loader.as_ref().map(|(_, label, on)| {
            Checkbox::new("modrinth-loader-filter", *on, *label).on_change(cx.listener(
                |this, value: &bool, _, cx| {
                    if let Some((_, _, on)) = this.loader.as_mut() {
                        *on = *value;
                    }
                    this.page = 0;
                    this.search(cx);
                },
            ))
        });
        let results: AnyElement = match &self.results {
            Load::Failed(error) => div()
                .text_size(text::MEDIUM)
                .text_color(colors::RED_200)
                .child(tr!(M::SearchFailed, error = error))
                .into_any_element(),
            Load::Loading => div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .children((0..5).map(|_| skeleton_card()))
                .into_any_element(),
            Load::Ready(results) if results.hits.is_empty() => div()
                .text_size(text::MEDIUM)
                .text_color(ink::muted())
                .child(t(M::SearchNoResults))
                .into_any_element(),
            Load::Ready(results) => {
                let results = results.clone();
                let total_pages = results.total_hits.div_ceil(PAGE_SIZE).max(1);
                let page = self.page;
                let cards: Vec<AnyElement> = results
                    .hits
                    .iter()
                    .map(|hit| self.hit_card(hit, cx))
                    .collect();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children(cards)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_center()
                            .gap(px(12.))
                            .pt(px(4.))
                            .text_size(text::MEDIUM)
                            .text_color(ink::muted())
                            .child(
                                IconButton::new("previous-page", Icon::ChevronLeft)
                                    .label(t(M::PreviousPage))
                                    .disabled(page == 0)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_page(page.saturating_sub(1), cx)
                                    })),
                            )
                            .child(tr!(M::SearchPage, page = page + 1, total = total_pages))
                            .child(
                                IconButton::new("next-page", Icon::ChevronRight)
                                    .label(t(M::NextPage))
                                    .disabled(page + 1 >= total_pages)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_page(page + 1, cx)
                                    })),
                            ),
                    )
                    .into_any_element()
            }
        };
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .relative()
                    .child(self.input.clone())
                    .when(opening, |this| {
                        this.child(
                            div()
                                .absolute()
                                .right(px(12.))
                                .top(px(8.))
                                .flex()
                                .flex_row()
                                .gap(px(6.))
                                .items_center()
                                .text_size(text::SMALL)
                                .text_color(ink::muted())
                                .child(LoadingDots)
                                .child(t(M::SearchOpening)),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .children(filter)
                    .children(compat)
                    .child(self.render_sort(cx)),
            )
            .children(link_status)
            .child(results)
    }
}
