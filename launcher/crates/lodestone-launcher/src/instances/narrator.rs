//! "Keep the narrator off" (Settings > Shared game settings > Narrator), and the
//! notification that offers it at start to people who don't have it on, until they
//! accept or dismiss it.

use gpui::Context;
use launcher_core::shared_options::{self, Category, Profile, SharedError};

use super::Instances;

impl Instances {
    /// Whether to offer "Keep the narrator off" at start: it's off, it could be turned
    /// on (Accessibility & chat is shared), and the offer hasn't been answered yet. It
    /// comes back at every start until it's accepted or dismissed.
    pub fn offers_narrator(&self) -> bool {
        if self.settings.narrator_offered {
            return false;
        }
        let Ok(profile) = shared_options::load_profile(self.data_dir()) else {
            return false;
        };
        profile.shares(Category::Accessibility) && !shared_options::narrator_off(&profile)
    }

    /// The offer was accepted or dismissed: it doesn't come back.
    pub fn answer_narrator_offer(&mut self, cx: &mut Context<Self>) {
        if let Err(err) = self.update_settings(|settings| settings.narrator_offered = true, cx) {
            self.notice = Some(err.into());
        }
    }

    /// Keeps the narrator off in every instance (or stops), with Undo.
    pub fn set_narrator_off(
        &mut self,
        off: bool,
        cx: &mut Context<Self>,
    ) -> Result<Profile, SharedError> {
        let data = self.data_dir().clone();
        let before = crate::undo::Files::read(&[shared_options::profile_path(&data)]);
        let result = shared_options::set_narrator_off(&data, off);
        self.record_shared(before, None, cx);
        if result.is_ok() {
            self.shared_settings_changed(cx);
        }
        result
    }
}

#[cfg(all(test, feature = "screenshot"))]
mod tests {
    use gpui::{AppContext as _, Entity, TestAppContext};

    use super::*;

    /// An empty launcher with a data folder of its own.
    fn open(cx: &mut TestAppContext) -> Entity<Instances> {
        cx.update(crate::components::init);
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "lodestone-narrator-offer-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        cx.update(|cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            let data_dir = launcher_core::paths::DataDir::at(dir);
            cx.new(|cx| Instances::new(data_dir, accounts, cx))
        })
    }

    fn offers(instances: &Entity<Instances>, cx: &mut TestAppContext) -> bool {
        instances.read_with(cx, |instances, _| instances.offers_narrator())
    }

    #[test]
    fn the_narrator_is_offered_until_its_answered() {
        let mut cx = TestAppContext::single();
        let instances = open(&mut cx);
        // Not answered: offered again at the next start.
        assert!(offers(&instances, &mut cx));
        assert!(offers(&instances, &mut cx));
        instances.update(&mut cx, |instances, cx| instances.answer_narrator_offer(cx));
        assert!(!offers(&instances, &mut cx));
        // Developer > "Reset first-time setup" offers it again.
        instances.update(&mut cx, |instances, cx| {
            instances.reset_first_time_setup(cx)
        });
        assert!(offers(&instances, &mut cx));
    }

    #[test]
    fn the_narrator_isnt_offered_when_its_already_kept_off() {
        let mut cx = TestAppContext::single();
        let instances = open(&mut cx);
        instances
            .update(&mut cx, |instances, cx| {
                instances.set_narrator_off(true, cx)
            })
            .unwrap();
        assert!(!offers(&instances, &mut cx));
    }

    #[test]
    fn the_narrator_isnt_offered_when_accessibility_isnt_shared() {
        let mut cx = TestAppContext::single();
        let instances = open(&mut cx);
        let data = instances.read_with(&cx, |instances, _| instances.data_dir().clone());
        shared_options::update_profile(&data, |profile| {
            profile.disabled.insert(Category::Accessibility);
        })
        .unwrap();
        assert!(!offers(&instances, &mut cx));
    }
}
