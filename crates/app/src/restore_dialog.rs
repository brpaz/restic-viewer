//! The Restore dialog: pick a Target Directory (or restore to the original location),
//! then run the restore with live progress and a Cancel button.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gettextrs::{gettext, ngettext};
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use restic_client::{
    RealResticClient, RepositoryConnection, ResticClient, ResticError, RestoreControl,
    RestoreRequest,
};

const RESOURCE_PATH: &str = "/dev/brunopaz/ResticViewer/restore_dialog.ui";

/// Presents the Restore dialog for either a whole Snapshot (`include_paths` empty) or a
/// specific set of selected Entries (`include_paths` non-empty).
pub fn present(
    parent: &adw::ApplicationWindow,
    connection: RepositoryConnection,
    snapshot_id: String,
    include_paths: Vec<String>,
    title: &str,
) {
    let builder = gtk4::Builder::from_resource(RESOURCE_PATH);
    let dialog: adw::Dialog = builder.object("dialog").expect("dialog");
    let window_title: adw::WindowTitle = builder.object("window_title").expect("window_title");
    let group: adw::PreferencesGroup = builder.object("group").expect("group");
    let target_row: adw::ActionRow = builder.object("target_row").expect("target_row");
    let original_location_row: adw::SwitchRow = builder
        .object("original_location_row")
        .expect("original_location_row");
    let include_row: adw::EntryRow = builder.object("include_row").expect("include_row");
    let exclude_row: adw::EntryRow = builder.object("exclude_row").expect("exclude_row");
    let verify_row: adw::SwitchRow = builder.object("verify_row").expect("verify_row");
    let progress_bar: gtk4::ProgressBar = builder.object("progress_bar").expect("progress_bar");
    let status_label: gtk4::Label = builder.object("status_label").expect("status_label");
    let error_label: gtk4::Label = builder.object("error_label").expect("error_label");
    let cancel_button: gtk4::Button = builder.object("cancel_button").expect("cancel_button");
    let start_button: gtk4::Button = builder.object("start_button").expect("start_button");

    dialog.set_title(title);
    window_title.set_title(title);

    let chosen_target: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

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

    let selected_count = include_paths.len();
    let run_restore: Rc<dyn Fn(&gtk4::Button)> = Rc::new({
        let dialog = dialog.clone();
        let group = group.clone();
        let progress_bar = progress_bar.clone();
        let status_label = status_label.clone();
        let error_label = error_label.clone();
        let cancel_button = cancel_button.clone();
        let original_location_row = original_location_row.clone();
        let chosen_target = chosen_target.clone();
        let control = control.clone();
        let include_row = include_row.clone();
        let exclude_row = exclude_row.clone();
        let verify_row = verify_row.clone();
        move |start_button: &gtk4::Button| {
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
                include_patterns: split_patterns(&include_row.text()),
                exclude_patterns: split_patterns(&exclude_row.text()),
                verify: verify_row.is_active(),
                target,
            };
            let connection = connection.clone();
            let dialog = dialog.clone();
            let progress_bar = progress_bar.clone();
            let status_label = status_label.clone();
            let error_label = error_label.clone();
            let cancel_button = cancel_button.clone();
            let control = control.clone();

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
                *control.borrow_mut() = None;
                let _ = &dialog;
            });
        }
    });

    start_button.connect_clicked({
        let dialog = dialog.clone();
        let original_location_row = original_location_row.clone();
        move |button| {
            if !original_location_row.is_active() {
                run_restore(button);
                return;
            }
            let run_restore = run_restore.clone();
            let button = button.clone();
            confirm_overwrite(&dialog, selected_count, move || run_restore(&button));
        }
    });

    dialog.present(Some(parent));
}

/// Asks before a Restore that writes over files at their original paths, which cannot be
/// undone from within the app.
fn confirm_overwrite(dialog: &adw::Dialog, selected_count: usize, on_confirm: impl Fn() + 'static) {
    let alert = adw::AlertDialog::new(
        Some(&gettext("Overwrite Original Files?")),
        Some(&overwrite_warning(selected_count)),
    );
    alert.add_responses(&[
        ("cancel", &gettext("Cancel")),
        ("overwrite", &gettext("Restore and Overwrite")),
    ]);
    alert.set_response_appearance("overwrite", adw::ResponseAppearance::Destructive);
    alert.set_default_response(Some("cancel"));
    alert.set_close_response("cancel");
    alert.connect_response(None, move |_, response| {
        if response == "overwrite" {
            on_confirm();
        }
    });
    alert.present(Some(dialog));
}

/// `selected_count` is the number of chosen Entries; zero means the whole Snapshot.
fn overwrite_warning(selected_count: usize) -> String {
    let subject = if selected_count == 0 {
        gettext("the entire Snapshot")
    } else {
        ngettext(
            "{} selected item",
            "{} selected items",
            selected_count.min(u32::MAX as usize) as u32,
        )
        .replace("{}", &selected_count.to_string())
    };
    gettext("Restoring {} to its original location replaces any files already at those paths. This can't be undone.")
        .replace("{}", &subject)
}

/// Splits a comma-separated list of restic patterns, dropping blanks.
fn split_patterns(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{overwrite_warning, split_patterns};

    #[test]
    fn patterns_are_trimmed_and_blanks_dropped() {
        assert_eq!(
            split_patterns(" *.tmp, ,node_modules ,"),
            vec!["*.tmp", "node_modules"]
        );
        assert!(split_patterns("  ").is_empty());
    }

    #[test]
    fn the_warning_names_what_will_be_restored() {
        assert!(overwrite_warning(0).contains("the entire Snapshot"));
        assert!(overwrite_warning(1).contains("1 selected item to"));
        assert!(overwrite_warning(3).contains("3 selected items"));
    }
}
