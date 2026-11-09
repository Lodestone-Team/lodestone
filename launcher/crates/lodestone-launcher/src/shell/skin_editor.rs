//! The skin editor: its own window, opened from Settings → Skin with "Pop out
//! editor", so the main window stays usable while a skin is made. One at a time;
//! opening it again brings it to the front.
//!
//! On the left, what can be edited: every surface, the text colours, the fonts, the
//! banners, the Play button and the readability suggestions, with switches that
//! show the Settings window, a menu or a notice in the main window. On the right,
//! the open one's controls. Every change saves the skin, shows in the main window at
//! once and goes through Undo (⌘Z here or there); closing the window keeps them.
//!
//! The editor draws itself in the launcher's own look, so a skin being made can't
//! make its own controls unreadable.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use gpui::{
    AnyElement, App, Context, Entity, EntityId, FocusHandle, Global, IntoElement, MouseButton,
    PathPromptOptions, Render, RenderImage, SharedString, Subscription, Task, WeakEntity, Window,
    WindowHandle, div, img, prelude::*,
};
use launcher_core::ui_skin::{
    BUILT_IN_IMAGES, Fill, Fit, FontPart, Gradient, GradientKind, ImageFill, ImageRole, MAX_LABEL,
    Rgb, Shape, SkinBanner, SkinEntry, SkinSpec, SkinStore, Stop, SurfaceId, readable,
    surface::{BUNDLED_FONTS, MAX_BANNERS, MAX_BORDER, MAX_FILL_BLUR, MAX_RADIUS, MAX_STOPS},
};

use super::{
    LauncherShell,
    skin_settings::{BRIGHT, swatch, swatch_none},
};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Icon, KeyboardFocus, Slider, TextInput, Toggle,
        segmented,
        settings::{SettingRow, Toast, card, muted_link, section},
    },
    i18n::{ShellMsg as M, t, tr},
    instances::Instances,
    theme::{self, Color, colors, fonts, px, radius, text, weights},
    undo::{self, Files, Step, StepKey, UndoError},
};

/// Dark colours offered for tints and panels.
const DARK: [u32; 8] = [
    0x000000, 0x0E111A, 0x140E08, 0x07070C, 0x1A1030, 0x0B1F14, 0x26282C, 0x1D1E21,
];
/// Light colours offered for text.
const LIGHT: [u32; 6] = [0xFFFFFF, 0xF4EEE6, 0xE3E3E4, 0xD9CDBE, 0xB9B4D0, 0x111114];

/// Changes the skin in use with `change`: a built-in skin is copied first (the
/// copy is used and changed). Saves it, shows it, and records it for Undo, where a
/// control's consecutive changes (`field`, any number unique to the control) in the
/// form `view` are one step. Returns the name of the skin copied, if one was.
pub(crate) fn edit_skin(
    instances: &Entity<Instances>,
    view: EntityId,
    field: u64,
    cx: &mut App,
    change: impl FnOnce(&mut SkinSpec),
) -> Result<Option<String>, String> {
    let store = instances.read(cx).skins();
    let current = crate::skin::look(cx);
    let mut copied = None;
    let id = if launcher_core::ui_skin::built_in(&current.id).is_none() {
        current.id.clone()
    } else {
        let name = tr!(M::SkinCopyName, name = current.spec.name);
        let id = store
            .duplicate(&current.id, &name)
            .map_err(|err| err.to_string())?;
        instances.update(cx, |instances, cx| instances.set_skin(Some(id.clone()), cx));
        copied = Some(current.spec.name.clone());
        id
    };
    let entry = store.get(&id).map_err(|err| err.to_string())?;
    let before_spec = entry.spec.clone();
    let mut spec = entry.spec;
    change(&mut spec);
    if spec == before_spec {
        return Ok(copied);
    }
    let path = store.spec_path(&id);
    let before = Files::read(std::slice::from_ref(&path));
    store.save(&id, &spec).map_err(|err| err.to_string())?;
    let after = Files::read(std::slice::from_ref(&path));
    launcher_core::trace::settings_changed(
        "skin",
        &before_spec,
        &spec,
        serde_json::json!({ "skin": id }),
    );
    crate::skin::update(
        SkinEntry {
            id: id.clone(),
            spec: spec.clone(),
            builtin: false,
        },
        store,
        cx,
    );
    if !undo::replaying(cx) {
        let instances = instances.clone();
        let restore = move |files: Files| {
            let instances = instances.clone();
            move |cx: &mut App| {
                files
                    .write()
                    .map_err(|err| UndoError::Refused(err.to_string()))?;
                instances.update(cx, |instances, cx| instances.reload_skin(cx));
                Ok(())
            }
        };
        let said = format!("{} · {}", t(M::Skin), spec.name);
        let said_too = said.clone();
        undo::record(
            Step::new(
                StepKey::Form {
                    view,
                    fields: field,
                },
                move || said.clone(),
                move || said_too.clone(),
                restore(before),
                restore(after),
            ),
            cx,
        );
    }
    Ok(copied)
}

/// A number for a control on a surface, for [`edit_skin`]'s Undo steps.
fn field(surface: Option<SurfaceId>, control: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    surface.map(SurfaceId::key).hash(&mut hasher);
    control.hash(&mut hasher);
    hasher.finish()
}

/// The open editor window, if there is one.
#[derive(Default)]
struct EditorWindow(Option<WindowHandle<SkinEditor>>);

impl Global for EditorWindow {}

/// Opens the skin editor, or brings it to the front if it is open.
pub fn open(
    instances: Entity<Instances>,
    shell: WeakEntity<LauncherShell>,
    cx: &mut App,
) -> Option<WindowHandle<SkinEditor>> {
    if let Some(handle) = cx.try_global::<EditorWindow>().and_then(|w| w.0)
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return Some(handle);
    }
    let options = crate::window_options(t(M::SkinEditorTitle), 1120., 780., cx);
    let Ok(handle) = cx.open_window(options, |window, cx| {
        cx.new(|cx| SkinEditor::new(instances, shell, window, cx))
    }) else {
        return None;
    };
    cx.set_global(EditorWindow(Some(handle)));
    #[cfg(feature = "screenshot")]
    if crate::preview::requested_scene().is_some_and(|scene| scene.starts_with("skin-editor"))
        && let Some(path) = crate::screenshot_path()
    {
        crate::capture_window(handle.into(), path, cx);
    }
    Some(handle)
}

/// What the editor shows on its right.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pane {
    Surface(SurfaceId),
    Tokens,
    Fonts,
    Banners,
    Play,
    Readability,
}

/// The colour fields: one per kind of colour a pane sets.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Field {
    Fill,
    Stop,
    Tint,
    Border,
    Text,
    Muted,
    Token(u8),
}

/// The fill kinds a surface's segmented control offers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    None,
    Solid,
    Gradient,
    Image,
}

