//! Settings > Launcher > Skin: the gallery of skins (built-in and imported) with
//! Import, Export, New skin… and Delete, then a card per surface the skin sets, a
//! readability check and Reset to Default. Changes apply to the window at once and
//! go through Undo.
//!
//! Built-in skins can't be changed: the first change to one makes a copy and
//! changes that.

use std::path::PathBuf;

use gpui::{
    AnyElement, App, Context, Entity, IntoElement, PathPromptOptions, Render, SharedString,
    Subscription, Task, Window, div, img, prelude::*,
};
use launcher_core::ui_skin::{
    Fit, ImageRole, MAX_BLUR, MAX_LABEL, Rgb, Shape, SkinEntry, SkinSpec, SkinStore,
};

use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Icon, KeyboardFocus, Slider, TextInput, Toggle,
        segmented,
        settings::{SettingRow, Toast, card, muted_link, section, tag},
    },
    i18n::{ShellMsg as M, t, tr},
    instances::Instances,
    theme::{self, Color, colors, px, radius, text, weights},
    undo::{self, Files, Step, StepKey, UndoError},
};

/// Colours offered for the accent, the Play button and the progress bar.
pub const BRIGHT: [u32; 10] = [
    0x59B2F3, 0x22D3EE, 0x6DD277, 0x8BD86A, 0xEFB440, 0xF08A4B, 0xE5546A, 0xFF2BD6, 0xA78BFA,
    0xF4F6FB,
];
/// Colours offered for the sidebar's tint.
const DARK: [u32; 6] = [0x000000, 0x10131A, 0x140E08, 0x07070C, 0x1A1030, 0x0B1F14];

/// The controls, one bit each, so a slider's drag is one Undo step.
#[derive(Clone, Copy)]
enum Control {
    SidebarImage = 0,
    Fit,
    Tint,
    TintStrength,
    Blur,
    Banner,
    Hero,
    PlayColor,
    Gradient,
    Shape,
    Label,
    Glow,
    Accent,
    Progress,
    Readability,
}

/// A colour that a hex field sets.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Hex {
    Tint,
    Play,
    Gradient,
    Accent,
    Progress,
}

pub struct SkinSettings {
    instances: Entity<Instances>,
    entries: Vec<SkinEntry>,
    label: Entity<TextInput>,
    hex: Vec<(Hex, Entity<TextInput>)>,
    confirm_delete: bool,
    toast: Toast,
    _prompt: Option<Task<()>>,
    _observe: Vec<Subscription>,
}

