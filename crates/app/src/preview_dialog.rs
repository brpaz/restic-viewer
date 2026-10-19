//! A read-only preview of one file in a Snapshot: text and images are fetched with
//! `restic dump` into memory and shown in a dialog, so the user can check a file's
//! contents before deciding to restore it.

use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use restic_client::{Entry, EntryKind, RealResticClient, RepositoryConnection, ResticClient};

const RESOURCE_PATH: &str = "/dev/brunopaz/ResticViewer/preview_dialog.ui";

/// Files above this size are not fetched; a preview is a quick look, not a download.
const MAX_PREVIEW_BYTES: u64 = 8 * 1024 * 1024;

enum Preview {
    Text(String),
    Image(gtk4::gdk::Texture),
    Unavailable(String),
}

pub fn present(
    parent: &adw::ApplicationWindow,
    connection: RepositoryConnection,
    snapshot_id: String,
    entry: Entry,
) {
    let builder = gtk4::Builder::from_resource(RESOURCE_PATH);
    let dialog: adw::Dialog = builder.object("dialog").expect("dialog");
    let window_title: adw::WindowTitle = builder.object("window_title").expect("window_title");
    let stack: gtk4::Stack = builder.object("stack").expect("stack");
    let text_view: gtk4::TextView = builder.object("text_view").expect("text_view");
    let picture: gtk4::Picture = builder.object("picture").expect("picture");
    let unavailable_status: adw::StatusPage = builder
        .object("unavailable_status")
        .expect("unavailable_status");

    dialog.set_title(&entry.name);
    window_title.set_title(&entry.name);
    if let Some(size) = entry.size {
        window_title.set_subtitle(&glib::format_size(size));
    }
    dialog.present(Some(parent));

    glib::spawn_future_local(async move {
        match load(&connection, &snapshot_id, &entry).await {
            Preview::Text(text) => {
                text_view.buffer().set_text(&text);
                stack.set_visible_child_name("text");
            }
            Preview::Image(texture) => {
                picture.set_paintable(Some(&texture));
                stack.set_visible_child_name("image");
            }
            Preview::Unavailable(reason) => {
                unavailable_status.set_description(Some(&reason));
                stack.set_visible_child_name("unavailable");
            }
        }
    });
}

async fn load(connection: &RepositoryConnection, snapshot_id: &str, entry: &Entry) -> Preview {
    if entry.kind != EntryKind::File {
        return Preview::Unavailable(gettext("Only regular files can be previewed."));
    }
    if entry.size.is_some_and(|size| size > MAX_PREVIEW_BYTES) {
        return Preview::Unavailable(gettext("This file is too large to preview."));
    }

    let bytes = match RealResticClient::system()
        .read_file(connection, snapshot_id, &entry.path)
        .await
    {
        Ok(bytes) => bytes,
        Err(err) => return Preview::Unavailable(err.to_string()),
    };

    let (content_type, _) = gio::content_type_guess(Some(&entry.name), &bytes[..]);
    if content_type.starts_with("image/") {
        match gtk4::gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)) {
            Ok(texture) => return Preview::Image(texture),
            Err(err) => return Preview::Unavailable(err.to_string()),
        }
    }
    match String::from_utf8(bytes) {
        Ok(text) if !text.contains('\0') => Preview::Text(text),
        _ => Preview::Unavailable(gettext("This file type can't be previewed.")),
    }
}
