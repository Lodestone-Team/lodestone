//! Every readable component with sample values from the design audit, for the apps'
//! `components` screenshots (the launcher's `--scene components`, the Host's
//! `--components`). In both themes and all four languages, it is how a change to a
//! component is checked by eye.

use gpui::{AnyElement, Context, IntoElement, Render, SharedString, Window, div, prelude::*};

use super::*;
use crate::{
    components::{Icon, ListRow, Tabs},
    i18n::{
        CommonMsg, FactMsg, ValueMsg, format,
        spans::{Mono, Obj, Ver},
        t,
    },
    theme::{self, Tone, colors, fonts, ink, px, text, weights},
    tr_spans,
    values::Product,
};

/// A home folder for the sample paths, so the picture is the same on every computer.
const HOME: &str = "/Users/alex";

/// How many sections a page shows: a screenshot window is no taller than the
/// screen, so the gallery comes in pages.
const PER_PAGE: usize = 5;
const PER_NARROW_PAGE: usize = 3;
const SECTIONS: usize = 14;

/// The gallery's view, one page of it. [`narrow`](Self::narrow) lays it out for a
/// 520px window, to show facts left out, columns folded and keys wrapping.
pub struct ComponentGallery {
    narrow: bool,
    page: usize,
    worlds: ListColumns,
}

impl ComponentGallery {
    /// The `page`th page (from 1) for a 1200px window.
    pub fn new(page: usize) -> Self {
        Self::with_width(false, page)
    }

    /// The `page`th page (from 1) for a 520px window.
    pub fn narrow(page: usize) -> Self {
        Self::with_width(true, page)
    }

    /// How many pages there are at a width.
    pub fn pages(narrow: bool) -> usize {
        SECTIONS.div_ceil(if narrow { PER_NARROW_PAGE } else { PER_PAGE })
    }

    fn with_width(narrow: bool, page: usize) -> Self {
        Self {
            narrow,
            page: page.clamp(1, Self::pages(narrow)),
            worlds: ListColumns::new(
                vec![
                    Column::size(t(FactMsg::Size)),
                    Column::date(t(FactMsg::LastPlayed)),
                ],
                px(420.),
            ),
        }
    }
}

/// A titled section.
fn section(title: &'static str, narrow: bool, content: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .when(narrow, |this| this.w_full())
        .when(!narrow, |this| this.w(px(560.)))
        .p(px(14.))
        .rounded(theme::radius::LG)
        .bg(colors::GRAY_850)
        .child(
            div()
                .font_family(fonts::heading())
                .text_size(theme::text_size(text::H3))
                .font_weight(weights::EXTRABOLD.into())
                .text_color(ink::text())
                .child(title),
        )
        .child(content)
        .into_any_element()
}

fn row() -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(8.))
}

fn column() -> gpui::Div {
    div().flex().flex_col().gap(px(8.)).w_full().min_w_0()
}

/// A mod row's second line: the file name first (cut before anything is left out),
/// then the version, size, date and author by priority.
fn mod_facts(id: &'static str) -> FactLine {
    FactLine::row(id)
        .fact(
            Fact::bare_named(
                t(FactMsg::File),
                PathText::file("sodium-fabric-0.6.0+mc1.21.1.jar")
                    .size(text::VERSION)
                    .color(ink::faint()),
            )
            .shrink()
            .priority(9),
        )
        .fact(
            Fact::bare_named(
                t(FactMsg::Version),
                VersionTag::new("mc1.21.1-0.6.0-fabric"),
            )
            .priority(0),
        )
        .fact(Fact::bare_named(t(FactMsg::Size), Num::bytes(2_480_000)).priority(1))
        .fact(
            Fact::icon(
                Icon::Clock,
                t(FactMsg::Added),
                FactValue::time(format_today("00:54")),
            )
            .priority(2),
        )
        .fact(Fact::icon(Icon::User, t(FactMsg::Author), "jellysquid3").priority(3))
}

