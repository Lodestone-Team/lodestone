//! What the apps say about moving a game to another computer as a `.lodeimg` file, so the
//! GPUI Host's "From a file" and the Launcher's "Host this" say it alike and lay it out
//! alike: what a file holds before a server is made from it ([`summary_lines`], drawn by
//! [`SummaryView`], from the core's [`ImageSummary`]), and what the server didn't get once
//! made ([`outcome_view`] and [`outcome_counts`], from the creation's [`CreationOutcome`]).
//! Names and values are never inside the sentences: the game and loader are facts, sizes
//! numbers, and lists of files counted name lists.

use gpui::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, prelude::FluentBuilder as _,
};
use launcher_core::cores::admin::images::{
    CreationOutcome, ImageMaker, ImageSummary, NameList, NotTaken,
};
use launcher_core::cores::admin::instances::NAME_FORBIDDEN;

use crate::components::{
    DiffValue, FactList, FactValue, ItemIcon, Kind, MeterBar, NameGroup, NameItem,
    NameList as Names, NameStyle, Num, ProductPhrase, StatRow, StatTile, StatusBadge, VersionTag,
    readable::key_value,
};
use crate::i18n::{FactMsg, SharingMsg as M, SharingPlural as P, format, t, trn};
use crate::theme::{self, Tone, ink, pill, px, radius, text};

/// A server's name from the file: its label without control and invisible characters,
/// else the file's name without `.lodeimg`, at most 64 characters, with the characters a
/// server's name can't have replaced (an unnamed backup's label has the time, `14:10`).
pub fn default_name(summary: &ImageSummary) -> String {
    let label = summary.label.as_deref().unwrap_or_default();
    let label = without_stamp(&visible(label));
    let name = if label.trim().is_empty() {
        let file = visible(&summary.file_name);
        file.strip_suffix(".lodeimg").unwrap_or(&file).to_string()
    } else {
        label
    };
    server_name(&name)
}

/// An unnamed backup's label without the time the core adds ("Survival – 2026-11-12
/// 14:10 UTC" is "Survival"): the time is the summary's Made fact, in local time, and a
/// stamp in the name reads like a version.
fn without_stamp(label: &str) -> String {
    let Some((name, stamp)) = label.rsplit_once(" – ") else {
        return label.to_string();
    };
    let stamp = stamp.trim();
    let digits =
        |part: &str, len: usize| part.len() == len && part.bytes().all(|b| b.is_ascii_digit());
    let is_stamp = stamp.strip_suffix(" UTC").is_some_and(|stamp| {
        let mut parts = stamp.split([' ', '-', ':', '.']);
        [4, 2, 2, 2, 2]
            .iter()
            .all(|len| parts.next().is_some_and(|part| digits(part, *len)))
            && parts.next().is_none()
    });
    if is_stamp && !name.trim().is_empty() {
        name.to_string()
    } else {
        label.to_string()
    }
}

/// `name` as a server's name the core takes: `:` becomes `.`, the other characters a
/// name can't have `-`, at most 64 characters.
pub fn server_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .map(|c| match c {
            ':' => '.',
            c if NAME_FORBIDDEN.contains(&c) => '-',
            c => c,
        })
        .collect::<String>()
        .trim()
        .chars()
        .take(64)
        .collect()
}

/// `text` without the invisible characters the core writes visibly (`\u{…}`) and without
/// control characters.
fn visible(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("\\u{") {
        out.push_str(&rest[..at]);
        rest = rest[at..].split_once('}').map_or("", |(_, after)| after);
    }
    out.push_str(rest);
    out.chars().filter(|c| !c.is_control()).collect()
}

/// The software the file names when Lodestone makes it as Paper (a Bukkit-family server:
/// Spigot, Purpur…), said only while Paper is the server picked: `(from, "Paper")`.
pub fn made_as(summary: &ImageSummary, picked: Option<&str>) -> Option<(String, String)> {
    let flavour = summary.flavour.as_deref()?;
    let paper = picked.or(summary.server.as_deref()) == Some("Paper");
    let relative = ["spigot", "bukkit", "purpur", "folia"]
        .iter()
        .any(|f| flavour.eq_ignore_ascii_case(f));
    (paper && relative).then(|| (capitalized(flavour), "Paper".to_string()))
}

fn capitalized(text: &str) -> String {
    let mut text = text.to_string();
    if let Some(first) = text.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    text
}

/// The loader Lodestone can't make servers with, when it can't make one from the file.
pub fn unsupported_loader(summary: &ImageSummary) -> Option<String> {
    summary.unsupported.as_ref()?;
    Some(capitalized(&summary.loader))
}

/// Why no server can be made from the file, as a sentence for a log or the control API.
pub fn unsupported(summary: &ImageSummary) -> Option<String> {
    let loader = unsupported_loader(summary)?;
    Some(format!(
        "{} {}",
        t(M::ImageUnsupported),
        key_value(t(FactMsg::Loader), &loader)
    ))
}

