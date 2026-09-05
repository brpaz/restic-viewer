# Use Blueprint for static UI layout, loaded via GtkBuilder — not composite templates

Widget layout was originally built entirely in Rust via gtk4-rs's builder-pattern calls (per ADR-0003). Moving it to [Blueprint](https://gitlab.gnome.org/GNOME/blueprint-compiler) markup (`.blp` files, compiled to GtkBuilder `.ui` XML by `blueprint-compiler`, bundled into a GResource via `glib-build-tools` in `build.rs`) separates declarative structure from Rust logic and matches how most GNOME apps are built today.

Two ways to consume the compiled `.ui` exist: composite templates (each dialog/view becomes a real GObject subclass with `#[derive(CompositeTemplate)]` and `#[template_child]` fields — the more idiomatic pattern for reusable custom widgets, but a much bigger rewrite requiring GObject subclassing throughout) or loading with a plain `gtk4::Builder` and grabbing widgets by ID (`builder.object::<T>("id")`). We chose the Builder approach: only the widget-construction code is replaced, everything else (closures, async wiring, state) stays exactly as before.

Only genuinely **static** layout moved to Blueprint — windows, dialogs, and fixed chrome (headers, forms, button bars). Per-item dynamically generated content (Repository rows, Snapshot rows, the Entry tree's `TreeListModel`/factory) stays built in Rust, since Blueprint describes fixed structure, not data-driven repetition.
