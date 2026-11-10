//! The app's account state: the account list from `launcher-core`, the
//! in-memory Minecraft sessions, the skin heads, and the Keychain.

pub mod auth_errors;
mod keychain;
pub mod offline_dialog;
pub mod sign_in;

use std::{
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime},
};

use gpui::{App, AppContext as _, AsyncApp, Context, Entity, RenderImage, SharedString, Task};
use http_client::HttpClient;
use launcher_core::{
    MemorySecretStore, SecretStore,
    accounts::{Account, AccountStore, OpenProblem},
    auth::{
        AuthError, Authenticator, ClientIdentity, MinecraftSession, MsaConfig, SignedIn, minecraft,
    },
    game::launch::LaunchAccount,
    paths::DataDir,
    settings, skin,
};

use crate::i18n::{ShellMsg as M, t, tr};

pub use keychain::KeychainStore;

/// Where refresh tokens are kept.
#[derive(Clone)]
pub enum Secrets {
    /// The macOS Keychain, through GPUI's credentials API.
    Keychain,
    /// In memory, for the offscreen screenshots.
    Memory(MemorySecretStore),
}

impl Secrets {
    pub fn store(&self, cx: &AsyncApp) -> Rc<dyn SecretStore> {
        match self {
            Secrets::Keychain => Rc::new(KeychainStore::new(cx.clone())),
            Secrets::Memory(store) => Rc::new(store.clone()),
        }
    }
}

/// The state of an account's Minecraft session.
#[derive(Debug)]
pub enum SessionState {
    /// Signing in again with the saved refresh token.
    Refreshing,
    Ready(MinecraftSession),
    /// The saved sign-in is gone or was refused; the user has to sign in again.
    NeedsSignIn,
    /// The refresh failed for another reason, such as no network. The account is
    /// kept, and the refresh is tried again on the next launch or switch.
    Failed(AuthError),
}

/// The accounts, shared by the sidebar and the sign-in dialogs.
pub struct Accounts {
    store: AccountStore,
    data_dir: DataDir,
    auth: Authenticator,
    http: Arc<dyn HttpClient>,
    secrets: Secrets,
    sessions: HashMap<String, SessionState>,
    /// Decoded skin heads, by account id.
    avatars: HashMap<String, Arc<RenderImage>>,
    /// A problem with `accounts.json` to show the user.
    notice: Option<SharedString>,
    /// Offline accounts are on whatever the setting says: a debug build, or
    /// `LODESTONE_LAUNCHER_OFFLINE=1`.
    offline_forced: bool,
    /// Settings > Developer > "Offline accounts".
    offline_setting: bool,
    /// Settings > Developer > "Sign in as the Minecraft Launcher". New Microsoft
    /// sign-ins use the official launcher's client while it is on.
    launcher_identity: bool,
    /// Refreshes and scheduled refreshes, by account id. Dropping one cancels it.
    tasks: HashMap<String, Task<()>>,
    avatar_tasks: HashMap<String, Task<()>>,
}

impl Accounts {
    /// Adds the decoded skin heads, for the performance monitor.
    pub fn tally_images(&self, tally: &mut crate::perf::ImageTally) {
        for image in self.avatars.values() {
            tally.add(image);
        }
    }

    /// Opens the account list in `data_dir` and refreshes the active account.
    pub fn new(
        data_dir: DataDir,
        http: Arc<dyn HttpClient>,
        config: MsaConfig,
        secrets: Secrets,
        offline_enabled: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let (store, problem) = AccountStore::open(data_dir.accounts_file());
        let notice = problem.map(|problem| {
            SharedString::from(match problem {
                OpenProblem::TooNew(_) => t(M::AccountsTooNew).to_string(),
                OpenProblem::Corrupt { moved_to, .. } => match moved_to {
                    Some(path) => tr!(M::AccountsCorruptMoved, path = path.display()),
                    None => t(M::AccountsCorrupt).to_string(),
                },
                OpenProblem::Unreadable(err) => tr!(M::CouldntReadAccounts, error = err),
            })
        });
        let developer = settings::load_or_default(&data_dir.settings_file()).developer;
        let launcher_identity = developer.sign_in_as_minecraft_launcher;
        let mut this = Self {
            store,
            data_dir,
            auth: Authenticator::new(http.clone(), config),
            http,
            secrets,
            sessions: HashMap::new(),
            avatars: HashMap::new(),
            notice,
            offline_forced: offline_enabled,
            offline_setting: developer.offline_accounts,
            launcher_identity,
            tasks: HashMap::new(),
            avatar_tasks: HashMap::new(),
        };
        for account in this.store.accounts().to_vec() {
            this.load_avatar(&account, cx);
        }
        if let Some(active) = this.store.active().cloned() {
            this.refresh(&active, cx);
        }
        this
    }