/// The world picked when the summary opens: the file's own choice when it names one it
/// has (a Host's level, a Launcher's last played world).
pub fn default_world(summary: &ImageSummary) -> Option<String> {
    summary
        .default_world
        .clone()
        .filter(|w| summary.worlds.iter().any(|world| &world.folder == w))
        .or_else(|| summary.worlds.first().map(|w| w.folder.clone()))
}

/// The server software to ask for: the one picked, only when the file's game allows a
/// choice and it is one of them; otherwise the core makes the file's own.
pub fn chosen_server(summary: &ImageSummary, picked: Option<&str>) -> Option<String> {
    let picked = picked?;
    (summary.server_choices.len() > 1 && summary.server_choices.iter().any(|c| c == picked))
        .then(|| picked.to_string())
}

/// The server software a caller typed (the control API, the CLI) as the file's
/// choices spell it, whatever its case; `None` when it is none of them.
pub fn server_choice<'a>(summary: &'a ImageSummary, typed: &str) -> Option<&'a str> {
    let typed = typed.trim();
    summary
        .server_choices
        .iter()
        .find(|choice| choice.eq_ignore_ascii_case(typed))
        .map(String::as_str)
}

/// Whether a server can be made from the file `summary` describes by someone who may
/// (`may_write_global`) or may not change global files: not when Lodestone makes no server
/// for it, nor when it carries protected files the person may not add, which the core
/// always refuses whatever world is picked. No room for the default world doesn't stop
/// it: a smaller world may fit, and the core refuses clearly when it doesn't.
pub fn can_create(summary: &ImageSummary, may_write_global: bool) -> bool {
    summary.unsupported.is_none() && (may_write_global || !summary.needs_global_files)
}

/// How bad an alert is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Nothing can be made as things stand.
    Error,
    /// Worth reading before making it.
    Warning,
}

/// What the person has picked so far, which some lines follow.
#[derive(Clone, Copy, Debug, Default)]
pub struct Picked<'a> {
    /// The world (its folder) that becomes the server's; `None` for none.
    pub world: Option<&'a str>,
    /// The server software, one of the summary's choices; `None` for the file's own.
    pub server: Option<&'a str>,
    /// The file's operators are kept (their tickbox then says so itself).
    pub keep_operators: bool,
    /// The person may change global files (what a file's jars need).
    pub may_write_global: bool,
    /// The file is the person's own game, sent from this computer (the Launcher's "Host
    /// this"): who made it and the warning to trust its sender say nothing then.
    pub own_file: bool,
}

/// A fact about the file: the server it makes, then the file itself.
#[derive(Clone, Debug, PartialEq)]
pub enum Note {
    /// The game and the loader (the server), and the file's files and size.
    Game {
        game: String,
        /// The loader's name and version; `None` for vanilla.
        loader: Option<(String, Option<String>)>,
        files: u64,
        bytes: u64,
    },
    /// Who made it: the app and its version, or another program.
    MadeWith {
        app: Option<&'static str>,
        version: String,
    },
    /// When the backup or snapshot was made, in local time.
    Made(String),
    /// The software it names, made as another (`Purpur` made as `Paper`).
    MadeAs { from: String, to: String },
    /// The pack it is built on, as the core describes it.
    Base(String),
}

/// Something to read before making the server.
#[derive(Clone, Debug, PartialEq)]
pub enum Alert {
    /// Lodestone makes no servers with this loader.
    Unsupported { loader: String },
    /// It has mods or plugins, and maybe files of its own (`carried`).
    Trust { count: u64, carried: NameList },
    /// No mods, but protected files of its own.
    Protected,
    /// CurseForge files the servers can't download.
    NotDownloadable(NameList),
    /// Players can join without a Minecraft account.
    OfflineMode,
    /// It needs permission to change global files.
    NeedsGlobal,
    /// No room for the world picked.
    NoRoom,
}

/// What is downloaded, left out, and the disk it needs.
#[derive(Clone, Debug, PartialEq)]
pub enum Detail {
    /// What is downloaded from Modrinth, and the mods among it.
    Downloads { bytes: u64, mods: NameList },
    /// Names left out, under a heading.
    Left { heading: M, names: NameList },
    /// What the server never takes from a file.
    NotTaken(Vec<NotTaken>),
    /// The space needed (with the spare) against the space free.
    Disk { needed: u64, free: u64 },
}

/// A file's summary: facts under the title, alerts to read before creating, and details.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SummaryLines {
    pub notes: Vec<Note>,
    pub alerts: Vec<(Level, Alert)>,
    pub details: Vec<Detail>,
}

