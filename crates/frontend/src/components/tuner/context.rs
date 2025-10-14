//! Tuner context for sharing state across components

use leptos::prelude::*;

use common::tuner::{Config, SpectrumData};

/// Tuner context for sharing configuration and spectrum data
#[derive(Clone, Copy)]
pub struct TunerContext {
    /// Current tuner configuration
    pub config: RwSignal<Option<Config>>,

    /// Current spectrum data from FFT analysis
    pub spectrum: RwSignal<Option<SpectrumData>>,

    /// Currently active (selected) sensor for editing
    pub active_sensor: RwSignal<Option<usize>>,
}

impl TunerContext {
    /// Create a new tuner context
    pub fn new() -> Self {
        Self {
            config: RwSignal::new(None),
            spectrum: RwSignal::new(None),
            active_sensor: RwSignal::new(None),
        }
    }
}

/// Provide tuner context into the current scope
pub fn provide_tuner_context() {
    let context = TunerContext::new();
    provide_context(context);
}

/// Provides tuner context to child components
#[component]
pub fn TunerContextProvider(children: Children) -> impl IntoView {
    provide_tuner_context();
    children()
}

/// Get the tuner context from the current scope
pub fn use_tuner_context() -> TunerContext {
    use_context::<TunerContext>().expect(
        "TunerContext not found. Make sure to wrap your component with TunerContextProvider",
    )
}