/// What can be shown in the main window while editing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Preview {
    Settings,
    Menu,
    Notice,
}

pub struct SkinEditor {
    instances: Entity<Instances>,
    shell: WeakEntity<LauncherShell>,
    pane: Pane,
    /// The gradient stop being edited.
    stop: usize,
    hex: HashMap<Field, Entity<TextInput>>,
    label: Entity<TextInput>,
    /// Banner thumbnails of the skin they were made for.
    banners: Vec<(String, Arc<RenderImage>)>,
    banners_for: Option<(String, Vec<SkinBanner>)>,
    previews: Vec<Preview>,
    toast: Toast,
    focus: FocusHandle,
    _prompt: Option<Task<()>>,
    _thumbs: Option<Task<()>>,
    _observe: Vec<Subscription>,
}

impl SkinEditor {
    fn new(
        instances: Entity<Instances>,
        shell: WeakEntity<LauncherShell>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let fields = [
            Field::Fill,
            Field::Stop,
            Field::Tint,
            Field::Border,
            Field::Text,
            Field::Muted,
            Field::Token(0),
            Field::Token(1),
            Field::Token(2),
            Field::Token(3),
            Field::Token(4),
            Field::Token(5),
        ];
        let hex: HashMap<Field, Entity<TextInput>> = fields
            .into_iter()
            .map(|f| {
                (
                    f,
                    cx.new(|cx| TextInput::new(cx).with_placeholder("#RRGGBB")),
                )
            })
            .collect();
        let label = cx.new(|cx| TextInput::new(cx).with_placeholder(M::Play));
        let mut observe = Vec::new();
        for (which, input) in &hex {
            let which = *which;
            observe.push(cx.observe(input, move |this, input, cx| {
                let text = input.read(cx).text().to_string();
                if let Some(rgb) = Rgb::parse(&text) {
                    this.set_color(which, Some(rgb), cx);
                    input.update(cx, |input, cx| input.set_text("", cx));
                }
            }));
        }
        observe.push(cx.observe(&label, |this, input, cx| {
            let text = input.read(cx).text().to_string();
            let current = crate::skin::look(cx)
                .spec
                .play
                .label
                .clone()
                .unwrap_or_default();
            if text.chars().count() <= MAX_LABEL && current != text.trim() {
                this.edit(None, "label", cx, move |spec| {
                    spec.play.label = (!text.trim().is_empty()).then(|| text.trim().to_string());
                });
            }
        }));
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let label_now = crate::skin::look(cx)
            .spec
            .play
            .label
            .clone()
            .unwrap_or_default();
        label.update(cx, |input, cx| input.set_text(label_now, cx));
        Self {
            instances,
            shell,
            pane: Pane::Surface(SurfaceId::Sidebar),
            stop: 0,
            hex,
            label,
            banners: Vec::new(),
            banners_for: None,
            previews: Vec::new(),
            toast: Toast::default(),
            focus,
            _prompt: None,
            _thumbs: None,
            _observe: observe,
        }
    }

    /// Opens `pane`, for the offscreen screenshots.
    pub fn show(&mut self, pane: Pane, cx: &mut Context<Self>) {
        self.pane = pane;
        self.stop = 0;
        cx.notify();
    }

    fn store(&self, cx: &App) -> SkinStore {
        self.instances.read(cx).skins()
    }

    fn say(&mut self, message: impl Into<SharedString>, error: bool, cx: &mut Context<Self>) {
        self.toast
            .show(message, error, |this: &mut Self| &mut this.toast, cx);
    }

