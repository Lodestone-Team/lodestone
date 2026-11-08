//! An instance's Settings tab: its name, group and icon, the mod loader, and its
//! overrides of the launcher's settings (memory, Java, arguments, window and
//! environment variables), with Duplicate, Export and Verify game files.

use std::{path::PathBuf, time::Duration};

use gpui::{
    App, Context, Entity, EventEmitter, IntoElement, PathPromptOptions, Render, SharedString,
    Subscription, Task, Window, div, img, prelude::*, px,
};
use launcher_core::{
    content::loader_change::{self, LoaderChangeCheck},
    game::runtime::{self, JavaInfo},
    instances::{Instance, Loader, Memory},
};

use super::{
    Instances,
    fields::{self, Fields, Values, field, inherited, section, status},
    loader_picker::LoaderPicker,
    shared_settings::InstanceSharedSettings,
};
use crate::{
    components::{Button, ButtonVariant, TextInput},
    theme::{self, colors, leading, radius, text, weights},
};

/// Asks the shell to confirm deleting the instance.
pub struct DeleteRequested(pub String);

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
    arguments: bool,
    window: bool,
    env: bool,
}

pub struct InstanceSettings {
    instances: Entity<Instances>,
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
    message: Option<(SharedString, bool)>,
    java_check: JavaCheck,
    _check: Option<Task<()>>,
    _prompt: Option<Task<()>>,
    _observe: Vec<Subscription>,
}

impl EventEmitter<DeleteRequested> for InstanceSettings {}
impl EventEmitter<LoaderChangePrompt> for InstanceSettings {}

impl InstanceSettings {
    pub fn new(instances: Entity<Instances>, instance: &Instance, cx: &mut Context<Self>) -> Self {
        let (defaults, total) = {
            let instances = instances.read(cx);
            (instances.defaults().clone(), instances.total_memory())
        };
        // The fields start with what the instance uses now, so turning an override
        // on starts from the launcher's values.
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
            cx.new(|cx| LoaderPicker::new(instances, Some(game), choice, 150., cx))
        };
        let observe = vec![
            cx.observe(&fields.min_ram, |_, _, cx| cx.notify()),
            cx.observe(&fields.max_ram, |_, _, cx| cx.notify()),
            cx.observe(&loader, |_, _, cx| cx.notify()),
        ];
        let shared = {
            let instances = instances.clone();
            let (id, game) = (instance.id.clone(), instance.game_version.clone());
            cx.new(|cx| InstanceSharedSettings::new(instances, id, game, cx))
        };
        Self {
            name: fields::input(cx, instance.name.clone(), "Name"),
            group: fields::input(cx, instance.group.clone().unwrap_or_default(), "No group"),
            java: fields::input(
                cx,
                instance
                    .java_override
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                "Use the Java Mojang provides",
            ),
            fields,
            loader,
            shared,
            overrides: Overrides {
                memory: instance.memory.is_some(),
                arguments: instance.jvm_args.is_some() || instance.game_args.is_some(),
                window: instance.window.is_some(),
                env: instance.env.is_some(),
            },
            instances,
            instance_id: instance.id.clone(),
            game_version: instance.game_version.clone(),
            message: None,
            java_check: JavaCheck::None,
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
        self.message = Some((message.into(), error));
        cx.notify();
    }

