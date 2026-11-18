//! What the control API does with cores and servers that the Servers page does in its
//! dialogs: adding a core (with the certificate check), hosting an instance and
//! downloading a server's world. The
//! steps are the dialogs' own, with the answers given as parameters instead of typed
//! into a dialog, and the outcome returned instead of shown in it. Starting, stopping,
//! joining and "Create matching instance" need no dialog, so the API calls the page's
//! own methods for those.

use std::sync::{Arc, Mutex};

use gpui::{AppContext as _, Context, Task};
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    Secret,
    cores::{
        self, CoreClient, ServerInstance,
        client::display_address,
        hosting::{self, HostChoices, HostError, HostProgress, HostStage},
        live::LauncherScope,
        store::{self, SavedCore},
        tls::CertificatePin,
    },
    game::download::CancelToken,
};
use serde_json::json;

use crate::components::{Kind, ValueKind};
use crate::i18n::{FactMsg, ServersMsg as M, t};
use crate::theme::Tone;

use super::{
    AddFailure, CoreEntry, CoreStatus, FINGERPRINT_HELP, Notice, Servers, certificate_outcome,
    pinned_http,
};
use crate::control::{api_error, core_error};

/// "Add server", with the answers as parameters.
pub struct AddCore {
    pub address: String,
    pub username: Option<String>,
    pub password: Option<Secret>,
    pub expires_at: Option<i64>,
    pub console: LauncherScope,
    /// The certificate fingerprint the user compared, to trust a certificate this Mac
    /// doesn't. Only a certificate with exactly this fingerprint is pinned.
    pub fingerprint: Option<String>,
}

/// What adding a core did.
pub struct Added {
    pub uuid: String,
    pub name: String,
    pub new: bool,
    pub trusted: bool,
    pub signed_in: bool,
}

/// "Host on Lodestone", with the answers as parameters.
pub struct HostCore {
    pub core_uuid: String,
    /// The username and password, unless this computer has the core's owner key.
    pub sign_in: Option<(String, Secret)>,
    pub name: String,
    pub max_ram: u32,
    /// The worlds sent, by folder (each checked against the instance's): `None` for the
    /// server's world when it is named, none for a new one, or else the last played
    /// ([`hosting::worlds_to_send`]).
    pub worlds: Option<Vec<String>>,
    /// The server's world (its folder), `Some(None)` for a new one, `None` for the core's
    /// choice.
    pub world: Option<Option<String>>,
    /// `None` for the first free one from [`hosting::FIRST_PORT`].
    pub port: Option<u16>,
    /// One of the summary's server choices (`Paper` for a vanilla game).
    pub server_software: Option<String>,
    /// Only say what the server would get: nothing is made, and the file is let go of.
    pub preview: bool,
    /// The user agreed to the Minecraft EULA.
    pub accept_eula: bool,
    /// The bytes written and sent, for the job's progress.
    pub progress: Arc<HostProgress>,
    /// The job's Cancel: stops the export and the upload (nothing is left on either side);
    /// once the creation was asked for, only the wait for it.
    pub cancel: CancelToken,
}

/// Fingerprints compare without separators and case: `AB:CD…` and `abcd…` are one.
pub fn same_fingerprint(a: &str, b: &str) -> bool {
    let norm = |s: &str| -> String {
        s.chars()
            .filter(char::is_ascii_hexdigit)
            .map(|c| c.to_ascii_uppercase())
            .collect()
    };
    let (a, b) = (norm(a), norm(b));
    !a.is_empty() && a == b
}

