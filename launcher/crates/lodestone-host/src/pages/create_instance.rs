//! The "New server" dialog: every game the core offers (`GET /games`, then the setup
//! manifest's form) and the Modrinth modpack flow, after `InstanceCreateForm.tsx` and
//! `ModpackCreateForm.tsx`.
//!
//! Steps: Select game → Basic settings (name, description) → Server settings (the
//! core's manifest) → Auto settings, then "Create Server" with the Minecraft EULA box,
//! which only the user ticks. The core makes the instance in the background; the
//! sidebar shows its progress. The shell opens the dialog from the sidebar's "New
//! server" and Home, and closes it on [`CreateInstanceEvent::Close`].

mod eula;
mod fields;
mod modpack;
mod modrinth_search;
mod wizard;

pub use modpack::ModpackPreset;
pub use modrinth_search::{ModrinthPick, ModrinthSearchEvent, ModrinthSearchView};

use std::rc::Rc;

use gpui::{
    AnyElement, Context, Entity, EventEmitter, FocusHandle, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Task, Window, div, prelude::*,
};
use launcher_core::cores::{
    CoreError,
    admin::instances::{HandlerGameType, SetupManifest, SetupValue},
};
use lodestone_ui::{
    components::{
        Accessible as _, Button, ButtonVariant, Dialog, KeyboardFocus as _, LoadingDots, Toast,
        Toggle,
    },
    theme::{colors, ink, px, radius, text, weights},
};

use self::{
    fields::{Field, HasFields},
    modpack::{ModpackEvent, ModpackFlow},
};
use crate::{
    i18n::{InstancesMsg as M, ShellMsg, t, tr},
    instance_icons,
    state::HostState,
};

/// What the dialog tells the shell.
pub enum CreateInstanceEvent {
    Close,
}

/// Something loading from the core.
pub enum Load<T> {
    Loading,
    Ready(T),
    Failed(String),
}

/// The game flow's steps.
const SELECT_GAME: usize = 0;
const BASIC: usize = 1;
const SERVER: usize = 2;
const AUTO: usize = 3;

/// The dialog's size, as the dashboard's (812 wide, at most 700 tall).
const WIDTH: f32 = 812.;
const HEIGHT: f32 = 640.;

pub struct CreateInstance {
    pub state: Entity<HostState>,
    focus: FocusHandle,
    step: usize,
    games: Load<Vec<HandlerGameType>>,
    game: Option<HandlerGameType>,
    /// The chosen game's form, and the fields made from it (made again when another
    /// game is chosen).
    manifest: Option<(HandlerGameType, Load<SetupManifest>)>,
    fields_for: Option<HandlerGameType>,
    /// Name and description.
    basic: Vec<Field>,
    /// The manifest's fields, by section.
    server: Vec<(String, Vec<Field>)>,
    auto_start: bool,
    restart_on_crash: bool,
    eula: bool,
    /// Errors after Next or Create, by field.
    errors: std::collections::HashMap<usize, String>,
    submitting: bool,
    modpack: Option<Entity<ModpackFlow>>,
    _subscriptions: Vec<Subscription>,
    _tasks: Vec<Task<()>>,
}

impl EventEmitter<CreateInstanceEvent> for CreateInstance {}

/// Field keys: the basic fields first, then the server fields in order.
const SERVER_KEYS: usize = 100;

impl HasFields for CreateInstance {
    fn field_mut(&mut self, key: usize) -> Option<&mut Field> {
        if key < SERVER_KEYS {
            self.basic.get_mut(key)
        } else {
            self.server
                .iter_mut()
                .flat_map(|(_, fields)| fields.iter_mut())
                .nth(key - SERVER_KEYS)
        }
    }
}