impl SkinSettings {
    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        let label = cx.new(|cx| TextInput::new(cx).with_placeholder(M::Play));
        let hex: Vec<(Hex, Entity<TextInput>)> = [
            Hex::Tint,
            Hex::Play,
            Hex::Gradient,
            Hex::Accent,
            Hex::Progress,
        ]
        .into_iter()
        .map(|which| {
            (
                which,
                cx.new(|cx| TextInput::new(cx).with_placeholder("#RRGGBB")),
            )
        })
        .collect();
        let mut observe = vec![cx.observe(&label, |this, input, cx| {
            let text = input.read(cx).text().to_string();
            if text.chars().count() <= MAX_LABEL
                && crate::skin::look(cx)
                    .spec
                    .play
                    .label
                    .as_deref()
                    .unwrap_or("")
                    != text.trim()
            {
                this.edit(Control::Label, cx, move |spec| {
                    spec.play.label = (!text.trim().is_empty()).then(|| text.trim().to_string());
                });
            }
        })];
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
        let mut this = Self {
            instances,
            entries: Vec::new(),
            label,
            hex,
            confirm_delete: false,
            toast: Toast::default(),
            _prompt: None,
            _observe: observe,
        };
        this.refresh(cx);
        this
    }

    pub fn toast(&self) -> &Toast {
        &self.toast
    }

    fn store(&self, cx: &App) -> SkinStore {
        self.instances.read(cx).skins()
    }

    /// Reads the skins again, and the label of the one in use.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.entries = self.store(cx).list();
        self.confirm_delete = false;
        let label = crate::skin::look(cx)
            .spec
            .play
            .label
            .clone()
            .unwrap_or_default();
        if self.label.read(cx).text().as_ref() != label {
            self.label.update(cx, |input, cx| input.set_text(label, cx));
        }
        cx.notify();
    }

    fn say(&mut self, message: impl Into<SharedString>, error: bool, cx: &mut Context<Self>) {
        self.toast
            .show(message, error, |this: &mut Self| &mut this.toast, cx);
    }

    fn use_skin(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.instances
            .update(cx, |instances, cx| instances.set_skin(id, cx));
        self.refresh(cx);
    }

    /// The skin to change: the one in use, or a new copy of it if it is built in.
    fn editable(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let current = crate::skin::look(cx);
        if !current.id.is_empty() && launcher_core::ui_skin::built_in(&current.id).is_none() {
            return Some(current.id.clone());
        }
        let store = self.store(cx);
        let name = tr!(M::SkinCopyName, name = current.spec.name);
        match store.duplicate(&current.id, &name) {
            Ok(id) => {
                self.use_skin(Some(id.clone()), cx);
                self.say(tr!(M::SkinCopied, name = current.spec.name), false, cx);
                Some(id)
            }
            Err(err) => {
                self.say(err.to_string(), true, cx);
                None
            }
        }
    }

    /// Changes the skin in use with `change`, saves it, shows it, and records it for
    /// Undo (a control's consecutive changes are one step).
    fn edit(
        &mut self,
        control: Control,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut SkinSpec),
    ) {
        let Some(id) = self.editable(cx) else {
            return;
        };
        let store = self.store(cx);
        let Ok(entry) = store.get(&id) else {
            return;
        };
        let before_spec = entry.spec.clone();
        let mut spec = entry.spec.clone();
        change(&mut spec);
        if spec == before_spec {
            return;
        }
        let path = store.spec_path(&id);
        let before = Files::read(std::slice::from_ref(&path));
        if let Err(err) = store.save(&id, &spec) {
            self.say(err.to_string(), true, cx);
            return;
        }
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
            let instances = self.instances.clone();
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
                        view: cx.entity_id(),
                        fields: 1 << control as u64,
                    },
                    move || said.clone(),
                    move || said_too.clone(),
                    restore(before),
                    restore(after),
                ),
                cx,
            );
        }
        cx.notify();
    }

    /// "Fix readability": every surface works its text out from its image again.
    fn fix_readability(&mut self, cx: &mut Context<Self>) {
        use launcher_core::ui_skin::Readability;
        self.edit(Control::Readability, cx, |spec| {
            spec.sidebar.readability = Readability::Auto;
            spec.hero.readability = Readability::Auto;
            spec.banner.readability = Readability::Auto;
        });
    }

    fn set_color(&mut self, which: Hex, rgb: Option<Rgb>, cx: &mut Context<Self>) {
        let control = match which {
            Hex::Tint => Control::Tint,
            Hex::Play => Control::PlayColor,
            Hex::Gradient => Control::Gradient,
            Hex::Accent => Control::Accent,
            Hex::Progress => Control::Progress,
        };
        self.edit(control, cx, move |spec| match which {
            Hex::Tint => spec.sidebar.tint = rgb,
            Hex::Play => spec.play.color = rgb,
            Hex::Gradient => spec.play.gradient = rgb,
            Hex::Accent => spec.accent = rgb,
            Hex::Progress => spec.progress = rgb,
        });
    }

    /// Asks for an image file and makes it the skin's image for `role`.
    fn choose_image(&mut self, role: ImageRole, cx: &mut Context<Self>) {
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
            this.update(cx, |this, cx| this.add_image(role, path, cx))
                .ok();
        }));
    }

    fn add_image(&mut self, role: ImageRole, path: PathBuf, cx: &mut Context<Self>) {
        let Some(id) = self.editable(cx) else {
            return;
        };
        match self.store(cx).add_image(&id, &path, role) {
            Ok(name) => {
                let control = match role {
                    ImageRole::Sidebar => Control::SidebarImage,
                    ImageRole::Hero => Control::Hero,
                };
                self.edit(control, cx, move |spec| match role {
                    ImageRole::Sidebar => spec.sidebar.image = Some(name),
                    ImageRole::Hero => spec.hero.image = Some(name),
                });
            }
            Err(err) => self.say(err.to_string(), true, cx),
        }
    }

    fn import(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: false,
            prompt: Some(t(M::SkinImportButton).into()),
        });
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                let store = this.store(cx);
                match store.import(&path) {
                    Ok(id) => {
                        let name = store.get(&id).map(|e| e.spec.name).unwrap_or_default();
                        this.use_skin(Some(id), cx);
                        this.say(tr!(M::SkinImported, name = name), false, cx);
                    }
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

    fn new_skin(&mut self, cx: &mut Context<Self>) {
        let look = crate::skin::look(cx);
        let name = tr!(M::SkinCopyName, name = look.spec.name);
        match self.store(cx).duplicate(&look.id, &name) {
            Ok(id) => self.use_skin(Some(id), cx),
            Err(err) => self.say(err.to_string(), true, cx),
        }
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        let id = crate::skin::look(cx).id.clone();
        if let Err(err) = self.store(cx).delete(&id) {
            self.say(err.to_string(), true, cx);
            return;
        }
        self.use_skin(None, cx);
    }

    fn hex_input(&self, which: Hex) -> Entity<TextInput> {
        self.hex
            .iter()
            .find(|(w, _)| *w == which)
            .map(|(_, input)| input.clone())
            .expect("a field for every colour")
    }

    /// A row of colours to pick from, a "Default" (or "None") choice, and a field
    /// for any colour.
    fn color_row(
        &self,
        label: &'static str,
        which: Hex,
        value: Option<Rgb>,
        palette: &[u32],
        none: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut choices: Vec<AnyElement> = vec![
            swatch_none(("skin-none", which as usize), none, value.is_none())
                .on_click(cx.listener(move |this, _, _, cx| this.set_color(which, None, cx)))
                .into_any_element(),
        ];
        for (i, rgb) in palette.iter().map(|&c| Rgb(c)).enumerate() {
            choices.push(
                swatch(
                    ("skin-swatch", which as usize * 100 + i),
                    rgb,
                    value == Some(rgb),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.set_color(which, Some(rgb), cx)))
                .into_any_element(),
            );
        }
        let input = self.hex_input(which);
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
                    .child(div().w(px(92.)).child(input)),
            )
            .into_any_element()
    }
}

