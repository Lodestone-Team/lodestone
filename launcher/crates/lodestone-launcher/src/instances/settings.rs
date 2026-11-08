//! An instance's Settings tab: its name, memory, arguments, window size and Java.

use std::{path::PathBuf, time::Duration};

use gpui::{
    App, Context, Entity, EventEmitter, IntoElement, Render, SharedString, Task, Window, div,
    prelude::*, px,
};
use launcher_core::{
    game::runtime::{self, JavaInfo},
    instances::{Instance, Resolution},
};

use super::Instances;
use crate::{
    components::{Button, ButtonVariant, TextInput, Toggle},
    theme::{self, colors, fonts, leading, radius, text, weights},
};

/// Asks the shell to confirm deleting the instance.
pub struct DeleteRequested(pub String);

/// The result of checking the Java override.
#[derive(Clone, Debug)]
enum JavaCheck {
    None,
    Checking,
    Found(JavaInfo),
    Failed(SharedString),
}

pub struct InstanceSettings {
    instances: Entity<Instances>,
    instance_id: String,
    name: Entity<TextInput>,
    min_ram: Entity<TextInput>,
    max_ram: Entity<TextInput>,
    jvm_args: Entity<TextInput>,
    game_args: Entity<TextInput>,
    width: Entity<TextInput>,
    height: Entity<TextInput>,
    java: Entity<TextInput>,
    fullscreen: bool,
    message: Option<(SharedString, bool)>,
    java_check: JavaCheck,
    _check: Option<Task<()>>,
}

impl EventEmitter<DeleteRequested> for InstanceSettings {}

fn input(cx: &mut Context<InstanceSettings>, text: String, placeholder: &str) -> Entity<TextInput> {
    let placeholder = placeholder.to_string();
    cx.new(|cx| {
        TextInput::new(cx)
            .with_placeholder(placeholder)
            .with_text(text)
    })
}

/// Parses memory in MiB, with an optional `G`/`M` suffix: `4G`, `4096`, `512M`.
pub fn parse_ram(text: &str) -> Option<u32> {
    let text = text.trim().to_ascii_uppercase();
    let (number, factor) = if let Some(gib) = text.strip_suffix("GB").or(text.strip_suffix('G')) {
        (gib.trim(), 1024.0)
    } else if let Some(mib) = text.strip_suffix("MB").or(text.strip_suffix('M')) {
        (mib.trim(), 1.0)
    } else {
        (text.as_str(), 1.0)
    };
    let value: f64 = number.parse().ok()?;
    let mib = (value * factor).round();
    (256.0..=1024.0 * 1024.0)
        .contains(&mib)
        .then_some(mib as u32)
}

impl InstanceSettings {
    pub fn new(instances: Entity<Instances>, instance: &Instance, cx: &mut Context<Self>) -> Self {
        let (width, height) = instance
            .resolution
            .map(|r| (r.width.to_string(), r.height.to_string()))
            .unwrap_or_default();
        Self {
            name: input(cx, instance.name.clone(), "Name"),
            min_ram: input(cx, instance.min_ram.to_string(), "512"),
            max_ram: input(cx, instance.max_ram.to_string(), "2048"),
            jvm_args: input(cx, instance.jvm_args.clone(), "-XX:+UseG1GC"),
            game_args: input(
                cx,
                instance.game_args.clone(),
                "--quickPlaySingleplayer World",
            ),
            width: input(cx, width, "854"),
            height: input(cx, height, "480"),
            java: input(
                cx,
                instance
                    .java_override
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                "Use the Java Mojang provides",
            ),
            fullscreen: instance.fullscreen,
            instances,
            instance_id: instance.id.clone(),
            message: None,
            java_check: JavaCheck::None,
            _check: None,
        }
    }

    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn text(&self, input: &Entity<TextInput>, cx: &App) -> String {
        input.read(cx).text().trim().to_string()
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let result = self.read_fields(cx).and_then(|instance| {
            self.instances
                .update(cx, |instances, cx| instances.save(&instance, cx))
        });
        self.message = Some(match result {
            Ok(()) => (
                "Saved. The changes apply the next time you press Play.".into(),
                false,
            ),
            Err(err) => (err.into(), true),
        });
        cx.notify();
    }