impl Servers {
    /// The core named by `reference`: its id, its name (any case) or its address.
    pub(crate) fn control_core(&self, reference: &str) -> Result<usize, ErrorObject> {
        if let Some(index) = self.core_index(reference) {
            return Ok(index);
        }
        let matches: Vec<usize> = self
            .cores
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.saved.name.eq_ignore_ascii_case(reference)
                    || display_address(&c.url).eq_ignore_ascii_case(reference)
                    || c.saved.url.eq_ignore_ascii_case(reference)
            })
            .map(|(i, _)| i)
            .collect();
        match matches.as_slice() {
            [index] => Ok(*index),
            [] => Err(api_error(
                codes::NOT_FOUND,
                format!("No core is called \"{reference}\". List them with `cores list`."),
            )),
            _ => Err(api_error(
                codes::AMBIGUOUS,
                format!("More than one core is called \"{reference}\"; use its id."),
            )
            .with_data(json!({
                "matches": matches.iter().map(|i| &self.cores[*i].saved.uuid).collect::<Vec<_>>()
            }))),
        }
    }

    /// Whether a core's list is being fetched.
    pub(crate) fn control_refreshing(&self) -> bool {
        self.cores
            .iter()
            .any(|c| c.refreshing || matches!(c.status, CoreStatus::Loading))
    }

    /// Links an existing instance to a server, as "Create matching instance" does for
    /// the instance it makes: Play then joins it and keeps its mods in step.
    pub(crate) fn control_link(
        &mut self,
        core_uuid: &str,
        server: &ServerInstance,
        instance_id: &str,
        cx: &mut Context<Self>,
    ) {
        self.link_instance(core_uuid, server, instance_id, None, cx);
    }

    /// "Trusted for mods" on or off, as answering "Do you trust <core>?" does. The
    /// caller has asked the user.
    pub(crate) fn control_set_trusted(&mut self, index: usize, trusted: bool) {
        self.cores[index].saved.set_trusted(trusted);
        self.save();
    }

    /// "Trust certificate…" on a core whose certificate this Mac doesn't trust, or that
    /// changed: pins the presented certificate if its fingerprint is `fingerprint`.
    pub(crate) fn control_trust_certificate(
        &mut self,
        index: usize,
        fingerprint: &str,
        cx: &mut Context<Self>,
    ) -> Result<bool, ErrorObject> {
        let core = &self.cores[index];
        let (host, presented) = match &core.status {
            CoreStatus::Failed(cores::CoreError::Untrusted {
                host,
                fingerprint: Some(presented),
            }) => (host.clone(), presented.clone()),
            CoreStatus::Failed(cores::CoreError::CertificateChanged {
                host, presented, ..
            }) => (host.clone(), presented.clone()),
            _ => {
                return Err(api_error(
                    codes::CONFLICT,
                    "The core's certificate needs no decision: it is trusted, or the core hasn't answered yet.",
                ));
            }
        };
        if !same_fingerprint(fingerprint, &presented) {
            return Err(api_error(
                codes::UNTRUSTED_CERTIFICATE,
                format!(
                    "The core presents another certificate than the fingerprint given. Nothing was trusted. {FINGERPRINT_HELP}"
                ),
            )
            .with_data(json!({ "host": host, "fingerprint": presented })));
        }
        let core = &mut self.cores[index];
        let untrusted = core
            .saved
            .pin_certificate(CertificatePin::new(&host, &presented));
        core.pinned = pinned_http(core.saved.certificate.as_ref());
        core.status = CoreStatus::Loading;
        self.save();
        self.refresh(index, cx);
        cx.notify();
        Ok(untrusted)
    }

    /// Adds (or signs in to) a core, as "Add server" does, without asking whether to
    /// trust it for mods: the caller says it isn't trusted yet.
    pub(crate) fn control_add(
        &mut self,
        request: AddCore,
        cx: &mut Context<Self>,
    ) -> Task<Result<Added, ErrorObject>> {
        let url = match cores::parse_address(&request.address) {
            Ok(url) => url,
            Err(err) => return Task::ready(Err(core_error(err))),
        };
        if request.username.is_some() != request.password.is_some() {
            return Task::ready(Err(api_error(
                codes::INVALID_PARAMS,
                "Give both a username and a password, or neither.",
            )));
        }
        let address = display_address(&url);
        let existing_pin = self
            .cores
            .iter()
            .find(|c| display_address(&c.url) == address)
            .and_then(|c| c.saved.certificate.clone());
        let http = self.http.clone();
        let secrets = self.accounts.read(cx).secrets().clone();
        let device = store::device_key_name(&store::host_name());
        let AddCore {
            username,
            password,
            expires_at,
            console,
            fingerprint,
            ..
        } = request;
        let probe_url = url.clone();
        let sign_in_as = username.clone();
        let work = cx.background_spawn(async move {
            let mut pin = existing_pin;
            for attempt in 0..2 {
                let client = match pinned_http(pin.as_ref()) {
                    Some(pinned) => {
                        CoreClient::new(http.clone(), probe_url.clone()).with_pinned(pinned)
                    }
                    None => CoreClient::new(http.clone(), probe_url.clone()),
                };
                let info = match client.info().await {
                    Ok(info) => info,
                    Err(err) => match certificate_outcome(err, &http, probe_url.clone()).await {
                        AddFailure::Certificate {
                            host,
                            presented,
                            previous,
                            name,
                        } => {
                            let given = fingerprint.as_deref();
                            if attempt == 0 && given.is_some_and(|f| same_fingerprint(f, &presented))
                            {
                                pin = Some(CertificatePin::new(&host, &presented));
                                continue;
                            }
                            let message = if given.is_some() {
                                format!(
                                    "The core presents another certificate than the fingerprint given, so nothing was sent to it. {FINGERPRINT_HELP}"
                                )
                            } else {
                                format!(
                                    "This {} doesn't trust the HTTPS certificate of {host}, so nothing was sent to it. Ask the user to compare its fingerprint, then add the core again with --fingerprint. {FINGERPRINT_HELP}", launcher_core::wording::COMPUTER
                                )
                            };
                            return Err(api_error(codes::UNTRUSTED_CERTIFICATE, message)
                                .with_data(json!({
                                    "host": host,
                                    "fingerprint": presented,
                                    "previous": previous,
                                    "name": name,
                                })));
                        }
                        AddFailure::Core(err) => return Err(core_error(err)),
                    },
                };
                // an old core has no list: fail now rather than after adding it
                client.public_servers().await.map_err(core_error)?;
                let key = match username.as_deref().zip(password.as_ref()) {
                    Some((username, password)) => Some(
                        client
                            .sign_in_with_scope(username, password, &device, expires_at, console)
                            .await
                            .map_err(core_error)?,
                    ),
                    None => None,
                };
                return Ok((info, key, pin));
            }
            Err(api_error(
                codes::UNTRUSTED_CERTIFICATE,
                "The certificate changed while connecting. Try again.",
            ))
        });
        cx.spawn(async move |this, cx| {
            let (info, key, pin) = work.await?;
            if let Some((secret, _)) = &key {
                let store = secrets.store(cx);
                store::write_key(&*store, &info.uuid, secret)
                    .await
                    .map_err(|err| {
                        api_error(
                            codes::FAILED,
                            format!("Couldn't save the key in the Keychain: {err:#}"),
                        )
                    })?;
            }
            this.update(cx, |this, cx| {
                let existing = this.core_index(&info.uuid);
                for core in &mut this.cores {
                    if display_address(&core.url) == address
                        && core.saved.uuid != info.uuid
                        && core.saved.check_identity(&info.uuid)
                    {
                        let (name, address) = (core.saved.name.clone(), address.clone());
                        this.notice = Some(
                            Notice::new(|| {
                                (
                                    t(M::NoLongerTrusted).into(),
                                    t(M::NoLongerTrustedOtherCore).into(),
                                )
                            })
                            .object(name, Kind::Computer)
                            .status(|| t(M::NotTrusted).into(), Tone::Danger)
                            .value(
                                || t(FactMsg::Address).into(),
                                address,
                                ValueKind::Code,
                            ),
                        );
                    }
                }
                let mut saved = match existing {
                    Some(index) => this.cores[index].saved.clone(),
                    None => SavedCore::default(),
                };
                saved.uuid = info.uuid.clone();
                saved.url = address.clone();
                saved.name = info.core_name.clone();
                saved.product = info.product;
                saved.launched_by = info.launched_by;
                let kept_key = this.carried_key(existing, &address);
                if key.is_some() {
                    saved.username = sign_in_as.clone();
                    saved.key_id = key.as_ref().map(|(_, info)| info.id.clone());
                    saved.key_expires_at = key.as_ref().and_then(|(_, info)| info.expires_at);
                    saved.local_claim = false;
                } else if kept_key.is_none() {
                    // no key goes with the entry to another address
                    saved.username = None;
                    saved.key_id = None;
                    saved.key_expires_at = None;
                    saved.local_claim = false;
                }
                if let Some(pin) = pin.clone()
                    && saved.pin_certificate(pin)
                {
                    let name = saved.name.clone();
                    this.notice = Some(
                        Notice::new(|| {
                            (
                                t(M::NoLongerTrusted).into(),
                                t(M::NoLongerTrustedNewCertificate).into(),
                            )
                        })
                        .object(name, Kind::Computer)
                        .status(|| t(M::NotTrusted).into(), Tone::Danger),
                    );
                }
                let signed_in = key.is_some();
                let entry = CoreEntry::new(saved, url, key.map(|(secret, _)| secret).or(kept_key));
                let added = Added {
                    uuid: entry.saved.uuid.clone(),
                    name: entry.saved.name.clone(),
                    new: existing.is_none(),
                    trusted: entry.saved.is_trusted(),
                    signed_in: signed_in || entry.signed_in(),
                };
                let index = match existing {
                    Some(index) => {
                        this.cores[index] = entry;
                        index
                    }
                    None => {
                        this.cores.push(entry);
                        this.cores.len() - 1
                    }
                };
                this.save();
                this.refresh(index, cx);
                cx.notify();
                Ok(added)
            })
            .map_err(|_| api_error(codes::FAILED, "The launcher is closing."))?
        })
    }

    /// "Host on Lodestone" (the dialog's steps, `servers::host`): writes the instance with
    /// the worlds asked for, sends it, and (unless `preview`) makes a stopped server from
    /// it, follows its creation to its end and links the instance to it once made.
    /// `stage` follows the steps.
    pub(crate) fn control_host(
        &mut self,
        instance_id: &str,
        request: HostCore,
        stage: Arc<Mutex<String>>,
        cx: &mut Context<Self>,
    ) -> Task<Result<serde_json::Value, ErrorObject>> {
        let to = match self.host_to(&request.core_uuid, request.sign_in.clone()) {
            Ok(to) => to,
            Err(false) => {
                return Task::ready(Err(api_error(codes::NOT_FOUND, "That core is gone.")));
            }
            Err(true) => {
                return Task::ready(Err(api_error(
                    codes::INVALID_PARAMS,
                    "Creating a server on this core needs a username and a password (the password from stdin), or this computer connected to it as its owner (`cores claim`).",
                )));
            }
        };
        let instances = self.instances.read(cx);
        let store = instances.store();
        let images = instances.images();
        let temp_dir = instances.host_uploads_dir();
        let Ok(instance) = store.get(instance_id) else {
            return Task::ready(Err(api_error(codes::NOT_FOUND, "The instance is gone.")));
        };
        let dirs = launcher_core::content::InstanceDirs::new(store.dir(instance_id));
        let set_stage = move |text: &str| {
            if let Ok(mut stage) = stage.lock() {
                *stage = text.to_string();
            }
        };
        let core_name = to.core_name.clone();
        let (core_uuid, core_url) = (to.core_uuid.clone(), to.core_url.clone());
        let instance_id = instance_id.to_string();
        let work = cx.background_spawn(async move {
            let failed = |err: HostError| host_api_error(&err, &core_name);
            // away from the window: each world's folder is walked for its size. Checked
            // before anything is written or sent.
            let worlds = hosting::worlds_to_send(
                &hosting::worlds(&dirs),
                request.worlds.as_deref(),
                request.world.as_ref().map(Option::as_deref),
            )
            .map_err(|why| api_error(codes::INVALID_PARAMS, why))?;
            set_stage("Signing in…");
            let admin = to
                .auth
                .admin(to.client)
                .await
                .map_err(|e| failed(e.into()))?;
            hosting::probe(&admin).await.map_err(failed)?;
            let (progress, cancel) = (request.progress.clone(), request.cancel.clone());
            set_stage("Writing the instance as a file…");
            let mut uploaded = hosting::upload(
                &admin, &images, &instance, &worlds, &temp_dir, &progress, &cancel,
            )
            .await
            .map_err(failed)?;
            debug_assert_eq!(progress.stage(), HostStage::Reading);
            let summary = uploaded.upload.summary.clone();
            // a server software the file doesn't allow is refused, never quietly
            // swapped for the file's own: checked before the preview returns too
            let flavour = match request.server_software.as_deref() {
                None => None,
                Some(typed) => Some(
                    server_software_named(&summary, typed)
                        .map_err(|why| api_error(codes::INVALID_PARAMS, why))?,
                ),
            };
            if request.preview {
                // dropping the upload lets the core forget it
                return Ok(Hosted::Preview(Box::new(summary)));
            }
            if !lodestone_ui::sharing::can_create(&summary, uploaded.may_write_global) {
                let why = lodestone_ui::sharing::unsupported(&summary).unwrap_or_else(|| {
                    if summary.needs_global_files && !uploaded.may_write_global {
                        "The instance has jars of its own, and your user on the host may not change global files, which they need. Ask its owner.".into()
                    } else {
                        "The host can't make a server from this instance.".into()
                    }
                });
                return Err(api_error(codes::UNAVAILABLE, why));
            }
            if cancel.is_cancelled() {
                return Err(failed(HostError::Cancelled));
            }
            let port = match request.port {
                Some(port) => port,
                None => match admin.free_port(hosting::FIRST_PORT, &cancel).await {
                    Ok(port) => port,
                    Err(_) if cancel.is_cancelled() => return Err(failed(HostError::Cancelled)),
                    Err(e) => return Err(failed(e.into())),
                },
            };
            let world = match &request.world {
                Some(world) => world.clone(),
                None => lodestone_ui::sharing::default_world(&summary),
            };
            let choices = HostChoices {
                name: request.name.clone(),
                port,
                world,
                flavour: flavour
                    .and_then(|picked| lodestone_ui::sharing::chosen_server(&summary, Some(&picked))),
                min_ram: Some(hosting::DEFAULT_MIN_RAM.min(request.max_ram)),
                max_ram: Some(request.max_ram),
                // checked by the caller: the user agreed
                accept_eula: request.accept_eula,
            };
            set_stage("Creating the server…");
            let uuid = hosting::create(&admin, &mut uploaded, &choices)
                .await
                .map_err(failed)?;
            drop(uploaded);
            set_stage("The host is making the server…");
            let ended = hosting::wait_created(
                &admin,
                &uuid,
                &|_| {},
                &cancel,
                &launcher_core::cores::player_image::tokio_sleep(),
            )
            .await;
            // Cancel past the creation's request stops only the wait: the core goes on.
            if matches!(ended, Err(HostError::Cancelled)) {
                return Err(api_error(
                    codes::CANCELLED,
                    format!(
                        "Stopped waiting: {core_name} goes on making the server. It shows in `servers list` once it's ready; this instance isn't linked to it."
                    ),
                ));
            }
            Ok(Hosted::Asked {
                uuid,
                name: request.name,
                port,
                ended: Box::new(ended),
            })
        });
        let core_name = self.cores[self.core_index(&core_uuid).unwrap_or_default()]
            .saved
            .name
            .clone();
        cx.spawn(async move |this, cx| {
            let (uuid, name, port, ended) = match work.await? {
                Hosted::Preview(summary) => {
                    return Ok(json!({ "preview": true, "summary": summary_json(&summary) }));
                }
                Hosted::Asked {
                    uuid,
                    name,
                    port,
                    ended,
                } => (uuid, name, port, ended),
            };
            // linked only once the creation ended well
            let linked = this
                .update(cx, |this, cx| {
                    this.link_hosted(&instance_id, (&core_uuid, &core_url), (&uuid, &name), *ended, cx)
                })
                .map_err(|_| api_error(codes::FAILED, "The launcher is closing."))?;
            let (end, relinked, linked) = match linked {
                Ok(hosting::Linked::Made { end, previous }) => (end, previous, true),
                Ok(hosting::Linked::InstanceGone { end }) => (end, None, false),
                Ok(hosting::Linked::Failed(why)) => {
                    return Err(api_error(
                        codes::FAILED,
                        why.unwrap_or_else(|| "The host couldn't make the server.".into()),
                    ));
                }
                Err(err) => return Err(host_api_error(&err, &core_name)),
            };
            Ok(json!({
                "server": uuid,
                "name": name,
                "core": core_name,
                "port": port,
                "state": "stopped",
                "relinked_from": relinked,
                "linked": linked,
                "outcome": end.outcome,
                "message": format!(
                    "{name} is on {core_name}, on port {port}. It's stopped until you start it, and {}.{}",
                    if linked {
                        "this instance is linked to it"
                    } else {
                        "the instance was deleted meanwhile, so nothing is linked to it"
                    },
                    end.message
                        .as_deref()
                        .and_then(|m| m.strip_prefix("Server created successfully. "))
                        .map(|m| format!(" Left out: {}", launcher_core::images::display_text(m)))
                        .unwrap_or_default()
                ),
            }))
        })
    }
}