    /// Saves the fields. A loader change that would leave enabled mods unable to load,
    /// or worlds without their modded blocks, asks first (see [`LoaderChangePrompt`]).
    fn save(&mut self, cx: &mut Context<Self>) {
        let before = self.instance(cx);
        let instance = match self.read_fields(cx) {
            Ok(instance) => instance,
            Err(err) => return self.set_message(err, true, cx),
        };
        if let Some(prompt) = self.loader_change_prompt(before.as_ref(), &instance, cx) {
            cx.emit(prompt);
            return;
        }
        self.commit(instance, &[], cx);
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

    /// The prompt for changing the loader to `loader` at `version`, as Save would ask
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

    /// Saves `instance`, after turning off `disable` (mods, with `.disabled`).
    pub fn commit(&mut self, instance: Instance, disable: &[String], cx: &mut Context<Self>) {
        let before = self.instance(cx);
        if !disable.is_empty() {
            let dirs = self.instances.read(cx).content_dirs(&instance.id);
            if let Err(err) = loader_change::disable_mods(&dirs, disable) {
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
                        "Saved. The game is running; {} installs the next time it starts.{turned_off}",
                        instance.loader_label()
                    ),
                    (true, false) => format!(
                        "Saved. {} installs the next time you press Play.{turned_off}",
                        instance.loader_label()
                    ),
                    (false, true) => {
                        "Saved. The game is running, so the changes apply the next time it starts."
                            .to_string()
                    }
                    (false, false) => {
                        "Saved. The changes apply the next time you press Play.".to_string()
                    }
                };
                self.set_message(message, false, cx);
            }
            Err(err) => self.set_message(err, true, cx),
        }
    }

    fn read_fields(&self, cx: &App) -> Result<Instance, String> {
        let mut instance = self.instance(cx).ok_or("This instance no longer exists.")?;
        let name = self.text(&self.name, cx);
        if name.is_empty() {
            return Err("Give the instance a name.".into());
        }
        let total = self.instances.read(cx).total_memory();
        instance.memory = if self.overrides.memory {
            let check = self.fields.memory(total, cx);
            let (min, max) = check
                .values
                .ok_or_else(|| format!("Memory: {}", check.error.unwrap_or_default()))?;
            Some(Memory { min, max })
        } else {
            None
        };
        (instance.jvm_args, instance.game_args) = if self.overrides.arguments {
            (
                Some(self.fields.jvm_args(cx)),
                Some(self.fields.game_args(cx)),
            )
        } else {
            (None, None)
        };
        instance.window = if self.overrides.window {
            Some(self.fields.window(cx)?)
        } else {
            None
        };
        instance.env = if self.overrides.env {
            Some(self.fields.env(cx)?)
        } else {
            None
        };
        let loader = self.loader.read(cx).choice()?;
        instance.loader = Loader::from_kind(loader.as_ref().map(|l| l.kind));
        instance.loader_version = loader.map(|l| l.version);
        let group = self.text(&self.group, cx);
        instance.group = (!group.is_empty()).then_some(group);
        let java = self.text(&self.java, cx);
        instance.name = name;
        instance.java_override = (!java.is_empty()).then(|| PathBuf::from(java));
        Ok(instance)
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
                    Ok(()) => this.set_message("The icon is saved.", false, cx),
                    Err(err) => this.set_message(err, true, cx),
                }
            })
            .ok();
        }));
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

    /// Shows a Java check result, for the offscreen screenshots.
    pub fn preview_java(&mut self, version: &str, major: u32) {
        self.java_check = JavaCheck::Found(JavaInfo {
            version: version.into(),
            major,
        });
    }

    fn override_toggle(
        &self,
        id: &'static str,
        label: &'static str,
        value: bool,
        set: fn(&mut Overrides, bool),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        fields::labelled_toggle(
            id,
            label,
            value,
            cx.listener(move |this, value: &bool, _, cx| {
                set(&mut this.overrides, *value);
                cx.notify();
            }),
        )
    }
}

