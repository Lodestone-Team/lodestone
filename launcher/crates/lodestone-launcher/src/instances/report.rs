//! A notice with its names and values set apart: what it is about in a card, its
//! values as keyed facts (a path to reveal, a version as a tag), a short sentence,
//! and lists of names (mods left behind, files left out) counted, never run into a
//! sentence. [`InstancesEvent::Report`](super::InstancesEvent::Report) carries one;
//! the window shows it as a dialog, and the control API reads its plain text.

use std::path::PathBuf;

use gpui::{AnyElement, IntoElement, SharedString, div, prelude::*};
use launcher_core::instances::Instance;

use super::About;
use crate::{
    components::{
        FactList, FactValue, ItemIcon, Kind, NameGroup, NameItem, NameList, NameStyle, Num,
        ObjectChip, ValueField, VersionTag, readable::key_value,
    },
    i18n::format,
};

/// A value in a report.
#[derive(Clone, Debug, PartialEq)]
pub enum ReportValue {
    Text(String),
    /// A name, with the kind of thing it names.
    Object(String, Kind),
    /// A mod's or modpack's version.
    Version(String),
    Count(u64),
    /// A file or folder, shown in a field to copy and reveal.
    Path(PathBuf),
}

impl ReportValue {
    fn plain(&self) -> String {
        match self {
            ReportValue::Text(text) | ReportValue::Object(text, _) | ReportValue::Version(text) => {
                text.clone()
            }
            ReportValue::Count(count) => format::number(*count),
            ReportValue::Path(path) => path.display().to_string(),
        }
    }
}

/// Names under a heading: mods, or files (in mono).
#[derive(Clone, Debug, PartialEq)]
pub struct ReportGroup {
    pub heading: SharedString,
    pub names: Vec<String>,
    pub kind: Kind,
    pub mono: bool,
}

/// See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// The action and the kind only ("Modpack updated"), never a name.
    pub title: SharedString,
    /// The instance it is about, shown in a card with its game and loader.
    pub instance: Option<Instance>,
    pub facts: Vec<(SharedString, ReportValue)>,
    /// A short sentence that names nothing.
    pub body: Option<SharedString>,
    /// Lists of names under a heading each.
    pub groups: Vec<ReportGroup>,
    pub about: About,
}

impl Report {
    pub fn new(title: impl Into<SharedString>, about: About) -> Self {
        Self {
            title: title.into(),
            instance: None,
            facts: Vec::new(),
            body: None,
            groups: Vec::new(),
            about,
        }
    }

    pub fn instance(mut self, instance: Instance) -> Self {
        self.instance = Some(instance);
        self
    }

    pub fn fact(mut self, key: impl Into<SharedString>, value: ReportValue) -> Self {
        self.facts.push((key.into(), value));
        self
    }

    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Mods under `heading`; an empty list is left out.
    pub fn mods(self, heading: impl Into<SharedString>, names: Vec<String>) -> Self {
        self.group(heading, names, Kind::Mod, false)
    }

    /// File names under `heading`, in mono; an empty list is left out.
    pub fn files(self, heading: impl Into<SharedString>, names: Vec<String>) -> Self {
        self.group(heading, names, Kind::File, true)
    }

    fn group(
        mut self,
        heading: impl Into<SharedString>,
        names: Vec<String>,
        kind: Kind,
        mono: bool,
    ) -> Self {
        if !names.is_empty() {
            self.groups.push(ReportGroup {
                heading: heading.into(),
                names,
                kind,
                mono,
            });
        }
        self
    }

    /// Everything it says as text, for the control API and logs: the instance, the
    /// facts as "key: value", the sentence and each list.
    pub fn plain(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(instance) = &self.instance {
            parts.push(instance.name.clone());
        }
        parts.extend(
            self.facts
                .iter()
                .map(|(key, value)| key_value(key, &value.plain()).to_string()),
        );
        parts.extend(self.body.iter().map(ToString::to_string));
        parts.extend(
            self.groups
                .iter()
                .map(|group| key_value(&group.heading, &format::list(&group.names)).to_string()),
        );
        parts.join("\n")
    }

    /// The dialog's content, under its title.
    pub fn render(&self, instances: &super::Instances) -> AnyElement {
        let mut list = FactList::new("report-facts");
        let mut has_list = false;
        let mut fields = Vec::new();
        for (index, (key, value)) in self.facts.iter().enumerate() {
            let value: FactValue = match value {
                ReportValue::Path(path) => {
                    let path = path.clone();
                    fields.push(
                        ValueField::new(("report-path", index), path.display().to_string())
                            .label(key.clone())
                            .on_reveal(move |_, cx| cx.reveal_path(&path)),
                    );
                    continue;
                }
                ReportValue::Text(text) => FactValue::text(text.clone()),
                ReportValue::Object(name, kind) => ObjectChip::new(name.clone()).kind(*kind).into(),
                ReportValue::Version(version) => VersionTag::new(version.clone()).into(),
                ReportValue::Count(count) => Num::count(*count).into(),
            };
            list = list.row(key.clone(), value);
            has_list = true;
        }
        let names = (!self.groups.is_empty()).then(|| {
            self.groups.iter().fold(
                NameList::new("report-names").collapsed_after(5),
                |names, group| {
                    names.group(NameGroup::new(group.heading.clone()).items(
                        group.names.iter().map(|name| {
                            NameItem::new(name.clone())
                                .icon(ItemIcon::Kind(group.kind))
                                .style(if group.mono {
                                    NameStyle::Mono
                                } else {
                                    NameStyle::Name
                                })
                        }),
                    ))
                },
            )
        });
        div()
            .flex()
            .flex_col()
            .gap_3()
            .text_color(crate::theme::ink::secondary())
            .children(
                self.instance
                    .as_ref()
                    .map(|instance| super::facts::card(instances, instance)),
            )
            .when(has_list, |this| this.child(list))
            .children(fields)
            .children(
                self.body
                    .clone()
                    .map(|body| div().whitespace_normal().child(body)),
            )
            .children(names)
            .into_any_element()
    }
}
