use crate::context::AppContext;
use adw::prelude::*;
use monarch_core::privileged::{ClassifiedError, HelperProgress};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Options for the operation dialog (install vs uninstall, success actions).
#[derive(Default)]
pub struct OperationDialogOptions {
    pub is_uninstall: bool,
    /// Display name shown on success (e.g. "Amberol").
    pub success_display_name: Option<String>,
    /// Called when user clicks "Launch app" on success. Dialog closes after.
    pub on_launch: Option<Box<dyn FnOnce()>>,
}

/// Returns current step 1..=4 from status message (Resolve/Download/Install/Done).
fn current_step_from_message(message: &str, fraction: f64) -> u8 {
    let s = message.to_lowercase();
    if s.contains("complete") || s.contains("success") || fraction >= 0.99 {
        return 4;
    }
    if s.contains("resolv") || s.contains("sync") || s.contains("lock") || s.contains("initializ") || s.contains("refresh") {
        return 1;
    }
    if s.contains("download") || s.contains("fetch") || s.contains("extract") {
        return 2;
    }
    if s.contains("install") || s.contains("commit") || s.contains("finaliz") || s.contains("housekeep") {
        return 3;
    }
    if fraction > 0.0 && fraction < 0.9 {
        return 2;
    }
    if fraction >= 0.9 {
        return 3;
    }
    1
}