    /// Changes the skin in use (a copy of a built-in one) with `change`.
    fn edit(
        &mut self,
        surface: Option<SurfaceId>,
        control: &str,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut SkinSpec),
    ) {
        let view = cx.entity_id();
        match edit_skin(&self.instances, view, field(surface, control), cx, change) {
            Ok(Some(name)) => self.say(tr!(M::SkinCopied, name = name), false, cx),
            Ok(None) => {}
            Err(err) => self.say(err, true, cx),
        }
        cx.notify();
    }

    /// Changes the open surface's own description: the one in `surfaces`, made from
    /// the older keys if the skin uses those, which it then replaces.
    fn edit_surface(
        &mut self,
        surface: SurfaceId,
        control: &str,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut launcher_core::ui_skin::SurfaceSpec),
    ) {
        self.edit(Some(surface), control, cx, move |spec| {
            let mut own = spec.surface(surface).unwrap_or_default();
            change(&mut own);
            // the older keys painted it: `surfaces` takes over
            match surface {
                SurfaceId::Sidebar => {
                    spec.sidebar.image = None;
                    spec.sidebar.tint = None;
                    spec.sidebar.text = Default::default();
                }
                SurfaceId::Hero => spec.hero.image = None,
                SurfaceId::PlayButton => {
                    spec.play.color = None;
                    spec.play.gradient = None;
                }
                _ => {}
            }
            if own.is_empty() {
                spec.surfaces.remove(&surface);
            } else {
                spec.surfaces.insert(surface, own);
            }
        });
    }

    /// Sets the colour a hex field (or a swatch) stands for, on the open pane.
    fn set_color(&mut self, which: Field, rgb: Option<Rgb>, cx: &mut Context<Self>) {
        match (self.pane, which) {
            (Pane::Tokens, Field::Token(slot)) => self.edit(None, "token", cx, move |spec| {
                let tokens = &mut spec.tokens;
                *match slot {
                    0 => &mut tokens.text,
                    1 => &mut tokens.secondary,
                    2 => &mut tokens.muted,
                    3 => &mut tokens.faint,
                    4 => &mut tokens.border,
                    _ => &mut spec.accent,
                } = rgb;
            }),
            (Pane::Surface(surface), _) => {
                let stop = self.stop;
                self.edit_surface(surface, &format!("{which:?}"), cx, move |own| match which {
                    Field::Fill => {
                        let opacity = match own.fill {
                            Some(Fill::Solid { opacity, .. }) => opacity,
                            _ => 100,
                        };
                        own.fill = rgb.map(|color| Fill::Solid { color, opacity });
                    }
                    Field::Stop => {
                        if let (Some(Fill::Gradient(gradient)), Some(color)) = (&mut own.fill, rgb)
                            && let Some(stop) = gradient.stops.get_mut(stop)
                        {
                            stop.color = color;
                        }
                    }
                    Field::Tint => {
                        if let Some(Fill::Image(image)) = &mut own.fill {
                            image.tint = rgb;
                            if rgb.is_some() && image.tint_opacity == 0 {
                                image.tint_opacity = 50;
                            }
                        }
                    }
                    Field::Border => own.border = rgb,
                    Field::Text => own.text = rgb,
                    Field::Muted => own.muted = rgb,
                    Field::Token(_) => {}
                })
            }
            _ => {}
        }
    }

    /// Switches the open surface's fill to `kind`, keeping its colours where it can.
    fn set_kind(&mut self, surface: SurfaceId, kind: Kind, cx: &mut Context<Self>) {
        self.stop = 0;
        self.edit_surface(surface, "kind", cx, move |own| {
            let first = match &own.fill {
                Some(Fill::Solid { color, .. }) => Some(*color),
                Some(Fill::Gradient(g)) => Some(g.stops[0].color),
                _ => None,
            }
            .unwrap_or(surface.base());
            own.fill = match kind {
                Kind::None => None,
                Kind::Solid => Some(Fill::solid(first)),
                Kind::Gradient => Some(Fill::Gradient(Gradient::vertical(
                    first,
                    first.mix(Rgb::BLACK, 0.4),
                ))),
                Kind::Image => Some(Fill::Image(ImageFill {
                    image: match surface {
                        SurfaceId::Sidebar => "builtin:dirt".into(),
                        SurfaceId::Hero => "builtin:sky".into(),
                        _ => "builtin:glass".into(),
                    },
                    fit: if surface == SurfaceId::Sidebar {
                        Fit::Tile
                    } else {
                        Fit::Cover
                    },
                    tint: Some(Rgb(0x0E111A)),
                    tint_opacity: 50,
                    blur: 0,
                })),
            };
        });
    }

    fn choose_image(&mut self, surface: SurfaceId, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t(M::SkinUseImage).into()),
        });
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| this.add_image(surface, path, cx))
                .ok();
        }));
    }

    fn add_image(&mut self, surface: SurfaceId, path: PathBuf, cx: &mut Context<Self>) {
        // a change first, so a built-in skin is copied before the image goes in
        self.edit(None, "copy", cx, |_| {});
        let id = crate::skin::look(cx).id.clone();
        let role = match surface {
            SurfaceId::Sidebar => ImageRole::Sidebar,
            SurfaceId::Hero => ImageRole::Hero,
            _ => ImageRole::Surface,
        };
        match self.store(cx).add_image(&id, &path, role) {
            Ok(name) => self.edit_surface(surface, "image", cx, move |own| match &mut own.fill {
                Some(Fill::Image(image)) => image.image = name,
                _ => {
                    own.fill = Some(Fill::Image(ImageFill {
                        image: name,
                        fit: Fit::Cover,
                        tint: None,
                        tint_opacity: 0,
                        blur: 0,
                    }))
                }
            }),
            Err(err) => self.say(err.to_string(), true, cx),
        }
    }

    fn choose_font(&mut self, part: FontPart, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t(M::SkinFontAdd).into()),
        });
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                this.edit(None, "copy", cx, |_| {});
                let id = crate::skin::look(cx).id.clone();
                match this.store(cx).add_font(&id, &path) {
                    Ok((file, _family)) => this.edit(None, part.key(), cx, move |spec| {
                        spec.fonts.set(part, Some(file))
                    }),
                    Err(err) => this.say(err.to_string(), true, cx),
                }
            })
            .ok();
        }));
    }

    fn add_banner(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t(M::SkinBannerAdd).into()),
        });
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                this.edit(None, "copy", cx, |_| {});
                let id = crate::skin::look(cx).id.clone();
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("banner")
                    .to_string();
                match this.store(cx).add_image(&id, &path, ImageRole::Banner) {
                    Ok(image) => this.edit(None, "banners", cx, move |spec| {
                        let slug: String = stem
                            .to_lowercase()
                            .chars()
                            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                            .take(24)
                            .collect();
                        let slug = slug.trim_matches('-').to_string();
                        let slug = if slug.is_empty() {
                            "banner".into()
                        } else {
                            slug
                        };
                        let banner_id = (1..)
                            .map(|n| {
                                if n == 1 {
                                    slug.clone()
                                } else {
                                    format!("{slug}-{n}")
                                }
                            })
                            .find(|id| spec.banner(id).is_none())
                            .expect("a free id");
                        spec.banners.push(SkinBanner {
                            id: banner_id,
                            name: Some(stem.chars().take(48).collect()),
                            image,
                        });
                    }),
                    Err(err) => this.say(err.to_string(), true, cx),
                }
            })
            .ok();
        }));
    }

    fn export(&mut self, cx: &mut Context<Self>) {
        let look = crate::skin::look(cx);
        let id = look.id.clone();
        let file_name = format!("{}.zip", look.spec.name);
        let folder = crate::instances::settings::dirs_downloads();
        let picked = cx.prompt_for_new_path(&folder, Some(&file_name));
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = picked.await else {
                return;
            };
            this.update(cx, |this, cx| match this.store(cx).export(&id, &path) {
                Ok(()) => this.say(
                    tr!(M::SkinExported, path = path.display().to_string()),
                    false,
                    cx,
                ),
                Err(err) => this.say(err.to_string(), true, cx),
            })
            .ok();
        }));
    }

    /// A copy of the skin in use, used from now on.
    fn save_as_new(&mut self, cx: &mut Context<Self>) {
        let look = crate::skin::look(cx);
        let name = tr!(M::SkinCopyName, name = look.spec.name);
        match self.store(cx).duplicate(&look.id, &name) {
            Ok(id) => {
                self.instances
                    .update(cx, |instances, cx| instances.set_skin(Some(id), cx));
                self.say(tr!(M::SkinCopied, name = look.spec.name), false, cx);
            }
            Err(err) => self.say(err.to_string(), true, cx),
        }
    }

    fn apply_fix(&mut self, fix: Option<readable::Fix>, cx: &mut Context<Self>) {
        let backdrops = crate::skin::look(cx).backdrops.clone();
        self.edit(None, "readability", cx, move |spec| match fix {
            Some(fix) => readable::apply(spec, &fix),
            None => {
                readable::fix_all(spec, &|surface| backdrops.get(&surface).copied());
            }
        });
    }

    fn toggle_preview(&mut self, preview: Preview, cx: &mut Context<Self>) {
        let on = !self.previews.contains(&preview);
        if on {
            self.previews.push(preview);
        } else {
            self.previews.retain(|p| *p != preview);
        }
        if preview == Preview::Notice {
            // a notice comes and goes on its own
            self.previews.retain(|p| *p != Preview::Notice);
        }
        self.shell
            .update(cx, |shell, cx| shell.preview_skin(preview, on, cx))
            .ok();
        cx.notify();
    }

    /// Makes the banners' thumbnails again when the skin or its banners changed.
    fn load_banners(&mut self, cx: &mut Context<Self>) {
        let look = crate::skin::look(cx);
        let wanted = (look.id.clone(), look.spec.banners.clone());
        if self.banners_for.as_ref() == Some(&wanted) {
            return;
        }
        self.banners_for = Some(wanted.clone());
        let store = self.store(cx);
        let work = cx.background_spawn(async move {
            let (id, banners) = wanted;
            banners
                .iter()
                .filter_map(|banner| {
                    let image = store.load_image(&id, &banner.image)?;
                    let thumb = image::DynamicImage::ImageRgba8(image)
                        .resize_to_fill(240, 135, image::imageops::FilterType::Triangle)
                        .to_rgba8();
                    Some((banner.id.clone(), crate::images::render_image(thumb)))
                })
                .collect::<Vec<_>>()
        });
        self._thumbs = Some(cx.spawn(async move |this, cx| {
            let thumbs = work.await;
            this.update(cx, |this, cx| {
                this.banners = thumbs;
                cx.notify();
            })
            .ok();
        }));
    }

    fn hex_input(&self, which: Field) -> Entity<TextInput> {
        self.hex[&which].clone()
    }

    /// A row of colours, a "Default"/"None" choice, and a field for any colour.
    fn color_row(
        &self,
        label: &'static str,
        which: Field,
        value: Option<Rgb>,
        palette: &[u32],
        none: Option<&'static str>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = format!("{which:?}");
        let mut choices: Vec<AnyElement> = Vec::new();
        if let Some(none) = none {
            choices.push(
                swatch_none(
                    SharedString::from(format!("none-{key}")),
                    none,
                    value.is_none(),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.set_color(which, None, cx)))
                .into_any_element(),
            );
        }
        for (i, rgb) in palette.iter().map(|&c| Rgb(c)).enumerate() {
            choices.push(
                swatch(
                    SharedString::from(format!("swatch-{key}-{i}")),
                    rgb,
                    value == Some(rgb),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.set_color(which, Some(rgb), cx)))
                .into_any_element(),
            );
        }
        SettingRow::new(label)
            .when_some(value, |row, rgb| row.hint(rgb.hex()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .justify_end()
                    .gap_1p5()
                    .children(choices)
                    .child(div().w(px(92.)).child(self.hex_input(which))),
            )
            .into_any_element()
    }
}