/// What `summary` says, with `picked` chosen.
pub fn summary_lines(summary: &ImageSummary, picked: Picked<'_>) -> SummaryLines {
    let mut lines = SummaryLines::default();
    let server = picked
        .server
        .map(str::to_string)
        .or_else(|| summary.server.clone())
        .or_else(|| summary.flavour.clone())
        .unwrap_or_else(|| summary.loader.clone());
    let loader = match summary.loader.as_str() {
        "vanilla" | "" => {
            (!server.eq_ignore_ascii_case("vanilla")).then(|| (capitalized(&server), None))
        }
        _ => Some((capitalized(&server), summary.loader_version.clone())),
    };
    lines.notes.push(Note::Game {
        game: summary.minecraft.clone(),
        loader,
        files: summary.files,
        bytes: summary.total_bytes,
    });
    if !picked.own_file {
        lines.notes.push(Note::MadeWith {
            app: match summary.made_by {
                ImageMaker::Host => Some("Lodestone Host"),
                ImageMaker::Launcher => Some("Lodestone Launcher"),
                ImageMaker::Unknown => None,
            },
            version: summary.producer_version.clone(),
        });
    }
    if summary.created > 0
        && let Some(time) = chrono::DateTime::from_timestamp(summary.created, 0)
    {
        lines.notes.push(Note::Made(format::day_and_time(
            time.with_timezone(&chrono::Local),
            chrono::Local::now().date_naive(),
        )));
    }
    if let Some((from, to)) = made_as(summary, picked.server) {
        lines.notes.push(Note::MadeAs { from, to });
    }
    if let Some(base) = &summary.base {
        lines.notes.push(Note::Base(
            launcher_core::images::display_text(base).into_owned(),
        ));
    }

    if let Some(loader) = unsupported_loader(summary) {
        lines
            .alerts
            .push((Level::Error, Alert::Unsupported { loader }));
    }
    if !picked.own_file {
        let count = summary.server_mods.max(summary.carried_mods.count);
        if count > 0 {
            lines.alerts.push((
                Level::Warning,
                Alert::Trust {
                    count,
                    carried: summary.carried_mods.clone(),
                },
            ));
        } else if summary.needs_global_files {
            lines.alerts.push((Level::Warning, Alert::Protected));
        }
    }
    if !summary.not_downloadable.is_empty() {
        lines.alerts.push((
            Level::Warning,
            Alert::NotDownloadable(summary.not_downloadable.clone()),
        ));
    }
    if summary.offline_mode {
        lines.alerts.push((Level::Warning, Alert::OfflineMode));
    }
    if summary.needs_global_files && !picked.may_write_global {
        lines.alerts.push((Level::Error, Alert::NeedsGlobal));
    }

    if summary.download_bytes > 0 {
        lines.details.push(Detail::Downloads {
            bytes: summary.download_bytes,
            mods: summary.mods_from_modrinth.clone(),
        });
    }
    for (list, heading) in [
        (&summary.client_only, M::ImageClientOnly),
        (&summary.left_out, M::ImageTakenOut),
        (&summary.withheld, M::ImageWithheld),
        (&summary.blanked, M::ImageBlanked),
        (&summary.secrets_not_taken, M::ImageSecretsNotTaken),
    ] {
        if !list.is_empty() {
            lines.details.push(Detail::Left {
                heading,
                names: list.clone(),
            });
        }
    }
    let not_taken: Vec<NotTaken> = summary
        .not_taken
        .iter()
        .filter(|n| **n != NotTaken::Unknown)
        .filter(|n| !(picked.keep_operators && **n == NotTaken::Operators))
        .copied()
        .collect();
    if !not_taken.is_empty() {
        lines.details.push(Detail::NotTaken(not_taken));
    }
    // for the world picked: the core checks again
    if let Some(free) = summary.disk_free {
        lines.details.push(Detail::Disk {
            needed: summary
                .needed_bytes_for(picked.world)
                .saturating_add(ImageSummary::SPARE_BYTES),
            free,
        });
    }
    if !summary.has_room_for(picked.world) {
        lines.alerts.push((Level::Warning, Alert::NoRoom));
    }
    lines
}

/// A not-taken item's name, and why, when it says.
fn not_taken_item(what: NotTaken) -> (&'static str, Option<&'static str>) {
    match what {
        NotTaken::ServerSoftware => (t(M::NotTakenSoftware), Some(t(M::NotTakenSoftwareReason))),
        NotTaken::Eula => (t(M::NotTakenEula), None),
        NotTaken::ClientFiles => (t(M::NotTakenClientFiles), None),
        NotTaken::Operators => (t(M::NotTakenOperators), Some(t(M::NotTakenOperatorsReason))),
        NotTaken::Scripts | NotTaken::Unknown => (t(M::NotTakenScripts), None),
    }
}

