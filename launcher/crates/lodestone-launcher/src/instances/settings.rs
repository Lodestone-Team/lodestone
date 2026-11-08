//! An instance's Settings tab: one column of sections, with a list of them beside it.
//! General (name, group, icon), Game & loader, the pack and the linked server when
//! there are any, Java & memory and Launch options (each a launcher default until
//! changed here), Shared game settings, and Maintenance (verify, export, duplicate,
//! host, delete).
//!
//! Changes to the settings are kept until Save, on the bar that appears with them;
//! Discard puts the saved ones back. A value that isn't valid says why under its row
//! and can't be saved. One-off actions (the icon, the background, Unlink and the
//! Maintenance buttons) happen at once.

use std::{path::PathBuf, sync::Arc, time::Duration};

use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, IntoElement, ObjectFit, PathPromptOptions,
    Render, RenderImage, SharedString, Subscription, Task, Window, div, img, prelude::*, px,
};
use launcher_core::{
    content::loader_change::{self, LoaderChangeCheck},
    game::runtime::{self, JavaInfo},
    instances::{Instance, Loader, Memory, format_env},
    modpack::ModpackRecord,
};
use lodestone_minecraft::modrinth::{ModrinthVersionType, RawVersion};

use super::{
    Instances,
    appearance::{self, IconChoice},
    content::Load,
    fields::{self, Fields, Values},
    loader_picker::LoaderPicker,
    modpacks::UpdatePrompt,
    shared_settings::InstanceSharedSettings,
};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, LoadingDots, TextInput, Toggle,
        settings::{
            CONTENT_WIDTH, SectionNav, SettingRow, Toast, card, left_overlay, link, muted_link,
            popover, popover_panel, section, section_list, section_menu_button, tag, unsaved_bar,
            value,
        },
    },
    servers::Servers,
    theme::{self, colors, leading, radius, text, weights},
};

/// Below this window width the list of sections opens from a button instead of
/// standing beside the settings.
const NARROW_BELOW: gpui::Pixels = px(1040.);

/// How many screenshots the background picker offers.
const SHOTS_OFFERED: usize = 24;

/// Asks the shell to confirm deleting the instance.
pub struct DeleteRequested(pub String);

/// "Host on a Lodestone core…" was clicked for this instance.
pub struct HostRequested(pub String);

/// Asks the shell to confirm a loader change that leaves mods behind or puts worlds at
/// risk.
#[derive(Clone, Debug)]
pub struct LoaderChangePrompt {
    /// The instance as it will be saved.
    pub instance: Instance,
    /// The loader it uses now.
    pub from: Loader,
    pub check: LoaderChangeCheck,
}

/// The result of checking the Java override.
#[derive(Clone, Debug)]
enum JavaCheck {
    None,
    Checking,
    Found(JavaInfo),
    Failed(SharedString),
}

/// Which settings the instance sets itself instead of following the launcher's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Overrides {
    memory: bool,
    java: bool,
    jvm_args: bool,
    game_args: bool,
    window: bool,
    env: bool,
}

/// The tabs of the icon picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IconTab {
    Library,
    Emoji,
}

/// A setting the instance can take from the launcher or set itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Setting {
    Memory,
    Java,
    JvmArgs,
    GameArgs,
    Window,
    Env,
}

impl Setting {
    fn id(self) -> &'static str {
        match self {
            Setting::Memory => "memory",
            Setting::Java => "java",
            Setting::JvmArgs => "jvm-args",
            Setting::GameArgs => "game-args",
            Setting::Window => "window",
            Setting::Env => "env",
        }
    }

    fn flag(self, overrides: &mut Overrides) -> &mut bool {
        match self {
            Setting::Memory => &mut overrides.memory,
            Setting::Java => &mut overrides.java,
            Setting::JvmArgs => &mut overrides.jvm_args,
            Setting::GameArgs => &mut overrides.game_args,
            Setting::Window => &mut overrides.window,
            Setting::Env => &mut overrides.env,
        }
    }
}

pub struct InstanceSettings {
    instances: Entity<Instances>,
    servers: Entity<Servers>,
    instance_id: String,
    game_version: String,
    name: Entity<TextInput>,
    group: Entity<TextInput>,
    java: Entity<TextInput>,
    fields: Fields,
    loader: Entity<LoaderPicker>,
    /// The "Shared game settings" section.
    shared: Entity<InstanceSharedSettings>,
    overrides: Overrides,
    java_check: JavaCheck,
    /// The pack the instance was made from, if any.
    modpack: Option<ModpackRecord>,
    /// The server the instance was made for: its pack is the server's client pack.
    server: Option<String>,
    /// The pack's versions on Modrinth, after "Check for updates".
    modpack_versions: Load<Vec<RawVersion>>,
    /// Downloading and planning an update.
    preparing_update: bool,
    /// "Unlink" was clicked and waits for the second click.
    confirm_unlink: bool,
    /// The icon picker, when it is open, and its tab.
    icon_picker: Option<IconTab>,
    /// The tile colour emoji icons are put on.
    emoji_tile: u32,
    /// The background picker is open, with the screenshots it offers.
    background_picker: bool,
    shots: Load<Vec<(PathBuf, Arc<RenderImage>)>>,
    _shots_task: Option<Task<()>>,
    /// A loader change is waiting for the shell's dialog.
    prompting: bool,
    nav: SectionNav,
    /// The list of sections is open over the settings, in a narrow window.
    nav_open: bool,
    toast: Toast,
    _modpack_task: Option<Task<()>>,
    _check: Option<Task<()>>,
    _prompt: Option<Task<()>>,
    _observe: Vec<Subscription>,
}

impl EventEmitter<DeleteRequested> for InstanceSettings {}
impl EventEmitter<HostRequested> for InstanceSettings {}
impl EventEmitter<LoaderChangePrompt> for InstanceSettings {}
impl EventEmitter<UpdatePrompt> for InstanceSettings {}

/// The version to offer as an update: the newest release, else the newest version,
/// if it is newer than `current`.
pub fn update_candidate(versions: &[RawVersion], current: &ModpackRecord) -> Option<RawVersion> {
    let newest = versions
        .iter()
        .filter(|v| v.version_type == ModrinthVersionType::Release)
        .max_by(|a, b| a.date_published.cmp(&b.date_published))
        .or_else(|| {
            versions
                .iter()
                .max_by(|a, b| a.date_published.cmp(&b.date_published))
        })?;
    (newest.id != current.version_id && newest.date_published > current.date_published)
        .then(|| newest.clone())
}

