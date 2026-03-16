use crate::context::AppContext;
use adw::prelude::*;

pub fn parent_window_for(widget: &impl IsA<gtk::Widget>) -> Option<gtk::Window> {
    widget.root().and_then(|root| root.downcast::<gtk::Window>().ok())
}

pub fn ensure_session_auth(
    context: &AppContext,
    parent: Option<&gtk::Window>,
    force_prompt: bool,
) -> Result<(), String> {
    let settings = context.settings.load()?;
    if !(force_prompt || settings.one_click_enabled || settings.reduce_password_prompts) {
        return Ok(());
    }
    if context.catalog.has_session_password()? {
        return Ok(());
    }

    let dialog = gtk::Dialog::builder()
        .title("MonARCH One-Click Authentication")
        .modal(true)
        .resizable(false)
        .default_width(420)
        .build();
    if let Some(parent) = parent {
        dialog.set_transient_for(Some(parent));
    }
    dialog.add_button("Use system prompt", gtk::ResponseType::Cancel);
    dialog.add_button("Use for session", gtk::ResponseType::Accept);
    dialog.set_default_response(gtk::ResponseType::Accept);

    let content = dialog.content_area();
    content.set_spacing(16);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let hero = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    hero.append(
        &gtk::Image::builder()
            .icon_name("dialog-password-symbolic")
            .pixel_size(28)
            .build(),
    );
    let copy = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();
    copy.append(
        &gtk::Label::builder()
            .label("Unlock MonARCH once for this session")
            .xalign(0.0)
            .css_classes(vec!["title-4".to_string()])
            .build(),
    );
    copy.append(
        &gtk::Label::builder()
            .label("Your password stays in memory only for this app session. If you prefer, choose the system prompt instead and MonARCH will fall back to Polkit.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(vec!["dim-label".to_string()])
            .build(),
    );
    hero.append(&copy);
    content.append(&hero);

    let password_entry = gtk::PasswordEntry::builder()
        .placeholder_text("System password")
        .show_peek_icon(true)
        .activates_default(true)
        .build();
    content.append(&password_entry);

    let result = std::rc::Rc::new(std::cell::RefCell::new(None::<Option<String>>));
    let loop_ = glib::MainLoop::new(None, false);
    dialog.connect_response({
        let result = result.clone();
        let loop_ = loop_.clone();
        let password_entry = password_entry.clone();
        move |dialog, response| {
            let value = match response {
                gtk::ResponseType::Accept => password_entry
                    .text()
                    .trim()
                    .to_string()
                    .into(),
                _ => None,
            };
            result.replace(Some(value));
            dialog.hide();
            loop_.quit();
        }
    });

    dialog.present();
    loop_.run();
    dialog.close();

    let password = result
        .borrow_mut()
        .take()
        .flatten()
        .filter(|value| !value.trim().is_empty());
    context.catalog.set_session_password(password)?;
    Ok(())
}