/// A slider row whose changes go to `apply`.
#[allow(clippy::too_many_arguments)]
fn slider_row(
    entity: &Entity<SkinEditor>,
    label: &'static str,
    id: &'static str,
    value: f64,
    (min, max, step): (f64, f64, f64),
    unit: &'static str,
    disabled: bool,
    apply: impl Fn(&mut SkinEditor, f64, &mut Context<SkinEditor>) + 'static,
) -> AnyElement {
    let entity = entity.clone();
    SettingRow::new(label)
        .child(
            Slider::new(id, value, min, max, step)
                .disabled(disabled)
                .format(move |v| format!("{v:.0}{unit}").into())
                .on_change(move |value, _, cx| {
                    entity.update(cx, |this, cx| apply(this, value, cx))
                }),
        )
        .into_any_element()
}

/// A small sample of a surface as the skin paints it, with text in its colours.
fn sample(surface: SurfaceId, look: &crate::skin::Look) -> AnyElement {
    let painted = look.surfaces.get(&surface).cloned();
    let inks = crate::skin::sidebar_inks_for(look);
    let (text_color, muted) = if surface == SurfaceId::Sidebar {
        (inks.primary, inks.muted)
    } else {
        let tokens = &look.spec.tokens;
        (
            painted
                .as_ref()
                .and_then(|p| p.text)
                .or(tokens.text.map(|c| Color::rgb(c.0)))
                .unwrap_or(colors::GRAY_300),
            tokens.muted.map_or(colors::GRAY_500, |c| Color::rgb(c.0)),
        )
    };
    let base = Color::rgb(surface.base().0);
    let mut tile = div()
        .relative()
        .flex()
        .flex_col()
        .justify_center()
        .gap_0p5()
        .h(px(84.))
        .w_full()
        .px_4()
        .rounded(radius::LG)
        .border_1()
        .border_color(colors::FADE_700)
        .bg(base)
        .overflow_hidden();
    if let Some(painted) = &painted {
        tile = crate::skin::paint(tile, painted);
    }
    tile.child(
        div()
            .relative()
            .font_weight(weights::BOLD.into())
            .text_color(text_color)
            .child(t(M::SkinSampleText)),
    )
    .child(
        div()
            .relative()
            .text_size(theme::text_size(text::SMALL))
            .text_color(muted)
            .child(t(M::SkinSampleMuted)),
    )
    .into_any_element()
}

