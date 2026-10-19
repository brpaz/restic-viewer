//! "Save As…": writes one file, or one directory packed as a tar/zip archive, from a
//! Snapshot straight to a file the user picks — no Restore Target Directory involved.

use std::path::PathBuf;

use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use restic_client::{
    ArchiveFormat, Entry, EntryKind, RealResticClient, RepositoryConnection, ResticClient,
    ResticError, RestoreControl, SaveRequest,
};

pub fn present(
    parent: &adw::ApplicationWindow,
    connection: RepositoryConnection,
    snapshot_id: String,
    entry: Entry,
) {
    let parent = parent.clone();
    glib::spawn_future_local(async move {
        let is_directory = entry.kind == EntryKind::Directory;

        let file_dialog = gtk4::FileDialog::builder()
            .title(gettext("Save As"))
            .initial_name(if is_directory {
                format!("{}.tar", entry.name)
            } else {
                entry.name.clone()
            })
            .build();
        if is_directory {
            let filters = gio::ListStore::new::<gtk4::FileFilter>();
            for (name, pattern) in [
                (gettext("Tar archive"), "*.tar"),
                (gettext("Zip archive"), "*.zip"),
            ] {
                let filter = gtk4::FileFilter::new();
                filter.set_name(Some(&name));
                filter.add_pattern(pattern);
                filters.append(&filter);
            }
            file_dialog.set_filters(Some(&filters));
        }

        let Ok(file) = file_dialog.save_future(Some(&parent)).await else {
            return; // dismissed
        };
        let Some(path) = file.path() else {
            return;
        };

        let (destination, archive) = if is_directory {
            let (destination, format) = archive_destination(path);
            (destination, Some(format))
        } else {
            (path, None)
        };
        run(
            &parent,
            connection,
            snapshot_id,
            entry.path,
            destination,
            archive,
        )
        .await;
    });
}

/// The chosen file name decides the archive format; names with neither extension get `.tar`.
fn archive_destination(path: PathBuf) -> (PathBuf, ArchiveFormat) {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("zip") => (path, ArchiveFormat::Zip),
        Some(ext) if ext.eq_ignore_ascii_case("tar") => (path, ArchiveFormat::Tar),
        _ => {
            let mut name = path.into_os_string();
            name.push(".tar");
            (PathBuf::from(name), ArchiveFormat::Tar)
        }
    }
}

async fn run(
    parent: &adw::ApplicationWindow,
    connection: RepositoryConnection,
    snapshot_id: String,
    path: String,
    destination: PathBuf,
    archive: Option<ArchiveFormat>,
) {
    let spinner = adw::Spinner::new();
    let dialog = adw::AlertDialog::new(
        Some(&gettext("Saving…")),
        Some(&destination.display().to_string()),
    );
    dialog.add_response("cancel", &gettext("Cancel"));
    dialog.set_close_response("cancel");
    dialog.set_extra_child(Some(&spinner));

    let control = RestoreControl::new();
    // Closing the dialog while the save runs cancels it; once it has finished this is a no-op.
    dialog.connect_response(None, {
        let control = control.clone();
        move |_, _| control.cancel()
    });
    dialog.present(Some(parent));

    let request = SaveRequest {
        snapshot_id,
        path,
        archive,
        destination: destination.clone(),
    };
    let result = RealResticClient::system()
        .save(&connection, &request, control)
        .await;

    match result {
        Ok(()) => {
            dialog.set_heading(Some(&gettext("Saved")));
            dialog.set_body(&destination.display().to_string());
        }
        Err(ResticError::Cancelled) => {
            dialog.close();
            return;
        }
        Err(err) => {
            dialog.set_heading(Some(&gettext("Couldn't Save")));
            dialog.set_body(&err.to_string());
        }
    }
    dialog.set_extra_child(None::<&gtk4::Widget>);
    dialog.set_response_label("cancel", &gettext("Close"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_picks_the_archive_format() {
        assert_eq!(
            archive_destination("/tmp/a.zip".into()),
            ("/tmp/a.zip".into(), ArchiveFormat::Zip)
        );
        assert_eq!(
            archive_destination("/tmp/a.TAR".into()),
            ("/tmp/a.TAR".into(), ArchiveFormat::Tar)
        );
        assert_eq!(
            archive_destination("/tmp/a".into()),
            ("/tmp/a.tar".into(), ArchiveFormat::Tar)
        );
    }
}
