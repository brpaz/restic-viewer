//! The Restore dialog: pick a Target Directory (or restore to the original location),
//! then run the restore with live progress and a Cancel button.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use restic_client::{
    RealResticClient, RepositoryConnection, ResticClient, ResticError, RestoreControl,
    RestoreRequest,
};

/// Presents the Restore dialog for either a whole Snapshot (`include_paths` empty) or a
/// specific set of selected Entries (`include_paths` non-empty).
pub fn present(
    parent: &adw::ApplicationWindow,
    connection: RepositoryConnection,
    snapshot_id: String,
    include_paths: Vec<String>,
    title: &str,
) {
    let chosen_target: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

    let target_row = adw::ActionRow::builder()
        .title("Target Directory")
        .subtitle("No folder selected")
        .activatable(true)
        .build();
    target_row.add_suffix(&gtk4::Image::from_icon_name("folder-symbolic"));

    let original_location_row = adw::SwitchRow::builder()
        .title("Restore to Original Location")
        .subtitle("Overwrites files at their original absolute path")
        .build();

    let group = adw::PreferencesGroup::new();
    group.add(&target_row);
    group.add(&original_location_row);

    let progress_bar = gtk4::ProgressBar::builder()
        .show_text(true)
        .visible(false)
        .build();
    let status_label = gtk4::Label::builder().wrap(true).visible(false).build();
    let error_label = gtk4::Label::builder()
        .css_classes(["error"])
        .wrap(true)
        .visible(false)
        .build();

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    let page = adw::PreferencesPage::new();
    page.add(&group);
    content.append(&page);
    content.append(&progress_bar);
    content.append(&status_label);
    content.append(&error_label);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(
        &adw::HeaderBar::builder()
            .title_widget(&adw::WindowTitle::new(title, ""))
            .build(),
    );
    toolbar_view.set_content(Some(&content));

    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(440)
        .child(&toolbar_view)
        .can_close(true)
        .build();

    let cancel_button = gtk4::Button::builder().label("Cancel").build();
    let start_button = gtk4::Button::builder()
        .label("Restore")
        .css_classes(["suggested-action"])
        .sensitive(false)
        .build();

    let bottom_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    bottom_bar.set_margin_top(8);
    bottom_bar.set_margin_bottom(8);
    bottom_bar.set_margin_start(12);
    bottom_bar.set_margin_end(12);
    bottom_bar.append(&cancel_button);
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    bottom_bar.append(&spacer);
    bottom_bar.append(&start_button);
    toolbar_view.add_bottom_bar(&bottom_bar);

    let update_sensitivity = {
        let original_location_row = original_location_row.clone();
        let chosen_target = chosen_target.clone();
        let start_button = start_button.clone();
        move || {
            let ready = original_location_row.is_active() || chosen_target.borrow().is_some();
            start_button.set_sensitive(ready);
        }
    };

    original_location_row.connect_active_notify({
        let target_row = target_row.clone();
        let update_sensitivity = update_sensitivity.clone();
        move |row| {
            target_row.set_sensitive(!row.is_active());
            update_sensitivity();
        }
    });

    target_row.connect_activated({
        let parent = parent.clone();
        let chosen_target = chosen_target.clone();
        let target_row = target_row.clone();
        let update_sensitivity = update_sensitivity.clone();
        move |_| {
            let file_dialog = gtk4::FileDialog::builder()
                .title("Choose Target Directory")
                .build();
            file_dialog.set_initial_folder(Some(&gio::File::for_path(glib::home_dir())));
            let parent = parent.clone();
            let chosen_target = chosen_target.clone();
            let target_row = target_row.clone();
            let update_sensitivity = update_sensitivity.clone();
            glib::spawn_future_local(async move {
                if let Ok(file) = file_dialog.select_folder_future(Some(&parent)).await
                    && let Some(path) = file.path()
                {
                    target_row.set_subtitle(&path.display().to_string());
                    *chosen_target.borrow_mut() = Some(path);
                    update_sensitivity();
                }
            });
        }
    });

    let control: Rc<RefCell<Option<RestoreControl>>> = Rc::new(RefCell::new(None));

    cancel_button.connect_clicked({
        let dialog = dialog.clone();
        let control = control.clone();
        move |_| {
            if let Some(control) = control.borrow().as_ref() {
                control.cancel();
            } else {
                dialog.close();
            }
        }
    });

    start_button.connect_clicked({
        let dialog = dialog.clone();
        let group = group.clone();
        let progress_bar = progress_bar.clone();
        let status_label = status_label.clone();
        let error_label = error_label.clone();
        let cancel_button = cancel_button.clone();
        let original_location_row = original_location_row.clone();
        let chosen_target = chosen_target.clone();
        let control = control.clone();
        move |start_button| {
            let target = if original_location_row.is_active() {
                PathBuf::from("/")
            } else {
                let Some(target) = chosen_target.borrow().clone() else {
                    return;
                };
                target
            };

            start_button.set_visible(false);
            group.set_sensitive(false);
            error_label.set_visible(false);
            progress_bar.set_visible(true);
            status_label.set_visible(true);
            status_label.set_label("Starting…");
            cancel_button.set_label("Cancel Restore");

            let new_control = RestoreControl::new();
            *control.borrow_mut() = Some(new_control.clone());

            let request = RestoreRequest {
                snapshot_id: snapshot_id.clone(),
                include_paths: include_paths.clone(),
                target,
            };
            let connection = connection.clone();
            let dialog = dialog.clone();
            let progress_bar = progress_bar.clone();
            let status_label = status_label.clone();
            let error_label = error_label.clone();
            let cancel_button = cancel_button.clone();

            glib::spawn_future_local(async move {
                let progress_bar_cb = progress_bar.clone();
                let status_label_cb = status_label.clone();
                let result = RealResticClient::system()
                    .restore(
                        &connection,
                        &request,
                        {
                            move |progress| {
                                progress_bar_cb.set_fraction(progress.percent_done.clamp(0.0, 1.0));
                                status_label_cb.set_label(&format!(
                                    "{} of {} files restored",
                                    progress.files_restored, progress.total_files
                                ));
                            }
                        },
                        new_control,
                    )
                    .await;

                match result {
                    Ok(outcome) => {
                        status_label
                            .set_label(&format!("Restored {} files.", outcome.files_restored));
                        progress_bar.set_fraction(1.0);
                        cancel_button.set_label("Close");
                    }
                    Err(ResticError::Cancelled) => {
                        status_label.set_label("Restore cancelled.");
                        cancel_button.set_label("Close");
                    }
                    Err(err) => {
                        status_label.set_visible(false);
                        error_label.set_label(&err.to_string());
                        error_label.set_visible(true);
                        cancel_button.set_label("Close");
                    }
                }
                let _ = &dialog;
            });
        }
    });

    dialog.present(Some(parent));
}