impl SkinEditor {
    fn surface_pane(
        &mut self,
        surface: SurfaceId,
        look: &crate::skin::Look,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let own = look.spec.surface(surface).unwrap_or_default();
        let kind = match &own.fill {
            None => Kind::None,
            Some(Fill::Solid { .. }) => Kind::Solid,
            Some(Fill::Gradient(_)) => Kind::Gradient,
            Some(Fill::Image(_)) => Kind::Image,
        };
        let mut kinds = vec![
            (Kind::None, t(M::SkinNone).into()),
            (Kind::Solid, t(M::SkinFillColour).into()),
            (Kind::Gradient, t(M::SkinFillGradient).into()),
        ];
        if surface.takes_image() {
            kinds.push((Kind::Image, t(M::SkinImage).into()));
        }
        let mut fill_rows = vec![
            SettingRow::new(t(M::SkinFill))
                .child(segmented("skin-fill-kind", kinds, Some(&kind), false, {
                    let entity = entity.clone();
                    move |kind, _, cx| {
                        entity.update(cx, |this, cx| this.set_kind(surface, kind, cx))
                    }
                }))
                .into_any_element(),
        ];
        let mut palette: Vec<u32> = DARK.to_vec();
        palette.extend(BRIGHT);
        match own.fill.clone() {
            Some(Fill::Solid { color, opacity }) => {
                fill_rows.push(self.color_row(
                    t(M::SkinFillColour),
                    Field::Fill,
                    Some(color),
                    &palette,
                    None,
                    cx,
                ));
                fill_rows.push(slider_row(
                    &entity,
                    t(M::SkinOpacity),
                    "skin-opacity",
                    f64::from(opacity),
                    (0., 100., 5.),
                    "%",
                    false,
                    move |this, value, cx| {
                        this.edit_surface(surface, "opacity", cx, move |own| {
                            if let Some(Fill::Solid { opacity, .. }) = &mut own.fill {
                                *opacity = value.round() as u8;
                            }
                        })
                    },
                ));
            }
            Some(Fill::Gradient(gradient)) => {
                let stop = self.stop.min(gradient.stops.len() - 1);
                fill_rows.push(
                    SettingRow::new(t(M::SkinFillGradient))
                        .child(segmented(
                            "skin-gradient-kind",
                            vec![
                                (GradientKind::Linear, t(M::SkinGradientLinear).into()),
                                (GradientKind::Radial, t(M::SkinGradientRadial).into()),
                            ],
                            Some(&gradient.kind),
                            false,
                            {
                                let entity = entity.clone();
                                move |kind, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.edit_surface(
                                            surface,
                                            "gradient-kind",
                                            cx,
                                            move |own| {
                                                if let Some(Fill::Gradient(g)) = &mut own.fill {
                                                    g.kind = kind;
                                                }
                                            },
                                        )
                                    })
                                }
                            },
                        ))
                        .into_any_element(),
                );
                if gradient.kind == GradientKind::Linear {
                    fill_rows.push(slider_row(
                        &entity,
                        t(M::SkinAngle),
                        "skin-angle",
                        f64::from(gradient.angle.rem_euclid(360)),
                        (0., 345., 15.),
                        "°",
                        false,
                        move |this, value, cx| {
                            this.edit_surface(surface, "angle", cx, move |own| {
                                if let Some(Fill::Gradient(g)) = &mut own.fill {
                                    g.angle = value.round() as i16;
                                }
                            })
                        },
                    ));
                }
                // the stops: a chip each, the chosen one's colour and position
                let chips = gradient.stops.iter().enumerate().map(|(i, s)| {
                    div()
                        .id(("skin-stop", i))
                        .keyboard_focus()
                        .size(px(26.))
                        .rounded(radius::MD)
                        .bg(Color::rgb(s.color.0))
                        .border_2()
                        .cursor_pointer()
                        .border_color(if i == stop {
                            colors::WHITE
                        } else {
                            colors::FADE_700
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.stop = i;
                            cx.notify();
                        }))
                });
                let can_add = gradient.stops.len() < MAX_STOPS;
                let can_remove = gradient.stops.len() > 2;
                fill_rows.push(
                    SettingRow::new(t(M::SkinStops))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1p5()
                                .children(chips)
                                .when(can_add, |this| {
                                    this.child(
                                        Button::new("skin-add-stop", t(M::SkinAddStop))
                                            .variant(ButtonVariant::Secondary)
                                            .size(ButtonSize::Slim)
                                            .icon(Icon::Plus)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                let at = this.stop;
                                                this.edit_surface(
                                                    surface,
                                                    "stops",
                                                    cx,
                                                    move |own| {
                                                        if let Some(Fill::Gradient(g)) =
                                                            &mut own.fill
                                                        {
                                                            let next =
                                                                (at + 1).min(g.stops.len() - 1);
                                                            let (a, b) =
                                                                (g.stops[at], g.stops[next]);
                                                            let new = Stop {
                                                                color: a.color.mix(b.color, 0.5),
                                                                at: ((a.at as u16 + b.at as u16)
                                                                    / 2)
                                                                    as u8,
                                                            };
                                                            g.stops.insert(
                                                                next.max(at + 1).min(g.stops.len()),
                                                                new,
                                                            );
                                                            g.stops.sort_by_key(|s| s.at);
                                                        }
                                                    },
                                                );
                                                this.stop = at + 1;
                                            })),
                                    )
                                }),
                        )
                        .into_any_element(),
                );
                fill_rows.push(self.color_row(
                    t(M::SkinFillColour),
                    Field::Stop,
                    gradient.stops.get(stop).map(|s| s.color),
                    &palette,
                    None,
                    cx,
                ));
                fill_rows.push(slider_row(
                    &entity,
                    t(M::SkinStopAt),
                    "skin-stop-at",
                    f64::from(gradient.stops[stop].at),
                    (0., 100., 5.),
                    "%",
                    false,
                    move |this, value, cx| {
                        this.edit_surface(surface, "stop-at", cx, move |own| {
                            if let Some(Fill::Gradient(g)) = &mut own.fill {
                                // kept between its neighbours, so the stops stay in order
                                let low = if stop > 0 { g.stops[stop - 1].at } else { 0 };
                                let high = g.stops.get(stop + 1).map_or(100, |s| s.at);
                                g.stops[stop].at = (value.round() as u8).clamp(low, high);
                            }
                        })
                    },
                ));
                if can_remove {
                    fill_rows.push(
                        SettingRow::new("")
                            .child(
                                muted_link("skin-remove-stop", t(M::SkinRemoveStop)).on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.stop = stop.saturating_sub(1);
                                        this.edit_surface(surface, "stops", cx, move |own| {
                                            if let Some(Fill::Gradient(g)) = &mut own.fill {
                                                g.stops.remove(stop);
                                            }
                                        })
                                    }),
                                ),
                            )
                            .into_any_element(),
                    );
                }
            }
            Some(Fill::Image(image)) => {
                let thumb = look.surfaces.get(&surface).and_then(|p| match &p.paint {
                    Some(crate::skin::Paint::Image(image)) => Some(image.clone()),
                    _ => None,
                });
                let builtins = BUILT_IN_IMAGES.iter().enumerate().map(|(i, name)| {
                    let name = name.to_string();
                    let chosen = image.image == name;
                    div()
                        .id(("skin-builtin", i))
                        .keyboard_focus()
                        .px_2()
                        .py_0p5()
                        .rounded_full()
                        .border_1()
                        .cursor_pointer()
                        .text_size(theme::text_size(text::SMALL))
                        .border_color(if chosen {
                            colors::WHITE
                        } else {
                            colors::FADE_700
                        })
                        .text_color(if chosen {
                            colors::WHITE
                        } else {
                            colors::GRAY_400
                        })
                        .child(name.trim_start_matches("builtin:").to_string())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let name = name.clone();
                            this.edit_surface(surface, "image", cx, move |own| {
                                if let Some(Fill::Image(image)) = &mut own.fill {
                                    image.image = name;
                                }
                            })
                        }))
                });
                fill_rows.push(
                    SettingRow::new(t(M::SkinImage))
                        .hint(image.image.clone())
                        .when_some(thumb, |row, thumb| {
                            row.child(
                                img(thumb)
                                    .w(px(72.))
                                    .h(px(40.))
                                    .rounded(radius::MD)
                                    .object_fit(gpui::ObjectFit::Cover),
                            )
                        })
                        .child(
                            Button::new("skin-choose-image", t(M::SkinChoose))
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Slim)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.choose_image(surface, cx)
                                })),
                        )
                        .into_any_element(),
                );
                fill_rows.push(
                    SettingRow::new(t(M::SkinBuiltinImages))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .justify_end()
                                .gap_1p5()
                                .children(builtins),
                        )
                        .into_any_element(),
                );
                fill_rows.push(
                    SettingRow::new(t(M::SkinFit))
                        .child(segmented(
                            "skin-fit",
                            vec![
                                (Fit::Cover, t(M::SkinFitCover).into()),
                                (Fit::Tile, t(M::SkinFitTile).into()),
                            ],
                            Some(&image.fit),
                            false,
                            {
                                let entity = entity.clone();
                                move |fit, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.edit_surface(surface, "fit", cx, move |own| {
                                            if let Some(Fill::Image(image)) = &mut own.fill {
                                                image.fit = fit;
                                            }
                                        })
                                    })
                                }
                            },
                        ))
                        .into_any_element(),
                );
                fill_rows.push(self.color_row(
                    t(M::SkinTint),
                    Field::Tint,
                    image.tint,
                    &DARK,
                    Some(t(M::SkinNone)),
                    cx,
                ));
                fill_rows.push(slider_row(
                    &entity,
                    t(M::SkinTintStrength),
                    "skin-tint-strength",
                    f64::from(image.tint_opacity),
                    (0., 100., 5.),
                    "%",
                    image.tint.is_none(),
                    move |this, value, cx| {
                        this.edit_surface(surface, "tint-strength", cx, move |own| {
                            if let Some(Fill::Image(image)) = &mut own.fill {
                                image.tint_opacity = value.round() as u8;
                            }
                        })
                    },
                ));
                fill_rows.push(slider_row(
                    &entity,
                    t(M::SkinBlur),
                    "skin-blur",
                    f64::from(image.blur),
                    (0., f64::from(MAX_FILL_BLUR), 1.),
                    " px",
                    false,
                    move |this, value, cx| {
                        this.edit_surface(surface, "blur", cx, move |own| {
                            if let Some(Fill::Image(image)) = &mut own.fill {
                                image.blur = value.round() as u8;
                            }
                        })
                    },
                ));
            }
            None => {}
        }

        let shape_rows = vec![
            self.color_row(
                t(M::SkinBorder),
                Field::Border,
                own.border,
                &[0x2E1F4A, 0x343B4F, 0x4A3A2A, 0x22D3EE, 0xFFFFFF, 0x000000],
                Some(t(M::SkinNone)),
                cx,
            ),
            slider_row(
                &entity,
                t(M::SkinBorderWidth),
                "skin-border-width",
                f64::from(own.border_width.unwrap_or(1)),
                (0., f64::from(MAX_BORDER), 1.),
                " px",
                own.border.is_none(),
                move |this, value, cx| {
                    this.edit_surface(surface, "border-width", cx, move |own| {
                        own.border_width = Some(value.round() as u8)
                    })
                },
            ),
            slider_row(
                &entity,
                t(M::SkinRadius),
                "skin-radius",
                f64::from(own.radius.unwrap_or(0)),
                (0., f64::from(MAX_RADIUS), 1.),
                " px",
                false,
                move |this, value, cx| {
                    this.edit_surface(surface, "radius", cx, move |own| {
                        own.radius = Some(value.round() as u8)
                    })
                },
            ),
        ];

        let mut shape_rows = shape_rows;
        if surface == SurfaceId::Hero {
            shape_rows.push(slider_row(
                &entity,
                t(M::SkinScrim),
                "skin-scrim",
                f64::from(look.spec.hero.scrim.unwrap_or(0)),
                (0., 90., 5.),
                "%",
                false,
                |this, value, cx| {
                    this.edit(Some(SurfaceId::Hero), "scrim", cx, move |spec| {
                        spec.hero.scrim = Some(value.round() as u8)
                    })
                },
            ));
        }
        let text_rows: Vec<AnyElement> = if readable::own_text(surface) {
            let mut rows = vec![self.color_row(
                t(M::SkinTextColour),
                Field::Text,
                own.text,
                &LIGHT,
                Some(t(M::SkinDefault)),
                cx,
            )];
            if surface == SurfaceId::Sidebar {
                rows.push(self.color_row(
                    t(M::SkinMutedColour),
                    Field::Muted,
                    own.muted,
                    &LIGHT,
                    Some(t(M::SkinDefault)),
                    cx,
                ));
            }
            rows
        } else {
            vec![
                SettingRow::new(t(M::SkinTextColour))
                    .hint(t(M::SkinTextElsewhere))
                    .into_any_element(),
            ]
        };

        let painted = look.spec.surfaces.contains_key(&surface);
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_family(fonts::heading())
                            .text_size(theme::text_size(text::H2))
                            .font_weight(weights::BOLD.into())
                            .child(super::skin_checks::surface_name(surface)),
                    )
                    .when(painted, |this| {
                        this.child(
                            muted_link("skin-surface-reset", t(M::SkinUseLaunchers)).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.edit(Some(surface), "reset", cx, move |spec| {
                                        spec.surfaces.remove(&surface);
                                    })
                                }),
                            ),
                        )
                    }),
            )
            .child(sample(surface, look))
            .child(card(fill_rows))
            .child(card(shape_rows))
            .child(card(text_rows))
            .into_any_element()
    }

    fn tokens_pane(&mut self, look: &crate::skin::Look, cx: &mut Context<Self>) -> AnyElement {
        let tokens = look.spec.tokens.clone();
        let rows = [
            (M::SkinTokenText, tokens.text, &LIGHT[..]),
            (M::SkinTokenSecondary, tokens.secondary, &LIGHT[..]),
            (M::SkinTokenMuted, tokens.muted, &LIGHT[..]),
            (M::SkinTokenFaint, tokens.faint, &LIGHT[..]),
            (M::SkinTokenBorder, tokens.border, &DARK[..]),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (label, value, palette))| {
            self.color_row(
                t(label),
                Field::Token(i as u8),
                value,
                palette,
                Some(t(M::SkinDefault)),
                cx,
            )
        })
        .collect::<Vec<_>>();
        let mut rows = rows;
        rows.push(self.color_row(
            t(M::SkinAccent),
            Field::Token(5),
            look.spec.accent,
            &BRIGHT,
            Some(t(M::SkinDefault)),
            cx,
        ));
        section(t(M::SkinTokens), None)
            .child(card(rows))
            .into_any_element()
    }

    fn fonts_pane(&mut self, look: &crate::skin::Look, cx: &mut Context<Self>) -> AnyElement {
        let spec = &look.spec;
        let files = spec.fonts.files();
        let rows = FontPart::ALL
            .into_iter()
            .map(|part| {
                let current = spec.fonts.get(part).map(str::to_string);
                let label = t(match part {
                    FontPart::Heading => M::SkinFontHeading,
                    FontPart::Body => M::SkinFontBody,
                    FontPart::Sidebar => M::SkinFontSidebar,
                    FontPart::Button => M::SkinFontButton,
                    FontPart::Play => M::SkinFontPlay,
                    FontPart::Tab => M::SkinFontTab,
                    FontPart::Mono => M::SkinFontMono,
                });
                let mut options: Vec<(Option<String>, SharedString, SharedString)> =
                    vec![(None, t(M::SkinDefault).into(), fonts::body())];
                for family in BUNDLED_FONTS {
                    options.push((Some(family.into()), family.into(), family.into()));
                }
                for file in &files {
                    // shown in its own font once it is loaded
                    let face = look
                        .fonts
                        .iter()
                        .find(|(p, _)| spec.fonts.get(**p) == Some(*file))
                        .map(|(_, f)| f.clone())
                        .unwrap_or_else(fonts::body);
                    options.push((Some(file.to_string()), file.to_string().into(), face));
                }
                let chips = options
                    .into_iter()
                    .enumerate()
                    .map(|(i, (value, name, face))| {
                        let chosen = value == current;
                        div()
                            .id(SharedString::from(format!("font-{}-{i}", part.key())))
                            .keyboard_focus()
                            .px_2()
                            .py_0p5()
                            .rounded(radius::MD)
                            .border_1()
                            .cursor_pointer()
                            .font_family(face)
                            .border_color(if chosen {
                                colors::WHITE
                            } else {
                                colors::FADE_700
                            })
                            .text_color(if chosen {
                                colors::WHITE
                            } else {
                                colors::GRAY_400
                            })
                            .child(name)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let value = value.clone();
                                this.edit(None, part.key(), cx, move |spec| {
                                    spec.fonts.set(part, value)
                                })
                            }))
                    });
                SettingRow::new(label)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .justify_end()
                            .gap_1p5()
                            .max_w(px(560.))
                            .children(chips)
                            .child(
                                Button::new(
                                    SharedString::from(format!("font-add-{}", part.key())),
                                    t(M::SkinFontAdd),
                                )
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.choose_font(part, cx)),
                                ),
                            ),
                    )
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        section(t(M::SkinFonts), None)
            .child(card(rows))
            .into_any_element()
    }

    fn banners_pane(&mut self, look: &crate::skin::Look, cx: &mut Context<Self>) -> AnyElement {
        self.load_banners(cx);
        let tiles = look.spec.banners.iter().enumerate().map(|(i, banner)| {
            let thumb = self
                .banners
                .iter()
                .find(|(id, _)| *id == banner.id)
                .map(|(_, image)| image.clone());
            let id = banner.id.clone();
            div()
                .flex()
                .flex_col()
                .gap_1()
                .w(px(180.))
                .child(
                    div()
                        .h(px(101.))
                        .w_full()
                        .rounded(radius::LG)
                        .overflow_hidden()
                        .bg(colors::GRAY_800)
                        .when_some(thumb, |this, thumb| {
                            this.child(img(thumb).size_full().object_fit(gpui::ObjectFit::Cover))
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .child(
                                    div()
                                        .truncate()
                                        .font_weight(weights::BOLD.into())
                                        .child(banner.name.clone().unwrap_or(banner.id.clone())),
                                )
                                .child(
                                    div()
                                        .text_size(theme::text_size(text::SMALL))
                                        .text_color(colors::GRAY_500)
                                        .child(banner.id.clone()),
                                ),
                        )
                        .child(
                            muted_link(("skin-banner-remove", i), t(M::SkinRemove)).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    let id = id.clone();
                                    this.edit(None, "banners", cx, move |spec| {
                                        spec.banners.retain(|b| b.id != id)
                                    })
                                }),
                            ),
                        ),
                )
        });
        let full = look.spec.banners.len() >= MAX_BANNERS;
        let in_sidebar = look.spec.banner.in_sidebar;
        section(t(M::SkinBannerSet), Some(t(M::SkinBannersLead).into()))
            .child(card([SettingRow::new(t(M::SkinBannerInSidebar))
                .child(
                    Toggle::new("skin-banner-rows", in_sidebar).on_change(cx.listener(
                        |this, on: &bool, _, cx| {
                            let on = *on;
                            this.edit(None, "in-sidebar", cx, move |spec| {
                                spec.banner.in_sidebar = on
                            });
                            if !on {
                                this.instances
                                    .update(cx, |instances, _| instances.forget_sidebar_banners());
                            }
                        },
                    )),
                )
                .into_any_element()]))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_4()
                    .children(tiles)
                    .when(look.spec.banners.is_empty(), |this| {
                        this.child(
                            div()
                                .text_color(colors::GRAY_500)
                                .child(t(M::SkinBannersNone)),
                        )
                    }),
            )
            .when(!full, |this| {
                this.child(
                    div().flex().child(
                        Button::new("skin-banner-add", t(M::SkinBannerAdd))
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Slim)
                            .icon(Icon::Plus)
                            .on_click(cx.listener(|this, _, _, cx| this.add_banner(cx))),
                    ),
                )
            })
            .into_any_element()
    }

    fn play_pane(&mut self, look: &crate::skin::Look, cx: &mut Context<Self>) -> AnyElement {
        let spec = &look.spec;
        let entity = cx.entity();
        section(t(M::SkinPlayButtonSection), None)
            .child(card([
                SettingRow::new(t(M::SkinShape))
                    .child(segmented(
                        "skin-shape",
                        vec![
                            (Shape::Rounded, t(M::SkinShapeRounded).into()),
                            (Shape::Pill, t(M::SkinShapePill).into()),
                            (Shape::Pixel, t(M::SkinShapePixel).into()),
                        ],
                        Some(&spec.play.shape),
                        false,
                        move |shape, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.edit(None, "shape", cx, move |spec| spec.play.shape = shape)
                            })
                        },
                    ))
                    .into_any_element(),
                SettingRow::new(t(M::SkinLabel))
                    .child(div().w(px(220.)).child(self.label.clone()))
                    .into_any_element(),
                SettingRow::new(t(M::SkinGlow))
                    .child(
                        Toggle::new("skin-glow", spec.play.glow).on_change(cx.listener(
                            |this, on: &bool, _, cx| {
                                let on = *on;
                                this.edit(None, "glow", cx, move |spec| spec.play.glow = on)
                            },
                        )),
                    )
                    .into_any_element(),
                SettingRow::new(t(M::SkinFill))
                    .hint(super::skin_checks::surface_name(SurfaceId::PlayButton))
                    .child(
                        Button::new("skin-play-fill", t(M::SkinFill))
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Slim)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show(Pane::Surface(SurfaceId::PlayButton), cx)
                            })),
                    )
                    .into_any_element(),
            ]))
            .into_any_element()
    }

    fn readability_pane(&mut self, look: &crate::skin::Look, cx: &mut Context<Self>) -> AnyElement {
        let checks = crate::skin::checks(look);
        section(t(M::SkinReadability), None)
            .child(super::skin_checks::render(
                &checks,
                true,
                "editor-fix",
                |this: &mut Self, fix, cx| this.apply_fix(fix, cx),
                cx,
            ))
            .into_any_element()
    }

    /// An entry of the list on the left.
    fn nav_item(
        &self,
        id: impl Into<gpui::ElementId>,
        pane: Pane,
        label: impl Into<SharedString>,
        swatch: Option<AnyElement>,
        badge: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.pane == pane;
        div()
            .id(id.into())
            .keyboard_focus()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(radius::MD)
            .cursor_pointer()
            .text_color(if open {
                colors::WHITE
            } else {
                colors::GRAY_400
            })
            .when(open, |this| this.bg(colors::GRAY_750))
            .hover(|style| style.bg(colors::GRAY_800))
            .children(swatch)
            .child(div().flex_1().truncate().child(label.into()))
            .when_some(badge.filter(|n| *n > 0), |this, n| {
                this.child(
                    div()
                        .px_1p5()
                        .rounded_full()
                        .bg(colors::YELLOW.opacity(20))
                        .text_color(colors::YELLOW)
                        .text_size(theme::text_size(text::SMALL))
                        .child(n.to_string()),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| this.show(pane, cx)))
            .into_any_element()
    }
}

