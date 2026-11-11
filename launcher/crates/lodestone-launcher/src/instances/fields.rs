//! The game settings both Settings pages edit: memory, arguments, the window and
//! environment variables. The launcher's Settings page edits the global values; an
//! instance's Settings tab edits its overrides of them.

use gpui::{
    App, AppContext as _, Context, Entity, Focusable as _, IntoElement, SharedString, div,
    prelude::*,
};
use launcher_core::wording;
use launcher_core::{
    instances::{Resolution, WindowSettings, format_env, parse_env},
    settings::max_ram_is_high,
};

use crate::{
    components::TextInput,
    i18n::{CommonMsg, InstanceSettingsMsg as M, Text, format, t, tr},
    theme::{self, px, text, weights},
};

/// Parses memory in MiB, with an optional `G`/`M` suffix: `4G`, `4096`, `512M`, and
/// the French units: `4 Go`, `512 Mo`. A decimal comma is read as a point.
pub fn parse_ram(text: &str) -> Option<u32> {
    let text = text.trim().to_ascii_uppercase().replace(',', ".");
    let (number, factor) = if let Some(gib) = text
        .strip_suffix("GB")
        .or(text.strip_suffix("GO"))
        .or(text.strip_suffix('G'))
    {
        (gib.trim(), 1024.0)
    } else if let Some(mib) = text
        .strip_suffix("MB")
        .or(text.strip_suffix("MO"))
        .or(text.strip_suffix('M'))
    {
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

/// What is wrong with the memory fields as typed: an error that stops saving, and a
/// warning that does not.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryCheck {
    pub values: Option<(u32, u32)>,
    pub error: Option<String>,
    pub warning: Option<String>,
}

pub fn check_memory(min: &str, max: &str, total_bytes: Option<u64>) -> MemoryCheck {
    let (Some(min), Some(max)) = (parse_ram(min), parse_ram(max)) else {
        return MemoryCheck {
            error: Some(t(M::MemoryFormat).into()),
            ..MemoryCheck::default()
        };
    };
    if min > max {
        return MemoryCheck {
            error: Some(t(M::MemoryMinAboveMax).into()),
            ..MemoryCheck::default()
        };
    }
    let warning = max_ram_is_high(max, total_bytes).then(|| {
        tr!(
            M::MemoryTooHigh,
            computer = t(CommonMsg::ThisComputer),
            os = wording::OS
        )
    });
    MemoryCheck {
        values: Some((min, max)),
        error: None,
        warning,
    }
}

/// "This Mac has 16 GB of memory."
pub fn total_memory_label(total_bytes: Option<u64>) -> Option<SharedString> {
    total_bytes.map(|bytes| {
        tr!(
            M::TotalMemory,
            computer = t(CommonMsg::ThisComputerStart),
            size = format::memory_mb(bytes / (1024 * 1024))
        )
        .into()
    })
}

/// "512 MB – 4 GB".
pub fn memory_label((min, max): (u32, u32)) -> String {
    tr!(
        M::MemoryRange,
        min = format::memory_mb(min.into()),
        max = format::memory_mb(max.into())
    )
}

/// The size Minecraft opens its window at when given none.
pub const GAME_DEFAULT_SIZE: Resolution = Resolution {
    width: 854,
    height: 480,
};

/// The sizes the Window list names, with the names people know them by.
const NAMED_SIZES: [(u32, u32, &str); 4] = [
    (1280, 720, "720p"),
    (1920, 1080, "1080p"),
    (2560, 1440, "1440p"),
    (3840, 2160, "4K"),
];

/// A choice in the window size list, before Custom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SizeOption {
    /// `None` for the game's default size.
    pub resolution: Option<Resolution>,
    /// "1080p".
    pub name: Option<&'static str>,
    /// The size of the screen the launcher is on.
    pub this_screen: bool,
}

impl SizeOption {
    /// Whether a saved size is this choice. The game's default size is also 854 × 480
    /// written out.
    pub fn matches(&self, resolution: Option<Resolution>) -> bool {
        match self.resolution {
            None => resolution.is_none_or(|r| r == GAME_DEFAULT_SIZE),
            some => some == resolution,
        }
    }
}

