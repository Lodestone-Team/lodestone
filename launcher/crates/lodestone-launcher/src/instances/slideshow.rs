//! The Play tab's slideshow: screenshots, or the instance's own set of images, each
//! shown for a while and then crossfading into the next.
//!
//! Only the image on screen and the one fading in are decoded, on a background
//! thread. The view redraws every frame only while it fades; between fades it waits
//! on a timer, and it stops while the Play tab isn't shown.

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    Context, Entity, IntoElement, ObjectFit, Render, RenderImage, Task, Window, div, img,
    prelude::*,
};
use launcher_core::{
    content::InstanceDirs, hero_cache::HeroCache, instances::SlideshowSource, screenshots,
};

use super::{
    Instances,
    appearance::{Background, Scrim, fit_for_hero, scrim_for, scrim_layers},
};

/// How long each image is shown.
pub const SHOW_FOR: Duration = Duration::from_secs(15);
/// How long one image takes to fade into the next.
pub const FADE: Duration = Duration::from_millis(1500);
/// With reduced motion: no fade, and a slower change.
pub const SHOW_FOR_REDUCED: Duration = Duration::from_secs(45);
/// A slideshow goes through at most this many images.
const MAX_IMAGES: usize = 200;

/// The image after `current` in a slideshow of `len`, starting over at the end.
pub fn next_index(current: usize, len: usize) -> usize {
    if len == 0 { 0 } else { (current + 1) % len }
}

/// A slow ease in and out over `t` from 0 to 1.
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The scrim between `from` and `to`, `t` of the way, so the text stays readable
/// while one image fades into the next.
pub fn blend(from: Scrim, to: Scrim, t: f32) -> Scrim {
    let t = t.clamp(0.0, 1.0);
    Scrim {
        text: from.text + (to.text - from.text) * t,
        dim: from.dim + (to.dim - from.dim) * t,
    }
}

/// The images of a slideshow from `source`, in the order it shows them: the
/// instance's screenshots or everyone's, newest first, or its own set as added.
pub fn slideshow_paths(
    source: SlideshowSource,
    own: &InstanceDirs,
    all: &[InstanceDirs],
    custom: Vec<PathBuf>,
) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = match source {
        SlideshowSource::Instance => screenshots::list(own).into_iter().map(|s| s.path).collect(),
        SlideshowSource::AllInstances => {
            let mut shots: Vec<_> = all.iter().flat_map(screenshots::list).collect();
            shots.sort_by_key(|shot| std::cmp::Reverse(shot.modified));
            shots.into_iter().map(|s| s.path).collect()
        }
        SlideshowSource::Custom => custom,
    };
    paths.truncate(MAX_IMAGES);
    paths
}

/// Reads an image for the slideshow from its display-size copy (made the first time),
/// never the original, with its scrim.
fn decode(cache: &HeroCache, path: &std::path::Path) -> Option<(Arc<RenderImage>, Scrim)> {
    let copy = cache.copy_of(path).ok()?;
    let image = fit_for_hero(screenshots::decode(&copy).ok()?);
    let scrim = scrim_for(&image);
    Some((crate::images::render_image(image), scrim))
}

/// What the slideshow shows: an instance and where its images come from.
type Key = (String, SlideshowSource, u64);

pub struct Slideshow {
    instances: Entity<Instances>,
    key: Option<Key>,
    paths: Vec<PathBuf>,
    index: usize,
    current: Option<Background>,
    /// The image fading in, and when it started.
    next: Option<(Background, Instant)>,
    /// The Play tab shows the slideshow.
    shown: bool,
    /// Lists the images, or decodes the first.
    _load: Option<Task<()>>,
    /// Waits for the next change, then decodes the next image.
    timer: Option<Task<()>>,
    /// Holds the fade at this point, for the offscreen screenshots.
    frozen: Option<f32>,
}

impl Slideshow {
    pub fn new(instances: Entity<Instances>) -> Self {
        Self {
            instances,
            key: None,
            paths: Vec::new(),
            index: 0,
            current: None,
            next: None,
            shown: false,
            _load: None,
            timer: None,
            frozen: None,
        }
    }

    /// Follows the shell: `target` is the selected instance and its slideshow's source
    /// (`None` for no slideshow), `shown` whether the Play tab is on screen.
    pub fn sync(
        &mut self,
        target: Option<(String, SlideshowSource)>,
        shown: bool,
        cx: &mut Context<Self>,
    ) {
        let revision = self.instances.read(cx).slideshow_revision();
        let key = target.map(|(id, source)| (id, source, revision));
        if key != self.key {
            self.key = key;
            self.paths.clear();
            self.index = 0;
            self.set_current(None, cx);
            if let Some((next, _)) = self.next.take() {
                crate::images::release([next.image], cx);
            }
            self.timer = None;
            self._load = None;
            if self.key.is_some() {
                self.list(true, cx);
            }
        }
        let shown = shown && self.key.is_some();
        if shown != self.shown {
            self.shown = shown;
            if shown {
                self.schedule(cx);
            } else {
                // Off screen: no timer, and a fade under way just ends.
                self.timer = None;
                if let Some((next, _)) = self.next.take() {
                    self.set_current(Some(next), cx);
                    self.index = next_index(self.index, self.paths.len());
                }
            }
        }
    }

