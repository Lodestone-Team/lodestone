//! The offer under "On this computer" while no Lodestone Host or Lodestone Core runs
//! here: download Lodestone Host to host a server for friends, or Lodestone Core on
//! its own. It goes once something is found, and for good once it's dismissed (its
//! ×; Settings > Developer > "Reset first-time setup" brings it back).

use gpui::{AnyElement, Context, IntoElement, SharedString, div, img, prelude::*};
use launcher_core::downloads;

use super::Servers;
use crate::{
    components::{
        Button, ButtonVariant, Icon, IconButton, KeyboardFocus,
        a11y::{Accessible, Role},
    },
    i18n::{ServersMsg as M, t},
    theme::{self, px, text, weights},
};

impl Servers {
    /// Whether "On this computer" offers Lodestone Host: the scan found nothing, and
    /// the offer wasn't dismissed.
    pub(super) fn offers_host(&self, cx: &Context<Self>) -> bool {
        let local = &self.local;
        local.scanned
            && !local.scanning
            && local.found.is_empty()
            && !self.instances.read(cx).settings().host_offer_dismissed
    }

    /// "Download Lodestone Host": looks up the newest Host release's installer for
    /// this computer and opens it in the browser, or the list of Host releases when
    /// there's none yet or GitHub can't be reached.
    fn download_host(&mut self, cx: &mut Context<Self>) {
        if self.host_download.is_some() {
            return;
        }
        let http = self.http.clone();
        let lookup = cx.background_spawn(launcher_core::http::in_http_runtime(async move {
            downloads::host_download_url(http.as_ref()).await
        }));
        self.host_download = Some(cx.spawn(async move |this, cx| {
            let url = lookup.await;
            this.update(cx, |this, cx| {
                this.host_download = None;
                cx.open_url(&url);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// The offer's ×: it doesn't come back.
    fn dismiss_host_offer(&mut self, cx: &mut Context<Self>) {
        self.instances
            .update(cx, |instances, cx| instances.dismiss_host_offer(cx));
        cx.notify();
    }

    /// The offer, a row of the "On this computer" box.
    pub(super) fn render_host_offer(&self, cx: &mut Context<Self>) -> AnyElement {
        let title = t(M::HostOfferTitle);
        // Lodestone Host has no app for Linux: Lodestone CLI is the way to host there,
        // so the button gets it and the "Core only" link would be the same.
        let has_app = downloads::host_app_available();
        let download_label = if has_app {
            t(M::HostOfferDownload)
        } else {
            t(M::GetLodestoneCli)
        };
        div()
            .id("host-offer")
            .accessible(Role::Group, "host-offer", title)
            .flex()
            .flex_row()
            .items_center()
            .gap_4()
            .pl_4()
            .pr_2()
            .py_4()
            .border_t_1()
            .border_color(crate::theme::ink::border())
            .child(img(crate::images::mark(true, cx)).size(px(40.)).flex_none())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .text_size(theme::text_size(text::MEDIUM))
                            .font_weight(weights::BOLD.into())
                            .text_color(crate::theme::ink::text())
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(theme::text_size(text::SMALL))
                            .text_color(crate::theme::ink::secondary())
                            .child(t(M::HostOfferBody)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Button::new("host-offer-download", download_label)
                            .variant(ButtonVariant::Primary)
                            .icon(Icon::Download)
                            .loading(self.host_download.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.download_host(cx))),
                    )
                    .when(has_app, |this| this.child(core_only_link())),
            )
            .child(
                div().flex_none().self_start().child(
                    IconButton::new("host-offer-dismiss", Icon::Xmark)
                        .label(t(M::Dismiss))
                        .on_click(cx.listener(|this, _, _, cx| this.dismiss_host_offer(cx))),
                ),
            )
            .into_any_element()
    }
}

/// "Download Lodestone Core only": small, muted text that opens Lodestone CLI's
/// newest release, which installs Lodestone Core and keeps it updated.
fn core_only_link() -> impl IntoElement {
    let label: SharedString = t(M::HostOfferCoreOnly).into();
    div()
        .id("host-offer-core-only")
        .accessible(Role::Link, "host-offer-core-only", label.clone())
        .keyboard_focus()
        .rounded(theme::radius::DEFAULT)
        .cursor_pointer()
        .text_size(theme::text_size(text::SMALL))
        .text_color(crate::theme::ink::muted())
        .hover(|style| style.text_color(crate::theme::ink::secondary()).underline())
        .child(label)
        .on_press(|_, _, cx| cx.open_url(downloads::CORE_ONLY_PAGE))
}