/// `~/Downloads`, where exports are offered first.
fn dirs_downloads() -> PathBuf {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Downloads"))
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

impl Render for InstanceSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (defaults, total, busy, icon, required_java) = {
            let instances = self.instances.read(cx);
            (
                instances.defaults().clone(),
                instances.total_memory(),
                instances.is_active(&self.instance_id),
                instances.icon(&self.instance_id),
                instances.required_java(&self.game_version),
            )
        };
        let java_status: Option<(SharedString, theme::Color)> = match &self.java_check {
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
        let memory_check = self.fields.memory(total, cx);
        let overrides = self.overrides;
        let id = self.instance_id.clone();
        let has_icon = icon.is_some();

        let identity = section("Instance", None)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .max_w(px(520.))
                    .child(field("NAME", self.name.clone()))
                    .child(field("GROUP", self.group.clone())),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(match icon {
                        Some(image) => img(image)
                            .size(px(40.))
                            .rounded(radius::LG)
                            .into_any_element(),
                        None => img("images/placeholder-cube.png")
                            .size(px(40.))
                            .rounded(radius::LG)
                            .into_any_element(),
                    })
                    .child(
                        Button::new("choose-icon", "Choose icon…")
                            .variant(ButtonVariant::Secondary)
                            .on_click(cx.listener(|this, _, _, cx| this.choose_icon(cx))),
                    )
                    .when(has_icon, |this| {
                        this.child(
                            Button::new("clear-icon", "Use the default")
                                .variant(ButtonVariant::Ghost)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let id = this.instance_id.clone();
                                    let result = this
                                        .instances
                                        .update(cx, |instances, cx| instances.clear_icon(&id, cx));
                                    if let Err(err) = result {
                                        this.set_message(err, true, cx);
                                    }
                                })),
                        )
                    }),
            );

        let loader = section(
            "Mod loader",
            Some(format!("For Minecraft {}.", self.game_version).into()),
        )
        .child(div().max_w(px(520.)).child(self.loader.clone()));

        let memory = section("Memory", fields::total_memory_label(total))
            .child(self.override_toggle(
                "override-memory",
                "Override memory",
                overrides.memory,
                |o, v| o.memory = v,
                cx,
            ))
            .child(if overrides.memory {
                fields::memory_inputs(&self.fields, &memory_check).into_any_element()
            } else {
                inherited(fields::memory_label(defaults.memory(total))).into_any_element()
            });

        let java = section(
            "Java",
            Some("A java binary to use instead of the one Mojang provides.".into()),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(div().flex_1().child(self.java.clone()))
                .child(
                    Button::new("browse-java", "Browse…")
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| this.browse_java(cx))),
                )
                .child(
                    Button::new("check-java", "Check")
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| this.check_java(cx))),
                ),
        )
        .when_some(java_status, |this, (text_, color)| {
            this.child(status(text_, color))
        });

        let arguments = section(
            "Arguments",
            Some("Separated by spaces; quote values with spaces.".into()),
        )
        .child(self.override_toggle(
            "override-arguments",
            "Override arguments",
            overrides.arguments,
            |o, v| o.arguments = v,
            cx,
        ))
        .child(if overrides.arguments {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(field("JVM ARGUMENTS", self.fields.jvm_args.clone()))
                .child(field("GAME ARGUMENTS", self.fields.game_args.clone()))
                .into_any_element()
        } else {
            let text_ = match (defaults.jvm_args.is_empty(), defaults.game_args.is_empty()) {
                (true, true) => "No extra arguments".to_string(),
                _ => format!("{} {}", defaults.jvm_args, defaults.game_args)
                    .trim()
                    .to_string(),
            };
            inherited(text_).into_any_element()
        });

        let window = section(
            "Window",
            Some("Leave the size empty for the game's default.".into()),
        )
        .child(self.override_toggle(
            "override-window",
            "Override the window",
            overrides.window,
            |o, v| o.window = v,
            cx,
        ))
        .child(if overrides.window {
            fields::window_inputs(
                &self.fields,
                "fullscreen",
                cx.listener(|this, value: &bool, _, cx| {
                    this.fields.fullscreen = *value;
                    cx.notify();
                }),
            )
            .into_any_element()
        } else {
            inherited(fields::window_label(&defaults.window())).into_any_element()
        });

        let env = section(
            "Environment variables",
            Some("NAME=value, separated by spaces.".into()),
        )
        .child(self.override_toggle(
            "override-env",
            "Override environment variables",
            overrides.env,
            |o, v| o.env = v,
            cx,
        ))
        .child(if overrides.env {
            self.fields.env.clone().into_any_element()
        } else {
            let text_ = if defaults.env.is_empty() {
                "None".to_string()
            } else {
                launcher_core::instances::format_env(&defaults.env)
            };
            inherited(text_).into_any_element()
        });

        div()
            .id("instance-settings")
            .flex()
            .flex_col()
            .gap_6()
            .size_full()
            .overflow_y_scroll()
            .pr_2()
            .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_8()
                    .w_full()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .flex_1()
                            .min_w_0()
                            .child(identity)
                            .child(loader)
                            .child(memory),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .flex_1()
                            .min_w_0()
                            .child(java)
                            .child(arguments)
                            .child(window)
                            .child(env),
                    ),
            )
            .child(self.shared.clone())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .pt_2()
                    .pb_6()
                    .child(
                        Button::new("save-settings", "Save")
                            .variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                    )
                    .child(
                        Button::new("duplicate-instance", "Duplicate")
                            .variant(ButtonVariant::Secondary)
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
                    .child(
                        Button::new("export-instance", "Export .mrpack…")
                            .variant(ButtonVariant::Secondary)
                            .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
                    )
                    .child(
                        Button::new("verify-files", "Verify game files")
                            .variant(ButtonVariant::Secondary)
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let id = this.instance_id.clone();
                                this.instances
                                    .update(cx, |instances, cx| instances.verify(&id, cx));
                                this.set_message(
                                    "Checking every file; the progress is on the Play button.",
                                    false,
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new("delete-instance", "Delete instance…")
                            .variant(ButtonVariant::Danger)
                            .on_click(
                                cx.listener(move |_, _, _, cx| {
                                    cx.emit(DeleteRequested(id.clone()))
                                }),
                            ),
                    )
                    .when_some(self.message.clone(), |this, (message, error)| {
                        this.child(
                            div()
                                .px_3()
                                .py_1()
                                .rounded(radius::MD)
                                .text_size(theme::text_size(text::SMALL))
                                .font_weight(weights::BOLD.into())
                                .text_color(if error {
                                    colors::RED_200
                                } else {
                                    colors::GREEN_300
                                })
                                .child(message),
                        )
                    }),
            )
    }
}
