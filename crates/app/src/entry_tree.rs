//! The Entry tree browser for a single Snapshot: a lazily-loading GTK4 tree
//! (`TreeListModel` + `TreeExpander`) built on top of the restic client seam's
//! `list_tree` calls, with multi-selection feeding a "Restore Selected" action, a sort
//! menu, and a search box that swaps the tree for a flat list of matches.
//!
//! A directory's children are only fetched the first time its row is expanded —
//! `TreeListModel`'s `create_func` runs for every row as soon as it enters the model
//! (even collapsed ones), so eagerly fetching there would eventually crawl the entire
//! Snapshot tree on load. Loading is instead triggered from `TreeListRow`'s `expanded`
//! notification, guarded by a `loaded` flag on the node so a re-bound row (GTK4 list
//! items are recycled during scrolling) never re-fetches the same directory twice.
//!
//! While a fetch is in flight the directory holds a single placeholder child ("Loading…"
//! with a spinner), which the fetched children then replace. A failed fetch replaces it
//! with an error row and clears the `loaded` flag, so collapsing and re-expanding retries.

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::rc::Rc;

use gettextrs::gettext;
use gtk4::prelude::*;
use libadwaita as adw;
use restic_client::{Entry, EntryKind, RealResticClient, RepositoryConnection, ResticClient};

use crate::{preview_dialog, restore_dialog, save_dialog};

const RESOURCE_PATH: &str = "/dev/brunopaz/ResticViewer/entry_tree.ui";

/// Caps the flat results list so a one-letter query on a huge Snapshot stays responsive.
const MAX_SEARCH_RESULTS: usize = 5000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Name,
    Size,
    Modified,
}

impl SortKey {
    /// Matches the order of the strings in the sort dropdown (see entry_tree.blp).
    fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Size,
            2 => Self::Modified,
            _ => Self::Name,
        }
    }
}

enum Placeholder {
    Loading,
    Failed(String),
}

struct NodeData {
    entry: Entry,
    connection: RepositoryConnection,
    snapshot_id: String,
    child_store: RefCell<Option<gio::ListStore>>,
    loaded: Cell<bool>,
    placeholder: Option<Placeholder>,
}

impl NodeData {
    fn new(entry: Entry, connection: RepositoryConnection, snapshot_id: String) -> Self {
        Self {
            entry,
            connection,
            snapshot_id,
            child_store: RefCell::new(None),
            loaded: Cell::new(false),
            placeholder: None,
        }
    }

    fn placeholder(&self, kind: Placeholder) -> Self {
        let name = match kind {
            Placeholder::Loading => gettext("Loading…"),
            Placeholder::Failed(_) => gettext("Couldn't load this folder"),
        };
        let entry = Entry {
            name,
            path: String::new(),
            kind: EntryKind::File,
            size: None,
            mtime: String::new(),
        };
        Self {
            placeholder: Some(kind),
            ..Self::new(entry, self.connection.clone(), self.snapshot_id.clone())
        }
    }
}

/// The widgets of one list row, in the order `build` appends them.
struct RowWidgets {
    icon: gtk4::Image,
    name: gtk4::Label,
    meta: gtk4::Label,
    spinner: gtk4::Widget,
}

impl RowWidgets {
    fn build() -> gtk4::Box {
        let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        row_box.append(&gtk4::Image::new());
        row_box.append(&gtk4::Label::builder().xalign(0.0).hexpand(true).build());
        row_box.append(
            &gtk4::Label::builder()
                .css_classes(["dim-label"])
                .ellipsize(gtk4::pango::EllipsizeMode::Middle)
                .build(),
        );
        row_box.append(&adw::Spinner::builder().visible(false).build());
        row_box
    }

    fn from_box(row_box: &gtk4::Box) -> Option<Self> {
        let icon = row_box.first_child()?;
        let name = icon.next_sibling()?;
        let meta = name.next_sibling()?;
        let spinner = meta.next_sibling()?;
        Some(Self {
            icon: icon.downcast().ok()?,
            name: name.downcast().ok()?,
            meta: meta.downcast().ok()?,
            spinner,
        })
    }