    /// Lists the images again; `first` also decodes the first one to show.
    fn list(&mut self, first: bool, cx: &mut Context<Self>) {
        let Some((id, source, _)) = self.key.clone() else {
            return;
        };
        let instances = self.instances.read(cx);
        let own = instances.content_dirs(&id);
        let all: Vec<InstanceDirs> = match source {
            SlideshowSource::AllInstances => instances
                .list()
                .iter()
                .map(|i| instances.content_dirs(&i.id))
                .collect(),
            _ => Vec::new(),
        };
        let custom = match source {
            SlideshowSource::Custom => instances.store().slideshow_images(&id),
            _ => Vec::new(),
        };
        let key = self.key.clone();
        let cache = instances.hero_cache().clone();
        let work = cx.background_spawn(async move {
            let paths = slideshow_paths(source, &own, &all, custom);
            let decoded = if first {
                paths.first().and_then(|path| decode(&cache, path))
            } else {
                None
            };
            (paths, decoded)
        });
        self._load = Some(cx.spawn(async move |this, cx| {
            let (paths, decoded) = work.await;
            this.update(cx, |this, cx| {
                if this.key != key {
                    return;
                }
                this.paths = paths;
                if first {
                    this.index = 0;
                    this.set_current(
                        decoded.map(|(image, scrim)| Background { image, scrim }),
                        cx,
                    );
                } else if this.index >= this.paths.len() {
                    this.index = 0;
                }
                this.schedule(cx);
                cx.notify();
            })
            .ok();
        }));
    }

    /// Waits until the next image is due, then decodes it and starts the fade.
    fn schedule(&mut self, cx: &mut Context<Self>) {
        if !self.shown || self.timer.is_some() || self.next.is_some() || self.paths.len() < 2 {
            return;
        }
        let reduced = crate::components::prefers_reduced_motion();
        let wait = if reduced { SHOW_FOR_REDUCED } else { SHOW_FOR };
        let key = self.key.clone();
        let cache = self.instances.read(cx).hero_cache().clone();
        self.timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let Ok(Some(path)) = this.update(cx, |this, _| {
                let index = next_index(this.index, this.paths.len());
                this.paths.get(index).cloned()
            }) else {
                return;
            };
            let decoded = cx
                .background_executor()
                .spawn(async move { decode(&cache, &path) })
                .await;
            this.update(cx, |this, cx| {
                this.timer = None;
                if this.key != key || !this.shown {
                    return;
                }
                let index = next_index(this.index, this.paths.len());
                let Some((image, scrim)) = decoded else {
                    // An image that can't be read is skipped.
                    this.index = index;
                    this.schedule(cx);
                    return;
                };
                let next = Background { image, scrim };
                if reduced {
                    this.set_current(Some(next), cx);
                    this.advance(index, cx);
                } else {
                    this.next = Some((next, Instant::now()));
                }
                cx.notify();
            })
            .ok();
        }));
    }

    /// Shows `image`, or none, and frees the GPU copy of the image it replaces: a
    /// slideshow left on screen would otherwise add a full-size image to GPU memory
    /// with each change.
    fn set_current(&mut self, image: Option<Background>, cx: &mut gpui::App) {
        if let Some(old) = std::mem::replace(&mut self.current, image) {
            crate::images::release([old.image], cx);
        }
    }

    /// The image at `index` is now the one shown: wait for the next, and list the
    /// images again after a round, for new screenshots.
    fn advance(&mut self, index: usize, cx: &mut Context<Self>) {
        let wrapped = index == 0;
        self.index = index;
        if wrapped
            && self
                .key
                .as_ref()
                .is_some_and(|k| k.1 != SlideshowSource::Custom)
        {
            self.list(false, cx);
        } else {
            self.schedule(cx);
        }
    }

    /// Shows the second image fading in, halfway, for the offscreen screenshots.
    pub fn preview_fade(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.paths.get(1) else {
            return;
        };
        let cache = self.instances.read(cx).hero_cache().clone();
        if let Some((image, scrim)) = decode(&cache, path) {
            self.timer = None;
            self.frozen = Some(0.5);
            self.next = Some((Background { image, scrim }, Instant::now()));
            cx.notify();
        }
    }

    /// Whether the images are listed and the first is decoded, for the offscreen
    /// screenshots.
    pub fn is_ready(&self) -> bool {
        self.current.is_some() && !self.paths.is_empty()
    }
}