/// "2 hours ago" in the current locale.
fn hours_ago(hours: u64) -> String {
    crate::i18n::plural(crate::i18n::FormatPlural::HoursAgo, hours, &[])
}

/// "Today, 00:54" in the current locale.
fn format_today(time: &str) -> String {
    crate::i18n::interpolate(
        t(crate::i18n::FormatMsg::TodayAt),
        &[("time", &time as &dyn std::fmt::Display)],
    )
}

impl Render for ComponentGallery {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = self.narrow;
        let mut sections: Vec<AnyElement> = vec![
            section(
                "Text roles",
                narrow,
                column()
                    .child(
                        row()
                            .child(Num::bytes(35_000_000))
                            .child(Num::fraction_bytes(1_200_000, 3_400_000))
                            .child(Num::count(4_091))
                            .child(Num::fraction(3, 5))
                            .child(MonoValue::new("25565")),
                    )
                    .child(
                        row()
                            .text_color(ink::text())
                            .child(ProductPhrase::new(Product::Minecraft, "26.3"))
                            .child(ProductPhrase::new(Product::Fabric, "0.20.1"))
                            .child(object_name("Survival SMP"))
                            .child(time_text(hours_ago(2))),
                    )
                    .child(
                        div()
                            .flex()
                            .w(px(if narrow { 200. } else { 520. }))
                            .font_family(fonts::mono())
                            .text_size(theme::text_size(text::SMALL))
                            .child(
                                PathText::path(
                                    "/Users/alex/Library/Application Support/Lodestone/instances/old-pack",
                                )
                                .home(Some(HOME.into())),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .w(px(150.))
                            .text_size(theme::text_size(text::SMALL))
                            .child(PathText::file("sodium-fabric-0.6.0+mc1.21.1.jar")),
                    )
                    .child(CaptionLine::path("/Users/alex/Games/servers/survival").into_any_element()),
            ),
            section(
                "Badges",
                narrow,
                column()
                    .child(
                        row()
                            .child(VersionTag::new("0.116.17+1.21.1"))
                            .child(VersionTag::new("mc1.21.1-0.6.0-fabric-build.42+rev"))
                            .child(VersionTag::new("6.1.0").tone(VersionTone::From))
                            .child(VersionTag::new("6.2.0").tone(VersionTone::To)),
                    )
                    .child(
                        row()
                            .child(StatusBadge::new(t(ValueMsg::Online), Tone::Success))
                            .child(StatusBadge::new(t(ValueMsg::Stopped), Tone::Neutral))
                            .child(StatusBadge::new(t(ValueMsg::Downloading), Tone::Info))
                            .child(StatusBadge::new(t(ValueMsg::Paused), Tone::Warning))
                            .child(StatusBadge::new(t(ValueMsg::Failed), Tone::Danger).icon(Icon::TriangleExclamation)),
                    )
                    .child(
                        row()
                            .child(CountBadge::new(7))
                            .child(CountBadge::new(12_345))
                            .child(CountBadge::new(3).active(true))
                            .child(ReasonTag::new(t(ValueMsg::BeforeUpdate)).icon(Icon::ArrowsRotate))
                            .child(ReasonTag::new(t(ValueMsg::CopyOf)))
                            .child(UntitledName::new(t(ValueMsg::UntitledSnapshot)).on_rename(|_, _| {})),
                    )
                    .child(row().children(Kind::ALL.map(KindIcon::new)))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(object_name("Fabric API"))
                            .child(VersionTag::new("0.116.17+1.21.1")),
                    ),
            ),
            section(
                "Objects",
                narrow,
                column()
                    .child(
                        row()
                            .child(ObjectChip::new("Survival").kind(Kind::World))
                            .child(ObjectChip::new("Home Server").kind(Kind::Computer).muted())
                            .child(ObjectChip::new("Survival SMP").kind(Kind::Server).on_click(|_, _, _| {}))
                            .child(ObjectChip::new("fabric-api-0.116.17+1.21.1.jar").kind(Kind::File).mono()),
                    )
                    .child(
                        ObjectCard::new("Survival")
                            .kind(Kind::World)
                            .fact(Fact::new(t(FactMsg::LastPlayed), FactValue::time(format_today("20:53"))))
                            .fact(Fact::bare_named(t(FactMsg::Size), Num::bytes(35_000_000))),
                    )
                    .child(
                        ObjectCard::new(t(ValueMsg::UntitledBackup))
                            .kind(Kind::Backup)
                            .untitled(true)
                            .fact(Fact::new(t(FactMsg::Made), FactValue::time(format_today("14:30"))))
                            .fact(Fact::bare_named(t(FactMsg::Size), Num::bytes(1_200_000_000))),
                    )
                    .child(
                        ObjectCard::new("Fabulously Optimized")
                            .kind(Kind::Modpack)
                            .compact(true)
                            .fact(Fact::bare(DiffValue::versions("6.1.0", "6.2.0"))),
                    ),
            ),
            section(
                "Fact strip",
                narrow,
                FactLine::strip("strip")
                    .fact(Fact::new(t(FactMsg::Game), ProductPhrase::new(Product::Minecraft, "26.3")))
                    .fact(Fact::new(t(FactMsg::Loader), ProductPhrase::new(Product::Fabric, "0.20.1")))
                    .fact(Fact::new(t(FactMsg::PlayTime), format::play_time(120)))
                    .fact(Fact::new(t(FactMsg::LastPlayed), FactValue::time(hours_ago(20))))
                    .fact(Fact::new(
                        t(FactMsg::Server),
                        FactValue::element(
                            row()
                                .gap(px(6.))
                                .child(ObjectChip::new("Survival SMP").kind(Kind::Server))
                                .child(ObjectChip::new("Home Server").kind(Kind::Computer).muted())
                                .child(StatusBadge::new(t(ValueMsg::Online), Tone::Success)),
                            "Survival SMP, Home Server, Online",
                        ),
                    )),
            ),
            section(
                "Fact lines",
                narrow,
                column()
                    .child(mod_facts("facts-wide"))
                    .child(div().w(px(240.)).child(mod_facts("facts-narrow")))
                    .child(
                        div().w(px(240.)).child(
                            FactLine::row("facts-icons")
                                .fact(Fact::icon(Icon::Clock, t(FactMsg::LastPlayed), FactValue::time(hours_ago(2))))
                                .fact(Fact::icon(Icon::Hourglass, t(FactMsg::PlayTime), format::play_time(7_500))),
                        ),
                    ),
            ),
            section(
                "Fact list",
                narrow,
                column()
                    .child(
                        FactList::new("update")
                            .framed(true)
                            .row("Fabulously Optimized", DiffValue::versions("6.1.0", "6.2.0"))
                            .row(t(FactMsg::Game), ProductPhrase::new(Product::Minecraft, "1.21.1"))
                            .row(t(FactMsg::Loader), ProductPhrase::new(Product::Fabric, "0.16.9"))
                            .row(t(FactMsg::Download), Num::bytes(38_000_000)),
                    )
                    .child(
                        FactList::new("restore")
                            .row(t(FactMsg::Replaces), ObjectChip::new("Survival").kind(Kind::World))
                            .row(t(FactMsg::KeptAsideIn), PathText::path("/Users/alex/Games/old/survival").home(Some(HOME.into())))
                            .row(t(FactMsg::Loader), FactValue::text(t(FactMsg::Vanilla))),
                    ),
            ),
            section(
                "Values",
                narrow,
                column()
                    .child(
                        ValueField::new("folder", "/Users/alex/Library/Application Support/Lodestone/instances/old-pack")
                            .label(t(FactMsg::Folder))
                            .home(Some(HOME.into()))
                            .on_reveal(|_, _| {}),
                    )
                    .child(ValueField::new("url", "https://modrinth.com/mod/sodium").label(t(FactMsg::Url)).kind(ValueKind::Url))
                    .child(ValueField::new("exit", "-1073741819").label(t(FactMsg::ExitCode)).kind(ValueKind::Code))
                    .child(
                        ValueField::new("key", "lds_live_9f2c4a7e1b")
                            .label(t(FactMsg::Key))
                            .kind(ValueKind::Secret)
                            .visible_prefix("lds_live_"),
                    ),
            ),
            section(
                "Stats",
                narrow,
                column()
                    .child(
                        StatRow::new()
                            .tile(StatTile::bytes(t(FactMsg::Needed), 2_400_000_000))
                            .tile(StatTile::bytes(t(FactMsg::Free), 1_100_000_000).tone(Tone::Danger))
                            .meter(MeterBar::needed_free(2_400_000_000, 1_100_000_000)),
                    )
                    .child(
                        StatRow::new()
                            .tile(StatTile::bytes(t(FactMsg::FreesUp), 1_400_000_000))
                            .tile(StatTile::bytes(t(FactMsg::Free), 52_000_000_000)),
                    ),
            ),
            section(
                "Progress",
                narrow,
                column()
                    .gap(px(14.))
                    .child(
                        ProgressBlock::new("download", StatusBadge::new(t(ValueMsg::Downloading), Tone::Info), "Fabulously Optimized")
                            .version(VersionTag::new("6.2.0"))
                            .percent(42)
                            .stat(Fact::new(t(FactMsg::Files), Num::fraction(3, 5)))
                            .stat(Fact::new(t(FactMsg::Size), Num::fraction_bytes(1_200_000, 3_400_000))),
                    )
                    .child(
                        ProgressBlock::new("copy", StatusBadge::new(t(ValueMsg::Paused), Tone::Warning), "Survival")
                            .percent(73)
                            .stat(Fact::new(t(FactMsg::Items), Num::fraction(1_234, 4_567)))
                            .stat(Fact::new(t(FactMsg::To), PathText::path("/Volumes/Backup/Lodestone").color(ink::faint()))),
                    ),
            ),
            section(
                "Comparison",
                narrow,
                CompareTable::new("compare", [t(FactMsg::Server).into(), t(FactMsg::ThisInstance).into()])
                    .row(
                        t(FactMsg::Game),
                        ProductPhrase::new(Product::Minecraft, "1.21.1"),
                        ProductPhrase::new(Product::Minecraft, "1.21.1"),
                        false,
                    )
                    .row(
                        t(FactMsg::Loader),
                        ProductPhrase::new(Product::Fabric, "0.16.9"),
                        ProductPhrase::new(Product::Fabric, "0.15.11"),
                        true,
                    ),
            ),
            section(
                "Names",
                narrow,
                column()
                    .gap(px(16.))
                    .child(
                        NameList::new("outcomes")
                            .group(NameGroup::new(t(ValueMsg::Added)).item(NameItem::new("Sodium").icon(ItemIcon::Added)))
                            .group(
                                NameGroup::new(t(ValueMsg::Skipped))
                                    .item(NameItem::new("Lithium").icon(ItemIcon::Skipped).reason("Already installed")),
                            )
                            .group(
                                NameGroup::new(t(ValueMsg::NotAdded))
                                    .item(
                                        NameItem::new("notes.txt")
                                            .icon(ItemIcon::Failed)
                                            .style(NameStyle::Mono)
                                            .reason("Not a mod"),
                                    )
                                    .item(
                                        NameItem::new("sodium-fabric-0.5.8+mc1.20.1.jar")
                                            .icon(ItemIcon::Failed)
                                            .style(NameStyle::Mono)
                                            .reason(tr_spans!(ValueMsg::FromTo, from = Ver("0.5.8"), to = Ver("0.6.2"))),
                                    ),
                            ),
                    )
                    .child(
                        NameList::new("worlds").group(NameGroup::new(t(FactMsg::Worlds)).items(
                            ["Survival", "Creative", "Skyblock", "Parkour", "Hardcore", "Test world", "Old spawn"]
                                .map(|name| NameItem::new(name).icon(ItemIcon::Kind(Kind::World))),
                        )),
                    ),
            ),
            section("List columns", narrow, self.world_list()),
            section(
                "Sentences",
                narrow,
                column()
                    .text_size(theme::text_size(text::MEDIUM))
                    .text_color(ink::muted())
                    .child(tr_spans!(ValueMsg::FromTo, from = Ver("6.1.0"), to = Ver("6.2.0")))
                    .child(tr_spans!(
                        ValueMsg::NeededOfFree,
                        needed = format::bytes_parts(2_400_000_000),
                        free = format::bytes_parts(1_100_000_000)
                    ))
                    .child(tr_spans!(CommonMsg::NoSettingsMatch, query = Mono("JAVA_HOME")))
                    .child(tr_spans!(ValueMsg::KeyValue, key = Obj("Survival"), value = t(ValueMsg::Running))),
            ),
            section(
                "Counts on rows and tabs",
                narrow,
                column()
                    .child(Tabs::new("tabs", [t(FactMsg::Mods), t(FactMsg::Worlds), t(FactMsg::Backups)]).counts([Some(12), Some(3), None]).selected(1))
                    .child(ListRow::new("row-a", t(FactMsg::Worlds)).icon(Icon::Folder).count(3).selected(true))
                    .child(ListRow::new("row-b", t(FactMsg::Backups)).icon(Icon::ClockRotateLeft).count(1_250)),
            ),
        ];
        debug_assert_eq!(sections.len(), SECTIONS);
        let per_page = if narrow { PER_NARROW_PAGE } else { PER_PAGE };
        let start = (self.page - 1) * per_page;
        let sections: Vec<AnyElement> = sections
            .drain(start.min(SECTIONS)..)
            .take(per_page)
            .collect();
        crate::i18n::locale_fonts(
            div()
                .id("gallery")
                .size_full()
                .overflow_y_scroll()
                .bg(colors::GRAY_875)
                .font_family(fonts::body())
                .text_size(theme::text_size(text::MEDIUM))
                .text_color(ink::text())
                .p(px(16.))
                .flex()
                .flex_row()
                .flex_wrap()
                .items_start()
                .content_start()
                .gap(px(16.))
                .children(sections),
        )
    }
}

