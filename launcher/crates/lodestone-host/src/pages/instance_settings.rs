//! An instance's Settings tab (`InstanceSettings.tsx`): the core's settings manifest,
//! rendered setting by setting as the dashboard's `SettingField` does, then
//! automation, the resource pack, the Minecraft EULA and the danger zone.
//!
//! Each setting saves on its own: a switch or a list as soon as it changes, a text
//! field on Enter or its save button (Escape puts the saved value back). The resource
//! pack is kept as a draft until Save, so its lines change together.

mod field;

use std::collections::HashMap;

use gpui::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Task, Window, div, prelude::*,
};
use launcher_core::cores::{
    CoreError,
    admin::{
        InstanceInfo, InstanceState,
        core_settings::{InstancePublicListing, PublicScopeChange},
        instance_settings::{
            ConfigurableManifest, ConfigurableValue, ConfigurableValueType, EulaStatus,
            MINECRAFT_EULA_URL, PackUrlError, ResourcePackHash, ServerResourcePack, is_sha1,
            pack_url_error,
        },
        models::{CorePermission, Game, InstancePermission},
    },
};
use lodestone_ui::{
    components::{
        Button, ButtonSize, ButtonVariant, Dialog, DialogSlot, LoadingDots, TextInput, Toast,
        Toggle,
        a11y::Accessible as _,
        settings::{SectionNav, SettingRow, card, content_width, link, section, section_list, tag},
        window_layer,
    },
    i18n::format,
    theme::{colors, ink, leading, px, text, weights},
};

use self::field::{Field, FieldKind, Problem};
use crate::{
    i18n::{CommonMsg, CoreMsg, InstanceMsg as M, t, tr},
    routes::Route,
    state::HostState,
};

/// What a setting is called in the page's maps: its section and its id.
type Key = (String, String);

/// Something the page fetches when it opens.
enum Load<T> {
    Loading,
    Ready(T),
    Failed(String),
}

/// The resource pack's draft, while it isn't saved.
struct PackForm {
    saved: ServerResourcePack,
    url: Entity<TextInput>,
    sha1: Entity<TextInput>,
    prompt: Entity<TextInput>,
    required: bool,
    busy: Option<PackBusy>,
    /// "Downloaded 1.5 MB." after Compute.
    hash_note: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PackBusy {
    Saving,
    Hashing,
}

/// Auto start or restart on crash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Automation {
    AutoStart,
    RestartOnCrash,
}

/// The delete confirmation.
struct DeletePrompt {
    focus: FocusHandle,
    name: String,
}

