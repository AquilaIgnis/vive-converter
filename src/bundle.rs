use crate::ink::validate_gzip_proto;
use crate::model::{BundleAttachment, BundleCounts, BundleFile, ConvertedNotebook, Manifest};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

const FORMAT: &str = "com.vivenotes.notebook";
const FORMAT_VERSION: u32 = 1;
const APP_SCHEMA_VERSION: u32 = 13;
const APPLICATION_ID: u32 = 0x5649_5645;

pub struct BundleSummary {
    pub sections: usize,
    pub pages: usize,
    pub strokes: usize,
    pub attachments: usize,
    pub bytes: u64,
}

pub fn write_bundle(
    notebook: &ConvertedNotebook,
    output: &Path,
    force: bool,
) -> Result<BundleSummary> {
    if output.exists() && !force {
        bail!(
            "output already exists (pass --force to replace it): {}",
            output.display()
        )
    }
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("creating output directory {}", parent.display()))?;
    let staging = tempdir_in(parent)?;
    let database_path = staging.path().join("notebook.sqlite");
    write_database(notebook, &database_path)?;
    validate_database(notebook, &database_path)?;

    let database_bytes = std::fs::read(&database_path)?;
    let database_hash = sha256(&database_bytes);
    let sections = notebook.sections.len();
    let pages = notebook
        .sections
        .iter()
        .map(|section| section.pages.len())
        .sum();
    let strokes = notebook
        .sections
        .iter()
        .flat_map(|section| &section.pages)
        .map(|page| page.strokes.len())
        .sum();
    let counts = BundleCounts {
        sections,
        pages,
        strokes,
        revisions: 0,
        attachments: notebook.attachments.len(),
    };
    let attachment_descriptors = notebook
        .attachments
        .iter()
        .map(|attachment| BundleAttachment {
            id: attachment.id.clone(),
            path: format!("attachments/{}", attachment.id),
            mime_type: attachment.mime_type,
            pixel_width: attachment.pixel_width,
            pixel_height: attachment.pixel_height,
            byte_count: attachment.bytes.len() as u64,
            sha256: attachment.id.clone(),
        })
        .collect::<Vec<_>>();
    let manifest = Manifest {
        format: FORMAT,
        format_version: FORMAT_VERSION,
        app_schema_version: APP_SCHEMA_VERSION,
        bundle_id: Uuid::now_v7().to_string(),
        created_at: now_millis(),
        source_notebook_id: notebook.id.clone(),
        notebook_name: notebook.name.clone(),
        database: BundleFile {
            path: "notebook.sqlite",
            byte_count: database_bytes.len() as u64,
            sha256: database_hash,
        },
        attachments: attachment_descriptors,
        counts,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;

    let mut files = BTreeMap::<String, Vec<u8>>::new();
    files.insert("manifest.json".to_owned(), manifest_bytes);
    files.insert("notebook.sqlite".to_owned(), database_bytes);
    for attachment in &notebook.attachments {
        files.insert(
            format!("attachments/{}", attachment.id),
            attachment.bytes.clone(),
        );
    }
    let checksums = files
        .iter()
        .map(|(path, bytes)| format!("{}  {path}\n", sha256(bytes)))
        .collect::<String>();

    let mut temporary = NamedTempFile::new_in(parent)?;
    {
        let mut zip = zip::ZipWriter::new(temporary.as_file_mut());
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);
        for (path, bytes) in &files {
            zip.start_file(path, options)?;
            zip.write_all(bytes)?;
        }
        zip.start_file("checksums.sha256", options)?;
        zip.write_all(checksums.as_bytes())?;
        zip.finish()?;
    }
    temporary.as_file_mut().sync_all()?;
    temporary
        .persist(output)
        .map_err(|error| error.error)
        .with_context(|| format!("publishing {}", output.display()))?;
    validate_archive(output)?;

    Ok(BundleSummary {
        sections,
        pages,
        strokes,
        attachments: notebook.attachments.len(),
        bytes: std::fs::metadata(output)?.len(),
    })
}

