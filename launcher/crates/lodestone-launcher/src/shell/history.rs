//! Where the top bar's back and forward arrows go: the places the window showed, as a
//! browser keeps them. A place is a page with what it showed: an instance and its tab,
//! or the Lodestone Host page with a server's page open.

use super::{InstanceTab, Page};

/// The most places Back goes back through.
const MOST: usize = 50;

/// A place in the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub page: Page,
    /// On the instance page: the selected instance and its tab.
    pub instance: Option<String>,
    pub tab: InstanceTab,
    /// On the Lodestone Host page: the open server page, by core and server.
    pub server: Option<(String, String)>,
}

impl Place {
    /// The place for what the window shows. Only what the page shows counts: the
    /// selected instance doesn't, on the Lodestone Host page.
    pub fn new(
        page: Page,
        instance: Option<String>,
        tab: InstanceTab,
        server: Option<(String, String)>,
    ) -> Self {
        match page {
            Page::Instance => Self {
                page,
                instance,
                tab,
                server: None,
            },
            Page::Servers | Page::Friends => Self {
                page,
                instance: None,
                tab: InstanceTab::default(),
                server,
            },
        }
    }
}

/// The places before and after the one shown.
#[derive(Debug, Default)]
pub struct History {
    back: Vec<Place>,
    forward: Vec<Place>,
    here: Option<Place>,
}

impl History {
    /// The window shows `place`. A new place ends the places Forward went to, as in a
    /// browser. Returns whether the place changed.
    pub fn visit(&mut self, place: Place) -> bool {
        match self.here.replace(place.clone()) {
            Some(here) if here == place => false,
            Some(here) => {
                self.back.push(here);
                if self.back.len() > MOST {
                    self.back.remove(0);
                }
                self.forward.clear();
                true
            }
            None => true,
        }
    }

    /// The window shows `place` in place of the current one, which isn't kept: after
    /// going back to a place that is partly gone (a deleted instance).
    pub fn replace(&mut self, place: Place) {
        self.here = Some(place);
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// The place to show for Back, which is now the current one.
    pub fn back(&mut self) -> Option<Place> {
        let place = self.back.pop()?;
        if let Some(here) = self.here.replace(place.clone()) {
            self.forward.push(here);
        }
        Some(place)
    }

    /// The place to show for Forward, which is now the current one.
    pub fn forward(&mut self) -> Option<Place> {
        let place = self.forward.pop()?;
        if let Some(here) = self.here.replace(place.clone()) {
            self.back.push(here);
        }
        Some(place)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(id: &str, tab: InstanceTab) -> Place {
        Place::new(Page::Instance, Some(id.into()), tab, None)
    }

    fn servers(server: Option<&str>) -> Place {
        Place::new(
            Page::Servers,
            Some("ignored".into()),
            InstanceTab::Logs,
            server.map(|s| ("core".to_string(), s.to_string())),
        )
    }

    #[test]
    fn back_and_forward_walk_the_places_shown() {
        let mut history = History::default();
        assert!(history.visit(instance("a", InstanceTab::Play)));
        assert!(!history.can_go_back());
        history.visit(servers(None));
        history.visit(servers(Some("smp")));
        assert!(!history.visit(servers(Some("smp"))));
        assert!(history.can_go_back() && !history.can_go_forward());

        assert_eq!(history.back(), Some(servers(None)));
        assert_eq!(history.back(), Some(instance("a", InstanceTab::Play)));
        assert_eq!(history.back(), None);
        assert!(history.can_go_forward());
        assert_eq!(history.forward(), Some(servers(None)));
        // going back to the current place is no new place
        assert!(!history.visit(servers(None)));
        assert!(history.can_go_forward());

        // a new place ends the ones ahead
        history.visit(instance("b", InstanceTab::Content));
        assert!(!history.can_go_forward());
        assert_eq!(history.back(), Some(servers(None)));
    }

    #[test]
    fn the_host_page_ignores_the_selected_instance() {
        assert_eq!(servers(None).instance, None);
        assert_eq!(servers(None).tab, InstanceTab::default());
        assert_eq!(instance("a", InstanceTab::Play).server, None);
    }

    #[test]
    fn it_keeps_only_the_newest_places() {
        let mut history = History::default();
        for n in 0..(MOST + 10) {
            history.visit(instance(&n.to_string(), InstanceTab::Play));
        }
        let mut steps = 0;
        while history.back().is_some() {
            steps += 1;
        }
        assert_eq!(steps, MOST);
    }
}