    fn show(&self, node: &NodeData, meta: &str) {
        match &node.placeholder {
            Some(Placeholder::Loading) => self.icon.set_icon_name(None),
            Some(Placeholder::Failed(_)) => {
                self.icon.set_icon_name(Some("dialog-warning-symbolic"))
            }
            None => self.icon.set_from_gicon(&icon_for(&node.entry)),
        }
        self.name.set_label(&node.entry.name);
        self.meta.set_label(meta);
        self.spinner
            .set_visible(matches!(node.placeholder, Some(Placeholder::Loading)));
        if node.placeholder.is_some() {
            self.name.add_css_class("dim-label");
        } else {
            self.name.remove_css_class("dim-label");
        }
    }
}

pub fn build(
    parent: adw::ApplicationWindow,
    connection: RepositoryConnection,
    snapshot_id: String,
    root_paths: Vec<String>,
    on_compare: impl Fn() + 'static,
) -> gtk4::Widget {
    let builder = gtk4::Builder::from_resource(RESOURCE_PATH);
    let toolbar_view: adw::ToolbarView = builder.object("toolbar_view").expect("toolbar_view");
    let restore_button: gtk4::Button = builder.object("restore_button").expect("restore_button");
    let save_button: gtk4::Button = builder.object("save_button").expect("save_button");
    let compare_button: gtk4::Button = builder.object("compare_button").expect("compare_button");
    compare_button.connect_clicked(move |_| on_compare());
    let sort_dropdown: gtk4::DropDown = builder.object("sort_dropdown").expect("sort_dropdown");
    let search_entry: gtk4::SearchEntry = builder.object("search_entry").expect("search_entry");
    let content_stack: gtk4::Stack = builder.object("content_stack").expect("content_stack");
    let tree_view: gtk4::ListView = builder.object("tree_view").expect("tree_view");
    let results_view: gtk4::ListView = builder.object("results_view").expect("results_view");
    let search_error_status: adw::StatusPage = builder
        .object("search_error_status")
        .expect("search_error_status");

    let sort_key = Rc::new(Cell::new(SortKey::Name));
    let sorter = gtk4::CustomSorter::new({
        let sort_key = sort_key.clone();
        move |a, b| compare_nodes(a, b, sort_key.get()).into()
    });
    sort_dropdown.connect_selected_notify({
        let sort_key = sort_key.clone();
        let sorter = sorter.clone();
        move |dropdown| {
            sort_key.set(SortKey::from_index(dropdown.selected()));
            sorter.changed(gtk4::SorterChange::Different);
        }
    });

    let root_store = gio::ListStore::new::<glib::BoxedAnyObject>();
    for path in root_paths {
        let name = path
            .rsplit('/')
            .find(|s| !s.is_empty())
            .unwrap_or(&path)
            .to_string();
        let entry = Entry {
            name,
            path,
            kind: EntryKind::Directory,
            size: None,
            mtime: String::new(),
        };
        root_store.append(&glib::BoxedAnyObject::new(NodeData::new(
            entry,
            connection.clone(),
            snapshot_id.clone(),
        )));
    }

    let tree_model = gtk4::TreeListModel::new(
        gtk4::SortListModel::new(Some(root_store), Some(sorter.clone())),
        false,
        false,
        {
            let sorter = sorter.clone();
            move |obj| {
                let boxed = obj.downcast_ref::<glib::BoxedAnyObject>()?;
                let node = boxed.borrow::<NodeData>();
                if node.entry.kind != EntryKind::Directory {
                    return None;
                }
                let mut child_store = node.child_store.borrow_mut();
                let store = child_store
                    .get_or_insert_with(gio::ListStore::new::<glib::BoxedAnyObject>)
                    .clone();
                Some(gtk4::SortListModel::new(Some(store), Some(sorter.clone())).upcast())
            }
        },
    );
    let tree_selection = gtk4::MultiSelection::new(Some(tree_model));
    tree_view.set_model(Some(&tree_selection));
    tree_view.set_factory(Some(&tree_factory()));

    let results_store = gio::ListStore::new::<glib::BoxedAnyObject>();
    let results_selection = gtk4::MultiSelection::new(Some(gtk4::SortListModel::new(
        Some(results_store.clone()),
        Some(sorter),
    )));
    results_view.set_model(Some(&results_selection));
    results_view.set_factory(Some(&results_factory()));

    tree_view.connect_activate({
        let selection = tree_selection.clone();
        let parent = parent.clone();
        move |_, position| {
            let Some(row) = selection
                .item(position)
                .and_then(|item| item.downcast::<gtk4::TreeListRow>().ok())
            else {
                return;
            };
            let Some(boxed) = row
                .item()
                .and_then(|item| item.downcast::<glib::BoxedAnyObject>().ok())
            else {
                return;
            };
            let node = boxed.borrow::<NodeData>();
            if node.placeholder.is_some() {
                return;
            }
            if node.entry.kind == EntryKind::Directory {
                row.set_expanded(!row.is_expanded());
            } else {
                preview_dialog::present(
                    &parent,
                    node.connection.clone(),
                    node.snapshot_id.clone(),
                    node.entry.clone(),
                );
            }
        }
    });

    results_view.connect_activate({
        let selection = results_selection.clone();
        let parent = parent.clone();
        move |_, position| {
            let Some(boxed) = selection
                .item(position)
                .and_then(|item| item.downcast::<glib::BoxedAnyObject>().ok())
            else {
                return;
            };
            let node = boxed.borrow::<NodeData>();
            if node.entry.kind != EntryKind::Directory {
                preview_dialog::present(
                    &parent,
                    node.connection.clone(),
                    node.snapshot_id.clone(),
                    node.entry.clone(),
                );
            }
        }
    });

    let update_restore_button = {
        let restore_button = restore_button.clone();
        let save_button = save_button.clone();
        let content_stack = content_stack.clone();
        let tree_selection = tree_selection.clone();
        let results_selection = results_selection.clone();
        move || {
            let selected = selected_entries(&content_stack, &tree_selection, &results_selection);
            restore_button.set_sensitive(!selected.is_empty());
            // Saving packs exactly one file or directory into one local file.
            save_button.set_sensitive(
                matches!(selected.as_slice(), [entry] if entry.kind != EntryKind::Symlink),
            );
        }
    };
    tree_selection.connect_selection_changed({
        let update = update_restore_button.clone();
        move |_, _, _| update()
    });
    results_selection.connect_selection_changed({
        let update = update_restore_button.clone();
        move |_, _, _| update()
    });

    save_button.connect_clicked({
        let content_stack = content_stack.clone();
        let tree_selection = tree_selection.clone();
        let results_selection = results_selection.clone();
        let connection = connection.clone();
        let snapshot_id = snapshot_id.clone();
        let parent = parent.clone();
        move |_| {
            let selected = selected_entries(&content_stack, &tree_selection, &results_selection);
            if let [entry] = selected.as_slice() {
                save_dialog::present(
                    &parent,
                    connection.clone(),
                    snapshot_id.clone(),
                    entry.clone(),
                );
            }
        }
    });

    restore_button.connect_clicked({
        let content_stack = content_stack.clone();
        let tree_selection = tree_selection.clone();
        let results_selection = results_selection.clone();
        let connection = connection.clone();
        let snapshot_id = snapshot_id.clone();
        move |_| {
            let paths: Vec<String> =
                selected_entries(&content_stack, &tree_selection, &results_selection)
                    .into_iter()
                    .map(|entry| entry.path)
                    .collect();
            if paths.is_empty() {
                return;
            }
            restore_dialog::present(
                &parent,
                connection.clone(),
                snapshot_id.clone(),
                paths,
                "Restore Selected",
            );
        }
    });

    let search_generation = Rc::new(Cell::new(0u64));
    // The full Snapshot listing, fetched by the first search and reused by later ones.
    let listing: Rc<RefCell<Option<Rc<Vec<Entry>>>>> = Rc::new(RefCell::new(None));
    search_entry.connect_search_changed(move |entry| {
        let query = entry.text().trim().to_string();
        let generation = search_generation.get() + 1;
        search_generation.set(generation);

        if query.is_empty() {
            content_stack.set_visible_child_name("tree");
            update_restore_button();
            return;
        }
        content_stack.set_visible_child_name("searching");
        update_restore_button();

        let search_generation = search_generation.clone();
        let content_stack = content_stack.clone();
        let results_store = results_store.clone();
        let search_error_status = search_error_status.clone();
        let connection = connection.clone();
        let snapshot_id = snapshot_id.clone();
        let update_restore_button = update_restore_button.clone();
        let listing = listing.clone();
        glib::spawn_future_local(async move {
            let cached = listing.borrow().clone();
            let result = match cached {
                Some(all) => Ok(all),
                None => RealResticClient::system()
                    .list_all(&connection, &snapshot_id)
                    .await
                    .map(|all| {
                        let all = Rc::new(all);
                        *listing.borrow_mut() = Some(all.clone());
                        all
                    }),
            };
            if search_generation.get() != generation {
                return; // the query changed while this search was running
            }
            let result = result.map(|all| matching_entries(&all, &query));
            match result {
                Ok(entries) if entries.is_empty() => {
                    results_store.remove_all();
                    content_stack.set_visible_child_name("no-results");
                }
                Ok(entries) => {
                    let nodes: Vec<_> = entries
                        .into_iter()
                        .map(|entry| {
                            glib::BoxedAnyObject::new(NodeData::new(
                                entry,
                                connection.clone(),
                                snapshot_id.clone(),
                            ))
                        })
                        .collect();
                    results_store.splice(0, results_store.n_items(), &nodes);
                    content_stack.set_visible_child_name("results");
                }
                Err(err) => {
                    search_error_status.set_description(Some(&err.to_string()));
                    content_stack.set_visible_child_name("search-error");
                }
            }
            update_restore_button();
        });
    });

    toolbar_view.upcast()
}