/// The window sizes to offer: the game's default, the named sizes that fit on
/// `screen` (all of them when it isn't known), and the screen's own size. `current`
/// stays in the list even when it is too big for this screen.
pub fn size_options(screen: Option<Resolution>, current: Option<Resolution>) -> Vec<SizeOption> {
    let fits = |r: Resolution| screen.is_none_or(|s| r.width <= s.width && r.height <= s.height);
    let mut options: Vec<SizeOption> = NAMED_SIZES
        .into_iter()
        .map(|(width, height, name)| (Resolution { width, height }, name))
        .filter(|(r, _)| fits(*r) || current == Some(*r))
        .map(|(r, name)| SizeOption {
            resolution: Some(r),
            name: Some(name),
            this_screen: screen == Some(r),
        })
        .collect();
    if let Some(s) = screen
        && s != GAME_DEFAULT_SIZE
        && !options.iter().any(|o| o.resolution == Some(s))
    {
        options.push(SizeOption {
            resolution: Some(s),
            name: None,
            this_screen: true,
        });
    }
    options.sort_by_key(|o| {
        o.resolution
            .map(|r| u64::from(r.width) * u64::from(r.height))
    });
    options.insert(
        0,
        SizeOption {
            resolution: None,
            name: None,
            this_screen: false,
        },
    );
    options
}

/// Which of `options` the window size is: `None` for Custom, when it is none of
/// them, isn't a valid size, or Custom was picked.
pub fn selected_size(
    options: &[SizeOption],
    resolution: Result<Option<Resolution>, ()>,
    custom: bool,
) -> Option<usize> {
    if custom {
        return None;
    }
    let resolution = resolution.ok()?;
    options.iter().position(|o| o.matches(resolution))
}

/// The size of the screen the window is on, in the pixels the game's window size is
/// given in: points on macOS, the screen's pixels elsewhere.
pub fn screen_size(window: &gpui::Window, cx: &App) -> Option<Resolution> {
    let size = window.display(cx)?.bounds().size;
    let scale = if cfg!(target_os = "macos") {
        1.
    } else {
        window.scale_factor()
    };
    let (width, height) = (
        (f32::from(size.width) * scale).round(),
        (f32::from(size.height) * scale).round(),
    );
    (width >= 100. && height >= 100.).then_some(Resolution {
        width: width as u32,
        height: height as u32,
    })
}

fn size_text(r: Resolution) -> String {
    format!("{} × {}", r.width, r.height)
}

/// What a choice says: its size, or "Game default"; and its name and whether it is
/// this screen's size, after it.
fn size_option_text(option: &SizeOption) -> (String, Option<String>) {
    let label = match option.resolution {
        Some(r) => size_text(r),
        None => t(M::WindowSizeGameDefault).to_string(),
    };
    let screen = option.this_screen.then(|| t(M::WindowSizeThisScreen));
    let detail = match (option.resolution, option.name, screen) {
        (None, _, _) => Some(size_text(GAME_DEFAULT_SIZE)),
        (_, Some(name), Some(screen)) => Some(format!("{name} · {screen}")),
        (_, Some(name), None) => Some(name.to_string()),
        (_, None, Some(screen)) => Some(screen.to_string()),
        (_, None, None) => None,
    };
    (label, detail)
}

