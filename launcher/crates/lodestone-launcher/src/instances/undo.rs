//! The steps Undo puts back for the launcher's settings, an instance's settings and
//! the shared game settings (see `crate::undo` for the model).

use std::{path::PathBuf, sync::Arc};

use gpui::{App, Context, Entity, Task};
use launcher_core::{
    instances::{BACKGROUND_FILE, ICON_FILE, Instance},
    settings::{CliAccess, CliSettings, InstanceDefaults, Settings},
};

use super::{Instances, fields};
use crate::{
    i18n::{self, CommonMsg, InstanceSettingsMsg as IM, Language, ShellMsg as SM, format, t, tr},
    undo::{self, Files, Step, StepKey, UndoError},
};

/// "Language → Français".
pub(crate) fn arrow(setting: &str, value: &str) -> String {
    tr!(SM::ChangeTo, setting = setting, value = value)
}

fn on_off(on: bool) -> &'static str {
    t(if on { CommonMsg::On } else { CommonMsg::Off })
}

/// The settings in `settings.json` that Undo puts back. "Sign in as the Minecraft
/// Launcher" isn't among them: turning it on needs its warning answered first.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LauncherValues {
    language: Language,
    zoom: f32,
    auto_groups: bool,
    auto_update: bool,
    skin: Option<String>,
    instance_defaults: InstanceDefaults,
    cli: CliSettings,
}

impl LauncherValues {
    pub(crate) fn of(settings: &Settings) -> Self {
        Self {
            language: settings.language,
            zoom: settings.zoom,
            auto_groups: settings.auto_groups,
            auto_update: settings.auto_update,
            skin: settings.skin.clone(),
            instance_defaults: settings.instance_defaults.clone(),
            cli: settings.cli,
        }
    }

    fn apply(&self, settings: &mut Settings) {
        settings.language = self.language;
        settings.zoom = self.zoom;
        settings.auto_groups = self.auto_groups;
        settings.auto_update = self.auto_update;
        settings.skin = self.skin.clone();
        settings.instance_defaults = self.instance_defaults.clone();
        settings.cli = self.cli;
    }

    /// What `self` says where it differs from `other`.
    fn describe(&self, other: &Self) -> String {
        if self.language != other.language {
            let name = match self.language {
                Language::System => t(SM::LanguageSystem),
                language => i18n::resolve(language).native_name(),
            };
            arrow(t(SM::Language), name)
        } else if self.zoom != other.zoom {
            arrow(
                t(SM::Zoom),
                &format::percent((self.zoom * 100.0).round() as u64),
            )
        } else if self.auto_groups != other.auto_groups {
            arrow(t(SM::GroupAutomatically), on_off(self.auto_groups))
        } else if self.auto_update != other.auto_update {
            arrow(t(SM::UpdateAutomatically), on_off(self.auto_update))
        } else if self.skin != other.skin {
            let name = launcher_core::ui_skin::built_in(self.skin.as_deref().unwrap_or("default"))
                .map(|spec| spec.name)
                .unwrap_or_else(|| self.skin.clone().unwrap_or_default());
            arrow(t(SM::Skin), &name)
        } else if self.cli.enabled != other.cli.enabled {
            arrow(t(SM::AllowCli), on_off(self.cli.enabled))
        } else if self.cli.access != other.cli.access {
            let level = match self.cli.access {
                CliAccess::Full => t(SM::CliFullControl),
                CliAccess::ReadOnly => t(SM::CliReadOnly),
            };
            arrow(t(SM::CliAccess), level)
        } else if self.cli.traces != other.cli.traces {
            arrow(t(SM::DebugTraces), on_off(self.cli.traces))
        } else if self.cli.game_logs != other.cli.game_logs {
            arrow(t(SM::CliGameLogs), on_off(self.cli.game_logs))
        } else {
            t(SM::InstanceDefaults).to_string()
        }
    }
}

/// An instance's settings as Undo puts them back: its record, and its own icon and
/// Play background images.
#[derive(Clone, Debug, PartialEq)]
pub struct InstanceSnapshot {
    instance: Instance,
    icon: Image,
    background: Image,
}

