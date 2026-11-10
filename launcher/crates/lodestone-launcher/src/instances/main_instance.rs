//! The main instance of the shared game settings: the one whose in-game changes are
//! copied to the others ([`shared_options::Profile::preferred`], which the files still
//! call "preferred"). The launcher offers to make the first new instance the main one,
//! and asks before another instance takes over from the current main.

use gpui::Context;
use launcher_core::shared_options;

use super::{Instances, InstancesEvent};

impl Instances {
    /// The main instance's id, if there is one and it still exists.
    pub fn main_instance(&self) -> Option<String> {
        let profile = shared_options::load_profile(self.data_dir()).ok()?;
        profile
            .preferred
            .filter(|id| self.list.iter().any(|instance| instance.id == *id))
    }

    /// Makes `id` the main instance, after asking (in the shell, through
    /// [`InstancesEvent::ConfirmMain`]) when another instance is the main one now.
    /// Returns whether it was made main straight away.
    pub fn request_main(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        match self.main_instance() {
            Some(current) if current != id => {
                cx.emit(InstancesEvent::ConfirmMain {
                    id: id.to_string(),
                    current,
                });
                false
            }
            _ => {
                self.set_main(Some(id), cx);
                true
            }
        }
    }

    /// Makes `id` the main instance (`None`: no main instance), with Undo.
    pub fn set_main(&mut self, id: Option<&str>, cx: &mut Context<Self>) {
        let data = self.data_dir().clone();
        let dir = id.map(|id| self.instance_dir(id));
        let mut paths = vec![shared_options::profile_path(&data)];
        if let Some(dir) = &dir {
            paths.push(shared_options::instance_path(dir));
        }
        let before = crate::undo::Files::read(&paths);
        match shared_options::set_preferred(&data, id, dir.as_deref()) {
            Ok(_) => {
                self.record_shared(before, id.map(str::to_string), cx);
                self.shared_settings_changed(cx);
            }
            Err(err) => {
                self.notice = Some(err.to_string().into());
                cx.notify();
            }
        }
    }

    /// A new instance appeared in the list: the first time, with no main instance yet,
    /// offer to make it the main one.
    pub(super) fn offer_main(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.settings.main_instance_offered || self.main_instance().is_some() {
            return;
        }
        // Asked once, however it's answered.
        if self
            .update_settings(|settings| settings.main_instance_offered = true, cx)
            .is_ok()
        {
            cx.emit(InstancesEvent::OfferMain(id.to_string()));
        }
    }

    /// Settings > Developer > "Reset first-time setup": the next new instance is
    /// offered as the main one again, "Keep the narrator off" at the next start, and
    /// Lodestone Host on the Lodestone Host page. The main instance is cleared, as the
    /// offer only comes without one.
    pub fn reset_first_time_setup(&mut self, cx: &mut Context<Self>) {
        if let Err(err) = self.update_settings(
            |settings| {
                settings.main_instance_offered = false;
                settings.narrator_offered = false;
                settings.host_offer_dismissed = false;
            },
            cx,
        ) {
            self.notice = Some(err.into());
        }
        if self.main_instance().is_some() {
            self.set_main(None, cx);
        }
    }
}

#[cfg(all(test, feature = "screenshot"))]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui::{AppContext as _, Entity, TestAppContext};

    use super::*;

    /// An empty launcher, and the main-instance events it sends.
    fn open(cx: &mut TestAppContext) -> (Entity<Instances>, Rc<RefCell<Vec<String>>>) {
        cx.update(crate::components::init);
        let events = Rc::new(RefCell::new(Vec::new()));
        // A data folder of its own: the preview's is shared by the tests in this process.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "lodestone-main-instance-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let instances = cx.update(|cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            let data_dir = launcher_core::paths::DataDir::at(dir);
            let instances = cx.new(|cx| Instances::new(data_dir, accounts, cx));
            let seen = events.clone();
            cx.subscribe(
                &instances,
                move |_, event: &InstancesEvent, _| match event {
                    InstancesEvent::OfferMain(id) => seen.borrow_mut().push(format!("offer {id}")),
                    InstancesEvent::ConfirmMain { id, current } => seen
                        .borrow_mut()
                        .push(format!("confirm {id} over {current}")),
                    _ => {}
                },
            )
            .detach();
            instances
        });
        (instances, events)
    }

    fn create(instances: &Entity<Instances>, name: &str, cx: &mut TestAppContext) -> String {
        instances.update(cx, |instances, cx| {
            instances.create(name, "1.21.4", None, cx).unwrap().id
        })
    }

    #[test]
    fn the_first_new_instance_is_offered_once() {
        let mut cx = TestAppContext::single();
        let (instances, events) = open(&mut cx);
        let first = create(&instances, "First", &mut cx);
        create(&instances, "Second", &mut cx);
        cx.run_until_parked();
        assert_eq!(*events.borrow(), vec![format!("offer {first}")]);
    }

    #[test]
    fn no_offer_while_there_is_a_main_instance() {
        let mut cx = TestAppContext::single();
        let (instances, events) = open(&mut cx);
        let first = create(&instances, "First", &mut cx);
        instances.update(&mut cx, |instances, cx| {
            instances.set_main(Some(&first), cx);
            instances.reset_first_time_setup(cx);
            // The reset clears the main instance, so the next one is offered.
            assert_eq!(instances.main_instance(), None);
        });
        events.borrow_mut().clear();
        let second = create(&instances, "Second", &mut cx);
        cx.run_until_parked();
        assert_eq!(*events.borrow(), vec![format!("offer {second}")]);
    }

    #[test]
    fn taking_over_from_another_main_instance_asks_first() {
        let mut cx = TestAppContext::single();
        let (instances, events) = open(&mut cx);
        let first = create(&instances, "First", &mut cx);
        let second = create(&instances, "Second", &mut cx);
        events.borrow_mut().clear();
        instances.update(&mut cx, |instances, cx| {
            // No main yet: straight away.
            assert!(instances.request_main(&first, cx));
            assert_eq!(instances.main_instance(), Some(first.clone()));
            // The main one again: nothing to ask.
            assert!(instances.request_main(&first, cx));
            // Another: asks, and changes nothing yet.
            assert!(!instances.request_main(&second, cx));
            assert_eq!(instances.main_instance(), Some(first.clone()));
            instances.set_main(Some(&second), cx);
            assert_eq!(instances.main_instance(), Some(second.clone()));
        });
        cx.run_until_parked();
        assert_eq!(
            *events.borrow(),
            vec![format!("confirm {second} over {first}")]
        );
    }
}