fn tempdir_in(parent: &Path) -> Result<tempfile::TempDir> {
    tempfile::Builder::new()
        .prefix(".vive-converter-")
        .tempdir_in(parent)
        .context("creating conversion staging directory")
}

fn write_database(notebook: &ConvertedNotebook, path: &Path) -> Result<()> {
    let mut database = Connection::open(path)?;
    database.execute_batch(&format!(
        r#"
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = DELETE;
        PRAGMA application_id = {APPLICATION_ID};
        PRAGMA user_version = {FORMAT_VERSION};
        CREATE TABLE android_metadata (locale TEXT);
        CREATE TABLE attachments (
            id TEXT NOT NULL PRIMARY KEY, mimeType TEXT NOT NULL, pixelWidth INTEGER NOT NULL,
            pixelHeight INTEGER NOT NULL, byteCount INTEGER NOT NULL, refCount INTEGER NOT NULL,
            createdAt INTEGER NOT NULL
        );
        CREATE TABLE notebooks (
            id TEXT NOT NULL PRIMARY KEY, name TEXT NOT NULL, colorArgb INTEGER NOT NULL,
            sortIndex INTEGER NOT NULL, expanded INTEGER NOT NULL, createdAt INTEGER NOT NULL,
            updatedAt INTEGER NOT NULL, deletedAt INTEGER
        );
        CREATE TABLE sections (
            id TEXT NOT NULL PRIMARY KEY, notebookId TEXT NOT NULL, name TEXT NOT NULL,
            colorArgb INTEGER NOT NULL, sortIndex INTEGER NOT NULL, createdAt INTEGER NOT NULL,
            updatedAt INTEGER NOT NULL, deletedAt INTEGER,
            FOREIGN KEY(notebookId) REFERENCES notebooks(id) ON DELETE CASCADE
        );
        CREATE TABLE pages (
            id TEXT NOT NULL PRIMARY KEY, sectionId TEXT NOT NULL, title TEXT NOT NULL,
            sortIndex INTEGER NOT NULL, preview TEXT NOT NULL, createdAt INTEGER NOT NULL,
            updatedAt INTEGER NOT NULL, deletedAt INTEGER,
            FOREIGN KEY(sectionId) REFERENCES sections(id) ON DELETE CASCADE
        );
        CREATE TABLE page_content (
            pageId TEXT NOT NULL PRIMARY KEY, docJson TEXT NOT NULL, updatedAt INTEGER NOT NULL,
            format TEXT NOT NULL,
            FOREIGN KEY(pageId) REFERENCES pages(id) ON DELETE CASCADE
        );
        CREATE TABLE page_revisions (
            id TEXT NOT NULL PRIMARY KEY, pageId TEXT NOT NULL, createdAt INTEGER NOT NULL,
            format TEXT NOT NULL, encoding TEXT NOT NULL, byteCount INTEGER NOT NULL,
            sha256 TEXT NOT NULL, payload BLOB NOT NULL, inkFormat TEXT NOT NULL,
            inkEncoding TEXT NOT NULL, inkByteCount INTEGER NOT NULL, inkSha256 TEXT NOT NULL,
            inkPayload BLOB NOT NULL,
            FOREIGN KEY(pageId) REFERENCES pages(id) ON DELETE CASCADE
        );
        CREATE TABLE ink_strokes (
            id TEXT NOT NULL PRIMARY KEY, pageId TEXT NOT NULL, seq INTEGER NOT NULL,
            brushFamily TEXT NOT NULL, brushVersion INTEGER NOT NULL, sizeDp REAL NOT NULL,
            colorArgb INTEGER NOT NULL, epsilon REAL NOT NULL, stabilization INTEGER NOT NULL,
            minX REAL NOT NULL, minY REAL NOT NULL, maxX REAL NOT NULL, maxY REAL NOT NULL,
            points BLOB NOT NULL, enc TEXT NOT NULL, createdAt INTEGER NOT NULL, groupId TEXT,
            deletedAt INTEGER, colorFollowsTheme INTEGER,
            FOREIGN KEY(pageId) REFERENCES pages(id) ON DELETE CASCADE
        );
        CREATE TABLE ink_erases (
            id TEXT NOT NULL PRIMARY KEY, pageId TEXT NOT NULL, mode TEXT NOT NULL,
            sizeDp REAL NOT NULL, points BLOB NOT NULL, enc TEXT NOT NULL,
            createdAt INTEGER NOT NULL, deletedAt INTEGER,
            FOREIGN KEY(pageId) REFERENCES pages(id) ON DELETE CASCADE
        );
        CREATE TABLE ink_erase_targets (
            eraseId TEXT NOT NULL, strokeId TEXT NOT NULL, PRIMARY KEY(eraseId, strokeId),
            FOREIGN KEY(eraseId) REFERENCES ink_erases(id) ON DELETE CASCADE
        );
        CREATE TABLE ink_moves (
            id TEXT NOT NULL PRIMARY KEY, pageId TEXT NOT NULL, dxDp REAL NOT NULL,
            dyDp REAL NOT NULL, scaleX REAL NOT NULL, scaleY REAL NOT NULL,
            anchorX REAL NOT NULL, anchorY REAL NOT NULL, points BLOB NOT NULL, enc TEXT NOT NULL,
            createdAt INTEGER NOT NULL, deletedAt INTEGER,
            FOREIGN KEY(pageId) REFERENCES pages(id) ON DELETE CASCADE
        );
        CREATE TABLE ink_move_targets (
            moveId TEXT NOT NULL, strokeId TEXT NOT NULL, PRIMARY KEY(moveId, strokeId),
            FOREIGN KEY(moveId) REFERENCES ink_moves(id) ON DELETE CASCADE
        );
        CREATE TABLE room_master_table (id INTEGER NOT NULL PRIMARY KEY, identity_hash TEXT);
        CREATE TABLE vive_bundle (key TEXT NOT NULL PRIMARY KEY, value TEXT NOT NULL);
        "#
    ))?;

    let transaction = database.transaction()?;
    transaction.execute("INSERT INTO android_metadata(locale) VALUES ('en_US')", [])?;
    transaction.execute(
        "INSERT INTO room_master_table(id, identity_hash) VALUES (42, ?)",
        ["vive-portable-v1"],
    )?;
    transaction.execute(
        "INSERT INTO notebooks VALUES (?, ?, ?, 0, 1, ?, ?, NULL)",
        params![
            notebook.id,
            notebook.name,
            notebook.color_argb,
            notebook.created_at,
            notebook.updated_at
        ],
    )?;
    transaction.execute(
        "INSERT INTO vive_bundle VALUES ('format', ?), ('formatVersion', ?), ('appSchemaVersion', ?), ('notebookId', ?)",
        params![FORMAT, FORMAT_VERSION.to_string(), APP_SCHEMA_VERSION.to_string(), notebook.id],
    )?;
    for (section_index, section) in notebook.sections.iter().enumerate() {
        transaction.execute(
            "INSERT INTO sections VALUES (?, ?, ?, ?, ?, ?, ?, NULL)",
            params![
                section.id,
                notebook.id,
                section.name,
                section.color_argb,
                section_index as i32,
                section.created_at,
                section.updated_at
            ],
        )?;
        for (page_index, page) in section.pages.iter().enumerate() {
            transaction.execute(
                "INSERT INTO pages VALUES (?, ?, ?, ?, ?, ?, ?, NULL)",
                params![
                    page.id,
                    section.id,
                    page.title,
                    page_index as i32,
                    page.preview,
                    page.created_at,
                    page.updated_at
                ],
            )?;
            transaction.execute(
                "INSERT INTO page_content VALUES (?, ?, ?, 'json/1')",
                params![page.id, page.doc_json, page.updated_at],
            )?;
            for stroke in &page.strokes {
                transaction.execute(
                    "INSERT INTO ink_strokes VALUES (?, ?, ?, ?, 1, ?, ?, 0.1, 0, ?, ?, ?, ?, ?, 'ink/androidx1', ?, NULL, NULL, ?)",
                    params![
                        stroke.id,
                        page.id,
                        stroke.seq,
                        stroke.brush_family,
                        stroke.size_dp,
                        stroke.color_argb,
                        stroke.min_x,
                        stroke.min_y,
                        stroke.max_x,
                        stroke.max_y,
                        stroke.points,
                        stroke.created_at,
                        stroke.color_follows_theme
                    ],
                )?;
            }
        }
    }
    for attachment in &notebook.attachments {
        transaction.execute(
            "INSERT INTO attachments VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                attachment.id,
                attachment.mime_type,
                attachment.pixel_width,
                attachment.pixel_height,
                attachment.bytes.len() as u64,
                attachment.ref_count,
                attachment.created_at
            ],
        )?;
    }
    transaction.commit()?;
    database.execute_batch("VACUUM; PRAGMA optimize;")?;
    Ok(())
}