/// Entries whose name contains `query`, ignoring case, capped at [`MAX_SEARCH_RESULTS`].
fn matching_entries(all: &[Entry], query: &str) -> Vec<Entry> {
    let needle = query.to_lowercase();
    all.iter()
        .filter(|entry| contains_ignore_case(&entry.name, &needle))
        .take(MAX_SEARCH_RESULTS)
        .cloned()
        .collect()
}

/// `needle` must already be lowercase. ASCII names are compared without allocating,
/// which matters when scanning every Entry of a large Snapshot per keystroke.
fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    if haystack.is_ascii() && needle.is_ascii() {
        return haystack
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()));
    }
    haystack.to_lowercase().contains(needle)
}

fn showing_results(content_stack: &gtk4::Stack) -> bool {
    content_stack.visible_child_name().as_deref() == Some("results")
}

fn tree_factory() -> gtk4::SignalListItemFactory {
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(|_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("factory item is a ListItem");
        let expander = gtk4::TreeExpander::new();
        expander.set_child(Some(&RowWidgets::build()));
        list_item.set_child(Some(&expander));
    });

    factory.connect_bind(|_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("factory item is a ListItem");
        let Some(row) = list_item
            .item()
            .and_then(|item| item.downcast::<gtk4::TreeListRow>().ok())
        else {
            return;
        };
        let Some(expander) = list_item
            .child()
            .and_then(|child| child.downcast::<gtk4::TreeExpander>().ok())
        else {
            return;
        };
        expander.set_list_row(Some(&row));

        let Some(boxed) = row
            .item()
            .and_then(|item| item.downcast::<glib::BoxedAnyObject>().ok())
        else {
            return;
        };
        {
            let node = boxed.borrow::<NodeData>();
            if let Some(widgets) = expander
                .child()
                .and_then(|c| c.downcast::<gtk4::Box>().ok())
                .and_then(|row_box| RowWidgets::from_box(&row_box))
            {
                widgets.show(&node, &tree_meta(&node));
            }
            list_item.set_selectable(node.placeholder.is_none());
            list_item.set_activatable(node.placeholder.is_none());
        }

        maybe_load_children(&boxed, &row);
        row.connect_expanded_notify(move |row| maybe_load_children(&boxed, row));
    });
    factory
}

