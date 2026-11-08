// Release builds on Windows are GUI apps, so starting the launcher doesn't open an
// empty console window next to it. Debug builds keep the console for their output;
// without one, writes to stderr are dropped rather than failing.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use lodestone_launcher::{
    APP_NAME, accounts, assets::Assets, images, images::StartupTrace, shell::LauncherShell,
};

fn main() {
    let trace = StartupTrace::start();
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx| {
            // Decode the bundled images first, in the background, while the rest of
            // the app starts; the window opens once they are ready, so its first frame
            // has them.
            let decoding = images::decode(cx);
            lodestone_launcher::init(cx);
            let options = lodestone_launcher::window_options(APP_NAME, 1200., 760., cx);

            // `--scene <name>` shows a prepared state with made-up accounts, for the
            // offscreen screenshots. It never touches the Keychain or the network.
            #[cfg(feature = "screenshot")]
            if let Some(scene) = lodestone_launcher::preview::requested_scene() {
                if let Some(locale) = lodestone_launcher::preview::requested_locale() {
                    lodestone_launcher::i18n::force(locale);
                }
                let width = lodestone_launcher::preview::window_width(&scene);
                let height = lodestone_launcher::preview::window_height(&scene);
                let options = match (width, height) {
                    (None, None) => options,
                    _ => lodestone_launcher::window_options(
                        APP_NAME,
                        width.unwrap_or(1200.),
                        height.unwrap_or(760.),
                        cx,
                    ),
                };
                let accounts = lodestone_launcher::preview::accounts(&scene, cx);
                let instances = lodestone_launcher::preview::instances(&scene, &accounts, cx);
                if let Some(zoom) = lodestone_launcher::preview::requested_zoom() {
                    lodestone_launcher::theme::zoom::set(zoom);
                }
                cx.spawn(async move |cx| {
                    let decoded = decoding.await;
                    cx.update(|cx| {
                        images::install(decoded, cx);
                        lodestone_launcher::open_window(
                            options,
                            move |cx| {
                                let mut shell = LauncherShell::new(accounts, instances, cx);
                                lodestone_launcher::preview::apply(&scene, &mut shell, cx);
                                shell
                            },
                            cx,
                        );
                    });
                })
                .detach();
                return;
            }

            let accounts = accounts::create(cx);
            let instances = lodestone_launcher::instances::create(&accounts, cx);
            cx.spawn(async move |cx| {
                let decoded = decoding.await;
                trace.mark("bundled images decoded");
                cx.update(|cx| {
                    images::install(decoded, cx);
                    let window = lodestone_launcher::open_window(
                        options,
                        move |cx| LauncherShell::new(accounts, instances, cx),
                        cx,
                    );
                    trace.mark("window opened");
                    // The socket lodestone-launcher-cli talks to (Unix only, for now).
                    #[cfg(unix)]
                    window
                        .update(cx, |shell, _, cx| {
                            lodestone_launcher::control::start(shell, cx)
                        })
                        .ok();
                    let trace = trace.clone();
                    window
                        .update(cx, |_, window, _| {
                            window.on_next_frame(move |_, _| trace.mark("first frame drawn"));
                        })
                        .ok();
                });
            })
            .detach();
        });
}
