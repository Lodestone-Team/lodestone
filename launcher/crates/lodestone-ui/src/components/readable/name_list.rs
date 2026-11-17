//! Lists of names, instead of a comma list in a sentence: a group per outcome
//! ("Added", "Skipped", "Not added") with its count, each name on its own line with
//! an icon and, when there is one, its reason. The first five show; a counted
//! "Show 3 more" shows the rest in place, never an uncounted "…".

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*};

use super::{CountBadge, Kind, PathText, key_value};
use crate::{
    components::{
        Icon,
        a11y::{Accessible, Role, automation_id},
        settings,
    },
    i18n::{ValueMsg, ValuePlural, spans::Spans, t, trn},
    theme::{self, colors, fonts, ink, px, text, weights},
    values::truncate::{MAX_SHOWN, sanitize},
};

/// The icon before a name: its outcome, or its kind for a neutral list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemIcon {
    /// A green check.
    Added,
    /// A gray skip.
    Skipped,
    /// A red cross.
    Failed,
    Kind(Kind),
}

impl ItemIcon {
    fn render(self) -> gpui::Svg {
        let (icon, color) = match self {
            ItemIcon::Added => (Icon::Check, colors::GREEN_300),
            ItemIcon::Skipped => (Icon::Forward, colors::GRAY_500),
            ItemIcon::Failed => (Icon::Xmark, colors::RED_200),
            ItemIcon::Kind(kind) => (kind.icon(), colors::GRAY_500),
        };
        icon.render(px(12.), color)
    }

    /// What the icon says, for screen readers.
    fn label(self) -> &'static str {
        match self {
            ItemIcon::Added => t(ValueMsg::Added),
            ItemIcon::Skipped => t(ValueMsg::Skipped),
            ItemIcon::Failed => t(ValueMsg::NotAdded),
            ItemIcon::Kind(kind) => kind.label(),
        }
    }
}

/// How a name is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NameStyle {
    /// A mod's or a world's name: 14px in the main ink.
    #[default]
    Name,
    /// A file name: mono 12px, its extension kept when cut.
    Mono,
    /// A path: mono 12px, home as `~`, its last parts kept.
    Path,
}

/// Why a name ended up in its group, as its own message per cause ("Already
/// installed"), never "name: reason" text.
pub enum Reason {
    Text(SharedString),
    /// A reason with a value in it, from `tr_spans!`.
    Spans(Spans),
}

impl Reason {
    fn plain(&self) -> SharedString {
        match self {
            Reason::Text(text) => text.clone(),
            Reason::Spans(spans) => spans.plain().to_string().into(),
        }
    }
}

impl From<Spans> for Reason {
    fn from(spans: Spans) -> Self {
        Reason::Spans(spans)
    }
}

impl From<&'static str> for Reason {
    fn from(text: &'static str) -> Self {
        Reason::Text(text.into())
    }
}

impl From<String> for Reason {
    fn from(text: String) -> Self {
        Reason::Text(text.into())
    }
}

impl From<SharedString> for Reason {
    fn from(text: SharedString) -> Self {
        Reason::Text(text)
    }
}

/// A name in a [`NameList`].
pub struct NameItem {
    icon: Option<ItemIcon>,
    name: SharedString,
    style: NameStyle,
    reason: Option<Reason>,
}