fn results_factory() -> gtk4::SignalListItemFactory {
    let factory = gtk4::SignalListItemFactory::new();
    factory.connect_setup(|_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("factory item is a ListItem");
        list_item.set_child(Some(&RowWidgets::build()));
    });
    factory.connect_bind(|_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("factory item is a ListItem");
        let Some(boxed) = list_item
            .item()
            .and_then(|item| item.downcast::<glib::BoxedAnyObject>().ok())
        else {
            return;
        };
        let node = boxed.borrow::<NodeData>();
        if let Some(widgets) = list_item
            .child()
            .and_then(|c| c.downcast::<gtk4::Box>().ok())
            .and_then(|row_box| RowWidgets::from_box(&row_box))
        {
            widgets.show(&node, &result_meta(&node.entry));
        }
    });
    factory
}

/// The Entries selected in whichever view (tree or search results) is showing.
fn selected_entries(
    content_stack: &gtk4::Stack,
    tree_selection: &gtk4::MultiSelection,
    results_selection: &gtk4::MultiSelection,
) -> Vec<Entry> {
    let (model, in_tree) = if showing_results(content_stack) {
        (results_selection, false)
    } else {
        (tree_selection, true)
    };
    (0..model.n_items())
        .filter(|&position| model.is_selected(position))
        .filter_map(|position| {
            let item = model.item(position)?;
            // Tree rows wrap the node in a TreeListRow; result rows are the node itself.
            let item = if in_tree {
                item.downcast::<gtk4::TreeListRow>().ok()?.item()?
            } else {
                item
            };
            item.downcast::<glib::BoxedAnyObject>().ok()
        })
        .filter_map(|boxed| {
            let node = boxed.borrow::<NodeData>();
            node.placeholder.is_none().then(|| node.entry.clone())
        })
        .collect()
}

