use leptos::prelude::{Get, Signal};
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crate::util::layout_context::expect_layout_context;

/// Helper: acquire 2D rendering context
pub fn get_2d_ctx(canvas: &HtmlCanvasElement) -> Option<CanvasRenderingContext2d> {
    canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|ctx| ctx.dyn_into::<CanvasRenderingContext2d>().ok())
}

/// Try to resolve a CSS variable value from the document root.
/// Returns a CSS color string if found, else None.
pub fn resolve_theme_color(var_name: &str) -> Option<String> {
    let window = web_sys::window()?;
    let document = window.document()?;
    let root = document.document_element()?;
    let styles = window.get_computed_style(&root).ok()??;
    let value = styles.get_property_value(var_name).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn is_dark_mode() -> Signal<bool> {
    let ctx = expect_layout_context();
    Signal::derive(move || ctx.scale.get().is_dark())
}
