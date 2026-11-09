//! `perf.*`: the performance monitor's numbers ([`crate::perf`]) for
//! `lodestone-launcher-cli perf`. Read-only: sampling changes nothing.

use std::time::Duration;

use gpui::App;
use launcher_control::Message;
use serde_json::json;

use super::{Call, Control, Reply};
use crate::perf::{Sampler, Sources};

fn sources(control: &Control) -> Sources {
    Sources {
        instances: control.instances.clone(),
        accounts: control.accounts.clone(),
        content: control.content.clone(),
        worlds: control.worlds.clone(),
    }
}

/// One reading, a second after the first so CPU and rates have something to compare.
pub fn show(call: Call, cx: &mut App) -> Reply {
    let sources = sources(&call.control);
    let mut sampler = Sampler::new();
    sampler.sample(&sources, cx);
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs(1)).await;
        Ok(cx.update(|cx| sampler.sample(&sources, cx).to_json()))
    })
}

/// A reading every second, as `perf` events, until the client unsubscribes
/// (`events.unsubscribe`) or goes.
pub fn watch(call: Call, cx: &mut App) -> Reply {
    let control = call.control.clone();
    let sources = sources(&control);
    let number = control.reserve_subscription();
    let outbox = call.outbox.clone();
    let task = cx.spawn(async move |cx| {
        let mut sampler = Sampler::new();
        cx.update(|cx| sampler.sample(&sources, cx));
        loop {
            // the first wait also lets the response go out before any event
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let data = cx.update(|cx| sampler.sample(&sources, cx).to_json());
            let sent = outbox.send(&Message::Event {
                subscription: number,
                event: "perf".into(),
                data,
            });
            if !sent || outbox.is_closed() {
                return;
            }
        }
    });
    control.insert_subscription(call.connection, number, task);
    super::ready(Ok(json!({ "subscription": number, "every_secs": 1 })))
}