/// What [`Servers::control_host`] came to.
enum Hosted {
    Preview(Box<launcher_core::cores::admin::images::ImageSummary>),
    /// The creation was asked for, and followed to `ended`.
    Asked {
        uuid: String,
        name: String,
        port: u16,
        ended: Box<Result<hosting::CreationEnd, HostError>>,
    },
}

/// What the core read from the file, for the control API: the summary as the core gave
/// it.
fn summary_json(summary: &launcher_core::cores::admin::images::ImageSummary) -> serde_json::Value {
    serde_json::to_value(summary).unwrap_or(serde_json::Value::Null)
}

/// The server software `typed` by a caller, as the file's choices spell it, or why it
/// can't be made, naming the choices, in English.
fn server_software_named(
    summary: &launcher_core::cores::admin::images::ImageSummary,
    typed: &str,
) -> Result<String, String> {
    if let Some(choice) = lodestone_ui::sharing::server_choice(summary, typed) {
        return Ok(choice.to_string());
    }
    let choices = &summary.server_choices;
    Err(match choices.as_slice() {
        [] => "The host can't make a server from this instance, so there is no server software to pick.".into(),
        [only] => format!("This file's server can only be {only}."),
        [first @ .., last] => format!("This file's server can be {} or {last}.", first.join(", ")),
    })
}