/// The hero's corners: children are clipped to a rectangle, so each layer rounds
/// itself to match.
const HERO_RADIUS: gpui::Rems = crate::theme::radius::XL;

impl Render for Slideshow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut fade = None;
        if let Some((_, start)) = &self.next {
            let t = self
                .frozen
                .unwrap_or_else(|| start.elapsed().as_secs_f32() / FADE.as_secs_f32());
            if t >= 1.0 {
                let (next, _) = self.next.take().expect("checked");
                self.set_current(Some(next), cx);
                let index = next_index(self.index, self.paths.len());
                self.advance(index, cx);
            } else {
                fade = Some(ease(t));
                // Only while it fades: draw again on the next frame.
                if self.frozen.is_none() {
                    window.request_animation_frame();
                }
            }
        }
        let cover = |image: Arc<RenderImage>| {
            img(image)
                .absolute()
                .inset_0()
                .size_full()
                .rounded(HERO_RADIUS)
                .object_fit(ObjectFit::Cover)
        };
        let scrim = match (&self.current, &self.next, fade) {
            (Some(current), Some((next, _)), Some(t)) => Some(blend(current.scrim, next.scrim, t)),
            (Some(current), _, _) => Some(current.scrim),
            (None, Some((next, _)), Some(t)) => Some(blend(
                Scrim {
                    text: 0.0,
                    dim: 0.0,
                },
                next.scrim,
                t,
            )),
            _ => None,
        };
        div()
            .absolute()
            .inset_0()
            .size_full()
            .map(|this| match &self.current {
                Some(current) => this.child(cover(current.image.clone())),
                // Nothing to show yet, or no images: the launcher's artwork.
                None => this.child(
                    img(crate::images::hero(cx))
                        .absolute()
                        .inset_0()
                        .size_full()
                        .rounded(HERO_RADIUS)
                        .object_fit(ObjectFit::Cover),
                ),
            })
            .when_some(self.next.as_ref().zip(fade), |this, ((next, _), t)| {
                this.child(cover(next.image.clone()).opacity(t))
            })
            .children(
                scrim
                    .map(|scrim| scrim_layers(scrim, HERO_RADIUS))
                    .into_iter()
                    .flatten(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_image_wraps_around() {
        assert_eq!(next_index(0, 3), 1);
        assert_eq!(next_index(2, 3), 0);
        assert_eq!(next_index(0, 0), 0);
        assert_eq!(next_index(5, 1), 0);
    }

    #[test]
    fn the_fade_eases_and_the_scrim_follows_it() {
        assert_eq!(ease(0.0), 0.0);
        assert_eq!(ease(1.0), 1.0);
        assert_eq!(ease(0.5), 0.5);
        assert!(ease(0.1) < 0.1, "slow at the start");
        let dark = Scrim {
            text: 0.4,
            dim: 0.0,
        };
        let bright = Scrim {
            text: 0.9,
            dim: 0.2,
        };
        assert_eq!(blend(dark, bright, 0.0), dark);
        assert_eq!(blend(dark, bright, 1.0), bright);
        let half = blend(dark, bright, 0.5);
        assert!((half.text - 0.65).abs() < 1e-6 && (half.dim - 0.1).abs() < 1e-6);
    }

    fn shot(dir: &std::path::Path, name: &str, age_secs: u64) -> PathBuf {
        let path = dir.join(name);
        image::RgbImage::from_pixel(8, 8, image::Rgb([1, 2, 3]))
            .save(&path)
            .unwrap();
        let when = std::time::SystemTime::now() - Duration::from_secs(age_secs);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(when)
            .unwrap();
        path
    }

    #[test]
    fn sources_list_screenshots_newest_first_or_the_own_set() {
        let root =
            std::env::temp_dir().join(format!("lodestone-slideshow-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let a = InstanceDirs::new(root.join("a"));
        let b = InstanceDirs::new(root.join("b"));
        for dirs in [&a, &b] {
            std::fs::create_dir_all(screenshots::dir(dirs)).unwrap();
        }
        let a_old = shot(&screenshots::dir(&a), "old.png", 300);
        let b_mid = shot(&screenshots::dir(&b), "mid.png", 200);
        let a_new = shot(&screenshots::dir(&a), "new.png", 100);

        assert_eq!(
            slideshow_paths(SlideshowSource::Instance, &a, &[], Vec::new()),
            vec![a_new.clone(), a_old.clone()]
        );
        assert_eq!(
            slideshow_paths(
                SlideshowSource::AllInstances,
                &a,
                &[a.clone(), b.clone()],
                Vec::new()
            ),
            vec![a_new, b_mid, a_old]
        );
        let own = vec![root.join("1.jpg"), root.join("2.jpg")];
        assert_eq!(
            slideshow_paths(SlideshowSource::Custom, &a, &[], own.clone()),
            own
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
