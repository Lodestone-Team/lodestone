//! What the control API does with cores and servers that the Servers page does in its
//! dialogs: adding a core (with the certificate check), hosting an instance, and a
//! server's world and resource pack. The steps are the dialogs' own, with the answers
//! given as parameters instead of typed into a dialog, and the outcome returned instead
//! of shown in it. Starting, stopping, joining and "Create matching instance" need no
//! dialog, so the API calls the page's own methods for those.

use std::sync::{Arc, Mutex};

use gpui::{AppContext as _, Context, Task};
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    Secret,
    cores::{
        self, CoreClient, ServerInstance,
        client::display_address,
        hosting::{self, HostSettings},
        link,
        live::LauncherScope,
        resource_pack,
        store::{self, SavedCore},
        tls::CertificatePin,
        worlds as server_worlds,
    },
    export,
    game::download::{CancelToken, Progress},
};
use serde_json::json;

use super::{
    AddFailure, CoreEntry, CoreStatus, FINGERPRINT_HELP, Servers, certificate_outcome, pinned_http,
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

/// "Host on a Lodestone core", with the answers as parameters.
pub struct HostCore {
    pub core_uuid: String,
    pub username: String,
    pub password: Secret,
    pub name: String,
    pub max_ram: u32,
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

    /// The client for core `index` (accepting its pinned certificate), and its key. The
    /// key never leaves the launcher.
    pub(crate) fn control_client(&self, index: usize) -> (CoreClient, Option<Secret>) {
        let core = &self.cores[index];
        (self.client(core), core.key.clone())
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
                        this.notice = Some((
                            format!("{} is no longer trusted for mods", core.saved.name).into(),
                            format!("Another core answers at {address} now.").into(),
                        ));
                    }
                }
                let mut saved = match existing {
                    Some(index) => this.cores[index].saved.clone(),
                    None => SavedCore::default(),
                };
                saved.uuid = info.uuid.clone();
                saved.url = address.clone();
                saved.name = info.core_name.clone();
                if key.is_some() {
                    saved.username = sign_in_as.clone();
                    saved.key_id = key.as_ref().map(|(_, info)| info.id.clone());
                    saved.key_expires_at = key.as_ref().and_then(|(_, info)| info.expires_at);
                }
                if let Some(pin) = pin.clone()
                    && saved.pin_certificate(pin)
                {
                    this.notice = Some((
                        format!("{} is no longer trusted for mods", saved.name).into(),
                        "You trusted a new certificate for it. Trust it for mods again on its card if you're sure it is the same core.".into(),
                    ));
                }
                let signed_in = key.is_some();
                let entry = CoreEntry::new(
                    saved,
                    url,
                    key.map(|(secret, _)| secret)
                        .or_else(|| existing.and_then(|i| this.cores[i].key.clone())),
                );
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

    /// "Host on a Lodestone core": exports the instance, uploads it and creates a
    /// stopped server, then links the instance to it. `stage` follows the steps.
    pub(crate) fn control_host(
        &mut self,
        instance_id: &str,
        request: HostCore,
        stage: Arc<Mutex<String>>,
        cx: &mut Context<Self>,
    ) -> Task<Result<serde_json::Value, ErrorObject>> {
        let Some(index) = self.core_index(&request.core_uuid) else {
            return Task::ready(Err(api_error(codes::NOT_FOUND, "That core is gone.")));
        };
        let store = self.instances.read(cx).store();
        let Ok(instance) = store.get(instance_id) else {
            return Task::ready(Err(api_error(codes::NOT_FOUND, "The instance is gone.")));
        };
        if let Err(reason) = hosting::check_hostable(&instance) {
            return Task::ready(Err(api_error(codes::UNAVAILABLE, reason)));
        }
        let game_dir = store.game_dir(instance_id);
        let instance_dir = store.dir(instance_id);
        let core = &self.cores[index];
        let client = self.client(core);
        let core_name = core.saved.name.clone();
        let core_url = display_address(&core.url);
        let core_uuid = core.saved.uuid.clone();
        let previous = link::load_link(&instance_dir);
        let set_stage = move |text: &str| {
            if let Ok(mut stage) = stage.lock() {
                *stage = text.to_string();
            }
        };
        let work = cx.background_spawn(async move {
            set_stage("Exporting the modpack…");
            let on_modrinth = export::modrinth_files(&instance_dir);
            let plan = export::plan(&instance, &game_dir, &on_modrinth)
                .map_err(|e| api_error(codes::FAILED, e.to_string()))?;
            let file = instance_dir.join(format!(".lodestone-host-{}.mrpack", std::process::id()));
            let written = export::write(&plan, &file)
                .map_err(|e| e.to_string())
                .and_then(|()| hosting::read_pack_file(&file));
            let _ = std::fs::remove_file(&file);
            let bytes = written.map_err(|e| api_error(codes::FAILED, e))?;
            set_stage("Signing in…");
            let session = client
                .session(&request.username, &request.password)
                .await
                .map_err(core_error)?;
            drop(request.password);
            let port = client
                .free_port(&session, hosting::FIRST_PORT)
                .await
                .map_err(core_error)?;
            set_stage(&format!(
                "Uploading the modpack ({:.1} MB)…",
                bytes.len() as f64 / (1024.0 * 1024.0)
            ));
            let file_name = format!("{}.mrpack", request.name.replace(['/', '\\', ':'], "-"));
            let uploaded = client
                .upload_pack(&session, &file_name, &bytes)
                .await
                .map_err(core_error)?;
            if let Some(reason) = uploaded.unsupported_reason {
                return Err(api_error(codes::UNAVAILABLE, reason));
            }
            let upload_id = uploaded
                .upload_id
                .ok_or_else(|| api_error(codes::FAILED, "The core didn't keep the modpack."))?;
            set_stage("Creating the server…");
            let settings = HostSettings {
                name: request.name.clone(),
                port,
                min_ram: hosting::DEFAULT_MIN_RAM.min(request.max_ram),
                max_ram: request.max_ram,
            };
            let uuid = client
                .create_server(&session, &upload_id, &settings)
                .await
                .map_err(core_error)?;
            drop(session);
            let server_link = hosting::host_link(&core_uuid, &core_url, &uuid, &request.name);
            link::save_link(&instance_dir, &server_link)
                .map_err(|e| api_error(codes::FAILED, e.to_string()))?;
            Ok::<_, ErrorObject>((uuid, request.name, port, uploaded.server_file_count))
        });
        let refresh_core = self.cores[index].saved.uuid.clone();
        cx.spawn(async move |this, cx| {
            let (uuid, name, port, downloads) = work.await?;
            this.update(cx, |this, cx| {
                this.reload_links(cx);
                if let Some(index) = this.core_index(&refresh_core) {
                    this.refresh(index, cx);
                }
                cx.notify();
            })
            .ok();
            Ok(json!({
                "server": uuid,
                "name": name,
                "core": core_name,
                "port": port,
                "mod_files": downloads,
                "state": "stopped",
                "relinked_from": previous.map(|link| link.server_name),
                "message": format!("{name} is on {core_name}, on port {port}. The core is setting it up now; the server is stopped until you start it. This instance is linked to it."),
            }))
        })
    }

    /// A server's world, zipped by the core and imported into `instance_id` as a
    /// singleplayer copy. Signs in with the password once; the session is dropped after.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn control_download_world(
        &mut self,
        index: usize,
        server: ServerInstance,
        instance_id: &str,
        username: String,
        password: Secret,
        progress: Arc<Progress>,
        stage: Arc<Mutex<String>>,
        cancel: CancelToken,
        cx: &mut Context<Self>,
    ) -> Task<Result<String, ErrorObject>> {
        let client = self.client(&self.cores[index]);
        let dirs = self.instances.read(cx).content_dirs(instance_id);
        let set_stage = move |text: &str| {
            if let Ok(mut stage) = stage.lock() {
                *stage = text.to_string();
            }
        };
        cx.background_spawn(async move {
            set_stage("Signing in…");
            let session = client
                .session(&username, &password)
                .await
                .map_err(core_error)?;
            drop(password);
            set_stage("The core is zipping the world…");
            let download = client
                .world_download(&session, &server.uuid)
                .await
                .map_err(core_error)?;
            drop(session);
            set_stage("Downloading");
            std::fs::create_dir_all(&dirs.instance)
                .map_err(|e| api_error(codes::FAILED, e.to_string()))?;
            let zip = dirs.instance.join(format!(
                ".lodestone-server-world-{}.zip",
                std::process::id()
            ));
            let result = async {
                client
                    .download_file(&download.key, &zip, 64 << 30, &progress, &cancel)
                    .await
                    .map_err(core_error)?;
                set_stage("Importing…");
                server_worlds::import(&dirs, &zip, &download, &server.name, &progress, &cancel)
                    .map_err(|e| api_error(codes::FAILED, e.to_string()))
            }
            .await;
            let _ = std::fs::remove_file(&zip);
            result
        })
    }

    /// The server's resource pack, saved in the instance's `resourcepacks/`.
    pub(crate) fn control_download_pack(
        &mut self,
        index: usize,
        server: ServerInstance,
        instance_id: &str,
        cx: &mut Context<Self>,
    ) -> Task<Result<String, ErrorObject>> {
        let (client, key) = self.control_client(index);
        let Some(key) = key else {
            return Task::ready(Err(api_error(
                codes::NOT_SIGNED_IN,
                "Sign in to this core to get the server's resource pack: `cores add <address> --username <name>`.",
            )));
        };
        let packs = self
            .instances
            .read(cx)
            .game_dir(instance_id)
            .join("resourcepacks");
        let http = self.http.clone();
        cx.background_spawn(async move {
            let pack = client
                .resource_pack(&key, &server.uuid)
                .await
                .map_err(core_error)?
                .ok_or_else(|| api_error(codes::NOT_FOUND, "The server has no resource pack."))?;
            let path = resource_pack::download(http, &pack, &server.name, &packs)
                .await
                .map_err(|e| api_error(codes::FAILED, e))?;
            Ok(path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::same_fingerprint;

    #[test]
    fn fingerprints_compare_loosely() {
        assert!(same_fingerprint("AB:CD:01", "abcd01"));
        assert!(same_fingerprint(" ab cd 01 ", "AB:CD:01"));
        assert!(!same_fingerprint("AB:CD:01", "AB:CD:02"));
        assert!(!same_fingerprint("", ""));
    }
}
