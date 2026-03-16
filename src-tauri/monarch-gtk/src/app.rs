use crate::context::AppContext;
use crate::theme::portal::setup_css_and_portals;
use crate::ui::window::build_ui;
use adw::prelude::*;

pub fn run() -> glib::ExitCode {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
    glib::log_set_handler(
        Some("Adwaita"),
        glib::LogLevels::LEVEL_WARNING,
        false,
        false,
        |_, _, message| {
            if message.contains("gtk-application-prefer-dark-theme") {
                return;
            }
            eprintln!("Adwaita-WARNING: {message}");
        },
    );

    adw::init().expect("libadwaita init failed");

    let app = adw::Application::builder()
        .application_id("io.github.monarch_store")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_startup(setup_css_and_portals);
    app.connect_activate(|app| match AppContext::new() {
        Ok(context) => build_ui(app, context),
        Err(error) => {
            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("MonARCH Store")
                .default_width(1080)
                .default_height(760)
                .build();
            let status = adw::StatusPage::builder()
                .icon_name("dialog-error-symbolic")
                .title("MonARCH could not start")
                .description(&error)
                .build();
            window.set_content(Some(&status));
            window.present();
        }
    });

    app.run()
}
