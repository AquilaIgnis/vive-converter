# ViveNotes OneNote converter

This standalone CLI converts a OneNote package, notebook, or section into a portable ViveNotes
`.vive` notebook. It uses the adjacent `onenote.rs` checkout and does not modify the Android app.

```sh
cd converter/vive-converter
cargo run --release -- ../Calculus2.onepkg
```

The output defaults to the input name with a `.vive` extension. Use `--output PATH` to choose a
different destination, `--force` to replace an existing file, or `--inspect --verbose` to parse and
summarize a source without converting it.

The converter preserves sections, pages, positioned rich text, common inline equations, lists,
tables, images, and ink. Section groups become a slash-separated section path because `.vive` v1
has no group entity. Embedded files and unrecognized source objects are omitted with an explicit
warning.

Before publishing the output, the CLI validates SQLite integrity and foreign keys, page JSON,
AndroidX Ink gzip payloads, ZIP entry names, and every archive checksum. ViveNotes performs the
authoritative full validation, including native AndroidX Ink decoding, when the notebook is
imported.

OneNote stores explicit ink colours as Windows `COLORREF` values (`0x00BBGGRR`); the converter
normalizes them to ARGB and preserves highlighter transparency. It also samples OneNote's sparse
shape-highlighter paths densely enough for AndroidX Ink to render their straight edges and corners
without diagonal joins.

If a notebook made by an older converter build is already in ViveNotes, remove that imported copy
before importing the corrected bundle. Stable stroke IDs deliberately prevent a re-import from
silently replacing existing strokes with different geometry.

# SRC

- https://github.com/msiemens/onenote.rs
