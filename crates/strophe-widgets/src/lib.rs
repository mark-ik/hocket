//! Custom Masonry/Vello widgets for Strophe.
//!
//! The waveform widget + peak computation that shipped here at FT5
//! have been extracted to the shared [`audio_widgets`] crate (in the
//! woodshed repo) — the first realized step of the pressure-vessel
//! doctrine's `audio-widgets` extraction. They are re-exported below
//! so existing `strophe_widgets::{waveform_view, compute_peaks, Peak}`
//! call sites keep working unchanged.
//!
//! Strophe-specific widgets (track strip, transport, combination grid)
//! will live here directly when implemented, following the same
//! canvas-closure pattern.

pub use audio_widgets::{compute_peaks, db_to_norm, meter_view, theme, waveform_view, Peak};

// Domain-neutral components (combobox, …) from the layer beneath
// audio-widgets. Re-exported so Strophe call sites use
// `strophe_widgets::combobox` without a direct xilem-components dep.
pub use xilem_components::combobox;