fn maybe_load_children(boxed: &glib::BoxedAnyObject, row: &gtk4::TreeListRow) {
    if !row.is_expanded() {
        return;
    }
    let (connection, snapshot_id, path, store) = {
        let node = boxed.borrow::<NodeData>();
        if node.loaded.get() {
            return;
        }
        node.loaded.set(true);
        let Some(store) = node.child_store.borrow().clone() else {
            return;
        };
        store.remove_all();
        store.append(&glib::BoxedAnyObject::new(
            node.placeholder(Placeholder::Loading),
        ));
        (
            node.connection.clone(),
            node.snapshot_id.clone(),
            node.entry.path.clone(),
            store,
        )
    };

    let boxed = boxed.clone();
    glib::spawn_future_local(async move {
        let result = RealResticClient::system()
            .list_tree(&connection, &snapshot_id, &path)
            .await;
        let rows: Vec<_> = match result {
            Ok(children) => children
                .into_iter()
                .map(|child| {
                    glib::BoxedAnyObject::new(NodeData::new(
                        child,
                        connection.clone(),
                        snapshot_id.clone(),
                    ))
                })
                .collect(),
            Err(err) => {
                let node = boxed.borrow::<NodeData>();
                node.loaded.set(false);
                vec![glib::BoxedAnyObject::new(
                    node.placeholder(Placeholder::Failed(err.to_string())),
                )]
            }
        };
        store.splice(0, store.n_items(), &rows);
    });
}

