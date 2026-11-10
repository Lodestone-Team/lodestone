//! Accounts: the account box's menu and its dialogs. No method returns a token: the
//! Microsoft sign-in hands out the device code and the URL to enter it at, and the
//! tokens stay in the launcher (the long-lived one in the Keychain).

use std::sync::{Arc, Mutex};

use futures::FutureExt as _;
use gpui::App;
use launcher_control::{ErrorObject, Message, codes};
use launcher_core::accounts::{Account, AccountKind};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Call, Control, Reply, api_error,
    instances::{params, tri},
    jobs::JobSpec,
    ready,
};
use crate::accounts::SessionState;

fn session_name(state: Option<&SessionState>) -> &'static str {
    match state {
        None => "none",
        Some(SessionState::Refreshing) => "refreshing",
        Some(SessionState::Ready(_)) => "ready",
        Some(SessionState::NeedsSignIn) => "needs_sign_in",
        Some(SessionState::Failed(_)) => "failed",
    }
}

fn account_json(account: &Account, control: &Control, cx: &App) -> Value {
    let accounts = control.accounts.read(cx);
    let session = accounts.session(&account.id);
    json!({
        "id": account.id,
        "username": account.username,
        "uuid": account.uuid.to_string(),
        "kind": match account.kind {
            AccountKind::Microsoft => "microsoft",
            AccountKind::Offline => "offline",
        },
        "active": accounts.active().is_some_and(|a| a.id == account.id),
        "client": account.client,
        "session": session_name(session),
        // the message is safe to show: errors never hold tokens
        "session_error": match session {
            Some(SessionState::Failed(err)) => Some(err.message()),
            _ => None,
        },
    })
}

/// The accounts, for `meta.status`.
pub fn summary(control: &Control, cx: &App) -> Value {
    let accounts = control.accounts.read(cx);
    json!({
        "active": accounts.active().map(|a| json!({ "id": a.id, "username": a.username })),
        "count": accounts.accounts().len(),
    })
}

pub fn list(call: Call, cx: &mut App) -> Reply {
    let control = &call.control;
    let accounts = control.accounts.read(cx);
    let list: Vec<Value> = accounts
        .accounts()
        .iter()
        .map(|a| account_json(a, control, cx))
        .collect();
    ready(Ok(json!({
        "accounts": list,
        "active": accounts.active().map(|a| &a.id),
        "offline_accounts_available": accounts.offline_enabled(),
        "sign_in_as_minecraft_launcher": accounts.launcher_identity_enabled(),
        "notice": accounts.notice().map(|n| n.to_string()),
    })))
}

/// The account named by the `account` parameter: its id, or its username in any case.
fn find_account(call: &Call, cx: &App) -> Result<Account, ErrorObject> {
    let reference = call
        .params
        .get("account")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            api_error(
                codes::INVALID_PARAMS,
                "Name the account: \"account\" is its id or username.",
            )
        })?;
    let accounts = call.control.accounts.read(cx);
    if let Some(account) = accounts.accounts().iter().find(|a| a.id == reference) {
        return Ok(account.clone());
    }
    let matches: Vec<&Account> = accounts
        .accounts()
        .iter()
        .filter(|a| a.username.eq_ignore_ascii_case(reference))
        .collect();
    match matches.as_slice() {
        [account] => Ok((*account).clone()),
        [] => Err(api_error(
            codes::NOT_FOUND,
            format!("No account is called \"{reference}\". List them with `accounts list`."),
        )),
        _ => Err(api_error(
            codes::AMBIGUOUS,
            format!("More than one account is called \"{reference}\" (Microsoft and offline?); use its id."),
        )
        .with_data(json!({ "matches": matches.iter().map(|a| &a.id).collect::<Vec<_>>() }))),
    }
}

pub fn switch(call: Call, cx: &mut App) -> Reply {
    let account = tri!(find_account(&call, cx));
    call.control
        .accounts
        .update(cx, |accounts, cx| accounts.switch(&account.id, cx));
    ready(Ok(
        json!({ "account": account_json(&account, &call.control, cx) }),
    ))
}