/// The Window row's controls: the size list, the width and height when the size is
/// Custom, and Fullscreen. `get` finds the fields in the view; `id` keeps the
/// controls' automation ids apart on pages that have more than one.
pub fn window_controls<V: 'static>(
    fields: &Fields,
    get: fn(&mut V) -> &mut Fields,
    id: &'static str,
    window: &gpui::Window,
    cx: &mut Context<V>,
) -> impl IntoElement + use<V> {
    use crate::components::{
        Dropdown, ListRow, Toggle,
        a11y::Accessible as _,
        settings::{popover_panel, select_button},
    };
    let screen = screen_size(window, cx);
    let resolution = fields.resolution(cx);
    let options = size_options(screen, resolution.ok().flatten());
    let selected = selected_size(&options, resolution, fields.custom_size);
    let (label, detail) = match selected {
        Some(index) => size_option_text(&options[index]),
        None => (t(M::WindowSizeCustom).to_string(), None),
    };
    let open = fields.size_menu_open;
    let toggle_menu = {
        let view = cx.entity().downgrade();
        move |_: &mut gpui::Window, cx: &mut App| {
            view.update(cx, |this, cx| {
                let fields = get(this);
                fields.size_menu_open = !fields.size_menu_open;
                cx.notify();
            })
            .ok();
        }
    };
    let button_label = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(div().font_weight(weights::BOLD.into()).child(label.clone()))
        .when_some(detail, |this, detail| {
            this.child(
                div()
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(crate::theme::ink::muted())
                    .child(detail),
            )
        });
    let list = Dropdown::new(
        select_button(SharedString::from(format!("{id}-size")), button_label)
            .aria_label(t(M::WindowSize))
            .aria_value(label)
            .expandable(open, toggle_menu.clone())
            .on_press(move |_, window, cx| toggle_menu(window, cx)),
    )
    .align_end()
    .when(open, |this| {
        let close = cx.listener(move |this, _: &(), _, cx| {
            get(this).size_menu_open = false;
            cx.notify();
        });
        let mut panel = popover_panel(SharedString::from(format!("{id}-size-list"))).w(px(240.));
        for (index, option) in options.iter().enumerate() {
            let (label, detail) = size_option_text(option);
            let resolution = option.resolution;
            panel = panel.child(
                ListRow::new(SharedString::from(format!("{id}-size-{index}")), label)
                    .when_some(detail, |row, detail| row.detail(detail))
                    .selected(selected == Some(index))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let fields = get(this);
                        fields.size_menu_open = false;
                        fields.custom_size = false;
                        fields.set_resolution(resolution, cx);
                        cx.notify();
                    })),
            );
        }
        panel = panel.child(
            ListRow::new(
                SharedString::from(format!("{id}-size-custom")),
                t(M::WindowSizeCustom),
            )
            .selected(selected.is_none())
            .on_click(cx.listener(move |this, _, window, cx| {
                let fields = get(this);
                fields.size_menu_open = false;
                fields.custom_size = true;
                // Start from the size it was, written out, to change from there.
                if Fields::text(&fields.width, cx).is_empty()
                    && Fields::text(&fields.height, cx).is_empty()
                {
                    fields.set_resolution(Some(GAME_DEFAULT_SIZE), cx);
                }
                let width = fields.width.read(cx).focus_handle(cx);
                window.focus(&width, cx);
                cx.notify();
            })),
        );
        this.popover(
            SharedString::from(format!("{id}-size-popover")),
            panel,
            move |window, cx| close(&(), window, cx),
        )
    });
    let sized = |input: &Entity<TextInput>| div().w(px(72.)).child(input.clone());
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .justify_end()
        .items_center()
        .gap_2()
        .child(list)
        .when(selected.is_none(), |this| {
            this.child(sized(&fields.width))
                .child("×")
                .child(sized(&fields.height))
        })
        .child(
            div()
                .ml_2()
                .text_color(crate::theme::ink::secondary())
                .child(t(M::Fullscreen)),
        )
        .child(
            Toggle::new(
                SharedString::from(format!("{id}-fullscreen")),
                fields.fullscreen,
            )
            .label(t(M::Fullscreen))
            .on_change(cx.listener(move |this, value: &bool, _, cx| {
                get(this).fullscreen = *value;
                cx.notify();
            })),
        )
}

pub fn window_label(window: &WindowSettings) -> String {
    let size = match window.resolution {
        Some(r) => format!("{} × {}", r.width, r.height),
        None => t(M::WindowDefaultSize).to_string(),
    };
    if window.fullscreen {
        tr!(M::WindowFullscreen, size = size)
    } else {
        size
    }
}

/// The text fields of the game settings.
pub struct Fields {
    pub min_ram: Entity<TextInput>,
    pub max_ram: Entity<TextInput>,
    pub jvm_args: Entity<TextInput>,
    pub game_args: Entity<TextInput>,
    pub width: Entity<TextInput>,
    pub height: Entity<TextInput>,
    pub env: Entity<TextInput>,
    pub fullscreen: bool,
    /// Custom was picked in the window size list, so the width and height show even
    /// when they are one of its sizes.
    pub custom_size: bool,
    /// The window size list is open.
    pub size_menu_open: bool,
}

pub fn input<V: 'static>(
    cx: &mut Context<V>,
    text: String,
    placeholder: &str,
) -> Entity<TextInput> {
    input_with(cx, text, placeholder.to_string())
}

/// A text field with a placeholder that can be a message, which follows the
/// language: `input_with(cx, text, InstanceSettingsMsg::Value)`.
pub fn input_with<V: 'static>(
    cx: &mut Context<V>,
    text: String,
    placeholder: impl Into<Text>,
) -> Entity<TextInput> {
    labeled(cx, text, placeholder, "", Text::default())
}

/// A text field named `label` for assistive technology and automation, which find it
/// by `automation_id`.
pub fn labeled<V: 'static>(
    cx: &mut Context<V>,
    text: String,
    placeholder: impl Into<Text>,
    automation_id: &'static str,
    label: impl Into<Text>,
) -> Entity<TextInput> {
    let (placeholder, label) = (placeholder.into(), label.into());
    cx.new(|cx| {
        TextInput::new(cx)
            .with_placeholder(placeholder)
            .with_label(label)
            .with_automation_id(automation_id)
            .with_text(text)
    })
}

