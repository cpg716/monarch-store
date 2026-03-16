mod app;
mod context;
mod telemetry;
mod theme;
mod ui;

fn main() -> glib::ExitCode {
    app::run()
}