impl InstanceSettings {
    pub fn new(
        instances: Entity<Instances>,
        servers: Entity<Servers>,
        instance: &Instance,
        cx: &mut Context<Self>,
    ) -> Self {
        let (defaults, total) = {
            let instances = instances.read(cx);
            (instances.defaults().clone(), instances.total_memory())
        };
        // The fields start with what the instance uses now, so changing a launcher
        // default starts from the launcher's value.
        let env = instance.env.clone().unwrap_or_else(|| defaults.env.clone());
        let fields = Fields::new(
            &Values {
                memory: instance.memory(&defaults, total),
                jvm_args: instance.jvm_args.as_deref().unwrap_or(&defaults.jvm_args),
                game_args: instance.game_args.as_deref().unwrap_or(&defaults.game_args),
                window: instance.window.unwrap_or_else(|| defaults.window()),
                env: &env,
            },
            cx,
        );
        let loader = {
            let instances = instances.clone();
            let game = instance.game_version.clone();
            let choice = instance.loader_choice();
            cx.new(|cx| LoaderPicker::new(instances, Some(game), choice, 150., cx).compact())
        };
        let modpack = instances.read(cx).modpack(&instance.id);
        let server = launcher_core::cores::link::load_link(
            &instances.read(cx).content_dirs(&instance.id).instance,
        )
        .map(|link| link.server_name);
        let shared = {
            let instances = instances.clone();
            let (id, game) = (instance.id.clone(), instance.game_version.clone());
            cx.new(|cx| InstanceSharedSettings::new(instances, id, game, cx))
        };
        let name = fields::input(cx, instance.name.clone(), "Name");
        let group = fields::input(cx, instance.group.clone().unwrap_or_default(), "No group");
        let java = fields::input(
            cx,
            instance
                .java_override
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            "/path/to/java",
        );
        let mut observe = vec![
            cx.observe(&loader, |_, _, cx| cx.notify()),
            cx.observe(&servers, |_, _, cx| cx.notify()),
        ];
        for input in [
            &name,
            &group,
            &java,
            &fields.min_ram,
            &fields.max_ram,
            &fields.jvm_args,
            &fields.game_args,
            &fields.width,
            &fields.height,
            &fields.env,
        ] {
            observe.push(cx.observe(input, |_, _, cx| cx.notify()));
        }
        Self {
            name,
            group,
            java,
            fields,
            loader,
            shared,
            overrides: Overrides {
                memory: instance.memory.is_some(),
                java: instance.java_override.is_some(),
                jvm_args: instance.jvm_args.is_some(),
                game_args: instance.game_args.is_some(),
                window: instance.window.is_some(),
                env: instance.env.is_some(),
            },
            instances,
            servers,
            instance_id: instance.id.clone(),
            game_version: instance.game_version.clone(),
            java_check: JavaCheck::None,
            modpack,
            server,
            modpack_versions: Load::NotLoaded,
            preparing_update: false,
            confirm_unlink: false,
            icon_picker: None,
            emoji_tile: appearance::TILE_COLORS[1],
            background_picker: false,
            shots: Load::NotLoaded,
            _shots_task: None,
            prompting: false,
            nav: SectionNav::default(),
            nav_open: false,
            toast: Toast::default(),
            _modpack_task: None,
            _check: None,
            _prompt: None,
            _observe: observe,
        }
    }

    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    pub fn shared_settings(&self) -> &Entity<InstanceSharedSettings> {
        &self.shared
    }

    fn text(&self, input: &Entity<TextInput>, cx: &App) -> String {
        input.read(cx).text().trim().to_string()
    }

    fn instance(&self, cx: &App) -> Option<Instance> {
        self.instances
            .read(cx)
            .list()
            .iter()
            .find(|i| i.id == self.instance_id)
            .cloned()
    }

    fn set_message(
        &mut self,
        message: impl Into<SharedString>,
        error: bool,
        cx: &mut Context<Self>,
    ) {
        self.toast
            .show(message, error, |this: &mut Self| &mut this.toast, cx);
    }

    /// Whether the settings differ from the saved instance (or aren't valid).
    pub fn has_changes(&self, cx: &App) -> bool {
        let Some(before) = self.instance(cx) else {
            return false;
        };
        match self.read_fields(&before, cx) {
            Ok(instance) => instance != before,
            Err(_) => true,
        }
    }

    fn can_save(&self, cx: &App) -> bool {
        self.instance(cx)
            .is_some_and(|before| self.read_fields(&before, cx).is_ok())
    }

    /// Saves the settings if they are valid. Returns whether they are saved now: not
    /// when a value isn't valid (it says why under its row), or when a loader change
    /// that would leave enabled mods unable to load, or worlds without their modded
    /// blocks, asks first (see [`LoaderChangePrompt`]).
    pub fn save(&mut self, cx: &mut Context<Self>) -> bool {
        if self.prompting {
            return false;
        }
        let Some(before) = self.instance(cx) else {
            return true;
        };
        let Ok(instance) = self.read_fields(&before, cx) else {
            cx.notify();
            return false;
        };
        if instance == before {
            return true;
        }
        if let Some(prompt) = self.loader_change_prompt(Some(&before), &instance, cx) {
            self.prompting = true;
            cx.emit(prompt);
            return false;
        }
        self.commit(instance, &[], cx);
        true
    }

    /// Changes the name without saving, for the offscreen screenshots.
    pub fn preview_change(&mut self, cx: &mut Context<Self>) {
        self.name
            .update(cx, |input, cx| input.set_text("Survival, renamed", cx));
    }

    /// Puts the saved settings back.
    pub fn discard(&mut self, cx: &mut Context<Self>) {
        let Some(instance) = self.instance(cx) else {
            return;
        };
        let (defaults, total) = {
            let instances = self.instances.read(cx);
            (instances.defaults().clone(), instances.total_memory())
        };
        let env = instance.env.clone().unwrap_or_else(|| defaults.env.clone());
        self.fields.set(
            &Values {
                memory: instance.memory(&defaults, total),
                jvm_args: instance.jvm_args.as_deref().unwrap_or(&defaults.jvm_args),
                game_args: instance.game_args.as_deref().unwrap_or(&defaults.game_args),
                window: instance.window.unwrap_or_else(|| defaults.window()),
                env: &env,
            },
            cx,
        );
        let java = instance
            .java_override
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        for (input, text_) in [
            (&self.name, instance.name.clone()),
            (&self.group, instance.group.clone().unwrap_or_default()),
            (&self.java, java),
        ] {
            input.update(cx, |input, cx| input.set_text(text_, cx));
        }
        self.overrides = Overrides {
            memory: instance.memory.is_some(),
            java: instance.java_override.is_some(),
            jvm_args: instance.jvm_args.is_some(),
            game_args: instance.game_args.is_some(),
            window: instance.window.is_some(),
            env: instance.env.is_some(),
        };
        self.java_check = JavaCheck::None;
        self.sync_loader(cx);
        cx.notify();
    }

    /// The question to ask before saving `instance`, if its loader change needs one.
    fn loader_change_prompt(
        &self,
        before: Option<&Instance>,
        instance: &Instance,
        cx: &App,
    ) -> Option<LoaderChangePrompt> {
        let before = before?;
        if before.loader == instance.loader {
            return None;
        }
        let dirs = self.instances.read(cx).content_dirs(&instance.id);
        let check = loader_change::check(
            &dirs,
            before.loader,
            instance.loader,
            &instance.game_version,
        )
        .ok()?;
        check.needs_confirmation().then(|| LoaderChangePrompt {
            instance: instance.clone(),
            from: before.loader,
            check,
        })
    }

