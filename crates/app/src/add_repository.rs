//! The "Add Repository" dialog: pick a Backend (Local, SFTP, or S3-compatible), enter
//! a name and Repository Password, validate against the restic client seam, then
//! persist.
//!
//! SFTP has no credential field: restic's SFTP backend shells out to the system SSH
//! client and has no password mechanism of its own — auth relies entirely on SSH
//! key/agent setup already on the host, matching restic's own documented design.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use restic_client::{Backend, RealResticClient, RepositoryConnection, ResticClient};

use crate::config::{RepositoryEntry, RepositoryStore};
use crate::keyring::{self, CredentialKind};

const BACKEND_LOCAL: u32 = 0;
const BACKEND_SFTP: u32 = 1;
const BACKEND_S3: u32 = 2;

const RESOURCE_PATH: &str = "/dev/brunopaz/ResticViewer/add_repository.ui";

pub fn present(
    parent: &adw::ApplicationWindow,
    store: Rc<RepositoryStore>,
    on_added: impl Fn(RepositoryEntry) + 'static,
) {
    let on_added = Rc::new(on_added);

    let builder = gtk4::Builder::from_resource(RESOURCE_PATH);
    let dialog: adw::Dialog = builder.object("dialog").expect("dialog");
    let name_row: adw::EntryRow = builder.object("name_row").expect("name_row");
    let backend_row: adw::ComboRow = builder.object("backend_row").expect("backend_row");
    let local_group: adw::PreferencesGroup = builder.object("local_group").expect("local_group");
    let location_row: adw::ActionRow = builder.object("location_row").expect("location_row");
    let sftp_group: adw::PreferencesGroup = builder.object("sftp_group").expect("sftp_group");
    let host_row: adw::EntryRow = builder.object("host_row").expect("host_row");
    let sftp_path_row: adw::EntryRow = builder.object("sftp_path_row").expect("sftp_path_row");
    let user_row: adw::EntryRow = builder.object("user_row").expect("user_row");
    let s3_group: adw::PreferencesGroup = builder.object("s3_group").expect("s3_group");
    let endpoint_row: adw::EntryRow = builder.object("endpoint_row").expect("endpoint_row");
    let bucket_row: adw::EntryRow = builder.object("bucket_row").expect("bucket_row");
    let access_key_row: adw::EntryRow = builder.object("access_key_row").expect("access_key_row");
    let secret_key_row: adw::PasswordEntryRow =
        builder.object("secret_key_row").expect("secret_key_row");
    let password_row: adw::PasswordEntryRow = builder.object("password_row").expect("password_row");
    let error_label: gtk4::Label = builder.object("error_label").expect("error_label");
    let cancel_button: gtk4::Button = builder.object("cancel_button").expect("cancel_button");
    let add_button: gtk4::Button = builder.object("add_button").expect("add_button");

    // Local fields
    let chosen_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

    let selected_backend = {
        let backend_row = backend_row.clone();
        move || backend_row.selected()
    };

    let update_sensitivity = {
        let name_row = name_row.clone();
        let password_row = password_row.clone();
        let chosen_path = chosen_path.clone();
        let host_row = host_row.clone();
        let sftp_path_row = sftp_path_row.clone();
        let user_row = user_row.clone();
        let endpoint_row = endpoint_row.clone();
        let bucket_row = bucket_row.clone();
        let access_key_row = access_key_row.clone();
        let secret_key_row = secret_key_row.clone();
        let add_button = add_button.clone();
        let selected_backend = selected_backend.clone();
        move || {
            let backend_ready = match selected_backend() {
                BACKEND_LOCAL => chosen_path.borrow().is_some(),
                BACKEND_SFTP => {
                    !host_row.text().is_empty()
                        && !sftp_path_row.text().is_empty()
                        && !user_row.text().is_empty()
                }
                _ => {
                    !endpoint_row.text().is_empty()
                        && !bucket_row.text().is_empty()
                        && !access_key_row.text().is_empty()
                        && !secret_key_row.text().is_empty()
                }
            };
            let ready =
                !name_row.text().is_empty() && !password_row.text().is_empty() && backend_ready;
            add_button.set_sensitive(ready);
        }
    };

    backend_row.connect_selected_notify({
        let local_group = local_group.clone();
        let sftp_group = sftp_group.clone();
        let s3_group = s3_group.clone();
        let update_sensitivity = update_sensitivity.clone();
        move |row| {
            let selected = row.selected();
            local_group.set_visible(selected == BACKEND_LOCAL);
            sftp_group.set_visible(selected == BACKEND_SFTP);
            s3_group.set_visible(selected == BACKEND_S3);
            update_sensitivity();
        }
    });

    let watched_entries: Vec<gtk4::Editable> = vec![
        name_row.clone().upcast(),
        password_row.clone().upcast(),
        host_row.clone().upcast(),
        sftp_path_row.clone().upcast(),
        user_row.clone().upcast(),
        endpoint_row.clone().upcast(),
        bucket_row.clone().upcast(),
        access_key_row.clone().upcast(),
        secret_key_row.clone().upcast(),
    ];
    for entry_row in watched_entries {
        entry_row.connect_changed({
            let update_sensitivity = update_sensitivity.clone();
            move |_| update_sensitivity()
        });
    }

    location_row.connect_activated({
        let parent = parent.clone();
        let chosen_path = chosen_path.clone();
        let location_row = location_row.clone();
        let update_sensitivity = update_sensitivity.clone();
        move |_| {
            let file_dialog = gtk4::FileDialog::builder()
                .title("Choose Repository Folder")
                .build();
            let parent = parent.clone();
            let chosen_path = chosen_path.clone();
            let location_row = location_row.clone();
            let update_sensitivity = update_sensitivity.clone();
            glib::spawn_future_local(async move {
                if let Ok(file) = file_dialog.select_folder_future(Some(&parent)).await
                    && let Some(path) = file.path()
                {
                    location_row.set_subtitle(&path.display().to_string());
                    *chosen_path.borrow_mut() = Some(path);
                    update_sensitivity();
                }
            });
        }
    });

    cancel_button.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });

    add_button.connect_clicked({
        let dialog = dialog.clone();
        let name_row = name_row.clone();
        let password_row = password_row.clone();
        let chosen_path = chosen_path.clone();
        let host_row = host_row.clone();
        let sftp_path_row = sftp_path_row.clone();
        let user_row = user_row.clone();
        let endpoint_row = endpoint_row.clone();
        let bucket_row = bucket_row.clone();
        let access_key_row = access_key_row.clone();
        let secret_key_row = secret_key_row.clone();
        let error_label = error_label.clone();
        let cancel_button = cancel_button.clone();
        let selected_backend = selected_backend.clone();
        move |add_button| {
            let name = name_row.text().to_string();
            let repository_password = password_row.text().to_string();

            let (backend, backend_secret) = match selected_backend() {
                BACKEND_LOCAL => {
                    let Some(path) = chosen_path.borrow().clone() else {
                        return;
                    };
                    (Backend::Local { path }, None)
                }
                BACKEND_SFTP => (
                    Backend::Sftp {
                        host: host_row.text().to_string(),
                        path: sftp_path_row.text().to_string(),
                        user: user_row.text().to_string(),
                    },
                    None,
                ),
                _ => (
                    Backend::S3 {
                        endpoint: endpoint_row.text().to_string(),
                        bucket: bucket_row.text().to_string(),
                        access_key_id: access_key_row.text().to_string(),
                    },
                    Some(secret_key_row.text().to_string()),
                ),
            };

            add_button.set_sensitive(false);
            cancel_button.set_sensitive(false);
            error_label.set_visible(false);

            let dialog = dialog.clone();
            let store = store.clone();
            let on_added = on_added.clone();
            let error_label = error_label.clone();
            let add_button = add_button.clone();
            let cancel_button = cancel_button.clone();

            glib::spawn_future_local(async move {
                let connection = RepositoryConnection {
                    backend: backend.clone(),
                    password: repository_password.clone(),
                    backend_secret: backend_secret.clone(),
                };

                match RealResticClient::system().list_snapshots(&connection).await {
                    Ok(_) => {
                        let id = glib::uuid_string_random().to_string();

                        let password_label = format!("Restic Viewer: {name} Repository password");
                        if let Err(err) = keyring::store(
                            &id,
                            CredentialKind::Password,
                            &repository_password,
                            &password_label,
                        )
                        .await
                        {
                            error_label.set_label(&format!("Could not save the password: {err}"));
                            error_label.set_visible(true);
                            add_button.set_sensitive(true);
                            cancel_button.set_sensitive(true);
                            return;
                        }

                        if let Some(secret) = &backend_secret {
                            let secret_label = format!("Restic Viewer: {name} Backend secret");
                            if let Err(err) = keyring::store(
                                &id,
                                CredentialKind::BackendSecret,
                                secret,
                                &secret_label,
                            )
                            .await
                            {
                                error_label.set_label(&format!(
                                    "Could not save the backend secret: {err}"
                                ));
                                error_label.set_visible(true);
                                add_button.set_sensitive(true);
                                cancel_button.set_sensitive(true);
                                return;
                            }
                        }

                        let entry = RepositoryEntry { id, name, backend };
                        if let Err(err) = store.add(entry.clone()) {
                            error_label.set_label(&format!("Could not save the Repository: {err}"));
                            error_label.set_visible(true);
                            add_button.set_sensitive(true);
                            cancel_button.set_sensitive(true);
                            return;
                        }

                        on_added(entry);
                        dialog.close();
                    }
                    Err(err) => {
                        error_label.set_label(&format!("Could not open this Repository: {err}"));
                        error_label.set_visible(true);
                        add_button.set_sensitive(true);
                        cancel_button.set_sensitive(true);
                    }
                }
            });
        }
    });

    dialog.present(Some(parent));
}
