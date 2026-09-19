use crate::{VERSION, convert_bytes};
use std::path::Path;
use wasm_bindgen::prelude::*;

/// Result returned by the browser conversion API.
#[wasm_bindgen]
pub struct WasmConversion {
    bytes: Vec<u8>,
    output_file_name: String,
    warnings_json: String,
    sections: usize,
    pages: usize,
    strokes: usize,
    attachments: usize,
}

#[wasm_bindgen]
impl WasmConversion {
    #[wasm_bindgen(getter, js_name = outputFileName)]
    pub fn output_file_name(&self) -> String {
        self.output_file_name.clone()
    }

    #[wasm_bindgen(getter, js_name = warningsJson)]
    pub fn warnings_json(&self) -> String {
        self.warnings_json.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn sections(&self) -> usize {
        self.sections
    }

    #[wasm_bindgen(getter)]
    pub fn pages(&self) -> usize {
        self.pages
    }

    #[wasm_bindgen(getter)]
    pub fn strokes(&self) -> usize {
        self.strokes
    }

    #[wasm_bindgen(getter)]
    pub fn attachments(&self) -> usize {
        self.attachments
    }

    #[wasm_bindgen(getter, js_name = byteLength)]
    pub fn byte_length(&self) -> usize {
        self.bytes.len()
    }

    /// Move the generated `.vive` archive into JavaScript as a `Uint8Array`.
    #[wasm_bindgen(js_name = intoBytes)]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Convert an uploaded `.onepkg` or `.one` byte array to a `.vive` archive.
#[wasm_bindgen(js_name = convert)]
pub fn convert_wasm(input: &[u8], file_name: &str) -> Result<WasmConversion, JsValue> {
    let conversion = convert_bytes(input, file_name)
        .map_err(|error| JsValue::from_str(&format!("{error:#}")))?;
    let warnings_json = serde_json::to_string(&conversion.warnings)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("notebook");

    Ok(WasmConversion {
        bytes: conversion.bytes,
        output_file_name: format!("{stem}.vive"),
        warnings_json,
        sections: conversion.summary.sections,
        pages: conversion.summary.pages,
        strokes: conversion.summary.strokes,
        attachments: conversion.summary.attachments,
    })
}

#[wasm_bindgen(js_name = version)]
pub fn wasm_version() -> String {
    VERSION.to_owned()
}