/// One of an instance's images as it was: its bytes, or `None` when it had none. The
/// bytes are shared with `Instances::undo_images`, so two snapshots of an image that
/// didn't change hold (and compare as) the same bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Image {
    path: PathBuf,
    bytes: Option<Arc<Vec<u8>>>,
}

impl Image {
    /// Puts the image back as it was.
    fn write(&self) -> std::io::Result<()> {
        match &self.bytes {
            Some(bytes) => launcher_core::storage::write_atomic(&self.path, bytes),
            None => match std::fs::remove_file(&self.path) {
                Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
                _ => Ok(()),
            },
        }
    }
}

/// An instance setting that Undo describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Name,
    Group,
    Icon,
    Background,
    Loader,
    Prerelease,
    Colors,
    Memory,
    Java,
    JvmArgs,
    GameArgs,
    Window,
    Env,
}

impl Field {
    pub(crate) fn bit(self) -> u64 {
        1 << self as u64
    }

    fn label(self) -> &'static str {
        match self {
            Field::Name => t(IM::Name),
            Field::Group => t(IM::Group),
            Field::Icon => t(IM::Icon),
            Field::Background => t(IM::PlayBackground),
            Field::Loader => t(IM::ModLoader),
            Field::Prerelease => t(IM::PrereleaseUpdates),
            Field::Colors => t(SM::Skin),
            Field::Memory => t(IM::Memory),
            Field::Java => "Java",
            Field::JvmArgs => t(IM::JvmArguments),
            Field::GameArgs => t(IM::GameArguments),
            Field::Window => t(IM::Window),
            Field::Env => t(IM::EnvironmentVariables),
        }
    }
}

/// The settings that differ between two versions of an instance's record.
pub(crate) fn changed_fields(a: &Instance, b: &Instance) -> Vec<Field> {
    let mut fields = Vec::new();
    let mut check = |same: bool, field: Field| {
        if !same {
            fields.push(field);
        }
    };
    check(a.name == b.name, Field::Name);
    check(a.group == b.group, Field::Group);
    check(a.icon == b.icon, Field::Icon);
    // The slideshow is part of the Play background.
    check(
        a.background == b.background && a.slideshow == b.slideshow,
        Field::Background,
    );
    check(
        (a.loader, &a.loader_version) == (b.loader, &b.loader_version),
        Field::Loader,
    );
    check(
        a.prerelease_updates == b.prerelease_updates,
        Field::Prerelease,
    );
    check(
        (a.play_color, a.accent) == (b.play_color, b.accent),
        Field::Colors,
    );
    check(a.memory == b.memory, Field::Memory);
    check(a.java_override == b.java_override, Field::Java);
    check(a.jvm_args == b.jvm_args, Field::JvmArgs);
    check(a.game_args == b.game_args, Field::GameArgs);
    check(a.window == b.window, Field::Window);
    check(a.env == b.env, Field::Env);
    fields
}

/// What `instance` says for `fields`: "Name → Survival", or "Settings of Survival"
/// for several.
pub(crate) fn describe_instance(instance: &Instance, fields: &[Field]) -> String {
    let [field] = fields else {
        return tr!(SM::SettingsOf, name = instance.name);
    };
    let value = match field {
        Field::Name => instance.name.clone(),
        Field::Group => instance
            .group
            .clone()
            .unwrap_or_else(|| t(IM::NoGroup).to_string()),
        Field::Loader => instance.loader_label(),
        Field::Prerelease => on_off(instance.prerelease_updates).to_string(),
        Field::Memory => instance.memory.map_or_else(
            || t(CommonMsg::Default).to_string(),
            |m| fields::memory_label((m.min, m.max)),
        ),
        Field::Window => instance
            .window
            .as_ref()
            .map_or_else(|| t(CommonMsg::Default).to_string(), fields::window_label),
        // the rest say only which setting it was
        _ => return format!("{} · {}", field.label(), instance.name),
    };
    arrow(field.label(), &value)
}

