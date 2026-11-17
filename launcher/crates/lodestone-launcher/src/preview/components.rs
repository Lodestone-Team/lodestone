//! The readable components' gallery (`lodestone_ui::components::readable`), for the
//! offscreen screenshots: `--scene components` in a 1200px window, and
//! `components-narrow` in a 520px one, which shows facts left out, columns folded
//! and keys wrapping. A window is no taller than the screen, so the gallery comes in
//! pages: `components-2`, `components-narrow-3` and so on. It opens the gallery
//! alone: no accounts, instances or data folder.

use gpui::App;
use lodestone_ui::components::readable::ComponentGallery;

/// The gallery's width (narrow or not) and page for a scene name, if it is one.
fn parse(scene: &str) -> Option<(bool, usize)> {
    let rest = scene.strip_prefix("components")?;
    let (narrow, rest) = match rest.strip_prefix("-narrow") {
        Some(rest) => (true, rest),
        None => (false, rest),
    };
    let page = match rest {
        "" => 1,
        _ => rest.strip_prefix('-')?.parse().ok()?,
    };
    (1..=ComponentGallery::pages(narrow))
        .contains(&page)
        .then_some((narrow, page))
}

pub fn is_scene(scene: &str) -> bool {
    parse(scene).is_some()
}

/// Opens the gallery when `scene` is one of its scenes, and says whether it did.
pub fn open(scene: &str, cx: &mut App) -> bool {
    let Some((narrow, page)) = parse(scene) else {
        return false;
    };
    let width = if narrow { 520. } else { 1200. };
    let options = crate::window_options(crate::APP_NAME, width, 1000., cx);
    crate::open_window(
        options,
        move |_| {
            if narrow {
                ComponentGallery::narrow(page)
            } else {
                ComponentGallery::new(page)
            }
        },
        cx,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_names_give_a_width_and_a_page() {
        assert_eq!(parse("components"), Some((false, 1)));
        assert_eq!(parse("components-2"), Some((false, 2)));
        assert_eq!(parse("components-narrow"), Some((true, 1)));
        assert_eq!(parse("components-narrow-3"), Some((true, 3)));
        assert_eq!(parse("components-99"), None);
        assert_eq!(parse("components-x"), None);
        assert_eq!(parse("content-mods"), None);
    }
}