impl ComponentGallery {
    /// A worlds list with right-aligned Size and Last played columns, which fold
    /// into each row's facts in the narrow gallery.
    fn world_list(&self) -> impl IntoElement + use<> {
        let columns = self.worlds.clone();
        let folded = columns.is_folded();
        let worlds = [
            ("Survival", 35_000_000u64, "20:53", "1.21.1"),
            ("Creative", 1_250_000_000, "09:12", "1.20.4"),
            ("Skyblock with a long name", 4_200_000, "00:54", "1.21.1"),
        ];
        div()
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .child(columns.probe())
            .child(ListHeader::new(&columns).leading(t(FactMsg::World)))
            .children(
                worlds
                    .into_iter()
                    .enumerate()
                    .map(|(index, (name, size, time, version))| {
                        let values = || {
                            vec![
                                FactValue::from(Num::bytes(size)),
                                FactValue::time(format_today(time)),
                            ]
                        };
                        let mut facts = vec![Fact::new(
                            t(FactMsg::Game),
                            ProductPhrase::new(Product::Minecraft, version),
                        )];
                        if folded {
                            facts.extend(columns.fold_into(values()));
                        }
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .min_h(px(44.))
                            .border_b_1()
                            .border_color(ink::border())
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .child(object_name(SharedString::from(name)))
                                    .child(FactLine::row(("world-facts", index)).facts(facts)),
                            )
                            .when(!folded, |this| this.child(columns.cells(values())))
                    }),
            )
    }
}