/// The values the fields start with.
pub struct Values<'a> {
    pub memory: (u32, u32),
    pub jvm_args: &'a str,
    pub game_args: &'a str,
    pub window: WindowSettings,
    pub env: &'a std::collections::BTreeMap<String, String>,
}

impl Fields {
    pub fn new<V: 'static>(values: &Values<'_>, cx: &mut Context<V>) -> Self {
        let (width, height) = values
            .window
            .resolution
            .map(|r| (r.width.to_string(), r.height.to_string()))
            .unwrap_or_default();
        Self {
            min_ram: labeled(cx, values.memory.0.to_string(), "512", "min-ram", M::Memory),
            max_ram: labeled(
                cx,
                values.memory.1.to_string(),
                "4096",
                "max-ram",
                M::Memory,
            ),
            jvm_args: labeled(
                cx,
                values.jvm_args.to_string(),
                "-XX:+UseG1GC",
                "jvm-args",
                M::JvmArguments,
            ),
            game_args: labeled(
                cx,
                values.game_args.to_string(),
                "--quickPlaySingleplayer World",
                "game-args",
                M::GameArguments,
            ),
            width: labeled(cx, width, "854", "window-width", M::WindowWidth),
            height: labeled(cx, height, "480", "window-height", M::WindowHeight),
            env: labeled(
                cx,
                format_env(values.env),
                M::EnvPlaceholder,
                "environment-variables",
                M::EnvironmentVariables,
            ),
            fullscreen: values.window.fullscreen,
            custom_size: false,
            size_menu_open: false,
        }
    }

    /// Puts `values` back in the fields, for "Discard".
    pub fn set<V: 'static>(&mut self, values: &Values<'_>, cx: &mut Context<V>) {
        let texts = [
            (&self.min_ram, values.memory.0.to_string()),
            (&self.max_ram, values.memory.1.to_string()),
            (&self.jvm_args, values.jvm_args.to_string()),
            (&self.game_args, values.game_args.to_string()),
            (&self.env, format_env(values.env)),
        ];
        for (input, text_) in texts {
            input.update(cx, |input, cx| input.set_text(text_, cx));
        }
        self.set_window(&values.window, cx);
    }

    /// Puts a window size and Fullscreen in the fields.
    pub fn set_window<V: 'static>(&mut self, window: &WindowSettings, cx: &mut Context<V>) {
        self.set_resolution(window.resolution, cx);
        self.fullscreen = window.fullscreen;
        self.custom_size = false;
        self.size_menu_open = false;
    }

    /// Puts a window size in the width and height, or empties them for the game's
    /// default size.
    pub fn set_resolution<V: 'static>(
        &mut self,
        resolution: Option<Resolution>,
        cx: &mut Context<V>,
    ) {
        let (width, height) = resolution
            .map(|r| (r.width.to_string(), r.height.to_string()))
            .unwrap_or_default();
        self.width.update(cx, |input, cx| input.set_text(width, cx));
        self.height
            .update(cx, |input, cx| input.set_text(height, cx));
    }

    fn text(input: &Entity<TextInput>, cx: &App) -> String {
        input.read(cx).text().trim().to_string()
    }

    pub fn memory(&self, total_bytes: Option<u64>, cx: &App) -> MemoryCheck {
        check_memory(
            &Self::text(&self.min_ram, cx),
            &Self::text(&self.max_ram, cx),
            total_bytes,
        )
    }

    pub fn jvm_args(&self, cx: &App) -> String {
        Self::text(&self.jvm_args, cx)
    }

    pub fn game_args(&self, cx: &App) -> String {
        Self::text(&self.game_args, cx)
    }

    /// The window size typed: `None` for the game's default (both empty), or `Err`
    /// when it isn't a size.
    fn resolution(&self, cx: &App) -> Result<Option<Resolution>, ()> {
        parse_resolution(&Self::text(&self.width, cx), &Self::text(&self.height, cx))
    }

    pub fn window(&self, cx: &App) -> Result<WindowSettings, String> {
        let resolution = self
            .resolution(cx)
            .map_err(|()| t(M::WindowSizeInvalid).to_string())?;
        Ok(WindowSettings {
            resolution,
            fullscreen: self.fullscreen,
        })
    }

    pub fn env(&self, cx: &App) -> Result<std::collections::BTreeMap<String, String>, String> {
        parse_env(&Self::text(&self.env, cx)).map_err(|err| tr!(M::EnvInvalid, error = err))
    }
}