fn timestamp() -> String {
    let now = std::time::SystemTime::now();
    let since_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = since_epoch.as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

pub fn present_operation_dialog<F>(
    context: AppContext,
    title: &str,
    initial_status: &str,
    receiver: tokio::sync::mpsc::Receiver<HelperProgress>,
    on_finish: F,
    options: OperationDialogOptions,
) where
    F: Fn(Result<(), String>) + 'static,
{
    let window = adw::Window::builder()
        .title(title)
        .default_width(640)
        .default_height(480)
        .modal(true)
        .build();

    let progress_bar = gtk::ProgressBar::builder()
        .show_text(true)
        .fraction(0.0)
        .build();
    progress_bar.pulse();

    let status_label = gtk::Label::builder()
        .label(initial_status)
        .xalign(0.0)
        .wrap(true)
        .css_classes(vec!["body".to_string()])
        .build();

    // Stepper: Resolve → Download → Install → Done
    let step_labels = ["Resolve", "Download", "Install", "Done"];
    let stepper_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();
    let step_labels_ref: Vec<gtk::Label> = step_labels
        .iter()
        .map(|l| gtk::Label::builder().label(*l).css_classes(vec!["caption-heading".to_string()]).build())
        .collect();
    for (i, lbl) in step_labels_ref.iter().enumerate() {
        stepper_box.append(lbl);
        if i < step_labels_ref.len() - 1 {
            let sep = gtk::Label::builder().label("→").build();
            sep.add_css_class("dim-label");
            stepper_box.append(&sep);
        }
    }

    let log_buffer = gtk::TextBuffer::new(None);
    let log_view = gtk::TextView::builder()
        .buffer(&log_buffer)
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .css_classes(vec!["monarch-log-view".to_string()])
        .build();
    let log_scroller = gtk::ScrolledWindow::builder()
        .child(&log_view)
        .vexpand(true)
        .min_content_height(120)
        .build();
    let expander = gtk::Expander::builder()
        .label("Transaction log")
        .expanded(true)
        .child(&log_scroller)
        .build();

    let cancel_button = gtk::Button::builder()
        .label("Cancel")
        .css_classes(vec!["destructive-action".to_string()])
        .halign(gtk::Align::End)
        .build();
    let close_button = gtk::Button::builder()
        .label("Done")
        .halign(gtk::Align::End)
        .sensitive(false)
        .build();
    let launch_button = gtk::Button::builder()
        .label("Launch app")
        .halign(gtk::Align::End)
        .visible(false)
        .build();
    let recovery_button = gtk::Button::builder()
        .label("Run Recovery")
        .halign(gtk::Align::End)
        .visible(false)
        .build();

    let button_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .halign(gtk::Align::End)
        .build();
    button_row.append(&cancel_button);
    button_row.append(&recovery_button);
    button_row.append(&launch_button);
    button_row.append(&close_button);

    let progress_content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    progress_content.append(&progress_bar);
    progress_content.append(&stepper_box);
    progress_content.append(&status_label);
    progress_content.append(&expander);

    // Success view (icon + text only; buttons stay in shared button_row)
    let success_icon = gtk::Image::builder()
        .icon_name("emblem-ok-symbolic")
        .pixel_size(64)
        .build();
    let success_title = gtk::Label::builder()
        .label(if options.is_uninstall {
            "Uninstallation complete"
        } else {
            "Installation complete"
        })
        .css_classes(vec!["title-1".to_string()])
        .build();
    let success_subtitle = gtk::Label::builder()
        .label(options.success_display_name.as_deref().unwrap_or(""))
        .css_classes(vec!["body".to_string()])
        .build();
    let success_content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();
    success_content.append(&success_icon);
    success_content.append(&success_title);
    success_content.append(&success_subtitle);

    let main_stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();
    main_stack.add_named(&progress_content, Some("progress"));
    main_stack.add_named(&success_content, Some("success"));

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .css_classes(vec!["monarch-progress-card".to_string()])
        .build();
    content.append(&main_stack);
    content.append(&button_row);
    window.set_content(Some(&content));

    let finished = Rc::new(Cell::new(false));
    let fraction = Rc::new(Cell::new(0.0f64));
    let status_message = Rc::new(RefCell::new(initial_status.to_string()));
    let log_lines: Rc<RefCell<Vec<(String, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let on_finish = Rc::new(on_finish);
    let classified_error = Rc::new(RefCell::new(None::<ClassifiedError>));
    let current_step = Rc::new(Cell::new(1u8));

    fn update_stepper_style(step_labels: &[gtk::Label], current: u8) {
        for (i, lbl) in step_labels.iter().enumerate() {
            let step = (i + 1) as u8;
            lbl.remove_css_class("success-step");
            lbl.remove_css_class("current-step");
            if step < current {
                lbl.add_css_class("success-step");
            } else if step == current {
                lbl.add_css_class("current-step");
            }
        }
    }
    update_stepper_style(&step_labels_ref, 1);

    close_button.connect_clicked({
        let window = window.clone();
        move |_| window.close()
    });

    let on_launch_cell = Rc::new(RefCell::new(options.on_launch));
    launch_button.connect_clicked({
        let window = window.clone();
        let on_launch_cell = on_launch_cell.clone();
        move |_| {
            if let Some(f) = on_launch_cell.borrow_mut().take() {
                f();
            }
            window.close();
        }
    });

    window.connect_close_request({
        let finished = finished.clone();
        let status_label = status_label.clone();
        move |_| {
            if finished.get() {
                glib::Propagation::Proceed
            } else {
                status_label.set_label("Use Cancel to stop the current transaction before closing this window.");
                glib::Propagation::Stop
            }
        }
    });

    let (cancel_tx, cancel_rx) = std::sync::mpsc::channel();
    cancel_button.connect_clicked({
        let cancel_button = cancel_button.clone();
        let status_label = status_label.clone();
        let context = context.clone();
        move |_| {
            cancel_button.set_sensitive(false);
            status_label.set_label("Requesting cancellation and repairing the pacman lock...");
            context.runtime.spawn({
                let catalog = context.catalog.clone();
                let cancel_tx = cancel_tx.clone();
                async move {
                    let _ = cancel_tx.send(catalog.cancel_active_operation().await);
                }
            });
        }
    });

    recovery_button.connect_clicked({
        let context = context.clone();
        let recovery_button = recovery_button.clone();
        let status_label = status_label.clone();
        let classified_error = classified_error.clone();
        move |_| {
            let Some(classified) = classified_error.borrow().clone() else {
                return;
            };
            let Some(action) = classified.recovery_action.clone() else {
                return;
            };

            recovery_button.set_sensitive(false);
            status_label.set_label(&format!("Running recovery: {}...", action));
            let (sender, receiver) = std::sync::mpsc::channel();
            context.runtime.spawn({
                let catalog = context.catalog.clone();
                async move {
                    let result = match action.as_str() {
                        "UnlockDatabase" | "RemoveLockAndSync" => catalog.repair_unlock_pacman().await,
                        "RepairKeyring" => catalog.refresh_keyring().await,
                        "ForceRefreshDb" | "RefreshMirrors" | "UpdateAndInstall" => {
                            catalog.force_refresh_databases().await
                        }
                        "CleanCache" => catalog.clear_pacman_cache().await,
                        _ => Err("This failure requires a manual recovery step outside the current dialog.".to_string()),
                    };
                    let _ = sender.send(result);
                }
            });

            let recovery_button_for_result = recovery_button.clone();
            let status_for_result = status_label.clone();
            glib::source::timeout_add_local(std::time::Duration::from_millis(30), move || {
                match receiver.try_recv() {
                    Ok(Ok(message)) => {
                        status_for_result.set_label(&message);
                        recovery_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                    Ok(Err(error)) => {
                        status_for_result.set_label(&error);
                        recovery_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        recovery_button_for_result.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                }
            });
        }
    });

    let poll_receiver = Rc::new(RefCell::new(receiver));
    let poll_receiver_for_timeout = poll_receiver.clone();
    let step_labels_for_timeout = step_labels_ref.clone();
    let on_launch_cell_for_timeout = on_launch_cell.clone();
    glib::source::timeout_add_local(std::time::Duration::from_millis(60), move || {
        let mut should_break = false;

        loop {
            match poll_receiver_for_timeout.borrow_mut().try_recv() {
                Ok(HelperProgress::Message { message, percent }) => {
                    if !message.trim().is_empty() {
                        status_label.set_label(&message);
                        status_message.borrow_mut().clone_from(&message);
                        let ts = timestamp();
                        log_lines.borrow_mut().push((ts.clone(), message.clone()));
                        let formatted: String = log_lines
                            .borrow()
                            .iter()
                            .map(|(t, m)| format!("[{}] {}", t, m))
                            .collect::<Vec<_>>()
                            .join("\n");
                        log_buffer.set_text(&formatted);
                    }

                    if let Some(percent) = percent {
                        let next_fraction = (percent as f64 / 100.0).clamp(0.0, 1.0);
                        fraction.set(next_fraction);
                        progress_bar.set_fraction(next_fraction);
                        progress_bar.set_text(Some(&format!("{percent}%")));
                    } else {
                        progress_bar.pulse();
                    }

                    let step = current_step_from_message(&status_message.borrow(), fraction.get());
                    current_step.set(step);
                    update_stepper_style(&step_labels_for_timeout, step);
                }
                Ok(HelperProgress::ClassifiedError(error)) => {
                    let recovery_label = recovery_label(&error);
                    classified_error.borrow_mut().replace(error.clone());
                    if let Some(label) = recovery_label {
                        recovery_button.set_label(&label);
                        recovery_button.set_visible(true);
                        recovery_button.set_sensitive(true);
                    }
                    status_label.set_label(&error.description);
                    let ts = timestamp();
                    log_lines.borrow_mut().push((ts, error.raw_message.clone()));
                    let formatted: String = log_lines
                        .borrow()
                        .iter()
                        .map(|(t, m)| format!("[{}] {}", t, m))
                        .collect::<Vec<_>>()
                        .join("\n");
                    log_buffer.set_text(&formatted);
                }
                Ok(HelperProgress::Finished(result)) => {
                    finished.set(true);
                    cancel_button.set_sensitive(false);
                    close_button.set_sensitive(true);
                    match &result {
                        Ok(_) => {
                            progress_bar.set_fraction(1.0);
                            progress_bar.set_text(Some("100%"));
                            current_step.set(4);
                            update_stepper_style(&step_labels_for_timeout, 4);
                            main_stack.set_visible_child_name("success");
                            cancel_button.set_visible(false);
                            recovery_button.set_visible(false);
                            if let Some(name) = &options.success_display_name {
                                success_subtitle.set_label(name);
                                success_subtitle.set_visible(true);
                            } else {
                                success_subtitle.set_visible(false);
                            }
                            if !options.is_uninstall && on_launch_cell_for_timeout.borrow().is_some() {
                                launch_button.set_visible(true);
                                launch_button.set_sensitive(true);
                            }
                            (*on_finish)(Ok(()));
                        }
                        Err(error) => {
                            progress_bar.set_fraction(fraction.get());
                            progress_bar.set_text(Some("Failed"));
                            let final_error = classified_error
                                .borrow()
                                .as_ref()
                                .map(|classified| classified.description.clone())
                                .unwrap_or_else(|| error.clone());
                            status_label.set_label(&final_error);
                            (*on_finish)(Err(final_error));
                        }
                    }
                    should_break = true;
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    finished.set(true);
                    cancel_button.set_sensitive(false);
                    close_button.set_sensitive(true);
                    status_label.set_label("The helper stream disconnected unexpectedly.");
                    (*on_finish)(Err("The helper stream disconnected unexpectedly.".to_string()));
                    should_break = true;
                    break;
                }
            }
        }

        match cancel_rx.try_recv() {
            Ok(Ok(())) => {
                status_label.set_label("Cancellation requested. Waiting for the helper to stop...");
            }
            Ok(Err(error)) => {
                status_label.set_label(&error);
                cancel_button.set_sensitive(true);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
        }

        if should_break {
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });

    window.present();
}

fn recovery_label(error: &ClassifiedError) -> Option<String> {
    match error.recovery_action.as_deref()? {
        "UnlockDatabase" | "RemoveLockAndSync" => Some("Unlock Database".to_string()),
        "RepairKeyring" => Some("Refresh Keyrings".to_string()),
        "ForceRefreshDb" | "RefreshMirrors" | "UpdateAndInstall" => {
            Some("Refresh Databases".to_string())
        }
        "CleanCache" => Some("Clear Cache".to_string()),
        _ => None,
    }
}