/// The names of a core's list, shown safely (hidden characters written out).
fn shown_names(names: &NameList) -> Vec<String> {
    names
        .names
        .iter()
        .map(|name| launcher_core::images::display_text(name).into_owned())
        .collect()
}

/// A group of exact names under `heading`, counted with the core's total.
fn name_group(heading: impl Into<SharedString>, names: &NameList, icon: ItemIcon) -> NameGroup {
    let shown = shown_names(names);
    let more = names.count.saturating_sub(shown.len() as u64);
    let mut group = NameGroup::new(heading).items(shown.into_iter().map(|name| {
        NameItem::new(name)
            .icon(icon.clone())
            .style(NameStyle::Path)
    }));
    // names the core counted but didn't send: counted, never an uncounted "…"
    if more > 0 {
        group = group.item(
            NameItem::new(trn!(P::ImageMoreNames, more))
                .icon(ItemIcon::Skipped)
                .style(NameStyle::Name),
        );
    }
    group
}

impl Note {
    /// The note as text, for tests and logs.
    pub fn plain(&self) -> String {
        match self {
            Note::Game {
                game,
                loader,
                files,
                bytes,
            } => {
                let mut parts = vec![crate::values::phrase(
                    crate::values::Product::Minecraft,
                    game,
                )];
                if let Some((name, version)) = loader {
                    parts.push(
                        format!("{name} {}", version.clone().unwrap_or_default())
                            .trim()
                            .to_string(),
                    );
                }
                parts.push(key_value(t(M::ImageFiles), &format::number(*files)).to_string());
                parts.push(key_value(t(M::ImageSize), &format::bytes(*bytes)).to_string());
                parts.join("\n")
            }
            Note::MadeWith { app, version } => match app {
                Some(app) => {
                    key_value(t(M::ImageMadeWith), &format!("{app} {version}")).to_string()
                }
                None => t(M::ImageMadeByOther).to_string(),
            },
            Note::Made(time) => key_value(t(FactMsg::Made), time).to_string(),
            Note::MadeAs { from, to } => key_value(
                t(M::ImageServerSoftware),
                &DiffValue::text(from.clone(), to.clone()).plain(),
            )
            .to_string(),
            Note::Base(base) => key_value(t(M::ImageBasedOn), base).to_string(),
        }
    }
}

impl Alert {
    /// The alert's sentence, for tests and logs (its lists are left out).
    pub fn plain(&self) -> String {
        match self {
            Alert::Unsupported { loader } => {
                format!(
                    "{} {}",
                    t(M::ImageUnsupported),
                    key_value(t(FactMsg::Loader), loader)
                )
            }
            Alert::Trust { count, .. } => trn!(P::ImageModsWarning, *count),
            Alert::Protected => t(M::ImageProtectedWarning).to_string(),
            Alert::NotDownloadable(_) => t(M::ImageNotDownloadable).to_string(),
            Alert::OfflineMode => t(M::ImageOfflineMode).to_string(),
            Alert::NeedsGlobal => t(M::ImageNeedsGlobal).to_string(),
            Alert::NoRoom => t(M::ImageNoRoom).to_string(),
        }
    }

    fn body(&self, id: &str) -> AnyElement {
        let sentence = div().child(self.plain());
        let column = div().flex().flex_col().gap(px(8.)).child(sentence);
        match self {
            Alert::Unsupported { loader } => column
                .child(
                    FactList::new(format!("{id}-loader")).row(t(FactMsg::Loader), loader.clone()),
                )
                .into_any_element(),
            Alert::Trust { carried, .. } if !carried.is_empty() => column
                .child(
                    Names::new(format!("{id}-carried"))
                        .group(name_group(
                            t(M::ImageCarried),
                            carried,
                            ItemIcon::Kind(Kind::Mod),
                        ))
                        .collapsed_after(5),
                )
                .into_any_element(),
            Alert::NotDownloadable(names) => column
                .child(
                    Names::new(format!("{id}-curseforge"))
                        .group(name_group(
                            t(M::ImageNotInstalled),
                            names,
                            ItemIcon::Kind(Kind::File),
                        ))
                        .collapsed_after(5),
                )
                .into_any_element(),
            // the setting itself, exact, and its state
            Alert::OfflineMode => column
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .child(crate::components::MonoValue::new("online-mode"))
                        .child(StatusBadge::new(t(M::ImageOff), Tone::Neutral)),
                )
                .into_any_element(),
            _ => column.into_any_element(),
        }
    }
}