    pub fn accounts(&self) -> &[Account] {
        self.store.accounts()
    }

    pub fn active(&self) -> Option<&Account> {
        self.store.active()
    }

    pub fn session(&self, id: &str) -> Option<&SessionState> {
        self.sessions.get(id)
    }

    pub fn avatar(&self, id: &str) -> Option<&Arc<RenderImage>> {
        self.avatars.get(id)
    }

    pub fn notice(&self) -> Option<&SharedString> {
        self.notice.as_ref()
    }

    /// Whether offline accounts can be added: a debug build,
    /// `LODESTONE_LAUNCHER_OFFLINE=1`, or Settings > Developer > "Offline accounts".
    pub fn offline_enabled(&self) -> bool {
        self.offline_forced || self.offline_setting
    }

    /// Offline accounts are on in this build whatever the setting says.
    pub fn offline_forced(&self) -> bool {
        self.offline_forced
    }

    /// Turns Settings > Developer > "Offline accounts" on or off and saves it in
    /// `settings.json`. Offline accounts already added are kept. The error says why
    /// it couldn't be saved; the setting is then unchanged.
    pub fn set_offline_accounts(
        &mut self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), SharedString> {
        let result = self.save_developer(|developer| developer.offline_accounts = enabled);
        if result.is_ok() {
            self.offline_setting = enabled;
        }
        cx.notify();
        result
    }

    /// Changes the developer section of `settings.json`, which the accounts own:
    /// read as the file is now, changed and written back, and traced.
    fn save_developer(
        &self,
        change: impl FnOnce(&mut settings::DeveloperSettings),
    ) -> Result<(), SharedString> {
        let mut before = None;
        let saved = settings::update(&self.data_dir.settings_file(), |settings| {
            before = Some(settings.developer.clone());
            change(&mut settings.developer);
        })
        .map_err(|err| SharedString::from(tr!(M::CouldntSaveSettings, error = err)))?;
        launcher_core::trace::settings_changed(
            "launcher",
            &serde_json::json!({ "developer": before }),
            &serde_json::json!({ "developer": saved.developer }),
            serde_json::Value::Null,
        );
        Ok(())
    }

    /// Whether "Sign in as the Minecraft Launcher" is on.
    pub fn launcher_identity_enabled(&self) -> bool {
        self.launcher_identity
    }

    /// The client a new Microsoft sign-in uses. Refreshes always use the client
    /// each account was signed in with.
    pub fn sign_in_identity(&self) -> ClientIdentity {
        if self.launcher_identity {
            ClientIdentity::MinecraftLauncher
        } else {
            ClientIdentity::Lodestone
        }
    }

