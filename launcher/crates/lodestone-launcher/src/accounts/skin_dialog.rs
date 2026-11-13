//! "Change skin": the active Microsoft account's skin, arm width and cape, changed
//! through the Minecraft services (`launcher_core::auth::wardrobe`).
//!
//! The dialog shows the player as the skin would look, as a model that turns when it
//! is dragged (`launcher_core::skin_model`), wearing the cape picked. Capes are
//! picked by their pictures. Nothing is
//! sent until Save: a new skin file is uploaded, "Use the default skin" removes the
//! skin, a change of arm width alone uploads the current skin again for the other
//! model, and the cape is put on or taken off.

use std::{collections::HashMap, sync::Arc};

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, RenderImage, SharedString, Task, Window,
    div, img, prelude::*,
};
use launcher_core::{
    Secret,
    accounts::Account,
    auth::wardrobe::{self, SkinFileError, SkinModel, Wardrobe, WardrobeError},
    skin,
    skin_model::{self, PlayerModel},
};

use super::Accounts;
use crate::{
    components::{
        Button, ButtonVariant, Dialog, KeyboardFocus, LoadingDots,
        a11y::{Accessible, Role, Toggled},
        segmented,
    },
    file_dialog::{self, Pick, ShowsError},
    i18n::{CommonMsg, ShellMsg as M, t, tr},
    theme::{self, colors, leading, px, radius, text, weights},
};

/// How many screen pixels a skin pixel is in the preview.
const PREVIEW_UNIT: f32 = 7.0;

/// How a cape's picture is enlarged in the list of capes.
const CAPE_PICTURE_SCALE: u32 = 6;

/// How the model stands at first: turned a little, seen from slightly above.
const START_YAW: f32 = 0.5;
const START_PITCH: f32 = 0.14;
/// Turned to show the back, and the cape on it.
const CAPE_YAW: f32 = std::f32::consts::PI - 0.55;
/// How far a drag of one pixel turns the model, in radians, and how far it tips.
const TURN_PER_PIXEL: f32 = 0.02;
const MAX_PITCH: f32 = 0.6;

pub struct SkinDialogDismissed;

/// What Save does to the skin.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Choice {
    /// The skin stays (its arm width may still change).
    Keep,
    /// A file the user chose, checked.
    File { name: String, png: Arc<Vec<u8>> },
    /// Back to the default skin.
    Default,
}

enum State {
    Loading,
    Failed(SharedString),
    Ready {
        /// What the account wears now.
        current: Wardrobe,
        /// The current skin's image, for the preview and for changing its arm width.
        current_png: Option<Arc<Vec<u8>>>,
    },
}

pub struct SkinDialog {
    accounts: Entity<Accounts>,
    account: Account,
    focus_handle: FocusHandle,
    state: State,
    choice: Choice,
    model: SkinModel,
    /// The cape picked, by id. `None` is no cape.
    cape: Option<String>,
    /// The capes' textures, by id, for the model and their pictures.
    cape_pngs: HashMap<String, Arc<Vec<u8>>>,
    cape_pictures: HashMap<String, Arc<RenderImage>>,
    /// The player as a model, for the skin, arms and cape picked.
    player: Option<PlayerModel>,
    /// How the model is turned, in radians, and where a drag last was.
    yaw: f32,
    pitch: f32,
    drag: Option<Point<Pixels>>,
    preview: Option<Arc<RenderImage>>,
    saving: bool,
    error: Option<SharedString>,
    _tasks: Vec<Task<()>>,
}

impl EventEmitter<SkinDialogDismissed> for SkinDialog {}

impl Focusable for SkinDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ShowsError for SkinDialog {
    fn show_error(&mut self, message: SharedString, cx: &mut Context<Self>) {
        self.error = Some(message);
        cx.notify();
    }
}