fn validate_database(notebook: &ConvertedNotebook, path: &Path) -> Result<()> {
    let database = Connection::open(path)?;
    let quick: String = database.query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?;
    if quick != "ok" {
        bail!("SQLite quick_check failed: {quick}")
    }
    let foreign_keys: i64 =
        database.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    if foreign_keys != 0 {
        bail!("SQLite foreign-key validation failed")
    }
    let app_id: u32 = database.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    let user_version: u32 = database.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if app_id != APPLICATION_ID || user_version != FORMAT_VERSION {
        bail!("SQLite bundle format markers are incorrect")
    }
    let page_count: usize = notebook
        .sections
        .iter()
        .map(|section| section.pages.len())
        .sum();
    let content_count: usize =
        database.query_row("SELECT COUNT(*) FROM page_content", [], |row| row.get(0))?;
    if content_count != page_count {
        bail!("not every page has a document")
    }
    let mut docs = database.prepare("SELECT docJson FROM page_content")?;
    for doc in docs.query_map([], |row| row.get::<_, String>(0))? {
        let value: serde_json::Value = serde_json::from_str(&doc?)?;
        if value.get("schema").and_then(serde_json::Value::as_u64) != Some(2) {
            bail!("generated an invalid page document")
        }
    }
    let mut points = database.prepare("SELECT points FROM ink_strokes")?;
    for encoded in points.query_map([], |row| row.get::<_, Vec<u8>>(0))? {
        validate_gzip_proto(&encoded?)?;
    }
    Ok(())
}

fn validate_archive(path: &Path) -> Result<()> {
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut names = BTreeSet::new();
    let mut files = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        if !names.insert(name.clone()) {
            bail!("generated archive has a duplicate entry: {name}")
        }
        if name != "manifest.json"
            && name != "notebook.sqlite"
            && name != "checksums.sha256"
            && !name.strip_prefix("attachments/").is_some_and(|id| {
                id.len() == 64 && id.chars().all(|character| character.is_ascii_hexdigit())
            })
        {
            bail!("generated archive has a forbidden entry: {name}")
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        files.insert(name, bytes);
    }
    if !names.contains("manifest.json")
        || !names.contains("notebook.sqlite")
        || !names.contains("checksums.sha256")
    {
        bail!("generated archive is missing a required entry")
    }
    let expected = String::from_utf8(files.remove("checksums.sha256").unwrap())?;
    let actual = files
        .iter()
        .map(|(name, bytes)| format!("{}  {name}\n", sha256(bytes)))
        .collect::<String>();
    if expected != actual {
        bail!("generated archive checksum list does not match its entries")
    }
    Ok(())
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