/// A hosting error as the control API says it.
fn host_api_error(err: &HostError, core_name: &str) -> ErrorObject {
    let code = match err {
        HostError::Core(err) => return core_error(err.clone()),
        HostError::NeedsUpdate | HostError::Quilt => codes::UNAVAILABLE,
        HostError::Forbidden(_) | HostError::Refused(_) => codes::PERMISSION_DENIED,
        HostError::Running => codes::BUSY,
        HostError::Cancelled => codes::CANCELLED,
        HostError::Image(_) | HostError::StillCreating => codes::FAILED,
    };
    let message = match err {
        HostError::NeedsUpdate => format!(
            "{core_name}'s Lodestone is too old to make servers from files. Update it on that computer, then try again."
        ),
        other => other.to_string(),
    };
    api_error(code, message)
}

#[cfg(test)]
mod tests {
    use super::{same_fingerprint, server_software_named};
    use launcher_core::cores::admin::images::ImageSummary;

    #[test]
    fn a_server_software_the_file_doesnt_allow_is_refused() {
        let summary = |choices: serde_json::Value| -> ImageSummary {
            serde_json::from_value(serde_json::json!({
                "minecraft": "1.21.1", "loader": "vanilla", "server_choices": choices
            }))
            .unwrap()
        };
        let vanilla = summary(serde_json::json!(["Vanilla", "Paper"]));
        assert_eq!(
            server_software_named(&vanilla, "vanilla").unwrap(),
            "Vanilla"
        );
        assert_eq!(
            server_software_named(&vanilla, "Spigot").unwrap_err(),
            "This file's server can be Vanilla or Paper."
        );
        let fabric = summary(serde_json::json!(["Fabric"]));
        assert_eq!(
            server_software_named(&fabric, "Paper").unwrap_err(),
            "This file's server can only be Fabric."
        );
        assert!(server_software_named(&summary(serde_json::json!([])), "Paper").is_err());
    }

    #[test]
    fn fingerprints_compare_loosely() {
        assert!(same_fingerprint("AB:CD:01", "abcd01"));
        assert!(same_fingerprint(" ab cd 01 ", "AB:CD:01"));
        assert!(!same_fingerprint("AB:CD:01", "AB:CD:02"));
        assert!(!same_fingerprint("", ""));
    }
}