/// Why a file can't be a skin, in the user's language.
fn file_error(error: &SkinFileError) -> String {
    match error {
        SkinFileError::TooLarge => t(M::PlayerSkinFileTooLarge).to_string(),
        SkinFileError::NotPng => t(M::PlayerSkinFileNotPng).to_string(),
        SkinFileError::WrongSize { width, height } => {
            tr!(M::PlayerSkinFileWrongSize, width = width, height = height)
        }
    }
}

/// Why the Minecraft services didn't do it, in the user's language.
fn wardrobe_error(error: &WardrobeError) -> String {
    match error {
        WardrobeError::Network(reason) => tr!(M::PlayerSkinNetworkError, error = reason),
        WardrobeError::RateLimited => t(M::PlayerSkinRateLimited).to_string(),
        WardrobeError::SignedOut => t(M::PlayerSkinSignedOut).to_string(),
        WardrobeError::Refused {
            message: Some(message),
            ..
        } => tr!(M::PlayerSkinRefusedBecause, reason = message),
        WardrobeError::Refused { status, .. } => tr!(M::PlayerSkinRefused, status = status),
    }
}

/// What Save has to send for the skin, given what the account wears.
#[derive(Clone, Debug, PartialEq, Eq)]
enum SkinChange {
    Nothing,
    Upload(Arc<Vec<u8>>),
    Reset,
}

fn skin_change(
    choice: &Choice,
    model: SkinModel,
    current: &Wardrobe,
    current_png: Option<&Arc<Vec<u8>>>,
) -> SkinChange {
    match choice {
        Choice::File { png, .. } => SkinChange::Upload(png.clone()),
        Choice::Default if current.skin_url.is_some() => SkinChange::Reset,
        Choice::Default => SkinChange::Nothing,
        // the same skin for the other arm width is uploaded again
        Choice::Keep => match current_png {
            Some(png) if current.skin_url.is_some() && model != current.model => {
                SkinChange::Upload(png.clone())
            }
            _ => SkinChange::Nothing,
        },
    }
}

impl SkinDialog {
    pub fn new(accounts: Entity<Accounts>, account: Account, cx: &mut Context<Self>) -> Self {
        let mut this = Self::empty(accounts, account, cx);
        this.load(cx);
        this
    }

    fn empty(accounts: Entity<Accounts>, account: Account, cx: &mut Context<Self>) -> Self {
        Self {
            accounts,
            account,
            focus_handle: cx.focus_handle(),
            state: State::Loading,
            choice: Choice::Keep,
            model: SkinModel::Classic,
            cape: None,
            cape_pngs: HashMap::new(),
            cape_pictures: HashMap::new(),
            player: None,
            yaw: START_YAW,
            pitch: START_PITCH,
            drag: None,
            preview: None,
            saving: false,
            error: None,
            _tasks: Vec::new(),
        }
    }

    /// The dialog as it is once loaded, without asking the services, for the
    /// offscreen screenshots.
    pub fn preview(
        accounts: Entity<Accounts>,
        account: Account,
        current: Wardrobe,
        skin_png: Vec<u8>,
        capes: Vec<(String, Vec<u8>)>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::empty(accounts, account, cx);
        let capes = capes
            .into_iter()
            .map(|(id, png)| (id, Arc::new(png)))
            .collect();
        this.loaded(current, Some(Arc::new(skin_png)), capes, cx);
        // turned to show the cape
        this.yaw = CAPE_YAW;
        this.redraw(cx);
        this
    }

    /// A Minecraft token for the account, signing in again with the saved sign-in if
    /// the last one has run out.
    fn token(&self, cx: &mut Context<Self>) -> Task<Result<Secret, String>> {
        let launch = self
            .accounts
            .update(cx, |accounts, cx| accounts.launch_account(cx));
        cx.background_spawn(async move { launch.await.map(|account| account.access_token) })
    }