/// A round colour to pick.
pub fn swatch(
    id: impl Into<gpui::ElementId>,
    rgb: Rgb,
    selected: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .keyboard_focus()
        .size(px(20.))
        .rounded_full()
        .bg(Color::rgb(rgb.0))
        .border_2()
        .cursor_pointer()
        .map(|this| {
            if selected {
                this.border_color(colors::WHITE)
            } else {
                this.border_color(colors::GRAY_FADED.opacity(30))
                    .hover(|style| style.border_color(colors::GRAY_400))
            }
        })
}

/// "Default" or "None" among the swatches.
pub fn swatch_none(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    selected: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .keyboard_focus()
        .px_2()
        .h(px(20.))
        .flex()
        .items_center()
        .rounded_full()
        .border_1()
        .cursor_pointer()
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::BOLD.into())
        .map(|this| {
            if selected {
                this.border_color(colors::WHITE)
                    .text_color(colors::GRAY_300)
            } else {
                this.border_color(colors::FADE_700)
                    .text_color(colors::GRAY_500)
                    .hover(|style| style.text_color(colors::GRAY_300))
            }
        })
        .child(label)
}

/// A skin's card in the gallery: a small drawing of the window in its colours.
fn gallery_card(entry: &SkinEntry, selected: bool) -> gpui::Stateful<gpui::Div> {
    let spec = &entry.spec;
    let sidebar = spec
        .sidebar
        .tint
        .map_or(colors::GRAY_850, |tint| Color::rgb(tint.0));
    let accent = spec.accent.map_or(colors::BLUE_200, |c| Color::rgb(c.0));
    let play = spec
        .play
        .color
        .map_or(colors::GREEN_400, |c| Color::rgb(c.0));
    let hero = match spec.hero.image.as_deref() {
        Some("builtin:sky") => Color::rgb(0x5FA8F0),
        Some("builtin:glass") => Color::rgb(0x3A3470),
        Some("builtin:grid") => Color::rgb(0x2A0A36),
        Some(_) => colors::GRAY_700,
        None => colors::GRAY_800,
    };
    div()
        .id(SharedString::from(format!("skin-{}", entry.id)))
        .keyboard_focus()
        .flex()
        .flex_col()
        .gap_1p5()
        .w(px(132.))
        .p_1p5()
        .rounded(radius::LG)
        .border_2()
        .cursor_pointer()
        .map(|this| {
            if selected {
                this.border_color(crate::theme::accent::light())
            } else {
                this.border_color(colors::TRANSPARENT)
                    .hover(|style| style.border_color(colors::FADE_700))
            }
        })
        .child(
            div()
                .flex()
                .flex_row()
                .h(px(72.))
                .rounded(radius::MD)
                .overflow_hidden()
                .border_1()
                .border_color(colors::FADE_700)
                .child(div().w(px(34.)).h_full().bg(sidebar))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(div().flex_1().bg(hero))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(px(22.))
                                .bg(colors::GRAY_850)
                                .child(div().w(px(40.)).h(px(10.)).bg(play).map(|this| {
                                    match spec.play.shape {
                                        Shape::Pill => this.rounded_full(),
                                        Shape::Rounded => this.rounded(radius::DEFAULT),
                                        Shape::Pixel => this.rounded_none(),
                                    }
                                })),
                        ),
                )
                .child(div().w(px(3.)).h_full().bg(accent)),
        )
        .child(
            div()
                .truncate()
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .text_color(if selected {
                    colors::WHITE
                } else {
                    colors::GRAY_400
                })
                .child(spec.name.clone()),
        )
}