impl Instances {
    /// Records a change of `settings.json` from `before`, if it changed something
    /// Undo puts back.
    pub(super) fn record_settings(&mut self, before: LauncherValues, cx: &mut Context<Self>) {
        let after = LauncherValues::of(&self.settings);
        if before == after || undo::replaying(cx) {
            return;
        }
        let entity = cx.entity();
        let (undo_to, redo_to) = (before.clone(), after.clone());
        let (said_before, said_after) = (before.describe(&after), after.describe(&before));
        let step = Step::new(
            StepKey::Saved,
            move || said_before.clone(),
            move || said_after.clone(),
            {
                let entity = entity.clone();
                move |cx| restore_settings(&entity, &undo_to, cx)
            },
            move |cx| restore_settings(&entity, &redo_to, cx),
        );
        undo::record(step, cx);
    }

    /// The instance's settings now, for Undo.
    pub fn snapshot(&mut self, id: &str) -> Option<InstanceSnapshot> {
        let instance = self.store.get(id).ok()?;
        let dir = self.store.dir(id);
        Some(InstanceSnapshot {
            icon: self.undo_image(dir.join(ICON_FILE)),
            background: self.undo_image(dir.join(BACKGROUND_FILE)),
            instance,
        })
    }

    /// The image at `path` as it is now. Each change is snapshotted before and after,
    /// and most leave the images alone: an image whose modification time and size are
    /// those it was read with isn't read again.
    fn undo_image(&mut self, path: PathBuf) -> Image {
        let Some((modified, len)) = std::fs::metadata(&path)
            .ok()
            .filter(|meta| meta.is_file())
            .and_then(|meta| Some((meta.modified().ok()?, meta.len())))
        else {
            self.undo_images.remove(&path);
            let bytes = std::fs::read(&path).ok().map(Arc::new);
            return Image { path, bytes };
        };
        if let Some((time, size, bytes)) = self.undo_images.get(&path)
            && (*time, *size) == (modified, len)
        {
            let bytes = Some(bytes.clone());
            return Image { path, bytes };
        }
        let bytes = std::fs::read(&path).ok().map(Arc::new);
        match &bytes {
            Some(read) => {
                self.undo_images
                    .insert(path.clone(), (modified, len, read.clone()));
            }
            None => {
                self.undo_images.remove(&path);
            }
        }
        Image { path, bytes }
    }