    fn read_fields(&self, cx: &App) -> Result<Instance, String> {
        let mut instance = self
            .instances
            .read(cx)
            .list()
            .iter()
            .find(|i| i.id == self.instance_id)
            .cloned()
            .ok_or("This instance no longer exists.")?;
        let name = self.text(&self.name, cx);
        if name.is_empty() {
            return Err("Give the instance a name.".into());
        }
        let min = parse_ram(&self.text(&self.min_ram, cx))
            .ok_or("Minimum memory: enter MB (such as 512) or GB (such as 2G), at least 256 MB.")?;
        let max = parse_ram(&self.text(&self.max_ram, cx)).ok_or(
            "Maximum memory: enter MB (such as 4096) or GB (such as 4G), at least 256 MB.",
        )?;
        if min > max {
            return Err("The minimum memory is more than the maximum.".into());
        }
        let (width, height) = (self.text(&self.width, cx), self.text(&self.height, cx));
        let resolution = match (width.is_empty(), height.is_empty()) {
            (true, true) => None,
            _ => {
                let parse = |s: &str| s.parse::<u32>().ok().filter(|v| (100..=16384).contains(v));
                match (parse(&width), parse(&height)) {
                    (Some(width), Some(height)) => Some(Resolution { width, height }),
                    _ => return Err("The window size needs a width and a height in pixels.".into()),
                }
            }
        };
        let java = self.text(&self.java, cx);
        instance.name = name;
        instance.min_ram = min;
        instance.max_ram = max;
        instance.jvm_args = self.text(&self.jvm_args, cx);
        instance.game_args = self.text(&self.game_args, cx);
        instance.resolution = resolution;
        instance.fullscreen = self.fullscreen;
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

    /// Shows a Java check result, for the offscreen screenshots.
    pub fn preview_java(&mut self, version: &str, major: u32) {
        self.java_check = JavaCheck::Found(JavaInfo {
            version: version.into(),
            major,
        });
    }
}

fn section(title: &'static str, hint: Option<SharedString>) -> gpui::Div {
    div().flex().flex_col().gap_2().child(
        div()
            .flex()
            .flex_row()
            .items_baseline()
            .gap_2()
            .child(
                div()
                    .font_family(fonts::HEADING)
                    .text_size(theme::text_size(text::H3))
                    .font_weight(weights::BOLD.into())
                    .text_color(colors::GRAY_300)
                    .child(title),
            )
            .when_some(hint, |this, hint| {
                this.child(
                    div()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(colors::GRAY_500)
                        .child(hint),
                )
            }),
    )
}

fn field(caption: &'static str, input: Entity<TextInput>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .gap_1()
        .child(
            div()
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .text_color(colors::GRAY_500)
                .child(caption),
        )
        .child(input)
}

impl Render for InstanceSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self
            .instances
            .read(cx)
            .total_memory()
            .map(|bytes| {
                format!(
                    "This Mac has {} GB of memory.",
                    bytes / (1024 * 1024 * 1024)
                )
            })
            .map(SharedString::from);
        let java_status: Option<(SharedString, theme::Color)> = match &self.java_check {
            JavaCheck::None => None,
            JavaCheck::Checking => Some(("Checking…".into(), colors::GRAY_500)),
            JavaCheck::Found(info) => Some((
                format!("Java {} (version {})", info.major, info.version).into(),
                colors::GREEN_300,
            )),
            JavaCheck::Failed(err) => Some((err.clone(), colors::RED_200)),
        };
        let fullscreen = self.fullscreen;
        let id = self.instance_id.clone();
        let busy = self.instances.read(cx).is_active(&id);
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
                            .child(
                                section("Instance", None).child(
                                    div()
                                        .max_w(px(420.))
                                        .child(field("NAME", self.name.clone())),
                                ),
                            )
                            .child(
                                section("Memory", total).child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap_4()
                                        .max_w(px(420.))
                                        .child(field("MINIMUM (MB)", self.min_ram.clone()))
                                        .child(field("MAXIMUM (MB)", self.max_ram.clone())),
                                ),
                            )
                            .child(
                                section(
                                    "Window",
                                    Some("Leave the size empty for the game's default.".into()),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_end()
                                        .gap_4()
                                        .child(
                                            div()
                                                .w(px(120.))
                                                .child(field("WIDTH", self.width.clone())),
                                        )
                                        .child(
                                            div()
                                                .w(px(120.))
                                                .child(field("HEIGHT", self.height.clone())),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap_2()
                                                .pb_2()
                                                .child(
                                                    Toggle::new("fullscreen", fullscreen)
                                                        .on_change(cx.listener(
                                                            |this, value: &bool, _, cx| {
                                                                this.fullscreen = *value;
                                                                cx.notify();
                                                            },
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .font_weight(weights::BOLD.into())
                                                        .text_color(colors::GRAY_400)
                                                        .child("Fullscreen"),
                                                ),
                                        ),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .flex_1()
                            .min_w_0()
                            .child(
                                section(
                                    "Arguments",
                                    Some("Separated by spaces; quote values with spaces.".into()),
                                )
                                .child(field("JVM ARGUMENTS", self.jvm_args.clone()))
                                .child(field("GAME ARGUMENTS", self.game_args.clone())),
                            )
                            .child(
                                section(
                                    "Java",
                                    Some(
                                        "A path to a java binary, to use instead of Mojang's."
                                            .into(),
                                    ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_2()
                                        .child(div().flex_1().child(self.java.clone()))
                                        .child(
                                            Button::new("check-java", "Check")
                                                .variant(ButtonVariant::Secondary)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.check_java(cx)
                                                })),
                                        ),
                                )
                                .when_some(
                                    java_status,
                                    |this, (status, color)| {
                                        this.child(
                                            div()
                                                .text_size(theme::text_size(text::SMALL))
                                                .font_weight(weights::BOLD.into())
                                                .text_color(color)
                                                .child(status),
                                        )
                                    },
                                ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
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
                        Button::new("verify-files", "Verify game files")
                            .variant(ButtonVariant::Secondary)
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let id = this.instance_id.clone();
                                this.instances
                                    .update(cx, |instances, cx| instances.verify(&id, cx));
                                this.message = Some((
                                    "Checking every file; the progress is on the Play button."
                                        .into(),
                                    false,
                                ));
                                cx.notify();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_memory() {
        assert_eq!(parse_ram("4096"), Some(4096));
        assert_eq!(parse_ram("4G"), Some(4096));
        assert_eq!(parse_ram(" 2.5 gb "), Some(2560));
        assert_eq!(parse_ram("512M"), Some(512));
        assert_eq!(parse_ram("100"), None);
        assert_eq!(parse_ram("lots"), None);
        assert_eq!(parse_ram(""), None);
    }
}