impl Render for SkinSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let look = crate::skin::look(cx);
        let spec = look.spec.clone();
        let builtin = launcher_core::ui_skin::built_in(&look.id).is_some();
        let entity = cx.entity();
        let on = |f: fn(&mut Self, &mut Context<Self>, f64)| {
            let entity = entity.clone();
            move |value: f64, _: &mut Window, cx: &mut App| {
                entity.update(cx, |this, cx| f(this, cx, value))
            }
        };

        // the gallery
        let cards: Vec<AnyElement> = self
            .entries
            .iter()
            .map(|entry| {
                let id = entry.id.clone();
                gallery_card(entry, entry.id == look.id)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.use_skin(Some(id.clone()), cx)),
                    )
                    .into_any_element()
            })
            .collect();
        let actions =
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    Button::new("skin-import", t(M::SkinImport))
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .icon(Icon::Download)
                        .on_click(cx.listener(|this, _, _, cx| this.import(cx))),
                )
                .child(
                    Button::new("skin-export", t(M::SkinExport))
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .icon(Icon::Upload)
                        .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
                )
                .child(
                    Button::new("skin-new", t(M::SkinNew))
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .icon(Icon::Plus)
                        .on_click(cx.listener(|this, _, _, cx| this.new_skin(cx))),
                )
                .when(!builtin, |this| {
                    if self.confirm_delete {
                        this.child(
                            Button::new(
                                "skin-delete-confirm",
                                tr!(M::SkinDeleteConfirm, name = spec.name),
                            )
                            .variant(ButtonVariant::Danger)
                            .size(ButtonSize::Slim)
                            .on_click(cx.listener(|this, _, _, cx| this.delete(cx))),
                        )
                    } else {
                        this.child(muted_link("skin-delete", t(M::SkinDelete)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.confirm_delete = true;
                                cx.notify();
                            }),
                        ))
                    }
                });
        let notice = if crate::skin::safe_mode() {
            Some((t(M::SkinSafeMode).to_string(), colors::YELLOW))
        } else {
            crate::skin::error(cx).map(|e| (tr!(M::SkinCouldntLoad, error = e), colors::RED_200))
        };

        // Sidebar
        let has_sidebar_image = spec.sidebar.image.is_some();
        let sidebar = card([
            self.image_row(
                ImageRole::Sidebar,
                spec.sidebar.image.is_some(),
                look.sidebar.clone(),
                cx,
            ),
            SettingRow::new(t(M::SkinFit))
                .child(segmented(
                    "skin-fit",
                    vec![
                        (Fit::Cover, t(M::SkinFitCover).into()),
                        (Fit::Tile, t(M::SkinFitTile).into()),
                    ],
                    Some(&spec.sidebar.fit),
                    !has_sidebar_image,
                    {
                        let entity = entity.clone();
                        move |fit: Fit, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.edit(Control::Fit, cx, move |spec| spec.sidebar.fit = fit)
                            })
                        }
                    },
                ))
                .into_any_element(),
            self.color_row(
                t(M::SkinTint),
                Hex::Tint,
                spec.sidebar.tint,
                &DARK,
                t(M::SkinNone),
                cx,
            ),
            SettingRow::new(t(M::SkinTintStrength))
                .child(
                    Slider::new(
                        "skin-tint-strength",
                        spec.sidebar.tint_opacity as f64,
                        0.,
                        100.,
                        5.,
                    )
                    .disabled(spec.sidebar.tint.is_none())
                    .format(|v| format!("{v:.0}%").into())
                    .on_change(on(|this, cx, value| {
                        this.edit(Control::TintStrength, cx, move |spec| {
                            spec.sidebar.tint_opacity = value.round() as u8
                        })
                    })),
                )
                .into_any_element(),
            SettingRow::new(t(M::SkinBlur))
                .child(
                    Slider::new(
                        "skin-blur",
                        spec.sidebar.blur as f64,
                        0.,
                        MAX_BLUR as f64,
                        1.,
                    )
                    .disabled(!has_sidebar_image)
                    .format(|v| format!("{v:.0} px").into())
                    .on_change(on(|this, cx, value| {
                        this.edit(Control::Blur, cx, move |spec| {
                            spec.sidebar.blur = value.round() as u8
                        })
                    })),
                )
                .into_any_element(),
        ]);

        let banner = card([SettingRow::new(t(M::SkinBannerInSidebar))
            .child(
                Toggle::new("skin-banner", spec.banner.in_sidebar).on_change(cx.listener(
                    |this, on: &bool, _, cx| {
                        let on = *on;
                        this.edit(Control::Banner, cx, move |spec| spec.banner.in_sidebar = on);
                        if !on {
                            this.instances
                                .update(cx, |instances, _| instances.forget_sidebar_banners());
                        }
                    },
                )),
            )
            .into_any_element()]);

        let hero = card([self.image_row(
            ImageRole::Hero,
            spec.hero.image.is_some(),
            look.hero.as_ref().map(|h| h.image.clone()),
            cx,
        )]);

        let play = card([
            self.color_row(
                t(M::SkinColor),
                Hex::Play,
                spec.play.color,
                &BRIGHT,
                t(M::SkinDefault),
                cx,
            ),
            self.color_row(
                t(M::SkinFadeTo),
                Hex::Gradient,
                spec.play.gradient,
                &BRIGHT,
                t(M::SkinNone),
                cx,
            ),
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
                    {
                        let entity = entity.clone();
                        move |shape: Shape, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.edit(Control::Shape, cx, move |spec| spec.play.shape = shape)
                            })
                        }
                    },
                ))
                .into_any_element(),
            SettingRow::new(t(M::SkinLabel))
                .child(div().w(px(160.)).child(self.label.clone()))
                .into_any_element(),
            SettingRow::new(t(M::SkinGlow))
                .child(
                    Toggle::new("skin-glow", spec.play.glow).on_change(cx.listener(
                        |this, on: &bool, _, cx| {
                            let on = *on;
                            this.edit(Control::Glow, cx, move |spec| spec.play.glow = on)
                        },
                    )),
                )
                .into_any_element(),
        ]);

        let colours = card([
            self.color_row(
                t(M::SkinAccent),
                Hex::Accent,
                spec.accent,
                &BRIGHT,
                t(M::SkinDefault),
                cx,
            ),
            self.color_row(
                t(M::SkinProgress),
                Hex::Progress,
                spec.progress,
                &BRIGHT,
                t(M::SkinDefault),
                cx,
            ),
        ]);

        // each surface's text contrast, as worked out from its image
        let ratio = |value: f32| crate::i18n::format::decimal(f64::from(value), 1);
        let mut checks: Vec<(String, bool)> = Vec::new();
        if let Some(palette) = crate::skin::sidebar_palette(&look) {
            checks.push((
                tr!(M::SkinContrastSidebar, ratio = ratio(palette.contrast)),
                palette.passes(),
            ));
        }
        if let Some(contrast) = crate::skin::hero_contrast(&look) {
            checks.push((
                tr!(M::SkinContrastHero, ratio = ratio(contrast)),
                contrast + 0.005 >= launcher_core::ui_skin::readable::BODY,
            ));
        }
        let short = checks.iter().any(|(_, ok)| !ok);
        let accent_adjusted = spec.accent.is_some_and(crate::theme::accent::adjusted);
        let checks_line: Vec<AnyElement> = checks
            .into_iter()
            .map(|(label, ok)| {
                let color = if ok {
                    colors::GREEN_300
                } else {
                    colors::YELLOW
                };
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .text_color(color)
                    .child(
                        if ok {
                            Icon::Check
                        } else {
                            Icon::TriangleExclamation
                        }
                        .render(px(14.), color),
                    )
                    .child(label)
                    .into_any_element()
            })
            .collect();
        let readable = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_x_4()
            .gap_y_2()
            .when(checks_line.is_empty(), |this| {
                this.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .text_color(colors::GREEN_300)
                        .child(Icon::Check.render(px(14.), colors::GREEN_300))
                        .child(t(M::SkinReadable)),
                )
            })
            .children(checks_line)
            .when(accent_adjusted, |this| {
                this.child(div().text_color(colors::YELLOW).child(t(M::SkinAdjusted)))
            })
            .when(short && !builtin, |this| {
                this.child(
                    Button::new("skin-fix-readability", t(M::SkinFixReadability))
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.fix_readability(cx))),
                )
            });

        div()
            .flex()
            .flex_col()
            .gap_6()
            .w_full()
            .when_some(notice, |this, (notice, color)| {
                this.child(div().text_color(color).child(notice))
            })
            .child(
                section(t(M::SkinGallery), None).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().flex().flex_row().flex_wrap().gap_2().children(cards))
                        .child(actions),
                ),
            )
            .child(section(t(M::SkinSidebar), None).child(sidebar))
            .child(section(t(M::SkinBanner), None).child(banner))
            .child(section(t(M::SkinHero), None).child(hero))
            .child(section(t(M::SkinPlayButton), None).child(play))
            .child(section(t(M::SkinColors), None).child(colours))
            .child(readable)
            .child(
                div().flex().child(
                    Button::new("skin-reset", t(M::SkinReset))
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .disabled(look.id == "default")
                        .on_click(cx.listener(|this, _, _, cx| this.use_skin(None, cx))),
                ),
            )
    }
}