    /// The prompt for changing the loader to `loader` at `version`, as saving would ask
    /// it, for the offscreen screenshots.
    pub fn preview_loader_change(
        &self,
        loader: Loader,
        version: &str,
        cx: &App,
    ) -> Option<LoaderChangePrompt> {
        let before = self.instance(cx)?;
        let mut instance = before.clone();
        instance.loader = loader;
        instance.loader_version = Some(version.to_string());
        self.loader_change_prompt(Some(&before), &instance, cx)
    }

    /// The loader change dialog was closed without saving: the loader shows what the
    /// instance has again.
    pub fn revert_loader(&mut self, cx: &mut Context<Self>) {
        self.prompting = false;
        self.sync_loader(cx);
    }

    fn sync_loader(&mut self, cx: &mut Context<Self>) {
        let choice = self.instance(cx).and_then(|i| i.loader_choice());
        self.loader
            .update(cx, |loader, cx| loader.set_choice(choice, cx));
    }

    /// Saves `instance`, after turning off `disable` (mods, with `.disabled`).
    pub fn commit(&mut self, instance: Instance, disable: &[String], cx: &mut Context<Self>) {
        self.prompting = false;
        let before = self.instance(cx);
        if !disable.is_empty() {
            let dirs = self.instances.read(cx).content_dirs(&instance.id);
            if let Err(err) = loader_change::disable_mods(&dirs, disable) {
                self.sync_loader(cx);
                return self.set_message(format!("Couldn't turn the mods off: {err}"), true, cx);
            }
        }
        let running = self.instances.read(cx).is_active(&instance.id);
        let result = self
            .instances
            .update(cx, |instances, cx| instances.save(&instance, cx));
        match result {
            Ok(()) => {
                let loader_changed = before.is_some_and(|before| {
                    (before.loader, &before.loader_version)
                        != (instance.loader, &instance.loader_version)
                });
                let turned_off = match disable.len() {
                    0 => String::new(),
                    1 => " 1 mod was turned off.".to_string(),
                    n => format!(" {n} mods were turned off."),
                };
                let message = match (loader_changed, running) {
                    (true, true) => format!(
                        "{} installs the next time the game starts.{turned_off}",
                        instance.loader_label()
                    ),
                    (true, false) => format!(
                        "{} installs the next time you press Play.{turned_off}",
                        instance.loader_label()
                    ),
                    (false, true) => "Saved. It applies the next time the game starts.".into(),
                    (false, false) => "Saved".to_string(),
                };
                self.set_message(message, false, cx);
            }
            Err(err) => self.set_message(err, true, cx),
        }
        self.sync_loader(cx);
    }

    /// The instance with the fields' values, or the first problem with them.
    fn read_fields(&self, before: &Instance, cx: &App) -> Result<Instance, String> {
        let mut instance = before.clone();
        let name = self.text(&self.name, cx);
        if name.is_empty() {
            return Err("Give the instance a name.".into());
        }
        let total = self.instances.read(cx).total_memory();
        let overrides = self.overrides;
        instance.memory = if overrides.memory {
            let check = self.fields.memory(total, cx);
            let (min, max) = check.values.ok_or(check.error.unwrap_or_default())?;
            Some(Memory { min, max })
        } else {
            None
        };
        instance.jvm_args = overrides.jvm_args.then(|| self.fields.jvm_args(cx));
        instance.game_args = overrides.game_args.then(|| self.fields.game_args(cx));
        instance.window = if overrides.window {
            Some(self.fields.window(cx)?)
        } else {
            None
        };
        instance.env = if overrides.env {
            Some(self.fields.env(cx)?)
        } else {
            None
        };
        // The loader changes only when one was picked here, and not while its versions
        // load (or can't): the recommended version picked when the list loads isn't a
        // change.
        let picker = self.loader.read(cx);
        if let (true, Ok(loader)) = (picker.touched(), picker.choice()) {
            instance.loader = Loader::from_kind(loader.as_ref().map(|l| l.kind));
            instance.loader_version = loader.map(|l| l.version);
        }
        let group = self.text(&self.group, cx);
        instance.group = (!group.is_empty()).then_some(group);
        let java = self.text(&self.java, cx);
        instance.name = name;
        instance.java_override = (overrides.java && !java.is_empty()).then(|| PathBuf::from(java));
        Ok(instance)
    }

    /// "Change": the instance sets `setting` itself, starting from the launcher's value.
    fn change(&mut self, setting: Setting, cx: &mut Context<Self>) {
        *setting.flag(&mut self.overrides) = true;
        cx.notify();
    }

    /// "Reset": `setting` follows the launcher again.
    fn reset(&mut self, setting: Setting, cx: &mut Context<Self>) {
        *setting.flag(&mut self.overrides) = false;
        let (defaults, total) = {
            let instances = self.instances.read(cx);
            (instances.defaults().clone(), instances.total_memory())
        };
        let set = |input: &Entity<TextInput>, text_: String, cx: &mut Context<Self>| {
            input.update(cx, |input, cx| input.set_text(text_, cx));
        };
        match setting {
            Setting::Memory => {
                let (min, max) = defaults.memory(total);
                set(&self.fields.min_ram, min.to_string(), cx);
                set(&self.fields.max_ram, max.to_string(), cx);
            }
            Setting::Java => {
                set(&self.java, String::new(), cx);
                self.java_check = JavaCheck::None;
            }
            Setting::JvmArgs => set(&self.fields.jvm_args, defaults.jvm_args.clone(), cx),
            Setting::GameArgs => set(&self.fields.game_args, defaults.game_args.clone(), cx),
            Setting::Window => {
                let window = defaults.window();
                let (width, height) = window
                    .resolution
                    .map(|r| (r.width.to_string(), r.height.to_string()))
                    .unwrap_or_default();
                set(&self.fields.width, width, cx);
                set(&self.fields.height, height, cx);
                self.fields.fullscreen = window.fullscreen;
            }
            Setting::Env => set(&self.fields.env, format_env(&defaults.env), cx),
        }
        cx.notify();
    }

    fn check_java(&mut self, cx: &mut Context<Self>) {
        let path = self.text(&self.java, cx);
        if path.is_empty() {
            self.java_check = JavaCheck::None;
            cx.notify();
            return;
        }
        self.java_check = JavaCheck::Checking;
        cx.notify();
        let path = PathBuf::from(path);
        let check =
            cx.background_spawn(async move { runtime::check_java(&path, Duration::from_secs(15)) });
        self._check = Some(cx.spawn(async move |this, cx| {
            let result = check.await;
            this.update(cx, |this, cx| {
                this.java_check = match result {
                    Ok(info) => JavaCheck::Found(info),
                    Err(err) => JavaCheck::Failed(err.to_string().into()),
                };
                cx.notify();
            })
            .ok();
        }));
    }