impl NameItem {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            icon: None,
            name: sanitize(&name.into(), MAX_SHOWN).into(),
            style: NameStyle::Name,
            reason: None,
        }
    }

    pub fn icon(mut self, icon: ItemIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn style(mut self, style: NameStyle) -> Self {
        self.style = style;
        self
    }

    pub fn reason(mut self, reason: impl Into<Reason>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    fn label(&self) -> SharedString {
        let name = match self.icon {
            Some(icon) => key_value(icon.label(), &self.name),
            None => self.name.clone(),
        };
        match &self.reason {
            Some(reason) => key_value(&name, &reason.plain()),
            None => name,
        }
    }
}

/// A group of names under a heading and its count.
pub struct NameGroup {
    heading: SharedString,
    items: Vec<NameItem>,
}

impl NameGroup {
    pub fn new(heading: impl Into<SharedString>) -> Self {
        Self {
            heading: heading.into(),
            items: Vec::new(),
        }
    }

    pub fn item(mut self, item: NameItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn items(mut self, items: impl IntoIterator<Item = NameItem>) -> Self {
        self.items.extend(items);
        self
    }
}

/// Groups of names; see the module docs.
#[derive(IntoElement)]
pub struct NameList {
    id: ElementId,
    groups: Vec<NameGroup>,
    collapsed_after: usize,
    reason_below: bool,
}

impl NameList {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            groups: Vec::new(),
            collapsed_after: 5,
            reason_below: false,
        }
    }

    pub fn group(mut self, group: NameGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// How many names show before "Show N more" (5 by default).
    pub fn collapsed_after(mut self, count: usize) -> Self {
        self.collapsed_after = count.max(1);
        self
    }

    /// Reasons on a second line rather than at the end, for narrow dialogs.
    pub fn reason_below(mut self) -> Self {
        self.reason_below = true;
        self
    }
}

/// How many of each group's items show when `budget` names show in all, and how
/// many are left over.
fn shown_counts(sizes: &[usize], budget: usize) -> (Vec<usize>, usize) {
    let mut left = budget;
    let shown: Vec<usize> = sizes
        .iter()
        .map(|size| {
            let shown = (*size).min(left);
            left -= shown;
            shown
        })
        .collect();
    let total: usize = sizes.iter().sum();
    let hidden = total - shown.iter().sum::<usize>();
    (shown, hidden)
}

impl RenderOnce for NameList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let expanded = window.use_keyed_state(
            ElementId::from((self.id.clone(), "expanded")),
            cx,
            |_, _| false,
        );
        let sizes: Vec<usize> = self.groups.iter().map(|group| group.items.len()).collect();
        let budget = if *expanded.read(cx) {
            usize::MAX
        } else {
            self.collapsed_after
        };
        let (shown, hidden) = shown_counts(&sizes, budget);
        let list = automation_id(&self.id);
        let reason_below = self.reason_below;
        let more = (hidden > 0).then(|| {
            let expanded = expanded.clone();
            settings::muted_link(
                (self.id.clone(), "more"),
                trn!(ValuePlural::ShowMore, hidden),
            )
            .on_press(move |_, _, cx| {
                expanded.update(cx, |expanded, cx| {
                    *expanded = true;
                    cx.notify();
                })
            })
        });
        div()
            .id(self.id.clone())
            .role(Role::List)
            .accessibility_id(list.clone())
            .flex()
            .flex_col()
            .gap(px(12.))
            .w_full()
            .min_w_0()
            .children(
                self.groups
                    .into_iter()
                    .zip(shown)
                    .enumerate()
                    .filter(|(_, (_, shown))| *shown > 0)
                    .map(|(group_index, (group, shown))| {
                        let count = group.items.len() as u64;
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.))
                                    .pb(px(2.))
                                    .text_size(theme::text_size(text::SMALL))
                                    .font_weight(weights::MEDIUMBOLD.into())
                                    .text_color(ink::secondary())
                                    .child(group.heading)
                                    .child(CountBadge::new(count).id(("count", group_index))),
                            )
                            .children(group.items.into_iter().take(shown).enumerate().map(
                                |(index, item)| {
                                    let id: SharedString =
                                        format!("{list}-{group_index}-{index}").into();
                                    item_row(item, id, reason_below)
                                },
                            ))
                    }),
            )
            .children(more)
    }
}

/// A name's line: its icon, the name in its style, and its reason.
fn item_row(item: NameItem, id: SharedString, reason_below: bool) -> impl IntoElement {
    let label = item.label();
    let name = match item.style {
        NameStyle::Name => div()
            .min_w_0()
            .truncate()
            .text_size(theme::text_size(text::MEDIUM))
            .font_weight(weights::MEDIUM.into())
            .text_color(ink::text())
            .child(item.name)
            .into_any_element(),
        NameStyle::Mono | NameStyle::Path => {
            let path = if item.style == NameStyle::Mono {
                PathText::file(item.name)
            } else {
                PathText::path(item.name)
            };
            div()
                .flex()
                .min_w_0()
                .font_family(fonts::mono())
                .text_size(theme::text_size(text::SMALL))
                .text_color(ink::secondary())
                .child(path.id(ElementId::Name(format!("{id}-name").into())))
                .into_any_element()
        }
    };
    let reason = item.reason.map(|reason| {
        let content = match reason {
            Reason::Text(text) => text.into_any_element(),
            Reason::Spans(spans) => spans.into_any_element(),
        };
        div()
            .min_w_0()
            .text_size(theme::text_size(text::SMALL))
            .text_color(ink::muted())
            .when(!reason_below, |this| {
                this.flex_none().max_w(gpui::relative(0.5)).truncate()
            })
            .child(content)
    });
    // at the end of the line, or under the name in a narrow dialog
    let (inline, below) = if reason_below {
        (None, reason.map(|reason| reason.pl(px(18.)).pb(px(4.))))
    } else {
        (reason, None)
    };
    div()
        .id(ElementId::Name(id.clone()))
        .accessible(Role::ListItem, id, label)
        .flex()
        .flex_col()
        .min_w_0()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.))
                .min_h(px(24.))
                .min_w_0()
                .children(item.icon.map(ItemIcon::render))
                .child(div().flex().flex_1().min_w_0().child(name))
                .children(inline),
        )
        .children(below)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_names_show_and_the_rest_are_counted() {
        assert_eq!(shown_counts(&[1, 1, 2], 5), (vec![1, 1, 2], 0));
        assert_eq!(shown_counts(&[3, 4], 5), (vec![3, 2], 2));
        assert_eq!(shown_counts(&[7], 5), (vec![5], 2));
        assert_eq!(shown_counts(&[2, 0, 6], usize::MAX), (vec![2, 0, 6], 0));
        assert_eq!(shown_counts(&[], 5), (vec![], 0));
    }
}