pub struct InstanceSettingsPage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
    manifest: Load<ConfigurableManifest>,
    /// The text fields of the manifest's settings.
    fields: HashMap<Key, Field>,
    /// The list of the enum setting that is open.
    open_menu: Option<Key>,
    /// Switches and lists being saved, with the value shown meanwhile.
    pending: HashMap<Key, ConfigurableValue>,
    automation_pending: Option<(Automation, bool)>,
    pack: Option<PackForm>,
    eula: Option<EulaStatus>,
    eula_busy: bool,
    /// The public listing (package D's row), for a Minecraft Java server on a core
    /// that has its scopes.
    listing: Option<InstancePublicListing>,
    listing_saving: bool,
    delete: DialogSlot<Self, DeletePrompt>,
    deleting: bool,
    nav: SectionNav,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl InstanceSettingsPage {
    pub fn new(state: Entity<HostState>, uuid: String, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe(&state, |_, _, cx| cx.notify())];
        let mut this = Self {
            state,
            uuid,
            manifest: Load::Loading,
            fields: HashMap::new(),
            open_menu: None,
            pending: HashMap::new(),
            automation_pending: None,
            pack: None,
            eula: None,
            eula_busy: false,
            listing: None,
            listing_saving: false,
            delete: DialogSlot::new(|this: &mut Self| &mut this.delete),
            deleting: false,
            nav: SectionNav::default(),
            _tasks: Vec::new(),
            _subscriptions: subscriptions,
        };
        this.load(cx);
        this
    }

    fn instance<'a>(&self, cx: &'a Context<Self>) -> Option<&'a InstanceInfo> {
        self.state.read(cx).instance(&self.uuid)
    }

    fn is_java(&self, cx: &Context<Self>) -> bool {
        self.instance(cx)
            .is_some_and(|i| matches!(i.game_type, Game::MinecraftJava { .. }))
    }

    /// Atom instances are the owner's alone (`useCanControlInstance`).
    fn can_control(&self, cx: &Context<Self>) -> bool {
        let state = self.state.read(cx);
        state.me.is_owner || !self.instance(cx).is_some_and(|i| i.is_atom())
    }

    fn can_edit(&self, cx: &Context<Self>) -> bool {
        self.can_control(cx)
            && self
                .state
                .read(cx)
                .me
                .may(InstancePermission::Settings, &self.uuid)
    }

    /// Why `java_cmd` and `cmd_args` can't change, if they can't
    /// (`useCommandLineBlockedReason`).
    fn command_line_blocked(&self, cx: &Context<Self>) -> Option<M> {
        let state = self.state.read(cx);
        let settings = state.settings.as_ref();
        let enabled = settings.is_none_or(|s| !s.code_execution_locked && s.code_execution_enabled);
        if !enabled {
            return Some(M::CommandLineReadOnly);
        }
        if state.me.is_owner {
            return None;
        }
        let safe_mode_off = settings.is_some_and(|s| !s.safe_mode);
        if state.me.may_core(CorePermission::WriteGlobalFile) && safe_mode_off {
            return None;
        }
        Some(M::CommandLineOwnerOnly)
    }

    /// Fetches the settings (every visit, as the dashboard does), and for a Minecraft
    /// Java server its resource pack and EULA.
    fn load(&mut self, cx: &mut Context<Self>) {
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let java = self.is_java(cx);
        // auto start and restart on crash change from the CLI too
        self.state.update(cx, |state, cx| state.reload(cx));
        let task = cx.spawn(async move |this, cx| {
            let work = cx.background_spawn(async move {
                let manifest = admin.instance_settings(&uuid).await;
                let (pack, eula, listing) = if java {
                    (
                        admin.resource_pack(&uuid).await.ok(),
                        admin.eula(&uuid).await.ok(),
                        // hidden on cores before the listing's scopes
                        admin
                            .instance_public_listing(&uuid)
                            .await
                            .ok()
                            .filter(|l| l.has_scopes()),
                    )
                } else {
                    (None, None, None)
                };
                (manifest, pack, eula, listing)
            });
            let (manifest, pack, eula, listing) = work.await;
            this.update(cx, |this, cx| {
                match manifest {
                    Ok(manifest) => {
                        this.make_fields(&manifest, cx);
                        this.manifest = Load::Ready(manifest);
                    }
                    Err(CoreError::SignedOut) => {
                        this.state.update(cx, |state, cx| state.reload(cx));
                        this.manifest = Load::Failed(CoreError::SignedOut.to_string());
                    }
                    Err(err) => this.manifest = Load::Failed(err.to_string()),
                }
                if let Some(pack) = pack {
                    this.pack = Some(this.pack_form(pack, cx));
                }
                this.eula = eula;
                this.listing = listing;
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    fn push(&mut self, task: Task<()>) {
        self._tasks.retain(|task| !task.is_ready());
        self._tasks.push(task);
    }

    /// A text field for each setting typed into one.
    fn make_fields(&mut self, manifest: &ConfigurableManifest, cx: &mut Context<Self>) {
        let can_edit = self.can_edit(cx);
        let blocked = self.command_line_blocked(cx);
        self.fields.clear();
        for section in &manifest.setting_sections {
            let section_id = &section.section_id;
            for setting in &section.settings {
                let setting_id = &setting.setting_id;
                let Some(kind) = FieldKind::of(&setting.value_type, setting.is_secret) else {
                    continue;
                };
                let mutable = setting.is_mutable
                    && can_edit
                    && !(field::is_command_line(setting_id) && blocked.is_some());
                let text_ = setting
                    .value
                    .as_ref()
                    .map(|v| v.to_text())
                    .unwrap_or_default();
                let field = Field::new(kind, text_, !mutable, cx);
                self._subscriptions
                    .push(cx.observe(&field.input, |_, _, cx| cx.notify()));
                self.fields
                    .insert((section_id.clone(), setting_id.clone()), field);
            }
        }
    }

    fn pack_form(&mut self, saved: ServerResourcePack, cx: &mut Context<Self>) -> PackForm {
        let disabled = !self.can_edit(cx);
        let input = |text_: &str, label: M, cx: &mut Context<Self>| {
            let text_ = SharedString::from(text_.to_string());
            cx.new(|cx| {
                TextInput::new(cx)
                    .with_label(label)
                    .with_text(text_)
                    .with_disabled(disabled)
            })
        };
        let url = input(&saved.url, M::PackUrl, cx);
        url.update(cx, |input, cx| {
            input.set_placeholder("https://example.com/pack.zip", cx)
        });
        let sha1 = input(&saved.sha1, M::PackSha1, cx);
        let prompt = input(&saved.prompt, M::PackPrompt, cx);
        for input in [&url, &sha1, &prompt] {
            self._subscriptions
                .push(cx.observe(input, |this, input, cx| {
                    // a new URL makes the last download's size stale
                    if let Some(pack) = &mut this.pack
                        && pack.url == input
                    {
                        pack.hash_note = None;
                    }
                    cx.notify()
                }));
        }
        PackForm {
            required: saved.required,
            saved,
            url,
            sha1,
            prompt,
            busy: None,
            hash_note: None,
        }
    }

    fn show_error(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.state
            .update(cx, |state, cx| state.toast(Toast::error(message), cx));
    }

    /// The "Saved" toast after a change (`notifySaved`), one at a time.
    fn show_saved(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.toast(Toast::success(message).key("settings-saved"), cx)
        });
    }

    /// Saves a setting's new value.
    fn save_setting(&mut self, key: Key, value: ConfigurableValue, cx: &mut Context<Self>) {
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let (section, setting) = key.clone();
        let sent = value.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    admin
                        .set_instance_setting(&uuid, &section, &setting, &sent)
                        .await
                })
                .await;
            this.update(cx, |this, cx| {
                this.pending.remove(&key);
                if let Some(field) = this.fields.get_mut(&key) {
                    field.saving = false;
                }
                match result {
                    Ok(()) => {
                        if let Load::Ready(manifest) = &mut this.manifest {
                            manifest.set(&key.0, &key.1, value.clone());
                        }
                        if let Some(field) = this.fields.get_mut(&key) {
                            field.saved(value.to_text(), cx);
                        }
                        this.show_saved(t(CommonMsg::Saved), cx);
                    }
                    Err(err) => match this.fields.get_mut(&key) {
                        Some(field) => field.error = Some(err.to_string()),
                        None => this.show_error(err.to_string(), cx),
                    },
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// A switch or a list changed: saved at once, its new value shown meanwhile.
    fn change(&mut self, key: Key, value: ConfigurableValue, cx: &mut Context<Self>) {
        self.open_menu = None;
        self.pending.insert(key.clone(), value.clone());
        cx.notify();
        self.save_setting(key, value, cx);
    }

    /// Enter or the save button in a text field.
    fn submit(&mut self, key: Key, cx: &mut Context<Self>) {
        let Some(value_type) = self.value_type(&key) else {
            return;
        };
        let Some(field) = self.fields.get_mut(&key) else {
            return;
        };
        if field.disabled || field.saving {
            return;
        }
        let trimmed = field.text(cx).trim().to_string();
        field.input.update(cx, |input, cx| {
            if input.text().as_ref() != trimmed {
                input.set_text(trimmed.clone(), cx)
            }
        });
        match field::value_of(&value_type, &trimmed, true) {
            Ok(Some(value)) => {
                field.error = None;
                field.saving = true;
                cx.notify();
                self.save_setting(key, value, cx);
            }
            Ok(None) => {}
            Err(problem) => {
                field.error = Some(problem.text());
                cx.notify();
            }
        }
    }

    fn value_type(&self, key: &Key) -> Option<ConfigurableValueType> {
        let Load::Ready(manifest) = &self.manifest else {
            return None;
        };
        let section = manifest
            .setting_sections
            .iter()
            .find(|s| s.section_id == key.0)?;
        let setting = section.settings.iter().find(|s| s.setting_id == key.1)?;
        Some(setting.value_type.clone())
    }

    fn set_automation(&mut self, which: Automation, on: bool, cx: &mut Context<Self>) {
        self.automation_pending = Some((which, on));
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    match which {
                        Automation::AutoStart => admin.set_auto_start(&uuid, on).await,
                        Automation::RestartOnCrash => admin.set_restart_on_crash(&uuid, on).await,
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        // shown from the list until it is fetched again
                        this.state.update(cx, |state, cx| {
                            if let Some(list) = state.instances.as_mut()
                                && let Some(i) = list.iter_mut().find(|i| i.uuid == this.uuid)
                            {
                                match which {
                                    Automation::AutoStart => i.auto_start = on,
                                    Automation::RestartOnCrash => i.restart_on_crash = on,
                                }
                            }
                            state.reload(cx);
                        });
                        this.show_saved(t(CommonMsg::Saved), cx);
                    }
                    Err(err) => this.show_error(err.to_string(), cx),
                }
                this.automation_pending = None;
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    fn pack_draft(&self, cx: &Context<Self>) -> Option<ServerResourcePack> {
        let pack = self.pack.as_ref()?;
        Some(ServerResourcePack {
            url: pack.url.read(cx).text().to_string(),
            sha1: pack.sha1.read(cx).text().to_string(),
            required: pack.required,
            prompt: pack.prompt.read(cx).text().to_string(),
        })
    }

    fn save_pack(&mut self, pack: ServerResourcePack, cx: &mut Context<Self>) {
        let Some(form) = &mut self.pack else {
            return;
        };
        form.busy = Some(PackBusy::Saving);
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.set_resource_pack(&uuid, &pack).await })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(saved) => {
                        let stopped = this
                            .instance(cx)
                            .is_none_or(|i| i.state == InstanceState::Stopped);
                        this.pack = Some(this.pack_form(saved, cx));
                        this.show_saved(
                            t(if stopped {
                                M::PackSaved
                            } else {
                                M::PackSavedRestart
                            }),
                            cx,
                        );
                    }
                    Err(err) => {
                        if let Some(form) = &mut this.pack {
                            form.busy = None;
                        }
                        this.show_error(err.to_string(), cx);
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    fn compute_sha1(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &mut self.pack else {
            return;
        };
        let url = form.url.read(cx).text().trim().to_string();
        form.busy = Some(PackBusy::Hashing);
        form.hash_note = None;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.hash_resource_pack(&uuid, &url).await })
                .await;
            this.update(cx, |this, cx| {
                let Some(form) = &mut this.pack else {
                    return;
                };
                form.busy = None;
                match result {
                    Ok(ResourcePackHash { sha1, size }) => {
                        form.sha1.update(cx, |input, cx| input.set_text(sha1, cx));
                        form.hash_note = Some(tr!(M::PackDownloaded, size = format::bytes(size)));
                    }
                    Err(err) => this.show_error(err.to_string(), cx),
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// The user clicked "I agree to the Minecraft EULA". Nothing else accepts it.
    fn accept_eula(&mut self, cx: &mut Context<Self>) {
        self.eula_busy = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.accept_eula(&uuid).await })
                .await;
            this.update(cx, |this, cx| {
                this.eula_busy = false;
                match result {
                    Ok(status) => {
                        this.eula = Some(status);
                        this.show_saved(t(CommonMsg::Saved), cx);
                    }
                    Err(err) => {
                        this.show_error(tr!(M::EulaAcceptFailed, error = err), cx);
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    fn open_delete(&mut self, cx: &mut Context<Self>) {
        let name = self
            .instance(cx)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        let focus = cx.focus_handle();
        self.delete.open(DeletePrompt { focus, name }, cx);
    }

    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        if self.deleting {
            return;
        }
        self.deleting = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.delete_instance(&uuid).await })
                .await;
            this.update(cx, |this, cx| {
                this.deleting = false;
                this.delete.close(cx);
                match result {
                    Ok(()) => {
                        this.state.update(cx, |state, cx| {
                            state.reload(cx);
                            state.navigate(Route::Home, cx);
                        });
                    }
                    Err(err) => this.show_error(err.to_string(), cx),
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }
}

/// A paragraph under a card, such as a load error.
fn note(text_: impl Into<SharedString>, color: lodestone_ui::theme::Color) -> AnyElement {
    div()
        .text_size(text::SMALL)
        .text_color(color)
        .child(text_.into())
        .into_any_element()
}

impl InstanceSettingsPage {
    /// The manifest's sections, each a card of its settings.
    fn manifest_sections(&self, cx: &mut Context<Self>) -> Vec<(SharedString, AnyElement)> {
        let manifest = match &self.manifest {
            Load::Loading => {
                return vec![(
                    t(CommonMsg::Loading).into(),
                    card([div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(120.))
                        .child(LoadingDots)
                        .into_any_element()])
                    .into_any_element(),
                )];
            }
            Load::Failed(error) => {
                return vec![(
                    t(M::NoPermission).into(),
                    note(tr!(M::SettingsFailed, error = error), colors::RED_200),
                )];
            }
            Load::Ready(manifest) => manifest.clone(),
        };
        let can_edit = self.can_edit(cx);
        let can_control = self.can_control(cx);
        let blocked = self.command_line_blocked(cx);
        let mut sections = Vec::new();
        for section_ in &manifest.setting_sections {
            let section_id = &section_.section_id;
            let rows: Vec<AnyElement> = if section_.settings.is_empty() {
                vec![
                    SettingRow::new(t(M::NotAvailable))
                        .key(SharedString::from(format!("empty-{section_id}")))
                        .hint(t(M::NotAvailableHint))
                        .into_any_element(),
                ]
            } else {
                section_
                    .settings
                    .iter()
                    .map(|setting| {
                        let setting_id = &setting.setting_id;
                        let key = (section_id.clone(), setting_id.clone());
                        // why it is read-only, after its description
                        let reason = if !can_control {
                            Some(M::OwnerOnlyAtom)
                        } else if field::is_command_line(setting_id) {
                            blocked
                        } else {
                            None
                        };
                        let hint = if !can_edit {
                            t(M::NoPermission).to_string()
                        } else {
                            match reason {
                                Some(reason) if setting.description.is_empty() => {
                                    t(reason).to_string()
                                }
                                Some(reason) => format!("{} {}", setting.description, t(reason)),
                                None => setting.description.clone(),
                            }
                        };
                        let mutable = setting.is_mutable && can_edit && reason.is_none();
                        self.setting_row(key, setting, hint, mutable, cx)
                    })
                    .collect()
            };
            sections.push((
                SharedString::from(section_.name.clone()),
                section(
                    section_.name.clone(),
                    Some(section_.description.clone())
                        .filter(|d| !d.is_empty())
                        .map(Into::into),
                )
                .child(card(rows))
                .into_any_element(),
            ));
        }
        sections
    }

    fn setting_row(
        &self,
        key: Key,
        setting: &launcher_core::cores::admin::instance_settings::SettingManifest,
        hint: String,
        mutable: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row_id = SharedString::from(format!("setting-{}-{}", key.0, key.1));
        let row = SettingRow::new(setting.name.clone())
            .key(row_id.clone())
            .hint(hint);
        let value = self
            .pending
            .get(&key)
            .cloned()
            .or_else(|| setting.value.clone());
        let saving = self.pending.contains_key(&key);
        match &setting.value_type {
            ConfigurableValueType::Boolean => {
                let on = matches!(value, Some(ConfigurableValue::Boolean(true)));
                let changed = key.clone();
                row.child(
                    Toggle::new(row_id, on)
                        .label(setting.name.clone())
                        .disabled(!mutable || saving)
                        .on_change(cx.listener(move |this, on: &bool, _, cx| {
                            this.change(changed.clone(), ConfigurableValue::Boolean(*on), cx)
                        })),
                )
                .into_any_element()
            }
            ConfigurableValueType::Enum { options } => {
                let current = match &value {
                    Some(ConfigurableValue::Enum(v) | ConfigurableValue::String(v)) => v.clone(),
                    _ => String::new(),
                };
                let open = self.open_menu.as_ref() == Some(&key);
                row.child(field::enum_select(
                    row_id,
                    &setting.name,
                    &current,
                    options,
                    open,
                    mutable && !saving,
                    key,
                    cx,
                ))
                .into_any_element()
            }
            _ => {
                let Some(field) = self.fields.get(&key) else {
                    return row.into_any_element();
                };
                let typed = field.text(cx);
                let problem = if field.touched(cx) {
                    field::value_of(&setting.value_type, typed.trim(), false)
                        .err()
                        .map(|p: Problem| p.text())
                } else {
                    None
                };
                let error = field.error.clone().or(problem);
                let motd = (key.1 == "motd").then(|| field::motd_preview(&typed));
                row.child(field.render(key, cx))
                    .note_opt(error.map(|e| (e.into(), colors::RED_200)))
                    .when_some(motd, |row, motd| row.below(motd))
                    .into_any_element()
            }
        }
    }

    fn automation_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let instance = self.instance(cx)?.clone();
        let can_edit = self.can_edit(cx);
        let reason = if self.can_control(cx) {
            M::NoPermission
        } else {
            M::OwnerOnlyAtom
        };
        let saving = self.automation_pending.is_some();
        let rows = [
            (
                Automation::AutoStart,
                M::AutoStart,
                M::AutoStartHint,
                instance.auto_start,
            ),
            (
                Automation::RestartOnCrash,
                M::RestartOnCrash,
                M::RestartOnCrashHint,
                instance.restart_on_crash,
            ),
        ]
        .into_iter()
        .map(|(which, label, hint, saved)| {
            let value = match self.automation_pending {
                Some((pending, on)) if pending == which => on,
                _ => saved,
            };
            SettingRow::new(t(label))
                .hint(t(if can_edit { hint } else { reason }))
                .child(
                    Toggle::new(SharedString::from(format!("automation-{which:?}")), value)
                        .label(t(label))
                        .disabled(!can_edit || saving)
                        .on_change(cx.listener(move |this, on: &bool, _, cx| {
                            this.set_automation(which, *on, cx)
                        })),
                )
                .into_any_element()
        });
        Some(
            section(t(M::Automation), Some(t(M::AutomationLead).into()))
                .child(card(rows))
                .into_any_element(),
        )
    }

    /// Sharing (`PublicListingSetting.tsx`): whether the server is public and what it
    /// shows; only the owner changes it, and listing needs the core's switch on.
    fn sharing_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let listing = self.listing.clone()?;
        let can_edit = self.state.read(cx).me.is_owner;
        let row = crate::pages::public_listing::scope_row(
            "instance-listing",
            t(CoreMsg::ListedPublicly),
            listing.scope(),
            listing.list_instances_publicly,
            can_edit,
            self.listing_saving,
            {
                let page = cx.entity().downgrade();
                move |change: PublicScopeChange, _, cx| {
                    page.update(cx, |this, cx| this.change_listing(change, cx))
                        .ok();
                }
            },
        );
        Some(
            section(
                t(CoreMsg::ListingTitle),
                Some(t(CoreMsg::ListingLead).into()),
            )
            .child(card([row.into_any_element()]))
            .into_any_element(),
        )
    }

    fn change_listing(&mut self, change: PublicScopeChange, cx: &mut Context<Self>) {
        self.listing_saving = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.set_public_scope(&uuid, &change).await })
                .await;
            this.update(cx, |this, cx| {
                this.listing_saving = false;
                match result {
                    Ok(scope) => {
                        if let Some(listing) = &mut this.listing {
                            listing.listed = scope.listed;
                            listing.status = Some(scope.status);
                            listing.address = Some(scope.address);
                            listing.client_pack = Some(scope.client_pack);
                        }
                        this.show_saved(t(CommonMsg::Saved), cx);
                    }
                    Err(err) => this.show_error(err.to_string(), cx),
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    fn pack_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let form = self.pack.as_ref()?;
        let draft = self.pack_draft(cx)?;
        let can_edit = self.can_edit(cx);
        let busy = form.busy;
        let disabled = !can_edit || busy.is_some();
        let url_error = pack_url_error(&draft.url).map(|e| match e {
            PackUrlError::Spaces => t(M::PackUrlSpaces),
            PackUrlError::NotAUrl => t(M::PackNotAUrl),
            PackUrlError::Scheme => t(M::PackUrlScheme),
        });
        let sha1_error = (!is_sha1(&draft.sha1)).then(|| t(M::PackSha1Error));
        let saved = &form.saved;
        let changed = draft.url.trim() != saved.url
            || draft.sha1.trim().to_ascii_lowercase() != saved.sha1
            || draft.required != saved.required
            || draft.prompt.trim() != saved.prompt;
        let control = |input: &Entity<TextInput>| div().w(px(260.)).child(input.clone());
        let rows = vec![
            SettingRow::new(t(M::PackUrl))
                .key("pack-url")
                .hint(if can_edit {
                    t(M::PackUrlHint)
                } else {
                    t(M::NoPermission)
                })
                .child(control(&form.url))
                .note_opt(
                    url_error
                        .filter(|_| can_edit)
                        .map(|e| (e.into(), colors::RED_200)),
                )
                .into_any_element(),
            SettingRow::new(t(M::PackSha1))
                .key("pack-sha1")
                .hint(
                    form.hash_note
                        .clone()
                        .unwrap_or_else(|| t(M::PackSha1Hint).to_string()),
                )
                .child(control(&form.sha1).font_family(lodestone_ui::theme::fonts::mono()))
                .child(
                    Button::new("pack-compute", t(M::PackCompute))
                        .size(ButtonSize::Slim)
                        .loading(busy == Some(PackBusy::Hashing))
                        .disabled(disabled || draft.url.trim().is_empty() || url_error.is_some())
                        .on_click(cx.listener(|this, _, _, cx| this.compute_sha1(cx))),
                )
                .note_opt(sha1_error.map(|e| (e.into(), colors::RED_200)))
                .into_any_element(),
            SettingRow::new(t(M::PackRequired))
                .hint(t(M::PackRequiredHint))
                .child(
                    Toggle::new("pack-required", form.required)
                        .label(t(M::PackRequired))
                        .disabled(disabled)
                        .on_change(cx.listener(|this, on: &bool, _, cx| {
                            if let Some(form) = &mut this.pack {
                                form.required = *on;
                            }
                            cx.notify();
                        })),
                )
                .into_any_element(),
            SettingRow::new(t(M::PackPrompt))
                .key("pack-prompt")
                .hint(t(M::PackPromptHint))
                .child(control(&form.prompt))
                .into_any_element(),
            // changes here are kept until saved, so the pack is set at once
            div()
                .flex()
                .flex_row()
                .justify_end()
                .gap(px(8.))
                .px(px(16.))
                .py(px(12.))
                .child(
                    Button::new("pack-remove", t(M::PackRemove))
                        .size(ButtonSize::Slim)
                        .disabled(disabled || saved.url.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.save_pack(ServerResourcePack::default(), cx)
                        })),
                )
                .child(
                    Button::new("pack-save", t(CommonMsg::Save))
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Slim)
                        .loading(busy == Some(PackBusy::Saving))
                        .disabled(
                            disabled || !changed || url_error.is_some() || sha1_error.is_some(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(draft) = this.pack_draft(cx) {
                                this.save_pack(draft, cx)
                            }
                        })),
                )
                .into_any_element(),
        ];
        Some(
            section(t(M::ResourcePack), Some(t(M::ResourcePackLead).into()))
                .child(card(rows))
                .into_any_element(),
        )
    }

    fn eula_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let status = self.eula?;
        let may_accept = self
            .state
            .read(cx)
            .me
            .may(InstancePermission::Start, &self.uuid);
        let read = link("eula-read", t(M::EulaRead)).on_press(|_, _, cx| {
            cx.open_url(MINECRAFT_EULA_URL);
        });
        let row = if status.accepted {
            SettingRow::new(t(M::Eula))
                .hint(t(M::EulaAcceptedHint))
                .child(read)
                .child(tag(t(M::EulaAccepted), colors::GREEN))
        } else {
            SettingRow::new(t(M::Eula))
                .hint(t(if may_accept {
                    M::EulaNotAcceptedHint
                } else {
                    M::EulaNoPermission
                }))
                .child(tag(t(M::EulaNotAccepted), colors::YELLOW))
                .child(read)
                .child(
                    Button::new("eula-agree", t(M::EulaAgree))
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Slim)
                        .loading(self.eula_busy)
                        .disabled(!may_accept || self.eula_busy)
                        .on_click(cx.listener(|this, _, _, cx| this.accept_eula(cx))),
                )
        };
        Some(
            section(t(M::Eula), Some(t(M::EulaLead).into()))
                .child(card([row.into_any_element()]))
                .into_any_element(),
        )
    }

    fn danger_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let can_control = self.can_control(cx);
        let can_delete = can_control
            && self
                .state
                .read(cx)
                .me
                .may_core(CorePermission::DeleteInstance);
        let hint = if can_delete {
            M::DeleteServerHint
        } else if can_control {
            M::NoPermission
        } else {
            M::OwnerOnlyAtom
        };
        section(t(M::DangerZone), Some(t(M::DangerZoneLead).into()))
            .child(card([SettingRow::new(t(M::DeleteServer))
                .danger()
                .hint(t(hint))
                .child(
                    Button::new("delete-instance", t(CommonMsg::Delete))
                        .variant(ButtonVariant::Danger)
                        .size(ButtonSize::Slim)
                        .disabled(!can_delete)
                        .on_click(cx.listener(|this, _, _, cx| this.open_delete(cx))),
                )
                .into_any_element()]))
            .into_any_element()
    }

    fn delete_dialog(&self, prompt: &DeletePrompt, cx: &mut Context<Self>) -> AnyElement {
        let close = cx.listener(|this, _: &(), _, cx| {
            if !this.deleting {
                this.delete.close(cx);
            }
        });
        let close = std::rc::Rc::new(close);
        Dialog::new(
            "delete-instance-dialog",
            tr!(M::DeleteTitle, name = prompt.name),
            prompt.focus.clone(),
        )
        .on_close({
            let close = close.clone();
            move |window, cx| close(&(), window, cx)
        })
        .child(
            div()
                .text_size(text::MEDIUM)
                .line_height(lodestone_ui::theme::line_height(
                    text::MEDIUM,
                    leading::NORMAL,
                ))
                .text_color(ink::secondary())
                .child(
                    div()
                        .font_weight(weights::BOLD.face())
                        .text_color(ink::text())
                        .child(t(M::DeleteCannotUndo)),
                )
                .child(t(M::DeleteWarning)),
        )
        .footer(
            Button::new("delete-instance-confirm", t(M::DeleteConfirm))
                .variant(ButtonVariant::Danger)
                .loading(self.deleting)
                .disabled(self.deleting)
                .on_click(cx.listener(|this, _, _, cx| this.confirm_delete(cx))),
        )
        .into_any_element()
    }
}