impl Detail {
    /// The detail as text, for tests and logs.
    pub fn plain(&self) -> String {
        match self {
            Detail::Downloads { bytes, mods } => {
                let mut text = key_value(t(M::ImageDownloads), &format::bytes(*bytes)).to_string();
                for name in shown_names(mods) {
                    text.push('\n');
                    text.push_str(&name);
                }
                text
            }
            Detail::Left { heading, names } => {
                let mut text = t(*heading).to_string();
                for name in shown_names(names) {
                    text.push('\n');
                    text.push_str(&name);
                }
                text
            }
            Detail::NotTaken(what) => {
                let mut text = t(M::ImageNotTaken).to_string();
                for item in what {
                    let (name, reason) = not_taken_item(*item);
                    text.push('\n');
                    text.push_str(&match reason {
                        Some(reason) => key_value(name, reason).to_string(),
                        None => name.to_string(),
                    });
                }
                text
            }
            Detail::Disk { needed, free } => format!(
                "{}\n{}",
                key_value(t(FactMsg::Needed), &format::bytes(*needed)),
                key_value(t(FactMsg::Free), &format::bytes(*free))
            ),
        }
    }
}

/// A file's summary, laid out the same in both apps: the facts as a table, the alerts in
/// boxes of their tone, then what is downloaded, the names left out as one counted list,
/// and the disk space as two numbers over a meter.
#[derive(IntoElement)]
pub struct SummaryView {
    id: SharedString,
    lines: SummaryLines,
}

impl SummaryView {
    pub fn new(id: impl Into<SharedString>, lines: SummaryLines) -> Self {
        Self {
            id: id.into(),
            lines,
        }
    }
}

impl RenderOnce for SummaryView {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let id = self.id;
        // the facts: the server, then the file
        let mut facts = FactList::new(ElementId::Name(format!("{id}-facts").into()));
        for note in &self.lines.notes {
            match note {
                Note::Game {
                    game,
                    loader,
                    files,
                    bytes,
                } => {
                    let game = ProductPhrase::new(crate::values::Product::Minecraft, game.clone());
                    facts = facts.row(game.key(), game);
                    if let Some((name, version)) = loader {
                        let version = version.clone().unwrap_or_default();
                        facts = match ProductPhrase::from_name(name, version.clone()) {
                            Some(phrase) => facts.row(phrase.key(), phrase),
                            None => facts.row(
                                t(FactMsg::Loader),
                                format!("{name}\u{a0}{version}").trim().to_string(),
                            ),
                        };
                    }
                    facts = facts
                        .row(t(M::ImageFiles), Num::count(*files))
                        .row(t(M::ImageSize), Num::bytes(*bytes));
                }
                Note::MadeWith { app, version } => {
                    facts = match app {
                        Some(app) => facts.row(
                            t(M::ImageMadeWith),
                            FactValue::element(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(app.to_string())
                                    .when(!version.is_empty(), |this| {
                                        this.child(VersionTag::new(version.clone()))
                                    }),
                                format!("{app} {version}"),
                            ),
                        ),
                        None => facts.row(t(M::ImageMadeWith), t(M::ImageMadeByOther)),
                    };
                }
                Note::Made(time) => {
                    facts = facts.row(t(FactMsg::Made), FactValue::time(time.clone()))
                }
                Note::MadeAs { from, to } => {
                    facts = facts.row(
                        t(M::ImageServerSoftware),
                        DiffValue::text(from.clone(), to.clone()),
                    );
                }
                Note::Base(base) => facts = facts.row(t(M::ImageBasedOn), base.clone()),
            }
        }
        let made_as = self
            .lines
            .notes
            .iter()
            .any(|note| matches!(note, Note::MadeAs { .. }));
        let alerts = self
            .lines
            .alerts
            .iter()
            .enumerate()
            .map(|(index, (level, alert))| {
                let tone = match level {
                    Level::Error => Tone::Danger,
                    Level::Warning => Tone::Warning,
                };
                div()
                    .p(px(10.))
                    .rounded(radius::MD)
                    .bg(pill::bg(tone))
                    .text_size(theme::text_size(text::MEDIUM))
                    .text_color(ink::text())
                    .child(alert.body(&format!("{id}-alert-{index}")))
            });
        // what is downloaded, the names left out, what isn't taken, the disk
        let mut downloads = None;
        let mut names =
            Names::new(ElementId::Name(format!("{id}-names").into())).collapsed_after(5);
        let mut has_names = false;
        let mut disk = None;
        for detail in &self.lines.details {
            match detail {
                Detail::Downloads { bytes, mods } => {
                    downloads = Some(
                        FactList::new(ElementId::Name(format!("{id}-downloads").into()))
                            .row(t(M::ImageDownloads), Num::bytes(*bytes)),
                    );
                    if !mods.is_empty() {
                        names = names.group(name_group(
                            t(M::ImageDownloadedMods),
                            mods,
                            ItemIcon::Kind(Kind::Mod),
                        ));
                        has_names = true;
                    }
                }
                Detail::Left {
                    heading,
                    names: list,
                } => {
                    names = names.group(name_group(t(*heading), list, ItemIcon::Kind(Kind::File)));
                    has_names = true;
                }
                Detail::NotTaken(what) => {
                    names = names.group(NameGroup::new(t(M::ImageNotTaken)).items(
                        what.iter().map(|item| {
                            let (name, reason) = not_taken_item(*item);
                            let item = NameItem::new(name).icon(ItemIcon::Skipped);
                            match reason {
                                Some(reason) => item.reason(reason),
                                None => item,
                            }
                        }),
                    ));
                    has_names = true;
                }
                Detail::Disk { needed, free } => {
                    let short = needed > free;
                    disk =
                        Some(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    StatRow::new()
                                        .tile(StatTile::bytes(t(FactMsg::Needed), *needed).tone(
                                            if short { Tone::Warning } else { Tone::Neutral },
                                        ))
                                        .tile(StatTile::bytes(t(FactMsg::Free), *free))
                                        .meter(MeterBar::needed_free(*needed, *free)),
                                )
                                .child(
                                    div()
                                        .text_size(theme::text_size(text::SMALL))
                                        .text_color(ink::muted())
                                        .child(t(M::ImageDiskSpare)),
                                ),
                        );
                }
            }
        }
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(facts)
            .when(made_as, |this| {
                this.child(
                    div()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(ink::muted())
                        .child(t(M::ImageMadeAsPaper)),
                )
            })
            .children(alerts)
            .children(downloads)
            .when(has_names, |this| this.child(names))
            .children(disk)
    }
}