/// Placeholders first, then directories before files, then by the chosen key: names
/// A→Z, sizes and dates largest/newest first.
fn compare_nodes(a: &glib::Object, b: &glib::Object, key: SortKey) -> Ordering {
    let (Some(a), Some(b)) = (
        a.downcast_ref::<glib::BoxedAnyObject>(),
        b.downcast_ref::<glib::BoxedAnyObject>(),
    ) else {
        return Ordering::Equal;
    };
    let (a, b) = (a.borrow::<NodeData>(), b.borrow::<NodeData>());

    let by_name = || {
        a.entry
            .name
            .to_lowercase()
            .cmp(&b.entry.name.to_lowercase())
    };
    b.placeholder
        .is_some()
        .cmp(&a.placeholder.is_some())
        .then_with(|| {
            (b.entry.kind == EntryKind::Directory).cmp(&(a.entry.kind == EntryKind::Directory))
        })
        .then_with(|| match key {
            SortKey::Name => by_name(),
            SortKey::Size => b.entry.size.cmp(&a.entry.size).then_with(by_name),
            SortKey::Modified => b.entry.mtime.cmp(&a.entry.mtime).then_with(by_name),
        })
}

fn icon_for(entry: &Entry) -> gio::Icon {
    match entry.kind {
        EntryKind::Directory => gio::ThemedIcon::new("folder-symbolic").upcast(),
        EntryKind::Symlink => gio::ThemedIcon::new("insert-link-symbolic").upcast(),
        EntryKind::File => {
            let (content_type, _) = gio::content_type_guess(Some(&entry.name), None);
            gio::content_type_get_symbolic_icon(&content_type)
        }
    }
}

fn tree_meta(node: &NodeData) -> String {
    match &node.placeholder {
        Some(Placeholder::Failed(message)) => {
            message.lines().next().unwrap_or_default().to_string()
        }
        Some(Placeholder::Loading) => String::new(),
        None => format_meta(&node.entry),
    }
}

fn result_meta(entry: &Entry) -> String {
    let folder =
        entry.path.rsplit_once('/').map_or(
            "/",
            |(parent, _)| {
                if parent.is_empty() { "/" } else { parent }
            },
        );
    match entry.size {
        Some(size) => format!("{folder} · {}", glib::format_size(size)),
        None => folder.to_string(),
    }
}

fn format_meta(entry: &Entry) -> String {
    let date = entry
        .mtime
        .get(..19)
        .unwrap_or(&entry.mtime)
        .replace('T', " ");
    match entry.size {
        Some(size) => format!("{} · {date}", glib::format_size(size)),
        None => date,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> Entry {
        Entry {
            name: name.to_string(),
            path: format!("/{name}"),
            kind: EntryKind::File,
            size: None,
            mtime: String::new(),
        }
    }

    #[test]
    fn matching_ignores_case_for_ascii_and_unicode_names() {
        let all = vec![entry("Report.TXT"), entry("notes.md"), entry("Ünïcode.txt")];

        let names = |query: &str| -> Vec<String> {
            matching_entries(&all, query)
                .into_iter()
                .map(|e| e.name)
                .collect()
        };

        assert_eq!(names("report"), vec!["Report.TXT"]);
        assert_eq!(names(".TXT"), vec!["Report.TXT", "Ünïcode.txt"]);
        assert_eq!(names("ünï"), vec!["Ünïcode.txt"]);
        assert!(names("missing").is_empty());
    }

    #[test]
    fn results_are_capped() {
        let all: Vec<Entry> = (0..MAX_SEARCH_RESULTS + 10)
            .map(|i| entry(&format!("file{i}")))
            .collect();

        assert_eq!(matching_entries(&all, "file").len(), MAX_SEARCH_RESULTS);
    }
}
