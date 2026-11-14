//! This computer's core, as Restart to update and the Connect service see it: both
//! stop or restart it, and so its servers, and ask first while any run (AGENTS.md).
//!
//! The shell may show another core instead (the Cores dialog), whose servers a restart
//! doesn't touch, so the count is always this computer's: from the shell's state while
//! it shows this core, else asked of the core with the Host's session on it. When the
//! count can't be had, the Host asks anyway.

use std::time::Duration;

use futures::FutureExt as _;
use gpui::{App, AppContext as _, Entity, Global, Task, WeakEntity};
use launcher_core::cores::admin::{AdminClient, InstanceInfo};

use crate::state::HostState;

/// The window's last session on this computer's core (`crate::app` sets it).
struct LocalCore {
    /// The shell's state while it shows this computer's core.
    state: WeakEntity<HostState>,
    /// Signed in to this computer's core, to ask it while the shell shows another.
    admin: AdminClient,
}

impl Global for LocalCore {}

/// Remembers `state` as this computer's core: call it when the shell shows it.
pub fn set(state: &Entity<HostState>, cx: &mut App) {
    let admin = state.read(cx).admin.clone();
    cx.set_global(LocalCore {
        state: state.downgrade(),
        admin,
    });
}

/// How many of `instances` are running (or starting or stopping): a restart stops them.
pub fn count_running(instances: &[InstanceInfo]) -> usize {
    instances
        .iter()
        .filter(|instance| instance.state.is_live())
        .count()
}

/// Whether restarting this computer's core asks first: while servers run on it, or
/// when that isn't known (`None`).
pub fn asks(running: Option<usize>) -> bool {
    running != Some(0)
}

/// This computer's core's running servers, as the shell's state has them while it shows
/// that core and its list has arrived; `None` otherwise (see [`running_servers`]).
pub fn running_now(cx: &App) -> Option<usize> {
    let local = cx.try_global::<LocalCore>()?;
    let state = local.state.upgrade()?;
    state.read(cx).instances.as_deref().map(count_running)
}

/// How long to wait for this computer's core to list its servers before asking anyway:
/// the person pressed Restart and waits on it, and a core that hangs would leave the
/// button doing nothing (one that's gone refuses at once).
const ASK_FOR: Duration = Duration::from_secs(3);

/// How many of this computer's core's servers run, which restarting it stops: `None`
/// when that can't be had (the core doesn't answer, or not within [`ASK_FOR`], or the
/// Host never signed in to it), so the caller asks.
pub fn running_servers(cx: &mut App) -> Task<Option<usize>> {
    if let Some(running) = running_now(cx) {
        return Task::ready(Some(running));
    }
    let Some(admin) = cx
        .try_global::<LocalCore>()
        .map(|local| local.admin.clone())
    else {
        return Task::ready(None);
    };
    let timeout = cx.background_executor().timer(ASK_FOR);
    cx.background_spawn(async move {
        let listed = async {
            admin
                .instances()
                .await
                .ok()
                .map(|instances| count_running(&instances))
        };
        futures::select_biased! {
            running = listed.fuse() => running,
            _ = timeout.fuse() => None,
        }
    })
}

/// A server in `state` ("Running", "Stopped"...), for tests.
#[cfg(test)]
pub(crate) fn instance(uuid: &str, state: &str) -> InstanceInfo {
    serde_json::from_value(serde_json::json!({
        "uuid": uuid, "name": "SMP", "port": 25565, "state": state,
        "game_type": { "type": "MinecraftJava", "variant": { "type": "Vanilla" } },
    }))
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_starting_and_stopping_servers_count() {
        let list = [
            instance("a", "Running"),
            instance("b", "Starting"),
            instance("c", "Stopping"),
            instance("d", "Stopped"),
            instance("e", "Error"),
        ];
        assert_eq!(count_running(&list), 3);
        assert!(asks(Some(3)));
        assert!(!asks(Some(0)));
        assert!(asks(None), "an unknown count asks");
    }
}

/// With GPUI's test platform, which the `screenshot` feature turns on. The pages that
/// restart this computer's core use [`gpui_tests::state`] too.
#[cfg(all(test, feature = "screenshot"))]
pub(crate) mod gpui_tests {
    use std::{cell::Cell, rc::Rc, sync::Arc};

    use gpui::TestAppContext;
    use http_client::{AsyncBody, HttpClient, Request};
    use launcher_core::{
        Secret,
        cores::{
            CoreClient,
            admin::{FullCoreInfo, PublicUser, models::UserPermission},
        },
    };

    use super::*;
    use crate::session::Session;

    /// A core that never answers: every request fails at once.
    struct Offline;

    impl HttpClient for Offline {
        fn user_agent(&self) -> Option<&http_client::http::HeaderValue> {
            None
        }

        fn proxy(&self) -> Option<&url::Url> {
            None
        }

        fn send(
            &self,
            _: Request<AsyncBody>,
        ) -> futures::future::BoxFuture<'static, anyhow::Result<http_client::Response<AsyncBody>>>
        {
            Box::pin(async { Err(anyhow::anyhow!("offline")) })
        }
    }

    /// A signed-in state for a core that doesn't answer, listing `instances`.
    pub(crate) fn state(
        instances: Vec<InstanceInfo>,
        cx: &mut TestAppContext,
    ) -> Entity<HostState> {
        let core = CoreClient::new(
            Arc::new(Offline),
            url::Url::parse("http://127.0.0.1:9").unwrap(),
        );
        let session = Session {
            admin: AdminClient::new(core, Secret::new(String::from("test-token"))),
            info: FullCoreInfo::default(),
            me: PublicUser {
                uid: "1".into(),
                username: "owner".into(),
                is_owner: true,
                is_admin: false,
                permissions: UserPermission::default(),
                has_password: true,
            },
        };
        let state = cx.new(|cx| HostState::new(session, cx));
        cx.run_until_parked();
        state.update(cx, |state, _| state.instances = Some(instances));
        state
    }

    /// What [`running_servers`] finds.
    fn count(cx: &mut TestAppContext) -> Option<usize> {
        let found = Rc::new(Cell::new(None));
        cx.update(|cx| {
            let task = running_servers(cx);
            let found = found.clone();
            cx.spawn(async move |_| found.set(Some(task.await)))
                .detach();
        });
        cx.run_until_parked();
        found.take().expect("the count is found")
    }

    /// LS-11: the count is the remembered state's (the pages' tests show another core).
    #[test]
    fn the_count_is_the_remembered_cores() {
        let mut cx = TestAppContext::single();
        let local = state(vec![instance("a", "Running")], &mut cx);
        cx.update(|cx| set(&local, cx));
        assert_eq!(cx.update(|cx| running_now(cx)), Some(1));
        assert_eq!(count(&mut cx), Some(1));
    }

    /// LS-11: once the shell no longer shows this computer's core, it is asked; a core
    /// that doesn't answer is an unknown count, which asks.
    #[test]
    fn without_the_shells_state_the_core_is_asked() {
        let mut cx = TestAppContext::single();
        let local = state(vec![instance("a", "Stopped")], &mut cx);
        cx.update(|cx| set(&local, cx));
        assert_eq!(cx.update(|cx| running_now(cx)), Some(0));
        drop(local);
        cx.run_until_parked();
        assert_eq!(cx.update(|cx| running_now(cx)), None);
        assert_eq!(count(&mut cx), None);
    }

    #[test]
    fn never_signed_in_is_unknown() {
        let mut cx = TestAppContext::single();
        assert_eq!(count(&mut cx), None);
    }
}