/// What a server made from a file didn't get, as counted groups of names (empty when it
/// got everything it could take): one group a reason.
pub fn outcome_view(id: impl Into<ElementId>, outcome: &CreationOutcome) -> Names {
    let mut names = Names::new(id).collapsed_after(5);
    for (list, message) in outcome_groups(outcome) {
        names = names.group(name_group(
            trn!(message, list.count),
            list,
            ItemIcon::Kind(Kind::File),
        ));
    }
    if outcome.scripts > 0 {
        names = names.group(NameGroup::new(trn!(P::OutcomeScripts, outcome.scripts)));
    }
    names
}

/// What a server made from a file didn't get, a counted line per reason, with no names:
/// for a toast, whose details the server's own pages show.
pub fn outcome_counts(outcome: &CreationOutcome) -> Vec<String> {
    let mut lines: Vec<String> = outcome_groups(outcome)
        .into_iter()
        .map(|(list, message)| trn!(message, list.count))
        .collect();
    if outcome.scripts > 0 {
        lines.push(trn!(P::OutcomeScripts, outcome.scripts));
    }
    lines
}

fn outcome_groups(outcome: &CreationOutcome) -> Vec<(&NameList, P)> {
    [
        (&outcome.not_downloaded, P::OutcomeNotDownloaded),
        (&outcome.no_source, P::OutcomeNoSource),
        (&outcome.client_jars, P::OutcomeClientJars),
        (&outcome.secrets, P::OutcomeSecrets),
        (&outcome.settings_removed, P::OutcomeSettingsRemoved),
    ]
    .into_iter()
    .filter(|(list, _)| !list.is_empty())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::cores::admin::images::ImageWorld;

    fn summary(json: serde_json::Value) -> ImageSummary {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn a_server_is_named_after_the_file() {
        assert_eq!(
            default_name(&summary(serde_json::json!({
                "label": "Survival \\u{202E}SMP", "file_name": "x.lodeimg"
            }))),
            "Survival SMP"
        );
        assert_eq!(
            default_name(&summary(serde_json::json!({
                "label": "  ", "file_name": "Island.lodeimg"
            }))),
            "Island"
        );
        let long = "a".repeat(100);
        assert_eq!(
            default_name(&summary(serde_json::json!({ "label": long })))
                .chars()
                .count(),
            64
        );
        // an unnamed backup's label has the time, and a name may have any character: the
        // name made is one the core takes
        let name = default_name(&summary(serde_json::json!({
            "label": "Survival – 2026-11-12 14:10 UTC"
        })));
        assert_eq!(name, "Survival", "the time is a fact, not part of the name");
        let name = default_name(&summary(serde_json::json!({
            "label": "Before 1/2 \"big\" update? <a|b> *\\"
        })));
        assert!(!name.contains(NAME_FORBIDDEN), "{name}");
        assert_eq!(server_name("My: world/1"), "My. world-1");
    }

    #[test]
    fn the_files_own_world_is_picked() {
        let world = |folder: &str| ImageWorld {
            folder: folder.into(),
            ..Default::default()
        };
        let mut s = ImageSummary {
            worlds: vec![world("A"), world("B")],
            default_world: Some("B".into()),
            ..Default::default()
        };
        assert_eq!(default_world(&s).as_deref(), Some("B"));
        s.default_world = Some("gone".into());
        assert_eq!(default_world(&s).as_deref(), Some("A"));
        s.worlds.clear();
        assert_eq!(default_world(&s), None);
    }

    #[test]
    fn server_software_is_asked_for_only_when_there_is_a_choice() {
        let vanilla = summary(serde_json::json!({
            "server": "Vanilla", "server_choices": ["Vanilla", "Paper"]
        }));
        assert_eq!(
            chosen_server(&vanilla, Some("Paper")).as_deref(),
            Some("Paper")
        );
        assert_eq!(chosen_server(&vanilla, Some("Fabric")), None);
        assert_eq!(chosen_server(&vanilla, None), None);
        let fabric = summary(serde_json::json!({
            "server": "Fabric", "server_choices": ["Fabric"]
        }));
        assert_eq!(chosen_server(&fabric, Some("Fabric")), None);
    }

    #[test]
    fn a_typed_server_software_is_matched_whatever_its_case() {
        let vanilla = summary(serde_json::json!({
            "server": "Vanilla", "server_choices": ["Vanilla", "Paper"]
        }));
        assert_eq!(server_choice(&vanilla, "paper"), Some("Paper"));
        assert_eq!(server_choice(&vanilla, " VANILLA "), Some("Vanilla"));
        assert_eq!(server_choice(&vanilla, "Spigot"), None);
        let none = summary(serde_json::json!({ "server_choices": [] }));
        assert_eq!(server_choice(&none, "Paper"), None);
    }

    #[test]
    fn notes_on_the_server_are_the_apps_own_words() {
        crate::i18n::force(crate::i18n::Locale::En);
        let purpur = summary(serde_json::json!({
            "loader": "vanilla", "flavour": "purpur", "server": "Paper",
            "server_choices": ["Vanilla", "Paper"],
            "server_note": "the core's English"
        }));
        assert_eq!(
            made_as(&purpur, None),
            Some(("Purpur".to_string(), "Paper".to_string()))
        );
        assert_eq!(
            made_as(&purpur, Some("Vanilla")),
            None,
            "not once Vanilla is picked"
        );
        let paper = summary(serde_json::json!({ "flavour": "paper", "server": "Paper" }));
        assert_eq!(made_as(&paper, None), None);
        let quilt = summary(serde_json::json!({
            "loader": "quilt", "unsupported": "Lodestone can't make Quilt servers"
        }));
        assert_eq!(unsupported_loader(&quilt).as_deref(), Some("Quilt"));
        assert_eq!(unsupported_loader(&paper), None);
    }

    #[test]
    fn mods_from_anywhere_get_the_trust_warning() {
        crate::i18n::force(crate::i18n::Locale::En);
        let alerts = |json| summary_lines(&summary(json), Picked::default()).alerts;
        let downloaded = alerts(serde_json::json!({
            "server_mods": 40,
            "mods_from_modrinth": {"names": ["lithium.jar"], "count": 40}
        }));
        assert!(
            matches!(&downloaded[0], (Level::Warning, Alert::Trust { count: 40, carried }) if carried.is_empty())
        );
        assert_eq!(
            downloaded[0].1.plain(),
            "It has 40 mods or plugins: programs that run on the server. Only use files from people you trust."
        );
        let carried = alerts(serde_json::json!({
            "server_mods": 1, "carried_mods": {"names": ["own.jar"], "count": 1}
        }));
        assert!(
            matches!(&carried[0].1, Alert::Trust { count: 1, carried } if carried.names == ["own.jar"])
        );
        assert!(alerts(serde_json::json!({})).is_empty());
        // no mods, but protected files of its own: warned too
        let programs = alerts(serde_json::json!({ "needs_global_files": true }));
        assert!(programs.contains(&(Level::Warning, Alert::Protected)));
    }

    #[test]
    fn a_file_the_core_would_refuse_cant_be_created() {
        let fine = summary(serde_json::json!({ "server": "Fabric" }));
        assert!(can_create(&fine, false));
        let jars = summary(serde_json::json!({ "server": "Fabric", "needs_global_files": true }));
        assert!(can_create(&jars, true));
        assert!(!can_create(&jars, false));
        let quilt = summary(serde_json::json!({ "unsupported": "Quilt" }));
        assert!(!can_create(&quilt, true));
    }

    #[test]
    fn the_game_and_loader_are_facts() {
        crate::i18n::force(crate::i18n::Locale::En);
        let notes = |json| {
            summary_lines(
                &summary(json),
                Picked {
                    own_file: true,
                    ..Default::default()
                },
            )
            .notes
        };
        let fabric = notes(serde_json::json!({
            "minecraft": "1.21.1", "loader": "fabric", "loader_version": "0.16.10",
            "server": "Fabric", "files": 3, "total_bytes": 2048
        }));
        assert_eq!(
            fabric[0],
            Note::Game {
                game: "1.21.1".into(),
                loader: Some(("Fabric".into(), Some("0.16.10".into()))),
                files: 3,
                bytes: 2048
            }
        );
        let paper = notes(serde_json::json!({
            "minecraft": "1.21.3", "loader": "vanilla", "flavour": "purpur", "server": "Paper"
        }));
        assert!(
            matches!(&paper[0], Note::Game { loader: Some((name, None)), .. } if name == "Paper")
        );
        let vanilla = notes(serde_json::json!({
            "minecraft": "1.21.3", "loader": "vanilla", "server": "Vanilla"
        }));
        assert!(matches!(&vanilla[0], Note::Game { loader: None, .. }));
    }

    #[test]
    fn the_summary_follows_what_is_picked() {
        crate::i18n::force(crate::i18n::Locale::En);
        let file = summary(serde_json::json!({
            "made_by": "launcher", "producer_version": "1.4.0", "minecraft": "1.21.1",
            "loader": "fabric", "loader_version": "0.16.10", "server": "Fabric",
            "files": 3, "total_bytes": 2048,
            "client_only": {"names": ["sodium.jar"], "count": 1},
            "not_downloadable": {"names": ["mods/jade.jar"], "count": 1},
            "not_taken": ["client_files", "operators"], "needs_global_files": true,
            "worlds": [{"folder": "Big", "size": 20000}, {"folder": "Small", "size": 5}],
            "default_world": "Big", "needed_bytes": 50000,
            "disk_free": (1u64 << 30) + 20000
        }));
        let lines = summary_lines(
            &file,
            Picked {
                world: Some("Big"),
                ..Default::default()
            },
        );
        assert!(matches!(
            &lines.notes[0],
            Note::Game {
                files: 3,
                bytes: 2048,
                ..
            }
        ));
        assert_eq!(
            lines.notes[1],
            Note::MadeWith {
                app: Some("Lodestone Launcher"),
                version: "1.4.0".into()
            }
        );
        assert!(lines.alerts.contains(&(Level::Error, Alert::NeedsGlobal)));
        assert!(lines.alerts.contains(&(Level::Warning, Alert::NoRoom)));
        assert!(
            lines
                .details
                .iter()
                .any(|d| d.plain() == "Mods only players run, left out\nsodium.jar")
        );
        assert!(lines.details.contains(&Detail::NotTaken(vec![
            NotTaken::ClientFiles,
            NotTaken::Operators
        ])));
        // a smaller world fits, someone who may change global files may create, and kept
        // operators aren't "not taken"
        let lines = summary_lines(
            &file,
            Picked {
                world: Some("Small"),
                keep_operators: true,
                may_write_global: true,
                ..Default::default()
            },
        );
        assert!(!lines.alerts.iter().any(|(level, _)| *level == Level::Error));
        assert!(!lines.alerts.contains(&(Level::Warning, Alert::NoRoom)));
        assert!(
            lines
                .details
                .contains(&Detail::NotTaken(vec![NotTaken::ClientFiles]))
        );
        // the person's own game: no maker, no warning to trust the sender
        let own = summary_lines(
            &summary(serde_json::json!({
                "made_by": "launcher", "producer_version": "1.4.0", "server_mods": 3
            })),
            Picked {
                own_file: true,
                may_write_global: true,
                ..Default::default()
            },
        );
        assert_eq!(own.notes.len(), 1, "{own:?}");
        assert!(own.alerts.is_empty(), "{own:?}");
    }

    #[test]
    fn the_outcome_says_what_was_left_out() {
        crate::i18n::force(crate::i18n::Locale::En);
        let outcome: CreationOutcome = serde_json::from_value(serde_json::json!({
            "not_downloaded": {"names": ["mods/a.jar"], "count": 1},
            "no_source": {"names": ["mods/cf.jar", "mods/cf2.jar"], "count": 70},
            "scripts": 2
        }))
        .unwrap();
        assert_eq!(
            outcome_counts(&outcome),
            [
                "1 file couldn’t be downloaded and was left out",
                "70 files can’t be downloaded by Lodestone servers and were left out",
                "2 scripts were left out: only the owner can add scripts, while code execution is on.",
            ]
        );
        assert!(outcome_counts(&CreationOutcome::default()).is_empty());
    }

    #[test]
    fn an_unnamed_backups_time_leaves_the_name() {
        assert_eq!(without_stamp("Survival – 2026-11-12 14:10 UTC"), "Survival");
        assert_eq!(without_stamp("Survival – 2026-11-12 14.10 UTC"), "Survival");
        assert_eq!(without_stamp("Before – the update"), "Before – the update");
        assert_eq!(
            without_stamp("– 2026-11-12 14:10 UTC"),
            "– 2026-11-12 14:10 UTC"
        );
    }
}