impl Render for SkinEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let look = crate::skin::look(cx);
        // the editor itself in the launcher's own look
        crate::skin::set_frame(&crate::skin::launcher_look());
        window.set_rem_size(theme::zoom::rem_size());
        let checks = crate::skin::checks(&look);
        let short = checks.iter().filter(|c| !c.passes()).count();
        let surfaces = SurfaceId::ALL.into_iter().map(|surface| {
            let painted = look.surfaces.get(&surface).cloned();
            let mut dot = div()
                .size(px(14.))
                .rounded(radius::XS)
                .border_1()
                .border_color(colors::FADE_700)
                .bg(Color::rgb(surface.base().0))
                .overflow_hidden();
            if let Some(painted) = &painted {
                dot = crate::skin::paint(
                    dot,
                    &crate::skin::Painted {
                        paint: painted.paint.clone(),
                        ..Default::default()
                    },
                );
            }
            let badge = checks
                .iter()
                .filter(|c| c.surface == surface && !c.passes())
                .count();
            self.nav_item(
                SharedString::from(format!("nav-{}", surface.key())),
                Pane::Surface(surface),
                super::skin_checks::surface_name(surface),
                Some(dot.into_any_element()),
                Some(badge),
                cx,
            )
        });
        let surfaces: Vec<AnyElement> = surfaces.collect();
        let others = vec![
            self.nav_item("nav-tokens", Pane::Tokens, t(M::SkinTokens), None, None, cx),
            self.nav_item("nav-fonts", Pane::Fonts, t(M::SkinFonts), None, None, cx),
            self.nav_item(
                "nav-banners",
                Pane::Banners,
                t(M::SkinBannerSet),
                None,
                None,
                cx,
            ),
            self.nav_item(
                "nav-play",
                Pane::Play,
                t(M::SkinPlayButtonSection),
                None,
                None,
                cx,
            ),
            self.nav_item(
                "nav-readability",
                Pane::Readability,
                t(M::SkinReadability),
                None,
                Some(short),
                cx,
            ),
        ];
        let heading = |label: &'static str| {
            div()
                .px_2()
                .pt_3()
                .pb_1()
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .text_color(colors::GRAY_500)
                .child(label.to_uppercase())
        };
        let shown = self.previews.clone();
        let previews: Vec<AnyElement> = [
            (Preview::Settings, M::SkinPreviewSettings),
            (Preview::Menu, M::SkinPreviewMenu),
            (Preview::Notice, M::SkinPreviewNotice),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (preview, label))| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_2()
                .py_1()
                .child(t(label))
                .child(
                    Toggle::new(("skin-preview", i), shown.contains(&preview)).on_change(
                        cx.listener(move |this, _: &bool, _, cx| this.toggle_preview(preview, cx)),
                    ),
                )
                .into_any_element()
        })
        .collect();
        let pane = match self.pane {
            Pane::Surface(surface) => self.surface_pane(surface, &look, cx),
            Pane::Tokens => self.tokens_pane(&look, cx),
            Pane::Fonts => self.fonts_pane(&look, cx),
            Pane::Banners => self.banners_pane(&look, cx),
            Pane::Play => self.play_pane(&look, cx),
            Pane::Readability => self.readability_pane(&look, cx),
        };
        let mac = cfg!(target_os = "macos");
        div()
            .id("skin-editor")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &crate::Undo, _, cx| {
                this.shell
                    .update(cx, |shell, cx| shell.undo_step(true, cx))
                    .ok();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &crate::Redo, _, cx| {
                this.shell
                    .update(cx, |shell, cx| shell.undo_step(false, cx))
                    .ok();
                cx.notify();
            }))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::GRAY_875)
            .font_family(fonts::SANS)
            .map(crate::i18n::locale_fonts)
            .text_color(colors::GRAY_300)
            .text_size(theme::text_size(text::MEDIUM))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .items_center()
                    .gap_3()
                    .h(gpui::px(super::top_bar::TOP_BAR_HEIGHT + 1.0))
                    .pl(px(if mac { 84. } else { 16. }))
                    .pr_4()
                    .bg(colors::GRAY_800)
                    .border_b_1()
                    .border_color(colors::GRAY_FADED.opacity(30))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(Icon::Palette.render(px(16.), colors::GRAY_400))
                            .child(
                                div()
                                    .font_weight(weights::BOLD.into())
                                    .child(t(M::SkinEditorTitle)),
                            )
                            .child(
                                div()
                                    .text_color(colors::GRAY_500)
                                    .child(look.spec.name.clone()),
                            ),
                    )
                    // the empty part of the bar moves the window
                    .child(
                        div()
                            .id("skin-editor-drag")
                            .flex_1()
                            .h_full()
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.start_window_move()
                            }),
                    )
                    .child(
                        Button::new("skin-save-as-new", t(M::SkinSaveAsNew))
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Slim)
                            .icon(Icon::Copy)
                            .on_click(cx.listener(|this, _, _, cx| this.save_as_new(cx))),
                    )
                    .child(
                        Button::new("skin-editor-export", t(M::SkinExport))
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Slim)
                            .icon(Icon::Upload)
                            .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("skin-editor-nav")
                            .flex()
                            .flex_col()
                            .flex_none()
                            .w(px(250.))
                            .h_full()
                            .p_2()
                            .overflow_y_scroll()
                            .bg(colors::GRAY_850)
                            .border_r_1()
                            .border_color(colors::FADE_700)
                            .child(heading(t(M::SkinSurfaces)))
                            .children(surfaces)
                            .child(heading(t(M::Skin)))
                            .children(others)
                            .child(heading(t(M::SkinPreviewIn)))
                            .children(previews),
                    )
                    .child(
                        div()
                            .id("skin-editor-pane")
                            .flex_1()
                            .h_full()
                            .overflow_y_scroll()
                            .px_8()
                            .py_6()
                            .child(div().max_w(px(760.)).child(pane)),
                    ),
            )
            .children(self.toast.render())
            .children(super::top_bar::window_controls(window, cx))
    }
}