#[derive(Deserialize)]
struct OfflineParams {
    username: String,
}

pub fn add_offline(call: Call, cx: &mut App) -> Reply {
    let params: OfflineParams = params!(call);
    let control = &call.control;
    if !control.accounts.read(cx).offline_enabled() {
        return ready(Err(api_error(
            codes::UNAVAILABLE,
            "Offline accounts are off. Ask the user to turn on Settings > Developer > Accounts > Offline accounts in the launcher's window. They are always on in debug builds and with LODESTONE_LAUNCHER_OFFLINE=1.",
        )));
    }
    let account = tri!(
        Account::offline(params.username.trim())
            .map_err(|err| api_error(codes::INVALID_PARAMS, err.to_string()))
    );
    let added = account.clone();
    control
        .accounts
        .update(cx, |accounts, cx| accounts.add_offline(account, cx));
    ready(Ok(json!({ "account": account_json(&added, control, cx) })))
}

pub fn remove(call: Call, cx: &mut App) -> Reply {
    let account = tri!(find_account(&call, cx));
    tri!(call.confirm(format!(
        "removes the account {} and deletes its saved sign-in from the Keychain",
        account.username
    )));
    call.control
        .accounts
        .update(cx, |accounts, cx| accounts.remove(&account.id, cx));
    ready(Ok(
        json!({ "removed": account.id, "username": account.username }),
    ))
}

/// "Add Microsoft account" with a device code: asks Microsoft for a code, returns it
/// with the URL to enter it at, and finishes the sign-in in a job once the user has.
pub fn sign_in(call: Call, cx: &mut App) -> Reply {
    let control = call.control.clone();
    let (auth, identity, secrets) = {
        let accounts = control.accounts.read(cx);
        (
            accounts.authenticator().clone(),
            accounts.sign_in_identity(),
            accounts.secrets().clone(),
        )
    };
    cx.spawn(async move |cx| {
        let code = auth
            .start_device_code(identity)
            .await
            .map_err(|err| api_error(codes::FAILED, err.message()))?;
        let shown = json!({
            "user_code": code.user_code,
            "verification_uri": code.verification_uri,
            "expires_in_secs": code.expires_in.as_secs(),
            "client": identity,
            "message": format!(
                "Open {} and enter the code {}, then sign in with the Microsoft account that owns Minecraft.",
                code.verification_uri, code.user_code
            ),
        });
        let stage = Arc::new(Mutex::new(format!(
            "Waiting for the code {} to be entered at {}",
            code.user_code, code.verification_uri
        )));
        let job_stage = stage.clone();
        let accounts = control.accounts.clone();
        let executor = cx.background_executor().clone();
        let work = cx.spawn(async move |cx| {
            let sleep = move |duration| executor.timer(duration).boxed_local();
            let tokens = auth
                .complete_device_code(&code, &sleep)
                .await
                .map_err(|err| api_error(codes::FAILED, err.message()))?;
            if let Ok(mut stage) = job_stage.lock() {
                *stage = "Signing in to Xbox Live and Minecraft…".into();
            }
            let store = secrets.store(cx);
            let signed_in = auth
                .finish_sign_in(tokens, &*store)
                .await
                .map_err(|err| api_error(codes::FAILED, err.message()))?;
            let account = signed_in.account.clone();
            accounts.update(cx, |accounts, cx| accounts.add_signed_in(signed_in, cx));
            Ok(json!({ "account": { "id": account.id, "username": account.username } }))
        });
        let job = cx.update(|cx| {
            control.start_job(
                JobSpec::new("sign_in", "Signing in with Microsoft").stage(stage),
                work,
                cx,
            )
        });
        if !call.wait() {
            let mut result = shown;
            result["job"] = json!(job);
            return Ok(result);
        }
        // the code goes out first, as a progress line, so it can be shown while waiting
        call.outbox.send(&Message::Progress {
            id: call.id.clone(),
            progress: shown,
        });
        let wait = cx.update(|cx| control.reply_job(&call, job, cx));
        wait.await
    })
}
