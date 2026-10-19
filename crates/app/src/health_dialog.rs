//! The Repository Health dialog: size and content totals from `restic stats`, and a
//! read-only `restic check` the user can run (optionally re-reading all data) and cancel.

use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::{gettext, ngettext};
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use restic_client::{
    RealResticClient, RepositoryConnection, RepositoryStats, ResticClient, ResticError,
    RestoreControl,
};

const RESOURCE_PATH: &str = "/dev/brunopaz/ResticViewer/health_dialog.ui";

pub fn present(parent: &adw::ApplicationWindow, connection: RepositoryConnection) {
    let builder = gtk4::Builder::from_resource(RESOURCE_PATH);
    let dialog: adw::Dialog = builder.object("dialog").expect("dialog");
    let snapshots_row: adw::ActionRow = builder.object("snapshots_row").expect("snapshots_row");
    let files_row: adw::ActionRow = builder.object("files_row").expect("files_row");
    let snapshots_size_row: adw::ActionRow = builder
        .object("snapshots_size_row")
        .expect("snapshots_size_row");
    let stored_row: adw::ActionRow = builder.object("stored_row").expect("stored_row");
    let stats_error_label: gtk4::Label = builder
        .object("stats_error_label")
        .expect("stats_error_label");
    let read_data_row: adw::SwitchRow = builder.object("read_data_row").expect("read_data_row");
    let check_spinner: adw::Spinner = builder.object("check_spinner").expect("check_spinner");
    let check_status: gtk4::Label = builder.object("check_status").expect("check_status");
    let check_button: gtk4::Button = builder.object("check_button").expect("check_button");
    let check_output_scroller: gtk4::ScrolledWindow = builder
        .object("check_output_scroller")
        .expect("check_output_scroller");
    let check_output: gtk4::TextView = builder.object("check_output").expect("check_output");

    let control: Rc<RefCell<Option<RestoreControl>>> = Rc::new(RefCell::new(None));

    // Closing the dialog stops a running check instead of leaving restic running.
    dialog.connect_closed({
        let control = control.clone();
        move |_| {
            if let Some(control) = control.borrow().as_ref() {
                control.cancel();
            }
        }
    });

    glib::spawn_future_local({
        let connection = connection.clone();
        async move {
            match RealResticClient::system().stats(&connection).await {
                Ok(stats) => {
                    snapshots_row.set_subtitle(&stats.snapshot_count.to_string());
                    files_row.set_subtitle(&stats.file_count.to_string());
                    snapshots_size_row.set_subtitle(&glib::format_size(stats.snapshots_size));
                    stored_row.set_subtitle(&stored_description(&stats));
                }
                Err(err) => {
                    for row in [&snapshots_row, &files_row, &snapshots_size_row, &stored_row] {
                        row.set_subtitle("—");
                    }
                    stats_error_label.set_label(&err.to_string());
                    stats_error_label.set_visible(true);
                }
            }
        }
    });

    check_button.connect_clicked({
        let control = control.clone();
        move |button| {
            if let Some(running) = control.borrow().as_ref() {
                running.cancel();
                return;
            }

            let new_control = RestoreControl::new();
            *control.borrow_mut() = Some(new_control.clone());
            button.set_label(&gettext("Cancel"));
            button.remove_css_class("suggested-action");
            read_data_row.set_sensitive(false);
            check_spinner.set_visible(true);
            check_output_scroller.set_visible(false);
            check_status.remove_css_class("success");
            check_status.remove_css_class("error");
            check_status.set_label(&gettext("Checking…"));

            let read_data = read_data_row.is_active();
            let connection = connection.clone();
            let button = button.clone();
            let control = control.clone();
            let read_data_row = read_data_row.clone();
            let check_spinner = check_spinner.clone();
            let check_status = check_status.clone();
            let check_output = check_output.clone();
            let check_output_scroller = check_output_scroller.clone();
            glib::spawn_future_local(async move {
                let result = RealResticClient::system()
                    .check(&connection, read_data, new_control)
                    .await;

                match result {
                    Ok(report) if report.healthy => {
                        check_status.set_label(&gettext("No errors were found"));
                        check_status.add_css_class("success");
                    }
                    Ok(report) => {
                        check_status.set_label(&gettext("Problems were found"));
                        check_status.add_css_class("error");
                        check_output.buffer().set_text(&report.output);
                        check_output_scroller.set_visible(true);
                    }
                    Err(ResticError::Cancelled) => {
                        check_status.set_label(&gettext("Check cancelled"));
                    }
                    Err(err) => {
                        check_status.set_label(&err.to_string());
                        check_status.add_css_class("error");
                    }
                }
                *control.borrow_mut() = None;
                check_spinner.set_visible(false);
                read_data_row.set_sensitive(true);
                button.set_label(&gettext("Check Repository"));
                button.add_css_class("suggested-action");
            });
        }
    });

    dialog.present(Some(parent));
}

/// "1.2 GB in 4,512 blocks": what the Repository occupies and how many deduplicated
/// chunks hold it.
fn stored_description(stats: &RepositoryStats) -> String {
    format!(
        "{} · {}",
        glib::format_size(stats.stored_size),
        ngettext(
            "{} data block",
            "{} data blocks",
            stats.blob_count.min(u64::from(u32::MAX)) as u32
        )
        .replace("{}", &stats.blob_count.to_string()),
    )
}