impl SkinSettings {
    /// An image row: its thumbnail, Choose… and Remove.
    fn image_row(
        &self,
        role: ImageRole,
        has: bool,
        image: Option<std::sync::Arc<gpui::RenderImage>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (id, remove_id) = match role {
            ImageRole::Sidebar => ("skin-sidebar-image", "skin-sidebar-remove"),
            ImageRole::Hero => ("skin-hero-image", "skin-hero-remove"),
        };
        SettingRow::new(t(M::SkinImage))
            .when(role == ImageRole::Hero, |row| row.hint(t(M::SkinHeroHint)))
            .when_some(image, |row, image| {
                row.child(
                    img(image)
                        .w(px(72.))
                        .h(px(40.))
                        .rounded(radius::MD)
                        .object_fit(gpui::ObjectFit::Cover),
                )
            })
            .when(!has, |row| row.child(tag(t(M::SkinNone), colors::GRAY_500)))
            .child(
                Button::new(id, t(M::SkinChoose))
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Slim)
                    .on_click(cx.listener(move |this, _, _, cx| this.choose_image(role, cx))),
            )
            .when(has, |row| {
                row.child(
                    muted_link(remove_id, t(M::SkinRemove)).on_click(cx.listener(
                        move |this, _, _, cx| {
                            let control = match role {
                                ImageRole::Sidebar => Control::SidebarImage,
                                ImageRole::Hero => Control::Hero,
                            };
                            this.edit(control, cx, move |spec| match role {
                                ImageRole::Sidebar => spec.sidebar.image = None,
                                ImageRole::Hero => spec.hero.image = None,
                            })
                        },
                    )),
                )
            })
            .into_any_element()
    }
}