    /// The accounts signed in as the Minecraft Launcher.
    pub fn launcher_identity_accounts(&self) -> Vec<Account> {
        self.store
            .launcher_identity_accounts()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Turns "Sign in as the Minecraft Launcher" on or off and saves it in
    /// `settings.json`. Accounts already signed in are not changed. The error says
    /// why it couldn't be saved; the setting is then unchanged.
    pub fn set_launcher_identity(
        &mut self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), SharedString> {
        let result =
            self.save_developer(|developer| developer.sign_in_as_minecraft_launcher = enabled);
        if result.is_ok() {
            self.launcher_identity = enabled;
        }
        cx.notify();
        result
    }

    /// Signs out (removes) every account signed in as the Minecraft Launcher,
    /// deleting their saved sign-ins from the Keychain.
    pub fn sign_out_launcher_identity_accounts(&mut self, cx: &mut Context<Self>) {
        for account in self.launcher_identity_accounts() {
            self.remove(&account.id, cx);
        }
    }

    pub fn authenticator(&self) -> &Authenticator {
        &self.auth
    }

    pub fn secrets(&self) -> &Secrets {
        &self.secrets
    }

    pub fn data_dir(&self) -> &DataDir {
        &self.data_dir
    }

    fn report(&mut self, result: Result<(), impl std::fmt::Display>) {
        if let Err(err) = result {
            self.notice = Some(tr!(M::CouldntSaveAccounts, error = err).into());
        }
    }

    /// Makes `id` the active account, refreshing its session if needed.
    pub fn switch(&mut self, id: &str, cx: &mut Context<Self>) {
        let result = self.store.set_active(id);
        self.report(result.map(|_| ()));
        if let Some(account) = self.store.active().cloned() {
            let needs_refresh = match self.sessions.get(&account.id) {
                Some(SessionState::Ready(session)) => session.is_expired(SystemTime::now()),
                Some(SessionState::Refreshing) => false,
                _ => true,
            };
            if needs_refresh {
                self.refresh(&account, cx);
            }
        }
        cx.notify();
    }

    /// Adds a freshly signed-in account and makes it active.
    pub fn add_signed_in(&mut self, signed_in: SignedIn, cx: &mut Context<Self>) {
        let SignedIn { account, session } = signed_in;
        let result = self.store.upsert_and_activate(account.clone());
        self.report(result);
        self.set_session(&account.id, session, cx);
        self.load_avatar(&account, cx);
        cx.notify();
    }

    /// Adds an offline account and makes it active.
    pub fn add_offline(&mut self, account: Account, cx: &mut Context<Self>) {
        debug_assert!(account.is_offline());
        let result = self.store.upsert_and_activate(account);
        self.report(result);
        cx.notify();
    }

    /// Removes the account and deletes its Keychain item.
    pub fn remove(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(account) = self.store.get(id).cloned() else {
            return;
        };
        let secrets = self.secrets.clone();
        let task = cx.spawn(async move |this, cx| {
            if let Some(key) = account.secret_key() {
                let store = secrets.store(cx);
                if let Err(err) = store.delete(&key).await {
                    this.update(cx, |this, cx| {
                        this.notice = Some(
                            tr!(
                                M::CouldntRemoveSignIn,
                                name = account.username,
                                error = format!("{err:#}")
                            )
                            .into(),
                        );
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            }
            this.update(cx, |this, cx| {
                let result = this.store.forget(&account.id);
                this.report(result.map(|_| ()));
                this.sessions.remove(&account.id);
                this.avatars.remove(&account.id);
                this.avatar_tasks.remove(&account.id);
                if let Some(active) = this.store.active().cloned() {
                    this.switch(&active.id, cx);
                }
                cx.notify();
            })
            .ok();
        });
        // Replaces (and so cancels) any refresh of the account being removed.
        self.tasks.insert(id.to_string(), task);
    }

    /// Signs the account in again with its refresh token, in the background.
    pub fn refresh(&mut self, account: &Account, cx: &mut Context<Self>) {
        if account.is_offline() {
            return;
        }
        self.sessions
            .insert(account.id.clone(), SessionState::Refreshing);
        let auth = self.auth.clone();
        let secrets = self.secrets.clone();
        let account = account.clone();
        let id = account.id.clone();
        let task = cx.spawn(async move |this, cx| {
            let store = secrets.store(cx);
            let result = auth.refresh(&account, &*store).await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(signed_in) => {
                        let result = this.store.update_profile(
                            &account.id,
                            &signed_in.account.username,
                            signed_in.account.skin_url.clone(),
                        );
                        this.report(result);
                        this.set_session(&account.id, signed_in.session, cx);
                        this.load_avatar(&signed_in.account, cx);
                    }
                    Err(err) if err.needs_sign_in() => {
                        this.sessions
                            .insert(account.id.clone(), SessionState::NeedsSignIn);
                    }
                    Err(err) => {
                        // The message is safe to print: errors never hold tokens.
                        eprintln!("lodestone-launcher: refreshing {}: {err}", account.username);
                        this.sessions
                            .insert(account.id.clone(), SessionState::Failed(err));
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.tasks.insert(id, task);
    }

    /// Who to start the game as: the active account, with a fresh Minecraft token
    /// for a Microsoft account. An expired session is refreshed first; if that
    /// needs the browser, the error says so.
    pub fn launch_account(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Task<Result<LaunchAccount, String>> {
        let Some(account) = self.store.active().cloned() else {
            return Task::ready(Err(t(M::AddAccountFirst).into()));
        };
        if account.is_offline() {
            return Task::ready(Ok(LaunchAccount::offline(&account)));
        }
        if let Some(SessionState::Ready(session)) = self.sessions.get(&account.id)
            && !session.is_expired(SystemTime::now())
        {
            return Task::ready(Ok(launch_account_for(session)));
        }
        if matches!(
            self.sessions.get(&account.id),
            Some(SessionState::NeedsSignIn)
        ) {
            return Task::ready(Err(tr!(M::SignInExpired, name = account.username)));
        }
        self.sessions
            .insert(account.id.clone(), SessionState::Refreshing);
        cx.notify();
        let auth = self.auth.clone();
        let secrets = self.secrets.clone();
        cx.spawn(async move |this, cx| {
            let store = secrets.store(cx);
            let result = auth.refresh(&account, &*store).await;
            this.update(cx, |this, cx| {
                let outcome = match result {
                    Ok(signed_in) => {
                        let launch = launch_account_for(&signed_in.session);
                        let result = this.store.update_profile(
                            &account.id,
                            &signed_in.account.username,
                            signed_in.account.skin_url.clone(),
                        );
                        this.report(result);
                        this.set_session(&account.id, signed_in.session, cx);
                        Ok(launch)
                    }
                    Err(err) if err.needs_sign_in() => {
                        this.sessions
                            .insert(account.id.clone(), SessionState::NeedsSignIn);
                        Err(tr!(M::SignInExpired, name = account.username))
                    }
                    Err(err) => {
                        let message = auth_errors::message(&err);
                        this.sessions
                            .insert(account.id.clone(), SessionState::Failed(err));
                        Err(message)
                    }
                };
                cx.notify();
                outcome
            })
            .unwrap_or_else(|_| Err(t(M::LauncherClosing).into()))
        })
    }

    /// Stores the session and schedules a refresh shortly before it expires.
    fn set_session(&mut self, id: &str, session: MinecraftSession, cx: &mut Context<Self>) {
        let refresh_in = session
            .expires_at
            .duration_since(SystemTime::now())
            .unwrap_or_default()
            .saturating_sub(Duration::from_secs(10 * 60))
            .max(Duration::from_secs(60));
        self.sessions
            .insert(id.to_string(), SessionState::Ready(session));
        let id = id.to_string();
        let timer = cx.background_executor().timer(refresh_in);
        let task = cx.spawn({
            let id = id.clone();
            async move |this, cx| {
                timer.await;
                this.update(cx, |this, cx| {
                    // Only the active account is kept fresh; others refresh when
                    // they are switched to.
                    if let Some(account) = this.store.active().filter(|a| a.id == id).cloned() {
                        this.refresh(&account, cx);
                    } else {
                        this.sessions.remove(&id);
                    }
                })
                .ok();
            }
        });
        self.tasks.insert(id, task);
    }

    /// Shows the skin head. A head in the on-disk cache is decoded at once, so it
    /// is there on the first frame; otherwise it is downloaded, rendered and
    /// decoded in the background while the account box shows a placeholder of the
    /// same size.
    fn load_avatar(&mut self, account: &Account, cx: &mut Context<Self>) {
        let Some(skin_url) = account.skin_url.clone() else {
            self.avatars.remove(&account.id);
            return;
        };
        let Some(hash) = skin::texture_hash(&skin_url) else {
            return;
        };
        let cache_dir = self.data_dir.avatar_cache_dir();
        if let Some(image) = decode_cached(&skin::head_path(&cache_dir, &hash)) {
            self.avatars.insert(account.id.clone(), image);
            return;
        }
        let http = self.http.clone();
        let textures = self.auth.config().endpoints.textures.clone();
        let id = account.id.clone();
        let download = cx.background_spawn(async move {
            let path = skin::cached_head(&*http, &textures, &cache_dir, &skin_url).await?;
            crate::images::decode_png(&std::fs::read(path)?)
        });
        let task = cx.spawn({
            let id = id.clone();
            async move |this, cx| {
                let result = download.await;
                this.update(cx, |this, cx| {
                    match result {
                        Ok(image) => {
                            this.avatars.insert(id.clone(), image);
                        }
                        Err(err) => eprintln!("lodestone-launcher: loading a skin: {err:#}"),
                    }
                    this.avatar_tasks.remove(&id);
                    cx.notify();
                })
                .ok();
            }
        });
        self.avatar_tasks.insert(id, task);
    }

    /// Shows a cached head for an account without downloading anything. For the
    /// offscreen screenshots.
    pub fn set_avatar(&mut self, id: &str, path: PathBuf) {
        if let Some(image) = decode_cached(&path) {
            self.avatars.insert(id.to_string(), image);
        }
    }

    /// Turns the developer option on or off without saving it, for the offscreen
    /// screenshots.
    pub fn preview_launcher_identity(&mut self, enabled: bool) {
        self.launcher_identity = enabled;
    }

    /// Replaces the session state, for the offscreen screenshots.
    pub fn set_session_state(&mut self, id: &str, state: SessionState) {
        self.sessions.insert(id.to_string(), state);
    }

    /// Adds an account without signing in, for the offscreen screenshots.
    pub fn insert_for_preview(&mut self, account: Account) {
        let result = self.store.upsert_and_activate(account);
        self.report(result);
    }
}

/// The launch identity of a signed-in session. The token stays a [`Secret`].
///
/// [`Secret`]: launcher_core::Secret
fn launch_account_for(session: &MinecraftSession) -> LaunchAccount {
    LaunchAccount::microsoft(
        &session.profile.name,
        session.profile.id,
        session.access_token.clone(),
        minecraft::xuid_from_token(&session.access_token),
    )
}

/// Decodes a cached skin head; `None` if it is missing or unreadable.
fn decode_cached(path: &std::path::Path) -> Option<Arc<RenderImage>> {
    let bytes = std::fs::read(path).ok()?;
    crate::images::decode_png(&bytes).ok()
}

/// Creates the accounts entity for the app: the Keychain, the production
/// endpoints and the data directory.
pub fn create(cx: &mut App) -> Entity<Accounts> {
    let data_dir = DataDir::locate().unwrap_or_else(|| {
        let dir = std::env::temp_dir().join(launcher_core::paths::APP_DIR_NAME);
        eprintln!(
            "lodestone-launcher: no home directory; keeping data in {}",
            dir.display()
        );
        DataDir::at(dir)
    });
    let http = launcher_core::http::default_client().expect("could not start the HTTP client");
    let offline = launcher_core::offline_mode_enabled();
    cx.new(|cx| {
        Accounts::new(
            data_dir,
            http,
            MsaConfig::from_env(),
            Secrets::Keychain,
            offline,
            cx,
        )
    })
}

/// Settings > Developer's options as a release build has them: not forced on.
#[cfg(all(test, feature = "screenshot"))]
mod developer_setting_tests {
    use std::path::{Path, PathBuf};

    use gpui::{AppContext as _, TestAppContext};
    use launcher_core::{auth::MsaConfig, paths::DataDir};

    use super::{Accounts, MemorySecretStore, Secrets};

    /// A data folder of its own, removed when the test ends.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "lodestone-launcher-test-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn accounts(dir: &Path, cx: &mut TestAppContext) -> gpui::Entity<Accounts> {
        let dir = DataDir::at(dir.to_path_buf());
        cx.update(|cx| {
            cx.new(|cx| {
                Accounts::new(
                    dir,
                    launcher_core::http::default_client().unwrap(),
                    MsaConfig::from_env(),
                    Secrets::Memory(MemorySecretStore::new()),
                    false,
                    cx,
                )
            })
        })
    }

    #[test]
    fn offline_accounts_follow_the_saved_setting() {
        let mut cx = TestAppContext::single();
        let dir = TempDir::new("offline-setting");
        let accounts = accounts(dir.path(), &mut cx);
        cx.update(|cx| {
            assert!(!accounts.read(cx).offline_enabled());
            assert!(!accounts.read(cx).offline_forced());
            accounts
                .update(cx, |a, cx| a.set_offline_accounts(true, cx))
                .unwrap();
            assert!(accounts.read(cx).offline_enabled());
        });
        // a new start reads it back
        let again = self::accounts(dir.path(), &mut cx);
        cx.update(|cx| {
            assert!(again.read(cx).offline_enabled());
            again
                .update(cx, |a, cx| a.set_offline_accounts(false, cx))
                .unwrap();
            assert!(!again.read(cx).offline_enabled());
        });
    }

    #[test]
    fn a_setting_that_cant_be_saved_says_why_and_stays() {
        let mut cx = TestAppContext::single();
        let dir = TempDir::new("unsaved-setting");
        // A file this build can't read (a newer version) isn't overwritten.
        let settings = DataDir::at(dir.path().to_path_buf()).settings_file();
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::write(&settings, r#"{"version":999}"#).unwrap();
        let accounts = accounts(dir.path(), &mut cx);
        cx.update(|cx| {
            let notice = accounts.read(cx).notice().cloned();
            let saved = accounts.update(cx, |a, cx| a.set_offline_accounts(true, cx));
            assert!(saved.is_err());
            assert!(!accounts.read(cx).offline_enabled());
            let launcher = accounts.read(cx).launcher_identity_enabled();
            let saved = accounts.update(cx, |a, cx| a.set_launcher_identity(!launcher, cx));
            assert!(saved.is_err());
            assert_eq!(accounts.read(cx).launcher_identity_enabled(), launcher);
            // the accounts' own notice is left alone
            assert_eq!(accounts.read(cx).notice().cloned(), notice);
        });
        assert_eq!(
            std::fs::read_to_string(&settings).unwrap(),
            r#"{"version":999}"#
        );
    }
}
