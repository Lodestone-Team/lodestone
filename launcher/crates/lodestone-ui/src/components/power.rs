//! A server's power buttons, as Lodestone Host's sidebar shows them after the
//! dashboard's instance card: Start; Stop and Restart while it runs; Kill while it
//! starts or stops; Restart after it crashed. Lodestone Launcher's server rows and
//! server page use the same buttons.

use std::rc::Rc;

use gpui::{
    App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
};
use launcher_core::cores::admin::{InstanceState, instances::Power};

use super::{Button, ButtonSize, ButtonVariant, LoadingDots};
use crate::i18n::{CommonMsg, t};
use crate::theme::{ink, px, text};

/// What the power buttons offer for a server in `state`, as the dashboard's instance
/// card does.
pub fn power_actions(state: InstanceState) -> &'static [Power] {
    match state {
        InstanceState::Stopped => &[Power::Start],
        InstanceState::Running => &[Power::Stop, Power::Restart],
        InstanceState::Starting | InstanceState::Stopping => &[Power::Kill],
        InstanceState::Error => &[Power::Restart],
        InstanceState::Unknown => &[],
    }
}

/// A power button's label: "Start", "Stop", "Restart", "Kill".
pub fn power_name(power: Power) -> &'static str {
    t(match power {
        Power::Start => CommonMsg::Start,
        Power::Stop => CommonMsg::Stop,
        Power::Restart => CommonMsg::Restart,
        Power::Kill => CommonMsg::Kill,
    })
}

type PowerHandler = Rc<dyn Fn(Power, &mut Window, &mut App)>;

/// The power buttons for a server in `state`. Each button's id is `{id}-{Power:?}`
/// ("power-Start"). Buttons that `allow` refuses are left out, as the Host leaves out
/// what the signed-in user may not do. While a request is on its way (`pending`), its
/// label shows with three dots in place of the buttons.
#[derive(IntoElement)]
pub struct PowerButtons {
    id: SharedString,
    state: InstanceState,
    allow: Box<dyn Fn(Power) -> bool>,
    pending: Option<SharedString>,
    on_power: Option<PowerHandler>,
}

impl PowerButtons {
    pub fn new(id: impl Into<SharedString>, state: InstanceState) -> Self {
        Self {
            id: id.into(),
            state,
            allow: Box::new(|_| true),
            pending: None,
            on_power: None,
        }
    }

    /// Leaves out the buttons this returns false for.
    pub fn allow(mut self, allow: impl Fn(Power) -> bool + 'static) -> Self {
        self.allow = Box::new(allow);
        self
    }

    /// Shows `label` with three dots instead of the buttons.
    pub fn pending(mut self, label: Option<impl Into<SharedString>>) -> Self {
        self.pending = label.map(Into::into);
        self
    }

    pub fn on_power(mut self, handler: impl Fn(Power, &mut Window, &mut App) + 'static) -> Self {
        self.on_power = Some(Rc::new(handler));
        self
    }

    /// The buttons it shows, in order: none while pending.
    pub fn actions(&self) -> Vec<Power> {
        if self.pending.is_some() {
            return Vec::new();
        }
        power_actions(self.state)
            .iter()
            .copied()
            .filter(|power| (self.allow)(*power))
            .collect()
    }

    /// Whether it shows anything at all.
    pub fn is_empty(&self) -> bool {
        self.pending.is_none() && self.actions().is_empty()
    }
}

impl RenderOnce for PowerButtons {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(6.));
        if let Some(label) = self.pending {
            return row
                .gap(px(8.))
                .text_size(crate::theme::text_size(text::SMALL))
                .text_color(ink::secondary())
                .child(LoadingDots)
                .child(label);
        }
        let actions = self.actions();
        row.children(actions.into_iter().map(|power| {
            let id = ElementId::Name(format!("{}-{power:?}", self.id).into());
            let button = Button::new(id, power_name(power))
                .size(ButtonSize::Slim)
                .variant(match power {
                    Power::Start => ButtonVariant::Primary,
                    Power::Kill => ButtonVariant::Danger,
                    _ => ButtonVariant::Secondary,
                });
            match self.on_power.clone() {
                Some(handler) => button.on_click(move |_, window, cx| handler(power, window, cx)),
                None => button,
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_follows_the_state() {
        assert_eq!(power_actions(InstanceState::Stopped), [Power::Start]);
        assert_eq!(
            power_actions(InstanceState::Running),
            [Power::Stop, Power::Restart]
        );
        assert_eq!(power_actions(InstanceState::Starting), [Power::Kill]);
        assert_eq!(power_actions(InstanceState::Stopping), [Power::Kill]);
        assert_eq!(power_actions(InstanceState::Error), [Power::Restart]);
        assert!(power_actions(InstanceState::Unknown).is_empty());
    }

    #[test]
    fn refused_and_pending_buttons_are_left_out() {
        let stop_only = PowerButtons::new("power", InstanceState::Running)
            .allow(|power| power != Power::Restart);
        assert_eq!(stop_only.actions(), [Power::Stop]);
        let pending = PowerButtons::new("power", InstanceState::Stopped).pending(Some("Starting…"));
        assert!(pending.actions().is_empty());
        assert!(!pending.is_empty());
        assert!(
            PowerButtons::new("power", InstanceState::Stopped)
                .allow(|_| false)
                .is_empty()
        );
    }
}