    /// Picks a `java` binary with the system's file dialog, then checks it.
    fn browse_java(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use this Java".into()),
        });
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                this.java.update(cx, |input, cx| {
                    input.set_text(path.display().to_string(), cx)
                });
                this.check_java(cx);
            })
            .ok();
        }));
    }

    fn choose_icon(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use as icon".into()),
        });
        let id = self.instance_id.clone();
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                let result = this
                    .instances
                    .update(cx, |instances, cx| instances.set_icon(&id, &path, cx));
                match result {
                    Ok(()) => this.set_message("Saved", false, cx),
                    Err(err) => this.set_message(err, true, cx),
                }
            })
            .ok();
        }));
    }

    /// Uses a library icon or an emoji, or the default for `None`.
    fn pick_icon(&mut self, icon: Option<String>, cx: &mut Context<Self>) {
        self.icon_picker = None;
        let id = self.instance_id.clone();
        let result = self.instances.update(cx, |instances, cx| {
            instances.set_icon_name(&id, icon.as_deref(), cx)
        });
        match result {
            Ok(()) => self.set_message("Saved", false, cx),
            Err(err) => self.set_message(err, true, cx),
        }
    }

    fn choose_background(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use as background".into()),
        });
        let id = self.instance_id.clone();
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                let result = this
                    .instances
                    .update(cx, |instances, cx| instances.set_background(&id, &path, cx));
                match result {
                    Ok(()) => this.set_message("Saved. See it on the Play tab.", false, cx),
                    Err(err) => this.set_message(err, true, cx),
                }
            })
            .ok();
        }));
    }

    /// Opens or closes the background picker; opening it finds the screenshots taken in
    /// this instance and then the others, newest first.
    fn toggle_background_picker(&mut self, cx: &mut Context<Self>) {
        self.background_picker = !self.background_picker;
        if !self.background_picker {
            cx.notify();
            return;
        }
        let instances = self.instances.read(cx);
        let mut dirs = vec![instances.content_dirs(&self.instance_id)];
        dirs.extend(
            instances
                .list()
                .iter()
                .filter(|i| i.id != self.instance_id)
                .map(|i| instances.content_dirs(&i.id)),
        );
        if !matches!(self.shots, Load::Ready(_)) {
            self.shots = Load::Loading;
        }
        let work = cx.background_spawn(async move {
            let (mut own, mut others) = (Vec::new(), Vec::new());
            for (index, dirs) in dirs.iter().enumerate() {
                let list = launcher_core::screenshots::list(dirs);
                if index == 0 {
                    own = list;
                } else {
                    others.extend(list);
                }
            }
            others.sort_by_key(|shot| std::cmp::Reverse(shot.modified));
            own.into_iter()
                .chain(others)
                .take(SHOTS_OFFERED)
                .filter_map(|shot| {
                    let thumb = launcher_core::screenshots::thumbnail(&shot.path, 240).ok()?;
                    Some((shot.path, crate::images::render_image(thumb)))
                })
                .collect::<Vec<_>>()
        });
        self._shots_task = Some(cx.spawn(async move |this, cx| {
            let shots = work.await;
            this.update(cx, |this, cx| {
                this.shots = Load::Ready(shots);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn use_background(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.background_picker = false;
        let id = self.instance_id.clone();
        let result = self
            .instances
            .update(cx, |instances, cx| instances.set_background(&id, &path, cx));
        match result {
            Ok(()) => self.set_message("Saved. See it on the Play tab.", false, cx),
            Err(err) => self.set_message(err, true, cx),
        }
    }

    /// The background picker: screenshots, a file, or the default.
    fn background_picker_panel(&self, has_background: bool, cx: &mut Context<Self>) -> AnyElement {
        let body: AnyElement = match &self.shots {
            Load::Ready(shots) if !shots.is_empty() => {
                div()
                    .id("background-shots")
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .max_h(px(300.))
                    .overflow_y_scroll()
                    .children(shots.iter().enumerate().map(|(index, (path, thumb))| {
                        let path = path.clone();
                        div()
                            .id(("background-shot", index))
                            .w(px(112.))
                            .h(px(63.))
                            .rounded(radius::MD)
                            .overflow_hidden()
                            .border_1()
                            .border_color(colors::TRANSPARENT)
                            .cursor_pointer()
                            .hover(|style| style.border_color(colors::BLUE_200))
                            .child(img(thumb.clone()).size_full().object_fit(ObjectFit::Cover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.use_background(path.clone(), cx)
                            }))
                    }))
                    .into_any_element()
            }
            Load::Ready(_) | Load::Failed(_) => div()
                .text_size(theme::text_size(text::SMALL))
                .text_color(colors::GRAY_500)
                .child("No screenshots yet. Press F2 in the game to take one, or choose a file.")
                .into_any_element(),
            Load::NotLoaded | Load::Loading => div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .text_size(theme::text_size(text::SMALL))
                .text_color(colors::GRAY_500)
                .child(LoadingDots)
                .child("Finding your screenshots")
                .into_any_element(),
        };
        popover_panel("background-picker")
            .w(px(380.))
            .p_3()
            .gap_3()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.background_picker = false;
                cx.notify();
            }))
            .child(
                div()
                    .font_weight(weights::BOLD.into())
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(colors::GRAY_400)
                    .child("Your screenshots"),
            )
            .child(body)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_4()
                    .pt_2()
                    .border_t_1()
                    .border_color(colors::FADE_700)
                    .child(
                        link("background-file", "Choose a file…").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.background_picker = false;
                                this.choose_background(cx);
                            },
                        )),
                    )
                    .child(div().flex_1())
                    .when(has_background, |this| {
                        this.child(
                            muted_link("background-default", "Use the default").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.background_picker = false;
                                    let id = this.instance_id.clone();
                                    let result = this.instances.update(cx, |instances, cx| {
                                        instances.clear_background(&id, cx)
                                    });
                                    match result {
                                        Ok(()) => this.set_message("Saved", false, cx),
                                        Err(err) => this.set_message(err, true, cx),
                                    }
                                }),
                            ),
                        )
                    }),
            )
            .into_any_element()
    }

    /// Opens the list of sections in a narrow window, for the offscreen screenshots.
    pub fn preview_nav_open(&mut self) {
        self.nav_open = true;
    }

    /// Opens the background picker, for the offscreen screenshots.
    pub fn preview_background_picker(&mut self, cx: &mut Context<Self>) {
        self.background_picker = false;
        self.toggle_background_picker(cx);
    }

    /// Opens the icon picker on `tab`, for the offscreen screenshots.
    pub fn preview_icon_picker(&mut self, emoji: bool) {
        self.icon_picker = Some(if emoji {
            IconTab::Emoji
        } else {
            IconTab::Library
        });
    }

    /// The icon picker: the launcher's icons or emoji on a tile, uploading an image,
    /// and going back to the default.
    fn icon_picker_panel(
        &self,
        tab: IconTab,
        current: Option<&str>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = IconChoice::parse(current);
        let tab_button = |id: &'static str, label: &'static str, this_tab: IconTab| {
            let selected = tab == this_tab;
            div()
                .id(id)
                .px_2p5()
                .py_1()
                .rounded(radius::MD)
                .cursor_pointer()
                .font_weight(weights::BOLD.into())
                .text_size(theme::text_size(text::SMALL))
                .map(|this| {
                    if selected {
                        this.bg(colors::GRAY_700).text_color(colors::GRAY_300)
                    } else {
                        this.text_color(colors::GRAY_400)
                            .hover(|style| style.text_color(colors::GRAY_300))
                    }
                })
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.icon_picker = Some(this_tab);
                    cx.notify();
                }))
        };
        let tile = |id: (&'static str, usize), selected: bool| {
            div()
                .id(id)
                .flex()
                .items_center()
                .justify_center()
                .size(px(44.))
                .rounded(radius::LG)
                .border_1()
                .cursor_pointer()
                .map(|this| {
                    if selected {
                        this.border_color(colors::BLUE_200).bg(colors::GRAY_750)
                    } else {
                        this.border_color(colors::TRANSPARENT)
                            .hover(|style| style.bg(colors::GRAY_700))
                    }
                })
        };
        let grid = div().flex().flex_row().flex_wrap().gap_1();
        let body = match tab {
            IconTab::Library => grid
                .children(
                    appearance::LIBRARY
                        .iter()
                        .enumerate()
                        .map(|(index, (name, _))| {
                            let selected = current == IconChoice::Library(name.to_string())
                                || (*name == appearance::DEFAULT_ICON
                                    && current == IconChoice::Default);
                            let value = IconChoice::library(name);
                            tile(("icon-library", index), selected)
                                .child(img(appearance::library_image(name)).size(px(36.)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.pick_icon(Some(value.clone()), cx)
                                }))
                        }),
                )
                .into_any_element(),
            IconTab::Emoji => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div().flex().flex_row().items_center().gap_2().children(
                        appearance::TILE_COLORS
                            .iter()
                            .enumerate()
                            .map(|(index, color)| {
                                let color = *color;
                                let selected = color == self.emoji_tile;
                                div()
                                    .id(("icon-tile-color", index))
                                    .size(px(20.))
                                    .rounded_full()
                                    .cursor_pointer()
                                    .bg(theme::Color::rgb(color))
                                    .border_2()
                                    .border_color(if selected {
                                        colors::GRAY_300
                                    } else {
                                        colors::TRANSPARENT
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.emoji_tile = color;
                                        cx.notify();
                                    }))
                            }),
                    ),
                )
                .child(
                    grid.children(appearance::EMOJI.iter().enumerate().map(|(index, emoji)| {
                        let selected =
                            matches!(&current, IconChoice::Emoji { emoji: e, .. } if e == emoji);
                        let tile_color = self.emoji_tile;
                        let value = IconChoice::emoji(tile_color, emoji);
                        tile(("icon-emoji", index), selected)
                            .child(appearance::emoji_tile(emoji, tile_color, px(36.)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.pick_icon(Some(value.clone()), cx)
                            }))
                    })),
                )
                .into_any_element(),
        };
        popover_panel("icon-picker")
            .w(px(392.))
            .p_3()
            .gap_3()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.icon_picker = None;
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(tab_button(
                        "icon-tab-library",
                        "Blocks & items",
                        IconTab::Library,
                    ))
                    .child(tab_button("icon-tab-emoji", "Emoji", IconTab::Emoji)),
            )
            .child(body)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_4()
                    .pt_2()
                    .border_t_1()
                    .border_color(colors::FADE_700)
                    .child(
                        link("icon-upload", "Upload an image…").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.icon_picker = None;
                                this.choose_icon(cx);
                            },
                        )),
                    )
                    .child(div().flex_1())
                    .child(
                        muted_link("icon-default", "Use the default")
                            .on_click(cx.listener(|this, _, _, cx| this.pick_icon(None, cx))),
                    ),
            )
            .into_any_element()
    }

    fn export(&mut self, cx: &mut Context<Self>) {
        let name = self
            .instance(cx)
            .map(|i| i.name)
            .unwrap_or_else(|| "Instance".into());
        let file_name: String = name
            .chars()
            .map(|c| if "/\\:".contains(c) { '-' } else { c })
            .collect();
        let folder = dirs_downloads();
        let picked = cx.prompt_for_new_path(&folder, Some(&format!("{file_name}.mrpack")));
        let id = self.instance_id.clone();
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = picked.await else {
                return;
            };
            this.update(cx, |this, cx| {
                this.instances
                    .update(cx, |instances, cx| instances.export(&id, path, cx));
                this.set_message("Exporting…", false, cx);
            })
            .ok();
        }));
    }

    fn unlink(&mut self, cx: &mut Context<Self>) {
        self.confirm_unlink = false;
        let id = self.instance_id.clone();
        let result = self
            .servers
            .update(cx, |servers, cx| servers.unlink_instance(&id, cx));
        match result {
            Ok(()) => {
                self.server = None;
                self.set_message("Unlinked", false, cx);
            }
            Err(err) => self.set_message(err, true, cx),
        }
    }

    fn check_modpack(&mut self, cx: &mut Context<Self>) {
        let Some(record) = self.modpack.clone().filter(ModpackRecord::on_modrinth) else {
            return;
        };
        let Some(task) = self.instances.update(cx, |instances, cx| {
            instances.modpack_versions(&record.project_id, cx)
        }) else {
            return self.set_message("Modrinth can't be reached.", true, cx);
        };
        self.modpack_versions = Load::Loading;
        self._modpack_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.modpack_versions = match result {
                    Ok(versions) => Load::Ready(versions),
                    Err(err) => Load::Failed(err.into()),
                };
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Downloads the version and plans the update, then asks the window to confirm it.
    fn prepare_update(&mut self, version: RawVersion, cx: &mut Context<Self>) {
        let id = self.instance_id.clone();
        let Some(task) = self.instances.update(cx, |instances, cx| {
            instances.plan_modpack_update(&id, version, cx)
        }) else {
            return self.set_message(
                "Stop the game first, or wait for the update to finish.",
                true,
                cx,
            );
        };
        self.preparing_update = true;
        self._modpack_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.preparing_update = false;
                match result {
                    Ok(prompt) => cx.emit(prompt),
                    Err(err) => {
                        this.set_message(format!("Couldn't prepare the update: {err}"), true, cx)
                    }
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Shows the pack and an available update, for the offscreen screenshots.
    pub fn preview_modpack(&mut self, record: ModpackRecord, versions: Vec<RawVersion>) {
        self.modpack = Some(record);
        self.modpack_versions = Load::Ready(versions);
    }

    /// Shows a Java check result, for the offscreen screenshots.
    pub fn preview_java(&mut self, version: &str, major: u32) {
        self.overrides.java = true;
        self.java_check = JavaCheck::Found(JavaInfo {
            version: version.into(),
            major,
        });
    }

    /// The Modpack section, for an instance made from a pack.
    fn modpack_section(&self, busy: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        let record = self.modpack.clone()?;
        let update_job = self
            .instances
            .read(cx)
            .modpack_update(&self.instance_id)
            .map(|job| (job.progress.label(), job.progress.downloads.snapshot()));
        let candidate = self
            .modpack_versions
            .ready()
            .and_then(|versions| update_candidate(versions, &record));
        let origin = match &self.server {
            _ if record.on_modrinth() => "From Modrinth".to_string(),
            Some(server) => {
                format!("From the mods of {server}: Play keeps them in step with the server.")
            }
            None => {
                "From a .mrpack file that isn't on Modrinth, so it can't be updated here.".into()
            }
        };
        let version = if record.version_number.is_empty() {
            String::new()
        } else {
            format!(" {}", record.version_number)
        };
        let page = (!record.slug.is_empty())
            .then(|| format!("https://modrinth.com/modpack/{}", record.slug));
        let working = |label: String| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .text_size(theme::text_size(text::SMALL))
                .text_color(colors::GRAY_400)
                .child(LoadingDots)
                .child(label)
                .into_any_element()
        };
        let status_text = |text_: SharedString, color: theme::Color| {
            div()
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .text_color(color)
                .child(text_)
                .into_any_element()
        };
        let update: Option<AnyElement> = if let Some((label, snapshot)) = update_job {
            let percent = if snapshot.bytes_total > 0 {
                format!(
                    " · {:.0}%",
                    snapshot.bytes_done as f64 * 100.0 / snapshot.bytes_total as f64
                )
            } else {
                String::new()
            };
            Some(working(format!("{label}{percent}")))
        } else if self.preparing_update {
            Some(working("Downloading the new version…".into()))
        } else {
            match (&self.modpack_versions, candidate) {
                (Load::Loading, _) => Some(working("Checking Modrinth…".into())),
                (Load::Failed(err), _) => Some(status_text(err.clone(), colors::RED_200)),
                (Load::Ready(_), Some(newer)) => Some(
                    Button::new(
                        "update-modpack",
                        format!("Update to {}…", newer.version_number),
                    )
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Slim)
                    .disabled(busy)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.prepare_update(newer.clone(), cx)),
                    )
                    .into_any_element(),
                ),
                (Load::Ready(_), None) => Some(status_text("Up to date".into(), colors::GREEN_300)),
                (Load::NotLoaded, _) if record.on_modrinth() => Some(
                    Button::new("check-modpack", "Check for updates")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.check_modpack(cx)))
                        .into_any_element(),
                ),
                (Load::NotLoaded, _) => None,
            }
        };
        let mut rows = vec![
            SettingRow::new(format!("{}{version}", record.title))
                .hint(origin)
                .when_some(page, |row, page| {
                    row.child(
                        link("modpack-page", "View on Modrinth")
                            .on_click(move |_, _, cx| cx.open_url(&page)),
                    )
                })
                .into_any_element(),
        ];
        if let Some(update) = update {
            rows.push(
                SettingRow::new("Updates")
                    .hint("An update replaces the pack's own files and keeps your mods, configs and worlds. You see what changes first.")
                    .child(update)
                    .into_any_element(),
            );
        }
        Some(
            section("Modpack", None)
                .child(card(rows))
                .into_any_element(),
        )
    }

    /// The Linked server section, for an instance made for a server on a core.
    fn server_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let link = self.servers.read(cx).instance_link(&self.instance_id)?;
        let hint = if link.core_known {
            format!(
                "On {}. Play joins it and keeps the mods in step.",
                link.core_name
            )
        } else {
            format!(
                "On {}, which was removed from the Servers page. Add the core again there to join the server.",
                link.core_name
            )
        };
        let mut row = SettingRow::new(link.server_name.clone()).hint(hint);
        if !link.core_known {
            row = row.child(tag("Core removed", colors::YELLOW));
        }
        row = if self.confirm_unlink {
            row.child(
                Button::new("confirm-unlink", "Unlink")
                    .variant(ButtonVariant::Danger)
                    .size(ButtonSize::Slim)
                    .on_click(cx.listener(|this, _, _, cx| this.unlink(cx))),
            )
            .child(
                muted_link("cancel-unlink", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                    this.confirm_unlink = false;
                    cx.notify();
                })),
            )
            .note(
                "Unlinking keeps the instance and its mods. Play won't join the server or update its mods.",
                colors::GRAY_400,
            )
        } else {
            row.child(
                muted_link("unlink", "Unlink").on_click(cx.listener(|this, _, _, cx| {
                    this.confirm_unlink = true;
                    cx.notify();
                })),
            )
        };
        Some(
            section("Linked server", None)
                .child(card([row.into_any_element()]))
                .into_any_element(),
        )
    }

    /// A row for a setting the instance can take from the launcher: the launcher's
    /// value with "Change", or `editor` with "Reset".
    fn default_row(
        &self,
        setting: Setting,
        label: &'static str,
        inherited: String,
        editor: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> SettingRow {
        let overridden = match setting {
            Setting::Memory => self.overrides.memory,
            Setting::Java => self.overrides.java,
            Setting::JvmArgs => self.overrides.jvm_args,
            Setting::GameArgs => self.overrides.game_args,
            Setting::Window => self.overrides.window,
            Setting::Env => self.overrides.env,
        };
        let row = SettingRow::new(label);
        if overridden {
            row.child(editor).child(
                muted_link(
                    SharedString::from(format!("reset-{}", setting.id())),
                    "Reset",
                )
                .on_click(cx.listener(move |this, _, _, cx| this.reset(setting, cx))),
            )
        } else {
            row.child(value(inherited, true))
                .child(tag("Default", colors::GRAY_500))
                .child(
                    link(
                        SharedString::from(format!("change-{}", setting.id())),
                        "Change",
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.change(setting, cx))),
                )
        }
    }
}

