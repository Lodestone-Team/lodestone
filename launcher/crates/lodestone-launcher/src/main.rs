// Release builds on Windows are GUI apps, so starting the launcher doesn't open an
// empty console window next to it. Debug builds keep the console for their output;
// without one, writes to stderr are dropped rather than failing.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use lodestone_launcher::{
    APP_NAME, accounts, assets::Assets, images, images::StartupTrace, shell::LauncherShell,
};

fn main() {
    // Before anything can rename the executable: an update swaps it on disk.
    launcher_core::update::remember_exe();
    // Started by an update's restart: let the old launcher finish quitting first.
    launcher_core::update::wait_for_previous();
    // An invite link Windows started this launcher for (`lodestone://join?…`).
    let link = lodestone_launcher::deep_link::from_args(std::env::args_os().skip(1));
    // Windows: a launcher already running is brought forward instead, and gets the
    // link. The offscreen screenshots use made-up data, so they may run beside it.
    #[cfg(not(feature = "screenshot"))]
    if !lodestone_launcher::single_instance::acquire(APP_NAME) {
        if let Some(link) = &link
            && !lodestone_launcher::deep_link::forward(link)
        {
            eprintln!(
                "lodestone-launcher: couldn't pass the invite link to the running launcher (its command-line access may be off). Paste the link into it instead."
            );
        }
        return;
    }
    // A data folder move finished before this restart: remove the old folder, in
    // the background (it can be large). Not for the offscreen scenes, which use
    // made-up data.
    #[cfg(feature = "screenshot")]
    let scene = lodestone_launcher::preview::requested_scene().is_some();
    #[cfg(not(feature = "screenshot"))]
    let scene = false;
    if !scene {
        std::thread::spawn(lodestone_launcher::shell::data_folder::finish_pending_move);
    }
    let trace = StartupTrace::start();
    // Links come from the command line and, on macOS, from the system.
    let links = lodestone_launcher::deep_link::Links::default();
    if let Some(link) = link {
        let _ = links.sender().unbounded_send(link);
    }
    let app = gpui_platform::application().with_assets(Assets);
    let sender = links.sender();
    app.on_open_urls(move |urls| {
        for url in urls {
            let _ = sender.unbounded_send(url);
        }
    });
    app.run(move |cx| {
        trace.mark("app started");
        // Decode the bundled images first, in the background, while the rest of
        // the app starts; the window opens once they are ready, so its first frame
        // has them. Renders from an earlier start are read back from the cache,
        // which the offscreen scenes leave alone.
        let decoding = images::decode(cx, if scene { None } else { images::cache_dir() });
        lodestone_launcher::init(cx);
        trace.mark("launcher set up");
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
        trace.mark("accounts and instances loaded");
        // The coding-agent skills, in the data folder (Settings > Developer).
        // Not into a moved data folder that isn't there (the window says so).
        let data_dir = accounts.read(cx).data_dir().clone();
        if lodestone_launcher::shell::data_folder::missing_data_folder().is_none() {
            gpui::AppContext::background_spawn(cx, async move {
                if let Err(err) = lodestone_launcher::agent_skills::install(&data_dir) {
                    eprintln!("lodestone-launcher: couldn't write the agent skills: {err}");
                }
            })
            .detach();
        }
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
                // The socket lodestone-launcher-cli talks to.
                window
                    .update(cx, |shell, _, cx| {
                        lodestone_launcher::control::start(shell, cx)
                    })
                    .ok();
                // "Keep the narrator off", offered until it's answered.
                window
                    .update(cx, |shell, _, cx| shell.offer_narrator(cx))
                    .ok();
                // A moved data folder that isn't there (a drive not connected).
                window
                    .update(cx, |shell, _, cx| shell.check_data_folder(cx))
                    .ok();
                // Invite links opened before or after the window.
                links.serve(window, cx);
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