impl CreateInstance {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            state,
            focus: cx.focus_handle(),
            step: SELECT_GAME,
            games: Load::Loading,
            game: None,
            manifest: None,
            fields_for: None,
            basic: Vec::new(),
            server: Vec::new(),
            auto_start: false,
            restart_on_crash: false,
            eula: false,
            errors: Default::default(),
            submitting: false,
            modpack: None,
            _subscriptions: Vec::new(),
            _tasks: Vec::new(),
        };
        this.load_games(cx);
        this
    }

    fn admin(&self, cx: &Context<Self>) -> launcher_core::cores::admin::AdminClient {
        self.state.read(cx).admin.clone()
    }

    fn load_games(&mut self, cx: &mut Context<Self>) {
        let admin = self.admin(cx);
        let task = cx.spawn(async move |this, cx| {
            let games = cx
                .background_spawn(async move { admin.games().await })
                .await;
            this.update(cx, |this, cx| {
                this.games = match games {
                    Ok(games) => {
                        Load::Ready(games.into_iter().filter(|g| g.route().is_some()).collect())
                    }
                    Err(err) => Load::Failed(err.to_string()),
                };
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
    }

    /// Picks a game, and fetches its form.
    pub fn choose_game(&mut self, game: HandlerGameType, cx: &mut Context<Self>) {
        self.game = Some(game.clone());
        if self
            .manifest
            .as_ref()
            .is_some_and(|(g, load)| *g == game && !matches!(load, Load::Failed(_)))
        {
            cx.notify();
            return;
        }
        self.manifest = Some((game.clone(), Load::Loading));
        let admin = self.admin(cx);
        let task = cx.spawn(async move |this, cx| {
            let wanted = game.clone();
            let manifest = cx
                .background_spawn(async move { admin.setup_manifest(&wanted).await })
                .await;
            this.update(cx, |this, cx| {
                // only the game still chosen
                if this.manifest.as_ref().is_some_and(|(g, _)| *g == game) {
                    this.manifest = Some((
                        game,
                        match manifest {
                            Ok(manifest) => Load::Ready(manifest),
                            Err(err) => Load::Failed(err.to_string()),
                        },
                    ));
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        cx.notify();
    }

    fn ready_manifest(&self) -> Option<&SetupManifest> {
        match &self.manifest {
            Some((game, Load::Ready(manifest))) if Some(game) == self.game.as_ref() => {
                Some(manifest)
            }
            _ => None,
        }
    }

    /// Makes the fields for the chosen game, the first time it's left for step 2
    /// (`setValues(initialValues)`); Back keeps what was typed.
    fn make_fields(&mut self, cx: &mut Context<Self>) {
        let Some(manifest) = self.ready_manifest().cloned() else {
            return;
        };
        if self.fields_for == self.game {
            return;
        }
        self.fields_for = self.game.clone();
        self.basic = basic_settings()
            .into_iter()
            .map(|setting| Field::new(setting, None, cx))
            .collect();
        let state = self.state.read(cx);
        let blocked =
            fields::command_line_blocked(&state.me, state.settings.as_ref()).map(str::to_string);
        self.server = manifest
            .setting_sections
            .iter()
            .map(|section| {
                (
                    section.name.clone(),
                    section
                        .settings
                        .iter()
                        .map(|setting| {
                            let locked = fields::COMMAND_LINE_SETTINGS
                                .contains(&setting.setting_id.as_str())
                                .then(|| blocked.clone())
                                .flatten();
                            Field::new(setting.clone(), locked, cx)
                        })
                        .collect(),
                )
            })
            .collect();
        self.auto_start = false;
        self.restart_on_crash = false;
    }

    /// The errors of the step's fields.
    fn check_step(&mut self, cx: &Context<Self>) -> bool {
        self.errors.clear();
        let mut check = |key: usize, field: &Field| {
            if let Err(error) = field.value(cx) {
                self.errors.insert(key, error);
            }
        };
        match self.step {
            BASIC => {
                for (i, field) in self.basic.iter().enumerate() {
                    check(i, field);
                }
            }
            SERVER => {
                for (i, field) in self
                    .server
                    .iter()
                    .flat_map(|(_, fields)| fields.iter())
                    .enumerate()
                {
                    check(SERVER_KEYS + i, field);
                }
            }
            _ => {}
        }
        self.errors.is_empty()
    }

    fn next(&mut self, cx: &mut Context<Self>) {
        if self.submitting {
            return;
        }
        match self.step {
            SELECT_GAME => {
                if self.ready_manifest().is_none() {
                    return;
                }
                self.make_fields(cx);
                self.step = BASIC;
            }
            BASIC | SERVER => {
                if self.check_step(cx) {
                    self.step += 1;
                }
            }
            _ => return self.create(cx),
        }
        cx.notify();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        self.go_to(self.step.saturating_sub(1), cx);
    }

    fn go_to(&mut self, step: usize, cx: &mut Context<Self>) {
        if step < self.step && !self.submitting {
            self.step = step;
            self.errors.clear();
            cx.notify();
        }
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let (Some(game), Some(manifest)) = (self.game.clone(), self.ready_manifest().cloned())
        else {
            return;
        };
        if !self.eula {
            return;
        }
        // every step's values, checked once more
        let mut values = std::collections::HashMap::new();
        for field in self.server.iter().flat_map(|(_, fields)| fields.iter()) {
            match field.value(cx) {
                Ok(value) => {
                    values.insert(field.setting.setting_id.clone(), value);
                }
                Err(_) => {
                    self.step = SERVER;
                    self.check_step(cx);
                    cx.notify();
                    return;
                }
            }
        }
        let text_of = |i: usize| match self.basic.get(i).map(|f| f.value(cx)) {
            Some(Ok(Some(launcher_core::cores::admin::instances::ConfigurableValue::String(
                s,
            )))) => s,
            _ => String::new(),
        };
        let value = SetupValue {
            name: text_of(0),
            description: Some(text_of(1)),
            auto_start: self.auto_start,
            restart_on_crash: self.restart_on_crash,
            setting_sections: SetupValue::sections(&manifest, |_, setting| {
                values.get(&setting.setting_id).cloned().flatten()
            }),
            // the user ticked the box themselves
            accept_eula: self.eula,
        };
        self.submitting = true;
        let admin = self.admin(cx);
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.create_instance(&game, &value).await })
                .await;
            this.update(cx, |this, cx| {
                this.submitting = false;
                match result {
                    Ok(()) => cx.emit(CreateInstanceEvent::Close),
                    Err(err) => this.failed(err, cx),
                }
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
        cx.notify();
    }

    /// A failed create: a toast, and the dialog stays open with what was typed.
    fn failed(&mut self, err: CoreError, cx: &mut Context<Self>) {
        let text = tr!(M::CreateFailed, error = err);
        self.state.update(cx, |state, cx| {
            state.toast(Toast::error(text).key("create-instance"), cx)
        });
    }

    /// Opens the modpack flow (the "Modrinth modpack" card).
    pub fn open_modpack(&mut self, cx: &mut Context<Self>) {
        self.open_modpack_with(None, cx);
    }

    /// Opens the modpack flow on a pack picked elsewhere (Mods › Browse).
    pub fn open_modpack_preset(&mut self, preset: ModpackPreset, cx: &mut Context<Self>) {
        self.open_modpack_with(Some(preset), cx);
    }

    fn open_modpack_with(&mut self, preset: Option<ModpackPreset>, cx: &mut Context<Self>) {
        let state = self.state.clone();
        let flow = cx.new(|cx| ModpackFlow::with_preset(state, preset, cx));
        self._subscriptions
            .push(cx.subscribe(&flow, |this, _, event, cx| match event {
                ModpackEvent::Back => {
                    this.modpack = None;
                    cx.notify();
                }
                ModpackEvent::Done => cx.emit(CreateInstanceEvent::Close),
            }));
        self.modpack = Some(flow);
        cx.notify();
    }

    /// For screenshots: moves the dialog on to `step` (`games`, `basic`, `server`,
    /// `auto`, `modpack`) as soon as what it needs has loaded.
    pub fn advance_for_screenshot(&mut self, step: &str, cx: &mut Context<Self>) {
        if step == "modpack" {
            return self.open_modpack(cx);
        }
        if let Some(project) = step.strip_prefix("modpack:") {
            let (project_id, version_id) = match project.split_once('/') {
                Some((p, v)) => (p.to_string(), Some(v.to_string())),
                None => (project.to_string(), None),
            };
            return self.open_modpack_preset(
                ModpackPreset {
                    project_id,
                    version_id,
                },
                cx,
            );
        }
        let target = match step {
            "basic" => BASIC,
            "server" => SERVER,
            "auto" => AUTO,
            _ => SELECT_GAME,
        };
        if target == SELECT_GAME {
            return;
        }
        let task = cx.spawn(async move |this, cx| {
            for _ in 0..200 {
                let done = this
                    .update(cx, |this, cx| {
                        if this.game.is_none()
                            && let Load::Ready(games) = &this.games
                            && let Some(first) = games.first().cloned()
                        {
                            this.choose_game(first, cx);
                        }
                        if this.step < target {
                            this.next(cx);
                        }
                        this.step >= target
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
            }
        });
        self._tasks.push(task);
    }
}

/// Name and description (`basicSettingsPage`).
fn basic_settings() -> Vec<launcher_core::cores::admin::instances::SettingManifest> {
    use launcher_core::cores::admin::instances::{
        ConfigurableValue, ConfigurableValueType, SettingManifest,
    };
    let text_setting =
        |id: &str, name: M, description: M, default: String, required: bool| SettingManifest {
            setting_id: id.into(),
            name: t(name).into(),
            description: t(description).into(),
            value: Some(ConfigurableValue::String(String::new())),
            value_type: ConfigurableValueType::String { regex: None },
            default_value: Some(ConfigurableValue::String(default)),
            is_secret: false,
            is_required: required,
            is_mutable: true,
        };
    vec![
        text_setting(
            "name",
            M::FieldName,
            M::FieldNameDescription,
            t(M::FieldNameDefault).into(),
            true,
        ),
        text_setting(
            "description",
            M::FieldDescription,
            M::FieldDescriptionDescription,
            String::new(),
            false,
        ),
    ]
}

/// A game's line on its card (`game_to_description`).
fn game_description(game: &HandlerGameType) -> &'static str {
    t(match game {
        HandlerGameType::MinecraftJavaVanilla => M::GameVanillaDescription,
        HandlerGameType::MinecraftFabric => M::GameFabricDescription,
        HandlerGameType::MinecraftForge => M::GameForgeDescription,
        HandlerGameType::MinecraftPaper => M::GamePaperDescription,
        HandlerGameType::MinecraftBedrock => M::GameBedrockDescription,
        HandlerGameType::Unknown => M::GameGenericDescription,
    })
}

/// A card to pick a game, or the modpack flow (`SelectGameCard.tsx`).
fn game_card(
    id: impl Into<SharedString>,
    game: &launcher_core::cores::admin::models::Game,
    title: SharedString,
    description: &'static str,
    badge: Option<&'static str>,
    selected: bool,
    cx: &mut gpui::App,
) -> gpui::Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let icon = instance_icons::game_icon(format!("{id}-icon"), game, None, px(24.), cx);
    div()
        .id(id.clone())
        .accessible(gpui::Role::RadioButton, id, title.clone())
        .keyboard_focus_outer()
        .flex()
        .flex_col()
        .gap(px(8.))
        .min_h(px(112.))
        .p(px(16.))
        .rounded(radius::MD)
        .border_1()
        .cursor_pointer()
        .map(|this| {
            if selected {
                this.bg(colors::GRAY_700)
                    .border_color(colors::WHITE.opacity(50))
            } else {
                this.bg(colors::GRAY_800)
                    .border_color(colors::GRAY_FADED.opacity(30))
                    .hover(|style| style.border_color(colors::WHITE.opacity(50)))
            }
        })
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(icon)
                .child(
                    div()
                        .text_size(text::H3)
                        .font_weight(weights::MEDIUMBOLD.face())
                        .text_color(ink::text())
                        .child(title),
                )
                .when_some(badge, |this, badge| {
                    this.child(
                        div()
                            .text_size(text::SMALL)
                            .font_weight(weights::BOLD.face())
                            .text_color(colors::YELLOW)
                            .child(badge),
                    )
                }),
        )
        .child(
            div()
                .text_size(text::MEDIUM)
                .italic()
                .text_color(ink::muted())
                .child(description),
        )
}

fn loading_line(label: &'static str) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .text_size(text::MEDIUM)
        .text_color(ink::muted())
        .child(LoadingDots)
        .child(label)
        .into_any_element()
}

fn error_line(text_: String) -> AnyElement {
    div()
        .text_size(text::MEDIUM)
        .text_color(colors::RED_200)
        .child(text_)
        .into_any_element()
}

impl CreateInstance {
    fn render_games(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = vec![wizard::title(t(M::GameTypeTitle), t(M::GameTypeSubtitle))];
        let modpack_game = HandlerGameType::MinecraftFabric.game();
        body.push(
            game_card(
                "game-modpack",
                &modpack_game,
                t(M::GameTypeModpack).into(),
                t(M::GameTypeModpackDescription),
                Some(t(M::GameTypeNew)),
                false,
                cx,
            )
            .on_press(cx.listener(|this, _, _, cx| this.open_modpack(cx)))
            .into_any_element(),
        );
        match &self.games {
            Load::Loading => body.push(loading_line(t(M::LoadingForm))),
            Load::Failed(error) => body.push(error_line(tr!(M::GamesFailed, error = error))),
            Load::Ready(games) => {
                let games = games.clone();
                let cards: Vec<AnyElement> = games
                    .into_iter()
                    .map(|game| {
                        let selected = self.game.as_ref() == Some(&game);
                        let title = instance_icons::game_title(&game.game());
                        let description = game_description(&game);
                        let id = format!("game-{}", game.route().unwrap_or("unknown"));
                        let pick = game.clone();
                        game_card(
                            id,
                            &game.game(),
                            title.into(),
                            description,
                            None,
                            selected,
                            cx,
                        )
                        .on_press(
                            cx.listener(move |this, _, _, cx| this.choose_game(pick.clone(), cx)),
                        )
                        .into_any_element()
                    })
                    .collect();
                body.push(
                    div()
                        .grid()
                        .grid_cols(2)
                        .gap(px(16.))
                        .children(cards)
                        .into_any_element(),
                );
            }
        }
        match &self.manifest {
            Some((_, Load::Failed(error))) => {
                body.push(error_line(tr!(M::FormFailed, error = error)))
            }
            Some((_, Load::Loading)) => body.push(loading_line(t(M::LoadingForm))),
            _ => {}
        }
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .children(body)
            .into_any_element()
    }

    fn render_fields(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (title, description) = match self.step {
            BASIC => (t(M::StepBasicSettings), t(M::BasicSettingsDescription)),
            SERVER => (t(M::StepServerSettings), t(M::ServerSettingsDescription)),
            _ => (t(M::StepAutoSettings), t(M::AutoSettingsDescription)),
        };
        let mut body = vec![wizard::title(title, description)];
        match self.step {
            BASIC => {
                let rows = (0..self.basic.len())
                    .map(|i| {
                        let error = self.errors.get(&i).cloned();
                        fields::field_row(&self.basic[i], i, "basic", error, cx)
                    })
                    .collect();
                body.push(fields::section(rows));
            }
            SERVER => {
                let mut key = SERVER_KEYS;
                let sections: Vec<(String, Vec<usize>)> = self
                    .server
                    .iter()
                    .map(|(name, fields)| {
                        let keys = (key..key + fields.len()).collect();
                        key += fields.len();
                        (name.clone(), keys)
                    })
                    .collect();
                for (name, keys) in sections {
                    if keys.is_empty() {
                        continue;
                    }
                    if !name.is_empty() {
                        body.push(
                            div()
                                .pt(px(4.))
                                .text_size(text::MEDIUM)
                                .font_weight(weights::BOLD.face())
                                .text_color(ink::secondary())
                                .child(name)
                                .into_any_element(),
                        );
                    }
                    let rows = keys
                        .into_iter()
                        .filter_map(|key| {
                            let error = self.errors.get(&key).cloned();
                            let field = self
                                .server
                                .iter()
                                .flat_map(|(_, fields)| fields.iter())
                                .nth(key - SERVER_KEYS)?;
                            Some(fields::field_row(field, key, "server", error, cx))
                        })
                        .collect();
                    body.push(fields::section(rows));
                }
            }
            _ => {
                let toggle =
                    |id: &'static str, value: bool, name: M| Toggle::new(id, value).label(t(name));
                body.push(fields::section(vec![
                    fields::row(
                        t(M::FieldAutoStart),
                        t(M::FieldAutoStartDescription),
                        false,
                        toggle("auto-start", self.auto_start, M::FieldAutoStart)
                            .on_change(cx.listener(|this, value: &bool, _, cx| {
                                this.auto_start = *value;
                                cx.notify();
                            }))
                            .into_any_element(),
                        None,
                    ),
                    fields::row(
                        t(M::FieldRestartOnCrash),
                        t(M::FieldRestartOnCrashDescription),
                        false,
                        toggle(
                            "restart-on-crash",
                            self.restart_on_crash,
                            M::FieldRestartOnCrash,
                        )
                        .on_change(cx.listener(|this, value: &bool, _, cx| {
                            this.restart_on_crash = *value;
                            cx.notify();
                        }))
                        .into_any_element(),
                        None,
                    ),
                ]));
            }
        }
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .children(body)
            .into_any_element()
    }
}

impl Render for CreateInstance {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let height = px(HEIGHT).min(window.viewport_size().height - px(48.));
        let content = match self.modpack.clone() {
            Some(flow) => flow.into_any_element(),
            None => self.render_flow(cx),
        };
        div().absolute().inset_0().child(
            Dialog::new(
                "create-instance",
                t(ShellMsg::NewServer),
                self.focus.clone(),
            )
            .frameless(px(WIDTH), height)
            .on_close(move |_, cx| {
                this.update(cx, |_, cx| cx.emit(CreateInstanceEvent::Close))
                    .ok();
            })
            .child(content),
        )
    }
}

impl CreateInstance {
    fn render_flow(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let steps = [
            t(M::StepSelectGame),
            t(M::StepBasicSettings),
            t(M::StepServerSettings),
            t(M::StepAutoSettings),
        ];
        let weak = cx.entity().downgrade();
        let on_select: Option<wizard::StepHandler> = (!self.submitting).then(|| {
            Rc::new(move |i: usize, _: &mut Window, cx: &mut gpui::App| {
                weak.update(cx, |this, cx| this.go_to(i, cx)).ok();
            }) as wizard::StepHandler
        });
        let rail = wizard::rail(&steps, self.step, |_| true, on_select);
        let body = if self.step == SELECT_GAME {
            self.render_games(cx)
        } else {
            self.render_fields(cx)
        };
        let last = self.step == AUTO;
        let manifest_loading = matches!(self.manifest, Some((_, Load::Loading)));
        let next_disabled = match self.step {
            SELECT_GAME => self.ready_manifest().is_none(),
            AUTO => !self.eula,
            _ => false,
        };
        let eula = last.then(|| {
            eula::eula_box(
                self.eula,
                self.submitting,
                cx.listener(|this, value: &bool, _, cx| {
                    this.eula = *value;
                    cx.notify();
                }),
            )
        });
        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(colors::GRAY_850)
            .child(rail)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .p(px(32.))
                    .pt(px(40.))
                    .child(
                        div()
                            .id("create-body")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .pr(px(4.))
                            .child(body),
                    )
                    .when_some(eula, |this, eula| this.child(div().pt(px(20.)).child(eula)))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .pt(px(20.))
                            .child(if self.step > SELECT_GAME {
                                Button::new("create-back", t(lodestone_ui::i18n::CommonMsg::Back))
                                    .disabled(self.submitting)
                                    .on_click(cx.listener(|this, _, _, cx| this.back(cx)))
                                    .into_any_element()
                            } else {
                                div().into_any_element()
                            })
                            .child(
                                Button::new(
                                    "create-next",
                                    if last {
                                        t(M::CreateServer)
                                    } else {
                                        t(lodestone_ui::i18n::CommonMsg::Next)
                                    },
                                )
                                .variant(ButtonVariant::Primary)
                                .loading(
                                    self.submitting
                                        || (self.step == SELECT_GAME && manifest_loading),
                                )
                                .disabled(next_disabled || self.submitting)
                                .on_click(cx.listener(|this, _, _, cx| this.next(cx))),
                            ),
                    ),
            )
            .into_any_element()
    }
}