impl Render for InstanceSettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut sections = self.manifest_sections(cx);
        if let Some(automation) = self.automation_section(cx) {
            sections.push((t(M::Automation).into(), automation));
        }
        if let Some(sharing) = self.sharing_section(cx) {
            sections.push((t(CoreMsg::ListingTitle).into(), sharing));
        }
        if let Some(pack) = self.pack_section(cx) {
            sections.push((t(M::ResourcePack).into(), pack));
        }
        if let Some(eula) = self.eula_section(cx) {
            sections.push((t(M::Eula).into(), eula));
        }
        sections.push((t(M::DangerZone).into(), self.danger_section(cx)));

        let titles: Vec<SharedString> = sections.iter().map(|(title, _)| title.clone()).collect();
        let active = self.nav.active(titles.len());
        // the list of sections, when the page is wide enough for it beside them
        let wide = window.viewport_size().width >= px(1080.);
        let list = wide.then(|| {
            let entity = cx.entity();
            section_list(
                "instance-settings-nav",
                &titles,
                active,
                move |index, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.nav.pick(index);
                        cx.notify();
                    })
                },
            )
        });
        let content = div()
            .id("instance-settings")
            .flex()
            .flex_col()
            .gap(px(32.))
            .flex_1()
            .min_w_0()
            .h_full()
            .pb(px(64.))
            .overflow_y_scroll()
            .track_scroll(&self.nav.scroll)
            .on_scroll(cx.listener(|this, _, _, _| this.nav.scrolled()))
            .children(
                sections
                    .into_iter()
                    .map(|(_, element)| div().w_full().max_w(content_width()).child(element)),
            );
        let dialogs = self
            .delete
            .render(window, |prompt| Some(self.delete_dialog(prompt, cx)));
        div()
            .relative()
            .size_full()
            .flex()
            .flex_row()
            .gap(px(32.))
            .text_size(text::MEDIUM)
            .line_height(lodestone_ui::theme::line_height(
                text::MEDIUM,
                leading::NORMAL,
            ))
            .children(list)
            .child(content)
            .children(window_layer(window, dialogs))
    }
}

#[cfg(test)]
mod tests;