fn parse_resolution(width: &str, height: &str) -> Result<Option<Resolution>, ()> {
    if width.is_empty() && height.is_empty() {
        return Ok(None);
    }
    let parse = |s: &str| s.parse::<u32>().ok().filter(|v| (100..=16384).contains(v));
    match (parse(width), parse(height)) {
        (Some(width), Some(height)) => Ok(Some(Resolution { width, height })),
        _ => Err(()),
    }
}

/// A line of feedback under a field.
pub fn status(text_: impl Into<SharedString>, color: theme::Color) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::BOLD.into())
        .text_color(color)
        .child(text_.into())
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
        assert_eq!(parse_ram("4 Go"), Some(4096));
        assert_eq!(parse_ram("1,5G"), Some(1536));
        assert_eq!(parse_ram("100"), None);
        assert_eq!(parse_ram("lots"), None);
        assert_eq!(parse_ram(""), None);
    }

    fn size(width: u32, height: u32) -> Resolution {
        Resolution { width, height }
    }

    fn sizes(options: &[SizeOption]) -> Vec<Option<(u32, u32)>> {
        options
            .iter()
            .map(|o| o.resolution.map(|r| (r.width, r.height)))
            .collect()
    }

    #[test]
    fn offers_sizes_that_fit_the_screen() {
        // A 1080p screen: the default, 720p and 1080p (marked as this screen).
        let options = size_options(Some(size(1920, 1080)), None);
        assert_eq!(
            sizes(&options),
            [None, Some((1280, 720)), Some((1920, 1080))]
        );
        assert!(options[2].this_screen && options[2].name == Some("1080p"));
        // A screen of no named size is offered among them, by area.
        let options = size_options(Some(size(2560, 1600)), None);
        assert_eq!(
            sizes(&options),
            [
                None,
                Some((1280, 720)),
                Some((1920, 1080)),
                Some((2560, 1440)),
                Some((2560, 1600))
            ]
        );
        assert!(options[4].this_screen && options[4].name.is_none());
        // Without a screen, every named size; a saved size too big for this screen
        // stays.
        assert_eq!(size_options(None, None).len(), 5);
        let options = size_options(Some(size(1920, 1080)), Some(size(3840, 2160)));
        assert_eq!(options.last().unwrap().name, Some("4K"));
    }

    #[test]
    fn picks_the_saved_size() {
        let options = size_options(Some(size(1920, 1080)), None);
        assert_eq!(selected_size(&options, Ok(None), false), Some(0));
        // 854 × 480 written out is the game's default.
        assert_eq!(
            selected_size(&options, Ok(Some(size(854, 480))), false),
            Some(0)
        );
        assert_eq!(
            selected_size(&options, Ok(Some(size(1280, 720))), false),
            Some(1)
        );
        // Any other size, a size being typed, or Custom picked, is Custom.
        assert_eq!(
            selected_size(&options, Ok(Some(size(1000, 700))), false),
            None
        );
        assert_eq!(selected_size(&options, Err(()), false), None);
        assert_eq!(
            selected_size(&options, Ok(Some(size(1280, 720))), true),
            None
        );
    }

    #[test]
    fn parses_window_sizes() {
        assert_eq!(parse_resolution("", ""), Ok(None));
        assert_eq!(parse_resolution("1280", "720"), Ok(Some(size(1280, 720))));
        assert_eq!(parse_resolution("1280", ""), Err(()));
        assert_eq!(parse_resolution("50", "720"), Err(()));
    }

    #[test]
    fn checks_memory() {
        let total = Some(16u64 << 30);
        assert_eq!(check_memory("512", "4G", total).values, Some((512, 4096)));
        assert!(check_memory("512", "4G", total).warning.is_none());
        let backwards = check_memory("8G", "4G", total);
        assert_eq!(backwards.values, None);
        assert!(backwards.error.unwrap().contains("more than the maximum"));
        let high = check_memory("1G", "14G", total);
        assert_eq!(high.values, Some((1024, 14336)));
        assert!(high.warning.unwrap().contains("three quarters"));
        assert!(check_memory("x", "4G", total).error.is_some());
        assert_eq!(memory_label((512, 4096)), "512 MB – 4 GB");
        assert_eq!(memory_label((512, 6144 + 512)), "512 MB – 6.5 GB");
        assert_eq!(
            total_memory_label(total).unwrap(),
            format!("{} has 16 GB of memory.", t(CommonMsg::ThisComputerStart))
        );
    }
}