    /// Asks the services what the account wears, and fetches the skin's image.
    fn load(&mut self, cx: &mut Context<Self>) {
        self.state = State::Loading;
        self.error = None;
        let token = self.token(cx);
        let (http, endpoints) = {
            let accounts = self.accounts.read(cx);
            (
                accounts.http(),
                accounts.authenticator().config().endpoints.clone(),
            )
        };
        let task = cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_spawn(async move {
                    let token = token.await?;
                    let current = wardrobe::wardrobe(&*http, &endpoints, &token)
                        .await
                        .map_err(|error| wardrobe_error(&error))?;
                    // without the image the dialog still works: only the preview is missing
                    let png = match &current.skin_url {
                        Some(url) => skin::skin_png(&*http, &endpoints.textures, url)
                            .await
                            .ok()
                            .map(Arc::new),
                        None => None,
                    };
                    // and the capes', for their pictures: one that can't be fetched
                    // is offered by its name
                    let mut capes = HashMap::new();
                    for cape in &current.capes {
                        if let Ok(png) =
                            skin::skin_png(&*http, &endpoints.textures, &cape.url).await
                        {
                            capes.insert(cape.id.clone(), Arc::new(png));
                        }
                    }
                    Ok::<_, String>((current, png, capes))
                })
                .await;
            this.update(cx, |this, cx| {
                match loaded {
                    Ok((current, png, capes)) => this.loaded(current, png, capes, cx),
                    Err(error) => this.state = State::Failed(error.into()),
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        cx.notify();
    }

    fn loaded(
        &mut self,
        current: Wardrobe,
        current_png: Option<Arc<Vec<u8>>>,
        capes: HashMap<String, Arc<Vec<u8>>>,
        cx: &mut Context<Self>,
    ) {
        crate::images::release(self.cape_pictures.drain().map(|(_, picture)| picture), cx);
        self.cape_pictures = capes
            .iter()
            .filter_map(|(id, png)| {
                let picture = skin_model::cape_picture(png, CAPE_PICTURE_SCALE).ok()?;
                Some((id.clone(), crate::images::render_image(picture)))
            })
            .collect();
        self.cape_pngs = capes;
        self.model = current.model;
        self.cape = current.active_cape().map(|cape| cape.id.clone());
        self.choice = Choice::Keep;
        self.state = State::Ready {
            current,
            current_png,
        };
        self.rebuild(cx);
    }

    /// The image the preview shows: the chosen file, else the current skin.
    fn shown_png(&self) -> Option<Arc<Vec<u8>>> {
        match (&self.choice, &self.state) {
            (Choice::File { png, .. }, _) => Some(png.clone()),
            (Choice::Keep, State::Ready { current_png, .. }) => current_png.clone(),
            _ => None,
        }
    }

    /// Makes the model again, for the skin, arms and cape picked, and draws it.
    fn rebuild(&mut self, cx: &mut Context<Self>) {
        let cape = self.cape.as_ref().and_then(|id| self.cape_pngs.get(id));
        self.player = self.shown_png().and_then(|png| {
            PlayerModel::new(
                &png,
                self.model == SkinModel::Slim,
                cape.map(|png| png.as_slice()),
            )
            .ok()
        });
        self.redraw(cx);
    }

    /// Draws the model as it is turned now, at twice the size shown, so its edges
    /// stay sharp on a dense screen.
    fn redraw(&mut self, cx: &mut Context<Self>) {
        let (width, height) = (
            (skin_model::VIEW_WIDTH * PREVIEW_UNIT * 2.0) as u32,
            (skin_model::VIEW_HEIGHT * PREVIEW_UNIT * 2.0) as u32,
        );
        let drawn = self.player.as_ref().map(|player| {
            crate::images::render_image(player.render(self.yaw, self.pitch, width, height))
        });
        crate::images::release(std::mem::replace(&mut self.preview, drawn), cx);
    }

    /// Turns the model by a drag of `delta`.
    fn turn(&mut self, delta: Point<Pixels>, cx: &mut Context<Self>) {
        self.yaw = (self.yaw + f32::from(delta.x) * TURN_PER_PIXEL) % std::f32::consts::TAU;
        self.pitch =
            (self.pitch + f32::from(delta.y) * TURN_PER_PIXEL * 0.5).clamp(-MAX_PITCH, MAX_PITCH);
        self.redraw(cx);
        cx.notify();
    }

    fn set_model(&mut self, model: SkinModel, cx: &mut Context<Self>) {
        if !self.saving {
            self.model = model;
            self.rebuild(cx);
            cx.notify();
        }
    }

    fn set_cape(&mut self, cape: Option<String>, cx: &mut Context<Self>) {
        if !self.saving {
            // a cape is on the back: turn the player to show the one picked
            if cape.is_some() && cape != self.cape {
                self.yaw = CAPE_YAW;
            }
            self.cape = cape;
            self.rebuild(cx);
            cx.notify();
        }
    }

    fn use_default(&mut self, cx: &mut Context<Self>) {
        self.choice = Choice::Default;
        self.error = None;
        self.rebuild(cx);
        cx.notify();
    }

    /// Takes a file as the new skin, if it can be one.
    fn choose(&mut self, name: String, png: Vec<u8>, cx: &mut Context<Self>) {
        match wardrobe::check_skin_file(&png) {
            Ok(()) => {
                self.choice = Choice::File {
                    name,
                    png: Arc::new(png),
                };
                self.error = None;
                self.rebuild(cx);
            }
            Err(error) => {
                self.error = Some(
                    tr!(
                        M::PlayerSkinFileRefused,
                        name = name,
                        error = file_error(&error)
                    )
                    .into(),
                );
            }
        }
        cx.notify();
    }

    fn browse(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(Pick::Files.options(false, t(M::PlayerSkinChooseFile)));
        let task = cx.spawn(async move |this, cx| {
            let Some(paths) = file_dialog::chosen(picked, &this, cx).await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            let read = cx
                .background_spawn(async move {
                    // a skin is a few kilobytes: a huge file is refused unread
                    let size = std::fs::metadata(&path)?.len();
                    if size > wardrobe::MAX_SKIN_BYTES as u64 {
                        return Ok(None);
                    }
                    std::fs::read(&path).map(Some)
                })
                .await;
            this.update(cx, |this, cx| match read {
                Ok(Some(png)) => this.choose(name, png, cx),
                Ok(None) => {
                    this.error = Some(
                        tr!(
                            M::PlayerSkinFileRefused,
                            name = name,
                            error = file_error(&SkinFileError::TooLarge)
                        )
                        .into(),
                    );
                    cx.notify();
                }
                Err(error) => {
                    this.error =
                        Some(tr!(M::PlayerSkinFileRefused, name = name, error = error).into());
                    cx.notify();
                }
            })
            .ok();
        });
        self._tasks.push(task);
    }

    /// Whether Save would change anything.
    fn changed(&self) -> bool {
        let State::Ready {
            current,
            current_png,
        } = &self.state
        else {
            return false;
        };
        skin_change(&self.choice, self.model, current, current_png.as_ref()) != SkinChange::Nothing
            || self.cape != current.active_cape().map(|cape| cape.id.clone())
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let State::Ready {
            current,
            current_png,
        } = &self.state
        else {
            return;
        };
        if self.saving || !self.changed() {
            return;
        }
        let change = skin_change(&self.choice, self.model, current, current_png.as_ref());
        let cape = (self.cape != current.active_cape().map(|cape| cape.id.clone()))
            .then(|| self.cape.clone());
        let model = self.model;
        let token = self.token(cx);
        let (http, endpoints) = {
            let accounts = self.accounts.read(cx);
            (
                accounts.http(),
                accounts.authenticator().config().endpoints.clone(),
            )
        };
        self.saving = true;
        self.error = None;
        let id = self.account.id.clone();
        let task = cx.spawn(async move |this, cx| {
            let saved = cx
                .background_spawn(async move {
                    let token = token.await?;
                    let mut after = None;
                    match change {
                        SkinChange::Upload(png) => {
                            after = Some(
                                wardrobe::upload_skin(&*http, &endpoints, &token, &png, model)
                                    .await,
                            );
                        }
                        SkinChange::Reset => {
                            after = Some(wardrobe::reset_skin(&*http, &endpoints, &token).await);
                        }
                        SkinChange::Nothing => {}
                    }
                    // the cape only once the skin went through
                    if let Some(cape) = cape
                        && !matches!(after, Some(Err(_)))
                    {
                        after = Some(
                            wardrobe::set_cape(&*http, &endpoints, &token, cape.as_deref()).await,
                        );
                    }
                    match after {
                        Some(result) => result.map(Some).map_err(|error| wardrobe_error(&error)),
                        None => Ok(None),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.saving = false;
                match saved {
                    Ok(after) => {
                        if let Some(after) = after {
                            this.accounts.update(cx, |accounts, cx| {
                                accounts.skin_changed(&id, after.skin_url, cx)
                            });
                        }
                        cx.emit(SkinDialogDismissed);
                    }
                    Err(error) => {
                        this.error = Some(error.into());
                        // part of it may have gone through: show what is worn now
                        this.load(cx);
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        cx.notify();
    }
}

fn label(text_: &'static str) -> gpui::Div {
    div()
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::BOLD.into())
        .text_color(crate::theme::ink::muted())
        .child(text_)
}

impl SkinDialog {
    /// The player, turned by dragging.
    fn render_preview_box(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let frame = div()
            .id("skin-preview")
            .accessible(Role::Image, "skin-preview", t(M::PlayerSkinPreview))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(skin_model::VIEW_WIDTH * PREVIEW_UNIT))
            .h(px(skin_model::VIEW_HEIGHT * PREVIEW_UNIT))
            .rounded(radius::LG)
            .border_1()
            .border_color(crate::theme::ink::border())
            .bg(colors::GRAY_900)
            .overflow_hidden();
        let picture = match &self.preview {
            Some(image) => frame
                .cursor_grab()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, _, _| {
                        this.drag = Some(event.position);
                    }),
                )
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                    if event.pressed_button != Some(MouseButton::Left) {
                        this.drag = None;
                        return;
                    }
                    if let Some(last) = this.drag.replace(event.position) {
                        this.turn(event.position - last, cx);
                    }
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, _| this.drag = None),
                )
                .child(img(image.clone()).size_full()),
            None => frame.child(
                div()
                    .px_3()
                    .text_center()
                    .text_size(theme::text_size(text::SMALL))
                    .font_weight(weights::BOLD.into())
                    .text_color(crate::theme::ink::faint())
                    .child(t(M::PlayerSkinDefault)),
            ),
        };
        div()
            .flex()
            .flex_none()
            .flex_col()
            .items_center()
            .gap_1p5()
            .child(picture)
            .when(self.preview.is_some(), |this| {
                this.child(
                    div()
                        .text_size(theme::text_size(text::CAPTION))
                        .font_weight(weights::BOLD.into())
                        .text_color(crate::theme::ink::faint())
                        .child(t(M::PlayerSkinDragHint)),
                )
            })
    }

    /// One choice in the list of capes: its picture (its name when there is none)
    /// over its name.
    fn cape_tile(
        &self,
        index: usize,
        id: Option<String>,
        name: SharedString,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.cape == id;
        let picture = id.as_ref().and_then(|id| self.cape_pictures.get(id));
        let pick = id.clone();
        div()
            .id(("skin-cape", index))
            .accessible(
                Role::RadioButton,
                format!("skin-cape-{index}"),
                name.clone(),
            )
            .aria_toggled(if selected {
                Toggled::True
            } else {
                Toggled::False
            })
            .keyboard_focus()
            .flex()
            .flex_none()
            .flex_col()
            .items_center()
            .gap_1()
            .p_1p5()
            .rounded(radius::LG)
            .border_1()
            .map(|this| {
                if selected {
                    this.border_color(crate::theme::accent::base())
                        .bg(colors::GRAY_700)
                } else {
                    this.border_color(crate::theme::ink::border())
                        .hover(|style| style.bg(colors::GRAY_800))
                }
            })
            .when(!self.saving, |this| {
                this.cursor_pointer()
                    .on_press(cx.listener(move |this, _, _, cx| this.set_cape(pick.clone(), cx)))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(10. * CAPE_PICTURE_SCALE as f32 / 2.0))
                    .h(px(16. * CAPE_PICTURE_SCALE as f32 / 2.0))
                    .rounded(radius::DEFAULT)
                    .overflow_hidden()
                    .map(|this| match picture {
                        Some(picture) => this.child(img(picture.clone()).size_full()),
                        // no cape, or a cape whose picture couldn't be fetched
                        None => this.bg(colors::GRAY_900).child(
                            div()
                                .w(px(14.))
                                .h(px(1.5))
                                .bg(crate::theme::ink::faint())
                                .when(id.is_some(), |this| this.h(px(14.)).opacity(0.3)),
                        ),
                    }),
            )
            .child(
                div()
                    .max_w(px(64.))
                    .truncate()
                    .text_size(theme::text_size(text::CAPTION))
                    .font_weight(weights::BOLD.into())
                    .text_color(if selected {
                        crate::theme::ink::text()
                    } else {
                        crate::theme::ink::muted()
                    })
                    .child(name),
            )
    }

    fn render_ready(&self, current: &Wardrobe, cx: &mut Context<Self>) -> gpui::Div {
        let weak = cx.entity().downgrade();
        let file_note: SharedString = match &self.choice {
            Choice::File { name, .. } => tr!(M::PlayerSkinFileChosen, name = name).into(),
            Choice::Default => t(M::PlayerSkinDefaultChosen).into(),
            Choice::Keep => t(M::PlayerSkinFileHint).into(),
        };
        let models = segmented(
            "skin-model",
            SkinModel::ALL
                .into_iter()
                .map(|model| {
                    let name = match model {
                        SkinModel::Classic => M::PlayerSkinModelClassic,
                        SkinModel::Slim => M::PlayerSkinModelSlim,
                    };
                    (model, SharedString::from(t(name)))
                })
                .collect(),
            Some(&self.model),
            self.saving,
            move |model, _, cx| {
                weak.update(cx, |this, cx| this.set_model(model, cx)).ok();
            },
        );
        let capes = (!current.capes.is_empty()).then(|| {
            let mut tiles = vec![
                self.cape_tile(0, None, t(M::PlayerSkinNoCape).into(), cx)
                    .into_any_element(),
            ];
            for (index, cape) in current.capes.iter().enumerate() {
                tiles.push(
                    self.cape_tile(
                        index + 1,
                        Some(cape.id.clone()),
                        cape.name.clone().into(),
                        cx,
                    )
                    .into_any_element(),
                );
            }
            div()
                .id("skin-capes")
                .role(Role::RadioGroup)
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .children(tiles)
        });
        let has_skin = current.skin_url.is_some() || matches!(self.choice, Choice::File { .. });
        div()
            .flex()
            .flex_row()
            .gap_5()
            .child(self.render_preview_box(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(label(t(M::PlayerSkinSection)))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        Button::new("skin-choose", t(M::PlayerSkinChooseFile))
                                            .variant(ButtonVariant::Secondary)
                                            .disabled(self.saving)
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.browse(cx)),
                                            ),
                                    )
                                    .child(
                                        Button::new("skin-default", t(M::PlayerSkinUseDefault))
                                            .variant(ButtonVariant::Ghost)
                                            .disabled(
                                                self.saving
                                                    || !has_skin
                                                    || self.choice == Choice::Default,
                                            )
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.use_default(cx)),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(theme::text_size(text::SMALL))
                                    .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                                    .text_color(crate::theme::ink::muted())
                                    .child(file_note),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(label(t(M::PlayerSkinModel)))
                            .child(models),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(label(t(M::PlayerSkinCape)))
                            .map(|this| match capes {
                                Some(capes) => this.child(capes),
                                None => this.child(
                                    div()
                                        .text_size(theme::text_size(text::SMALL))
                                        .text_color(crate::theme::ink::faint())
                                        .child(t(M::PlayerSkinNoCapes)),
                                ),
                            }),
                    ),
            )
    }
}

impl Render for SkinDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let body = match &self.state {
            State::Loading => div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .h(px(224.))
                .text_size(theme::text_size(text::MEDIUM))
                .text_color(crate::theme::ink::muted())
                .child(LoadingDots)
                .child(t(M::PlayerSkinLoading)),
            State::Failed(error) => div()
                .flex()
                .flex_col()
                .items_start()
                .gap_3()
                .child(
                    div()
                        .text_size(theme::text_size(text::MEDIUM))
                        .text_color(colors::RED_200)
                        .child(tr!(M::PlayerSkinLoadFailed, error = error)),
                )
                .child(
                    Button::new("skin-retry", t(CommonMsg::TryAgain))
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| this.load(cx))),
                ),
            State::Ready { current, .. } => {
                let current = current.clone();
                self.render_ready(&current, cx)
            }
        };
        let ready = matches!(self.state, State::Ready { .. });
        Dialog::new(
            "change-skin",
            tr!(M::PlayerSkinDialogTitle, name = self.account.username),
            self.focus_handle.clone(),
        )
        .width(px(600.))
        .on_close(move |_, cx| entity.update(cx, |_, cx| cx.emit(SkinDialogDismissed)))
        .child(div().flex().flex_col().gap_3().child(body).when_some(
            self.error.clone(),
            |this, error| {
                this.child(
                    div()
                        .id("skin-error")
                        .accessible(Role::Status, "skin-error", error.clone())
                        .text_size(theme::text_size(text::SMALL))
                        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                        .font_weight(weights::BOLD.into())
                        .text_color(colors::RED_200)
                        .child(error),
                )
            },
        ))
        .footer(
            Button::new("skin-save", t(CommonMsg::Save))
                .variant(ButtonVariant::Primary)
                .loading(self.saving)
                .disabled(!ready || self.saving || !self.changed())
                .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
        )
        .footer(
            Button::new("skin-cancel", t(CommonMsg::Cancel))
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|_, _, _, cx| cx.emit(SkinDialogDismissed))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wardrobe(skin: Option<&str>, model: SkinModel) -> Wardrobe {
        Wardrobe {
            skin_url: skin.map(str::to_string),
            model,
            capes: Vec::new(),
        }
    }

    #[test]
    fn save_sends_only_what_changed() {
        let png = Arc::new(vec![1, 2, 3]);
        let other = Arc::new(vec![9]);
        let worn = wardrobe(
            Some("http://textures.minecraft.net/texture/a"),
            SkinModel::Classic,
        );
        let plain = wardrobe(None, SkinModel::Classic);
        let file = Choice::File {
            name: "new.png".into(),
            png: other.clone(),
        };

        // nothing touched
        assert_eq!(
            skin_change(&Choice::Keep, SkinModel::Classic, &worn, Some(&png)),
            SkinChange::Nothing
        );
        // the arm width alone: the same skin goes up again
        assert_eq!(
            skin_change(&Choice::Keep, SkinModel::Slim, &worn, Some(&png)),
            SkinChange::Upload(png.clone())
        );
        // which needs its image, and a skin to begin with
        assert_eq!(
            skin_change(&Choice::Keep, SkinModel::Slim, &worn, None),
            SkinChange::Nothing
        );
        assert_eq!(
            skin_change(&Choice::Keep, SkinModel::Slim, &plain, None),
            SkinChange::Nothing
        );
        // a chosen file
        assert_eq!(
            skin_change(&file, SkinModel::Classic, &worn, Some(&png)),
            SkinChange::Upload(other)
        );
        // the default skin, when it isn't the default already
        assert_eq!(
            skin_change(&Choice::Default, SkinModel::Classic, &worn, Some(&png)),
            SkinChange::Reset
        );
        assert_eq!(
            skin_change(&Choice::Default, SkinModel::Classic, &plain, None),
            SkinChange::Nothing
        );
    }
}
