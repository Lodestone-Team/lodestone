//! Settings > Launcher > Skin: the gallery of skins (built-in and imported) with
//! Import, Export, New skin… and Delete; what the skin in use paints, with Pop out
//! editor, which opens the skin editor in its own window ([`super::skin_editor`]);
//! the readability suggestions; and Reset to Default.
//!
//! Built-in skins can't be changed: the first change to one makes a copy and
//! changes that.

use gpui::{
    AnyElement, App, Context, Entity, IntoElement, PathPromptOptions, Render, SharedString, Task,
    WeakEntity, Window, div, prelude::*,
};
use launcher_core::ui_skin::{Rgb, Shape, SkinEntry, SkinStore, readable};

use super::LauncherShell;
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Icon, KeyboardFocus,
        a11y::{Accessible, Role, Toggled, automation_id},
        settings::{Toast, muted_link, section},
    },
    i18n::{ShellMsg as M, t, tr},
    instances::Instances,
    theme::{self, Color, colors, px, radius, text, weights},
};

/// Colours offered for the accent, the Play button and the progress bar.
pub const BRIGHT: [u32; 10] = [
    0x59B2F3, 0x22D3EE, 0x6DD277, 0x8BD86A, 0xEFB440, 0xF08A4B, 0xE5546A, 0xFF2BD6, 0xA78BFA,
    0xF4F6FB,
];

pub struct SkinSettings {
    instances: Entity<Instances>,
    shell: WeakEntity<LauncherShell>,
    entries: Vec<SkinEntry>,
    confirm_delete: bool,
    toast: Toast,
    _prompt: Option<Task<()>>,
}

impl SkinSettings {
    pub fn new(
        instances: Entity<Instances>,
        shell: WeakEntity<LauncherShell>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self {
            instances,
            shell,
            entries: Vec::new(),
            confirm_delete: false,
            toast: Toast::default(),
            _prompt: None,
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

    /// Reads the skins again.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.entries = self.store(cx).list();
        self.confirm_delete = false;
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

    /// Applies a suggested fix, or all of them (`None`).
    fn apply_fix(&mut self, fix: Option<readable::Fix>, cx: &mut Context<Self>) {
        let backdrops = crate::skin::look(cx).backdrops.clone();
        let view = cx.entity_id();
        let result =
            super::skin_editor::edit_skin(&self.instances, view, 1, cx, move |spec| match fix {
                Some(fix) => readable::apply(spec, &fix),
                None => {
                    readable::fix_all(spec, &|surface| backdrops.get(&surface).copied());
                }
            });
        match result {
            Ok(Some(name)) => self.say(tr!(M::SkinCopied, name = name), false, cx),
            Ok(None) => {}
            Err(err) => self.say(err, true, cx),
        }
        self.refresh(cx);
    }

    fn pop_out(&mut self, cx: &mut Context<Self>) {
        self.shell
            .update(cx, |shell, cx| shell.open_skin_editor(cx))
            .ok();
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
}

/// A round colour to pick.
pub fn swatch(
    id: impl Into<gpui::ElementId>,
    rgb: Rgb,
    selected: bool,
) -> gpui::Stateful<gpui::Div> {
    let id = id.into();
    div()
        .id(id.clone())
        .accessible(Role::RadioButton, automation_id(&id), rgb.hex())
        .aria_toggled(if selected {
            Toggled::True
        } else {
            Toggled::False
        })
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
    let id = id.into();
    div()
        .id(id.clone())
        .accessible(Role::RadioButton, automation_id(&id), label)
        .aria_toggled(if selected {
            Toggled::True
        } else {
            Toggled::False
        })
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
                    .text_color(crate::theme::ink::text())
            } else {
                this.border_color(crate::theme::ink::border())
                    .text_color(crate::theme::ink::muted())
                    .hover(|style| style.text_color(crate::theme::ink::text()))
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
    let card_id = SharedString::from(format!("skin-{}", entry.id));
    div()
        .id(card_id.clone())
        .accessible(Role::ListItem, card_id, spec.name.clone())
        .aria_selected(selected)
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
                    .hover(|style| style.border_color(crate::theme::ink::border()))
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
                .border_color(crate::theme::ink::border())
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

        // what the skin paints, and the editor
        let painted = launcher_core::ui_skin::SurfaceId::ALL
            .into_iter()
            .filter(|id| spec.surface(*id).is_some())
            .count();
        let summary = tr!(
            M::SkinSummary,
            surfaces = painted.to_string(),
            fonts = launcher_core::ui_skin::FontPart::ALL
                .iter()
                .filter(|p| spec.fonts.get(**p).is_some())
                .count()
                .to_string(),
            banners = spec.banners.len().to_string()
        );
        let editor = section(t(M::SkinEditorTitle), Some(t(M::SkinEditorLead).into())).child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_4()
                .child(
                    Button::new("skin-pop-out", t(M::SkinPopOut))
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Slim)
                        .icon(Icon::ArrowUpRightFromSquare)
                        .on_click(cx.listener(|this, _, _, cx| this.pop_out(cx))),
                )
                .child(div().text_color(crate::theme::ink::muted()).child(summary)),
        );

        let accent_adjusted = spec.accent.is_some_and(crate::theme::accent::adjusted);
        let readable = section(t(M::SkinReadability), None)
            .child(super::skin_checks::render(
                &crate::skin::checks(&look),
                !builtin,
                "skin-fix",
                |this: &mut Self, fix, cx| this.apply_fix(fix, cx),
                cx,
            ))
            .when(accent_adjusted, |this| {
                this.child(div().text_color(colors::YELLOW).child(t(M::SkinAdjusted)))
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
            .child(editor)
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
