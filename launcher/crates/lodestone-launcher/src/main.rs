use gpui::{
    AppContext as _, Bounds, Context, IntoElement, Render, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, px, size,
};

struct Blank;

impl Render for Blank {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn main() {
    gpui_platform::application().run(|cx| {
        let bounds = Bounds::centered(None, size(px(1200.), px(760.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Lodestone Launcher".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        cx.open_window(options, |_, cx| cx.new(|_| Blank))
            .expect("could not open the launcher window");
        cx.activate(true);
    });
}
