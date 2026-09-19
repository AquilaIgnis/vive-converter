#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use anyhow::Result;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use clap::Parser as ClapParser;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use onenote_parser::section::{Section, SectionEntry};
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use std::path::PathBuf;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use vive_converter::{Source, VERSION, convert_source, parse_input, write_bundle};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn main() {}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
#[derive(Debug, ClapParser)]
#[command(
    about = "Convert OneNote notebooks to ViveNotes .vive bundles",
    version = VERSION,
    disable_version_flag = true
)]
struct Args {
    /// Print the converter version.
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    version: Option<bool>,

    /// A OneNote .onepkg, .onetoc2, or .one file.
    input: PathBuf,

    /// Output .vive path. Omit with --inspect to only summarize the source.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Replace an existing output file.
    #[arg(long)]
    force: bool,

    /// Parse and summarize the notebook without writing output.
    #[arg(long)]
    inspect: bool,

    /// Include representative layout and ink details in inspection output.
    #[arg(long, requires = "inspect")]
    verbose: bool,
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn main() -> Result<()> {
    let args = Args::parse();
    let notebook = parse_input(&args.input)?;
    print_summary(&notebook);
    if args.verbose {
        print_representative_content(&notebook);
    }

    if args.inspect {
        return Ok(());
    }
    let output = args
        .output
        .unwrap_or_else(|| args.input.with_extension("vive"));
    let converted = convert_source(&notebook, &args.input)?;
    for warning in &converted.warnings {
        eprintln!("conversion warning: {warning}");
    }
    let summary = write_bundle(&converted, &output, args.force)?;
    println!(
        "wrote {} ({} bytes; sections={}, pages={}, strokes={}, attachments={}, warnings={})",
        output.display(),
        summary.bytes,
        summary.sections,
        summary.pages,
        summary.strokes,
        summary.attachments,
        converted.warnings.len(),
    );
    Ok(())
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn print_summary(source: &Source) {
    #[derive(Default)]
    struct Counts {
        sections: usize,
        pages: usize,
        outlines: usize,
        inks: usize,
        strokes: usize,
        images: usize,
        unknown: usize,
    }

    fn summarize_section(section: &Section, counts: &mut Counts) {
        counts.sections += 1;
        println!("section: {}", section.display_name());
        for page in section
            .page_series()
            .iter()
            .flat_map(|series| series.pages())
        {
            counts.pages += 1;
            let mut page_strokes = 0usize;
            for content in page.contents() {
                if content.outline().is_some() {
                    counts.outlines += 1;
                } else if let Some(ink) = content.ink() {
                    counts.inks += 1;
                    page_strokes += count_strokes(ink);
                } else if content.image().is_some() {
                    counts.images += 1;
                } else {
                    counts.unknown += 1;
                }
            }
            counts.strokes += page_strokes;
            println!(
                "  page: {:?} (contents={}, strokes={})",
                page.title_text(),
                page.contents().len(),
                page_strokes
            );
        }
    }

    fn walk(entries: &[SectionEntry], counts: &mut Counts) {
        for entry in entries {
            match entry {
                SectionEntry::Section(section) => summarize_section(section, counts),
                SectionEntry::SectionGroup(group) => walk(group.entries(), counts),
            }
        }
    }

    let mut counts = Counts::default();
    match source {
        Source::Notebook(notebook) => {
            walk(notebook.entries(), &mut counts);
            for warning in notebook.report().warnings() {
                eprintln!("warning: {}", warning.message());
            }
        }
        Source::Section(section) => {
            summarize_section(section, &mut counts);
            for warning in section.report().warnings() {
                eprintln!("warning: {}", warning.message());
            }
        }
    }
    println!(
        "summary: sections={}, pages={}, outlines={}, ink_objects={}, strokes={}, images={}, unknown={}",
        counts.sections,
        counts.pages,
        counts.outlines,
        counts.inks,
        counts.strokes,
        counts.images,
        counts.unknown,
    );
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn count_strokes(ink: &onenote_parser::contents::Ink) -> usize {
    ink.ink_strokes().len() + ink.child_groups().iter().map(count_strokes).sum::<usize>()
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn print_representative_content(source: &Source) {
    fn inspect_section(section: &Section) -> bool {
        let Some(page) = section
            .page_series()
            .iter()
            .flat_map(|series| series.pages())
            .next()
        else {
            return false;
        };
        println!("representative page: {:?}", page.title_text());
        for (index, content) in page.contents().iter().take(20).enumerate() {
            if let Some(outline) = content.outline() {
                println!(
                    "  [{index}] outline x={:?} y={:?} width={:?} items={}",
                    outline.offset_horizontal(),
                    outline.offset_vertical(),
                    outline.layout_max_width(),
                    outline.items().len(),
                );
            } else if let Some(ink) = content.ink() {
                let stroke = ink.ink_strokes().first();
                println!(
                    "  [{index}] ink x={:?} y={:?} bbox={:?} strokes={} first={:?}",
                    ink.offset_horizontal(),
                    ink.offset_vertical(),
                    ink.bounding_box(),
                    count_strokes(ink),
                    stroke.map(|stroke| (
                        stroke.path().len(),
                        stroke.width(),
                        stroke.height(),
                        stroke.color(),
                        stroke.transparency(),
                        stroke.path().first(),
                        stroke.path().get(1),
                    )),
                );
            } else if let Some(image) = content.image() {
                println!(
                    "  [{index}] image x={:?} y={:?} size={:?}x{:?} ext={:?} bytes={:?}",
                    image.offset_horizontal(),
                    image.offset_vertical(),
                    image.picture_width(),
                    image.picture_height(),
                    image.extension(),
                    image.size(),
                );
            }
        }
        true
    }

    fn walk(entries: &[SectionEntry]) -> bool {
        for entry in entries {
            let found = match entry {
                SectionEntry::Section(section) => inspect_section(section),
                SectionEntry::SectionGroup(group) => walk(group.entries()),
            };
            if found {
                return true;
            }
        }
        false
    }

    match source {
        Source::Notebook(notebook) => {
            walk(notebook.entries());
        }
        Source::Section(section) => {
            inspect_section(section);
        }
    }
}
