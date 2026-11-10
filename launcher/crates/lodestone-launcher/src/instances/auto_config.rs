//! Automatic setup at launch ([`launcher_core::configurator`]): a new instance's memory
//! and Java arguments before the launch command is built, and its `options.txt` before
//! Java starts. What it chose is noted in the instance's console. Settings > Instance
//! defaults > "Set up new instances automatically" turns it off.

use gpui::Context;
use launcher_core::{
    configurator::{self, ControlsSource, Hardware},
    game::log4j::Level,
    instances::Instance,
    shared_options,
};

use super::Instances;
use crate::i18n::{InstancesMsg as M, format, t, tr};

impl Instances {
    /// "Set up new instances automatically".
    pub fn auto_configure(&self) -> bool {
        self.settings.auto_configure
    }

    pub fn set_auto_configure(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Err(err) = self.update_settings(|settings| settings.auto_configure = on, cx) {
            self.notice = Some(err.into());
        }
    }

    /// Before the launch command is built: chooses the memory and JVM arguments of a
    /// new instance, or follows its mods with the ones chosen before. Saves `instance`
    /// when they change.
    pub(super) fn auto_configure_launch(
        &mut self,
        instance: &mut Instance,
        cx: &mut Context<Self>,
    ) {
        if !self.settings.auto_configure {
            return;
        }
        let game_dir = self.store.game_dir(&instance.id);
        let defaults = self.settings.instance_defaults.clone();
        let outcome =
            configurator::configure_launch(instance, &defaults, Hardware::current(), &game_dir);
        if !outcome.changed {
            if let Some((have, recommended)) = outcome.low_memory {
                self.note(&instance.id, Level::Warn, low_memory(have, recommended));
            }
            return;
        }
        launcher_core::trace::emit("game.auto_config", || {
            serde_json::json!({
                "instance": instance.id,
                "hardware": Hardware::current(),
                "memory": outcome.memory.map(|m| [m.min, m.max]),
                "jvm_args": outcome.jvm_args,
                "limited": outcome.limited,
            })
        });
        if let Err(err) = self.save(instance, cx) {
            self.note(&instance.id, Level::Warn, err);
            return;
        }
        let id = instance.id.clone();
        if let Some(memory) = outcome.memory {
            let range = super::fields::memory_label((memory.min, memory.max));
            self.note(&id, Level::Info, tr!(M::AutoMemoryNote, memory = range));
        }
        if let Some((_, wanted)) = outcome.limited {
            let wanted = format::memory_mb(wanted.into());
            self.note(
                &id,
                Level::Warn,
                tr!(M::AutoMemoryLimitedNote, wanted = wanted),
            );
        }
        if let Some((have, recommended)) = outcome.low_memory {
            self.note(&id, Level::Warn, low_memory(have, recommended));
        }
        if outcome.jvm_args {
            self.note(&id, Level::Info, t(M::AutoJvmArgsNote).to_string());
        }
    }

    /// Before Java starts, once the client jar is there: a new instance's first
    /// `options.txt`, with video settings for this computer and the controls of the
    /// game played last, here or in another launcher (`outside`, found while the game
    /// prepared). Runs before the shared settings are merged, which win.
    pub(super) fn auto_configure_options(
        &mut self,
        id: &str,
        outside: Vec<configurator::Outside>,
        cx: &mut Context<Self>,
    ) {
        if !self.settings.auto_configure {
            return;
        }
        let Ok(mut instance) = self.store.get(id) else {
            return;
        };
        if instance
            .auto_config
            .as_ref()
            .is_none_or(|state| state.options)
        {
            return;
        }
        let preferred = shared_options::load_profile(&self.data_dir)
            .ok()
            .and_then(|profile| profile.preferred);
        let others: Vec<(Instance, std::path::PathBuf)> = self
            .list
            .iter()
            .map(|i| (i.clone(), self.store.dir(&i.id)))
            .collect();
        let candidates = configurator::candidates(preferred.as_deref(), &others, id, outside);
        let jar = self.client_jar(&instance.game_version);
        let result = configurator::configure_options(
            &self.data_dir,
            &mut instance,
            &self.store.dir(id),
            Some(&jar),
            Hardware::current(),
            &candidates,
        );
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(err) => {
                self.note(id, Level::Warn, err.to_string());
                return;
            }
        };
        if outcome.changed
            && let Err(err) = self.save(&instance, cx)
        {
            self.note(id, Level::Warn, err);
        }
        launcher_core::trace::emit("game.auto_options", || {
            serde_json::json!({
                "instance": id,
                "tier": outcome.video,
                "render_distance": outcome.render_distance,
                "controls": outcome.controls_from.as_ref().map(|s| match s {
                    ControlsSource::Instance(name) => name.clone(),
                    ControlsSource::MinecraftLauncher => "minecraft_launcher".into(),
                    ControlsSource::OtherLauncher { launcher, instance } => {
                        format!("{launcher}: {instance}")
                    }
                }),
                "written": outcome.written,
            })
        });
        if outcome.video.is_some() {
            let note = match outcome.render_distance {
                Some(distance) => tr!(M::AutoVideoNote, distance = distance),
                None => t(M::AutoVideoNoDistanceNote).to_string(),
            };
            self.note(id, Level::Info, note);
        }
        match outcome.controls_from {
            Some(ControlsSource::Instance(name)) => {
                self.note(id, Level::Info, tr!(M::AutoControlsNote, source = name))
            }
            Some(ControlsSource::MinecraftLauncher) => {
                self.note(id, Level::Info, t(M::AutoControlsLauncherNote).to_string())
            }
            Some(ControlsSource::OtherLauncher { launcher, instance }) => self.note(
                id,
                Level::Info,
                tr!(
                    M::AutoControlsOtherLauncherNote,
                    instance = instance,
                    launcher = launcher
                ),
            ),
            None => {}
        }
    }

    /// A line in the instance's console.
    fn note(&mut self, id: &str, level: Level, text: impl Into<String>) {
        if let Some(run) = self.runs.get_mut(id) {
            run.log.note(level, text.into());
        }
    }
}

fn low_memory(have: u32, recommended: u32) -> String {
    tr!(
        M::AutoLowMemoryNote,
        have = format::memory_mb(have.into()),
        recommended = format::memory_mb(recommended.into())
    )
}