    /// Changes an instance's settings with `change`, and records the change for Undo.
    pub fn undoable<R>(
        &mut self,
        id: &str,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Self, &mut Context<Self>) -> Result<R, String>,
    ) -> Result<R, String> {
        let before = self.snapshot(id);
        let result = change(self, cx);
        if result.is_ok()
            && let Some(before) = before
        {
            self.record_instance(before, cx);
        }
        result
    }

    /// Like [`Self::undoable`], for a change that finishes in the background (see
    /// `files.rs`): recorded once it is done.
    pub fn undoable_task<R: 'static>(
        &mut self,
        id: &str,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Self, &mut Context<Self>) -> Task<Result<R, String>>,
    ) -> Task<Result<R, String>> {
        let before = self.snapshot(id);
        let task = change(self, cx);
        self.then_detached(
            task,
            move |this, result, cx| {
                if result.is_ok()
                    && let Some(before) = before
                {
                    this.record_instance(before, cx);
                }
            },
            cx,
        )
    }

    /// Records the change of an instance's settings from `before` to now.
    pub fn record_instance(&mut self, before: InstanceSnapshot, cx: &mut Context<Self>) {
        let Some(after) = self.snapshot(&before.instance.id) else {
            return;
        };
        if undo::replaying(cx) {
            return;
        }
        let mut fields = changed_fields(&before.instance, &after.instance);
        // a new image under the same setting ("custom" before and after)
        for (same, field) in [
            (before.icon == after.icon, Field::Icon),
            (before.background == after.background, Field::Background),
        ] {
            if !same && !fields.contains(&field) {
                fields.push(field);
            }
        }
        if fields.is_empty() {
            return;
        }
        let said_before = describe_instance(&before.instance, &fields);
        let said_after = describe_instance(&after.instance, &fields);
        let entity = cx.entity();
        let step = Step::new(
            StepKey::Saved,
            move || said_before.clone(),
            move || said_after.clone(),
            {
                let entity = entity.clone();
                move |cx| entity.update(cx, |this, cx| this.restore_instance(&before, cx))
            },
            move |cx| entity.update(cx, |this, cx| this.restore_instance(&after, cx)),
        );
        undo::record(step, cx);
    }

    /// Puts an instance's settings back as `snapshot` has them. How much it was
    /// played stays.
    fn restore_instance(
        &mut self,
        snapshot: &InstanceSnapshot,
        cx: &mut Context<Self>,
    ) -> Result<(), UndoError> {
        let id = &snapshot.instance.id;
        let current = self.store.get(id).map_err(|_| UndoError::Gone)?;
        let mut instance = snapshot.instance.clone();
        instance.last_played = current.last_played;
        instance.play_time = current.play_time;
        for image in [&snapshot.icon, &snapshot.background] {
            image
                .write()
                .map_err(|err| UndoError::Refused(err.to_string()))?;
        }
        self.store
            .save(&instance)
            .map_err(|err| UndoError::Refused(err.to_string()))?;
        self.forget_background(id, cx);
        self.update_listed(instance, cx);
        Ok(())
    }

    /// Records a change of the shared game settings, kept in `before`'s files. While
    /// the game of `instance` runs, it can't be undone.
    pub fn record_shared(
        &mut self,
        before: Files,
        instance: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let after = Files::read(&before.paths());
        if before == after || undo::replaying(cx) {
            return;
        }
        let said = || t(SM::SharedGameSettings).to_string();
        let entity = cx.entity();
        let restore = move |files: Files| {
            let entity = entity.clone();
            let instance = instance.clone();
            move |cx: &mut App| {
                entity.update(cx, |this, cx| {
                    if let Some(id) = &instance {
                        if !this.store.dir(id).is_dir() {
                            return Err(UndoError::Gone);
                        }
                        if this.is_active(id) {
                            return Err(UndoError::Refused(t(IM::StopGameToChange).into()));
                        }
                    }
                    files
                        .write()
                        .map_err(|err| UndoError::Refused(err.to_string()))?;
                    // the pages that show them read them again
                    this.shared_settings_changed(cx);
                    Ok(())
                })
            }
        };
        let step = Step::new(StepKey::Saved, said, said, restore(before), restore(after));
        undo::record(step, cx);
    }
}

/// Puts the launcher's settings back as `values` has them, and applies them.
fn restore_settings(
    entity: &Entity<Instances>,
    values: &LauncherValues,
    cx: &mut App,
) -> Result<(), UndoError> {
    entity.update(cx, |this, cx| {
        this.update_settings(|settings| values.apply(settings), cx)
            .map_err(UndoError::Refused)?;
        i18n::apply(this.settings.language);
        crate::theme::zoom::set(this.settings.zoom);
        this.reload_skin(cx);
        crate::set_menus(cx);
        cx.refresh_windows();
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_fields_name_what_differs() {
        let a = Instance {
            name: "Survival".into(),
            ..Instance::default()
        };
        let mut b = a.clone();
        assert!(changed_fields(&a, &b).is_empty());
        let mut shows = a.clone();
        shows.slideshow = Some(launcher_core::instances::SlideshowSource::Custom);
        assert_eq!(changed_fields(&a, &shows), vec![Field::Background]);
        b.name = "Creative".into();
        b.prerelease_updates = true;
        assert_eq!(changed_fields(&a, &b), vec![Field::Name, Field::Prerelease]);
        // how much it was played isn't a setting
        b = a.clone();
        b.play_time = 100;
        assert!(changed_fields(&a, &b).is_empty());
    }

    #[test]
    fn launcher_values_put_back_only_their_own_settings() {
        let mut settings = Settings::default();
        let values = LauncherValues::of(&settings);
        settings.zoom = 1.5;
        settings.collapsed_groups.insert("Friends".into());
        values.apply(&mut settings);
        assert_eq!(settings.zoom, 1.0);
        assert!(settings.collapsed_groups.contains("Friends"));
    }
}