/// `~/Downloads`, where exports are offered first.
pub(crate) fn dirs_downloads() -> PathBuf {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Downloads"))
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

/// A text field of a fixed width, for a row's control.
fn sized(input: &Entity<TextInput>, width: f32) -> impl IntoElement {
    div().w(px(width)).child(input.clone())
}

/// "None", or the arguments.
fn or_none(text_: &str) -> String {
    if text_.trim().is_empty() {
        "None".to_string()
    } else {
        text_.to_string()
    }
}

impl Render for InstanceSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport_width = window.viewport_size().width;
        let (defaults, total, busy, background, required_java) = {
            let instances = self.instances.read(cx);
            (
                instances.defaults().clone(),
                instances.total_memory(),
                instances.is_active(&self.instance_id),
                instances
                    .background(&self.instance_id)
                    .map(|b| b.image.clone()),
                instances.required_java(&self.game_version),
            )
        };
        let instance = self.instance(cx);
        let has_background = instance.as_ref().is_some_and(|i| {
            i.background.as_deref() == Some(launcher_core::instances::CUSTOM_BACKGROUND)
        });
        let overrides = self.overrides;
        let mut sections: Vec<(SharedString, AnyElement)> = Vec::new();

        if let Some(modpack) = self.modpack_section(busy, cx) {
            sections.push(("Modpack".into(), modpack));
        }

        // General.
        let name_empty = self.text(&self.name, cx).is_empty();
        let general =
            card([
                SettingRow::new("Name")
                    .child(sized(&self.name, 260.))
                    .when(name_empty, |row| {
                        row.note("Give the instance a name.", colors::RED_200)
                    })
                    .into_any_element(),
                SettingRow::new("Group")
                    .hint("Instances in the same group are listed together.")
                    .child(sized(&self.group, 260.))
                    .into_any_element(),
                SettingRow::new("Icon")
                    .hint("Shown in the sidebar and around the launcher.")
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_3()
                                    .children(instance.as_ref().map(|instance| {
                                        appearance::instance_icon(
                                            self.instances.read(cx),
                                            instance,
                                            px(36.),
                                        )
                                    }))
                                    .child(
                                        Button::new("choose-icon", "Change…")
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Slim)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.icon_picker = match this.icon_picker {
                                                    Some(_) => None,
                                                    None => Some(IconTab::Library),
                                                };
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .when_some(self.icon_picker, |this, tab| {
                                let current = instance.as_ref().and_then(|i| i.icon.clone());
                                this.child(popover(self.icon_picker_panel(
                                    tab,
                                    current.as_deref(),
                                    cx,
                                )))
                            }),
                    )
                    .into_any_element(),
                SettingRow::new("Play background")
                    .hint("Shown behind the name on the Play tab. The launcher darkens it under the text as much as the image needs.")
                    .child(match (&background, has_background) {
                        (Some(image), _) => img(image.clone())
                            .w(px(72.))
                            .h(px(40.))
                            .rounded(radius::MD)
                            .object_fit(ObjectFit::Cover)
                            .into_any_element(),
                        (None, true) => LoadingDots.into_any_element(),
                        (None, false) => img(crate::images::hero(cx))
                            .w(px(72.))
                            .h(px(40.))
                            .rounded(radius::MD)
                            .object_fit(ObjectFit::Cover)
                            .into_any_element(),
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .child(
                                Button::new("choose-background", "Change…")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Slim)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_background_picker(cx)
                                    })),
                            )
                            .when(self.background_picker, |this| {
                                this.child(popover(self.background_picker_panel(has_background, cx)))
                            }),
                    )
                    .into_any_element(),
            ]);
        sections.push((
            "General".into(),
            section("General", None).child(general).into_any_element(),
        ));

        // Game & loader.
        let game = card([
            SettingRow::new("Minecraft")
                .hint("Set when the instance was made.")
                .child(value(self.game_version.clone(), false))
                .into_any_element(),
            self.loader.clone().into_any_element(),
        ]);
        sections.push((
            "Game & loader".into(),
            section("Game & loader", None)
                .child(game)
                .into_any_element(),
        ));

        if let Some(server) = self.server_section(cx) {
            sections.push(("Linked server".into(), server));
        }

        // Java & memory.
        let memory_check = self.fields.memory(total, cx);
        let memory = self
            .default_row(
                Setting::Memory,
                "Memory",
                fields::memory_label(defaults.memory(total)),
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(sized(&self.fields.min_ram, 84.))
                    .child("–")
                    .child(sized(&self.fields.max_ram, 84.))
                    .child(div().text_color(colors::GRAY_500).child("MB")),
                cx,
            )
            .hint_opt(fields::total_memory_label(total))
            .when(overrides.memory, |row| {
                row.note_opt(
                    memory_check
                        .error
                        .clone()
                        .map(|e| (e.into(), colors::RED_200))
                        .or(memory_check
                            .warning
                            .clone()
                            .map(|w| (w.into(), colors::YELLOW))),
                )
            });
        let java_note: Option<(SharedString, theme::Color)> = match &self.java_check {
            JavaCheck::None => None,
            JavaCheck::Checking => Some(("Checking…".into(), colors::GRAY_500)),
            JavaCheck::Found(info) => match required_java {
                Some(needed) if info.major < needed => Some((
                    format!(
                        "Java {} (version {}). Minecraft {} needs Java {needed} or newer.",
                        info.major, info.version, self.game_version
                    )
                    .into(),
                    colors::YELLOW,
                )),
                _ => Some((
                    format!("Java {} (version {})", info.major, info.version).into(),
                    colors::GREEN_300,
                )),
            },
            JavaCheck::Failed(err) => Some((err.clone(), colors::RED_200)),
        };
        let mojang_java = match required_java {
            Some(major) => format!("Mojang's Java {major}"),
            None => "Mojang's Java".to_string(),
        };
        let java = self
            .default_row(Setting::Java, "Java", mojang_java, div(), cx)
            .hint(if overrides.java {
                "A java binary to use instead of the one Mojang provides."
            } else {
                "Mojang provides the right Java for each Minecraft version."
            })
            .when(overrides.java, |row| {
                row.below(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(self.java.clone()))
                        .child(
                            Button::new("browse-java", "Browse…")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Slim)
                                .on_click(cx.listener(|this, _, _, cx| this.browse_java(cx))),
                        )
                        .child(
                            Button::new("check-java", "Check")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Slim)
                                .on_click(cx.listener(|this, _, _, cx| this.check_java(cx))),
                        ),
                )
                .note_opt(java_note)
            });
        sections.push((
            "Java & memory".into(),
            section(
                "Java & memory",
                Some("Values marked Default come from the launcher's Settings. Change one to set it for this instance only.".into()),
            )
            .child(card([memory.into_any_element(), java.into_any_element()]))
            .into_any_element(),
        ));

        // Launch options.
        let window_check = overrides
            .window
            .then(|| self.fields.window(cx))
            .and_then(Result::err);
        let env_check = overrides
            .env
            .then(|| self.fields.env(cx))
            .and_then(Result::err);
        let jvm = self
            .default_row(
                Setting::JvmArgs,
                "JVM arguments",
                or_none(&defaults.jvm_args),
                sized(&self.fields.jvm_args, 280.),
                cx,
            )
            .when(overrides.jvm_args, |row| {
                row.hint("Separated by spaces; quote values with spaces.")
            });
        let game_args = self
            .default_row(
                Setting::GameArgs,
                "Game arguments",
                or_none(&defaults.game_args),
                sized(&self.fields.game_args, 280.),
                cx,
            )
            .when(overrides.game_args, |row| {
                row.hint("Separated by spaces; quote values with spaces.")
            });
        let window = self
            .default_row(
                Setting::Window,
                "Window",
                fields::window_label(&defaults.window()),
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(sized(&self.fields.width, 72.))
                    .child("×")
                    .child(sized(&self.fields.height, 72.))
                    .child(
                        div()
                            .ml_2()
                            .text_color(colors::GRAY_400)
                            .child("Fullscreen"),
                    )
                    .child(Toggle::new("fullscreen", self.fields.fullscreen).on_change(
                        cx.listener(|this, value: &bool, _, cx| {
                            this.fields.fullscreen = *value;
                            cx.notify();
                        }),
                    )),
                cx,
            )
            .when(overrides.window, |row| {
                row.hint("Leave the size empty for the game's default.")
            })
            .note_opt(window_check.map(|e| (e.into(), colors::RED_200)));
        let env_text = if defaults.env.is_empty() {
            "None".to_string()
        } else {
            format_env(&defaults.env)
        };
        let env = self
            .default_row(
                Setting::Env,
                "Environment variables",
                env_text,
                sized(&self.fields.env, 280.),
                cx,
            )
            .when(overrides.env, |row| {
                row.hint("NAME=value, separated by spaces.")
            })
            .note_opt(env_check.map(|e| (e.into(), colors::RED_200)));
        sections.push((
            "Launch options".into(),
            section("Launch options", None)
                .child(card([
                    jvm.into_any_element(),
                    game_args.into_any_element(),
                    window.into_any_element(),
                    env.into_any_element(),
                ]))
                .into_any_element(),
        ));

        sections.push((
            "Shared game settings".into(),
            self.shared.clone().into_any_element(),
        ));

        // Maintenance.
        let id = self.instance_id.clone();
        let maintenance = card([
            SettingRow::new("Verify game files")
                .hint("Checks every file and downloads any that are missing or damaged.")
                .child(
                    Button::new("verify-files", "Verify")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let id = this.instance_id.clone();
                            this.instances
                                .update(cx, |instances, cx| instances.verify(&id, cx));
                            this.set_message(
                                "Checking every file. The progress is on the Play button.",
                                false,
                                cx,
                            );
                        })),
                )
                .into_any_element(),
            SettingRow::new("Export")
                .hint("A .mrpack file that other launchers can import.")
                .child(
                    Button::new("export-instance", "Export…")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
                )
                .into_any_element(),
            SettingRow::new("Duplicate")
                .hint("A copy with its own folder, mods and worlds.")
                .child(
                    Button::new("duplicate-instance", "Duplicate")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let id = this.instance_id.clone();
                            let result = this
                                .instances
                                .update(cx, |instances, cx| instances.duplicate(&id, cx));
                            if let Err(err) = result {
                                this.set_message(err, true, cx);
                            }
                        })),
                )
                .into_any_element(),
            SettingRow::new("Host on a Lodestone core")
                .hint("Makes a server on one of your cores with this instance's mods.")
                .child(
                    Button::new("host-instance", "Host…")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.emit(HostRequested(this.instance_id.clone()))
                        })),
                )
                .into_any_element(),
            SettingRow::new("Delete instance")
                .danger()
                .hint("Its folder, with its worlds and mods. You confirm first.")
                .child(
                    Button::new("delete-instance", "Delete…")
                        .variant(ButtonVariant::Danger)
                        .size(ButtonSize::Slim)
                        .on_click(
                            cx.listener(move |_, _, _, cx| cx.emit(DeleteRequested(id.clone()))),
                        ),
                )
                .into_any_element(),
        ]);
        sections.push((
            "Maintenance".into(),
            section("Maintenance", None)
                .child(maintenance)
                .into_any_element(),
        ));

        let titles: Vec<SharedString> = sections.iter().map(|(title, _)| title.clone()).collect();
        let active = self.nav.active(titles.len());
        // Too narrow for the list beside the settings (the window less the sidebar
        // and the page's padding): it opens over them from a button instead.
        let narrow = viewport_width < NARROW_BELOW;
        if !narrow {
            self.nav_open = false;
        }
        let list = {
            let entity = cx.entity();
            section_list(
                "instance-settings-nav",
                &titles,
                active,
                move |index, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.nav.pick(index);
                        this.nav_open = false;
                        cx.notify();
                    })
                },
            )
        };
        let content = div()
            .id("instance-settings")
            .flex()
            .flex_col()
            .gap_8()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .pb_16()
            .overflow_y_scroll()
            .track_scroll(&self.nav.scroll)
            .on_scroll(cx.listener(|this, _, _, _| this.nav.scrolled()))
            .children(
                sections
                    .into_iter()
                    .map(|(_, element)| div().w_full().max_w(CONTENT_WIDTH).child(element)),
            );
        let root = div()
            .relative()
            .flex()
            .size_full()
            .line_height(theme::line_height(text::MEDIUM, leading::NORMAL));
        let root = if narrow {
            let title = titles.get(active).cloned().unwrap_or_default();
            root.flex_col()
                .gap_3()
                .child(div().flex().flex_row().child(
                    section_menu_button("instance-settings-menu", title).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.nav_open = !this.nav_open;
                            cx.notify();
                        },
                    )),
                ))
                .child(content)
                .when(self.nav_open, |this| {
                    this.child(left_overlay(
                        "instance-settings-overlay",
                        list,
                        cx.listener(|this, _, _, cx| {
                            this.nav_open = false;
                            cx.notify();
                        }),
                    ))
                })
        } else {
            root.flex_row().gap_8().child(list).child(content)
        };
        let bar = self.has_changes(cx).then(|| {
            unsaved_bar(
                "instance-unsaved",
                "You have unsaved changes",
                self.can_save(cx),
                cx.listener(|this, _, _, cx| this.discard(cx)),
                cx.listener(|this, _, _, cx| {
                    this.save(cx);
                }),
            )
        });
        match bar {
            Some(bar) => root.pb_16().child(bar),
            None => root.children(self.toast.render()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(id: &str, kind: &str, date: &str) -> RawVersion {
        serde_json::from_value(serde_json::json!({
            "id": id, "project_id": "p", "name": id, "version_number": id,
            "version_type": kind, "loaders": ["fabric"], "game_versions": ["1.20.1"],
            "date_published": date,
        }))
        .unwrap()
    }

    #[test]
    fn offers_the_newest_release_after_the_current_version() {
        let current = ModpackRecord {
            version_id: "v2".into(),
            date_published: "2025-02-01".into(),
            ..ModpackRecord::default()
        };
        let mut versions = vec![
            version("v1", "release", "2025-01-01"),
            version("v2", "release", "2025-02-01"),
        ];
        assert!(update_candidate(&versions, &current).is_none());
        versions.push(version("v3-beta", "beta", "2025-04-01"));
        assert!(update_candidate(&versions, &current).is_none());
        versions.push(version("v3", "release", "2025-03-01"));
        assert_eq!(update_candidate(&versions, &current).unwrap().id, "v3");
        // A pack with only betas offers the newest beta.
        let betas = [version("b1", "beta", "2025-05-01")];
        assert_eq!(update_candidate(&betas, &current).unwrap().id, "b1");
    }
}
