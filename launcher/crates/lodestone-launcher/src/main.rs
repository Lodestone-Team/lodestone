use lodestone_launcher::{APP_NAME, assets::Assets, shell::LauncherShell};

fn main() {
    gpui_platform::application().with_assets(Assets).run(|cx| {
        lodestone_launcher::init(cx);
        let options = lodestone_launcher::window_options(APP_NAME, 1200., 760., cx);
        lodestone_launcher::open_window(options, LauncherShell::new, cx);
    });
}
