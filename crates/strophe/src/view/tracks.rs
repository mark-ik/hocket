//! Track surface — profile-aware strips.
//!
//! Dispatches on `track.playback_mode`: `Sum` (looper) renders a
//! compact strip that expands to per-layer mute/gain; `SelectOne`
//! (Deeler) renders a variation-slot row.

use masonry::layout::AsUnit;
use masonry::peniko::Color;
use xilem::core::one_of::OneOf2;
use xilem::style::Style;
use xilem::view::{flex_col, flex_row, label, sized_box, text_button, AnyFlexChild, FlexExt};
use xilem::WidgetView;

use strophe_model::PlaybackMode;
use strophe_widgets::theme::{mono_family, SP_1, SP_2, SP_3, TS_XS};
use strophe_widgets::waveform_view;

use crate::{AppState, WAVEFORM_H, WAVEFORM_W};

/// Height of each per-layer waveform in the expanded looper strip —
/// shorter than the compact combined waveform so a deep stack stays
/// scannable.
const LAYER_WAVEFORM_H: f64 = 24.0;

pub fn tracks_surface(state: &AppState) -> impl WidgetView<AppState> + use<> {
    let rows: Vec<AnyFlexChild<AppState>> = state
        .session
        .tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let strip = match track.playback_mode {
                PlaybackMode::Sum => OneOf2::A(sum_strip(state, i)),
                PlaybackMode::SelectOne { .. } => OneOf2::B(selectone_strip(state, i)),
            };
            strip.into_any_flex()
        })
        .collect();

    flex_col((
        label("Tracks (click name to arm)").text_size(TS_XS),
        flex_col(rows).gap(SP_2),
    ))
    .gap(SP_2)
}

/// Looper/Sum strip: compact header (arm + summed waveform + expand
/// toggle); when expanded, a per-layer mute/gain row for each layer.
fn sum_strip(state: &AppState, i: usize) -> impl WidgetView<AppState> + use<> {
    let track = &state.session.tracks[i];
    let arm_label = format!("{}  {}", if track.armed { "●" } else { "○" }, track.name);
    let wave_color = Color::from_rgb8(track.color.r, track.color.g, track.color.b);
    let zero = state.palette.text_disabled;
    // Compact strip shows the combined (all-layers-summed) waveform.
    let peaks = state.combined_peaks.get(i).cloned().unwrap_or_default();
    let n = track.layers.len();
    let expanded = state.expanded_track == Some(i);
    let expand_label = if expanded {
        "▾ layers".to_string()
    } else {
        format!("▸ {n} layer(s)")
    };

    let header = flex_row((
        text_button(arm_label, move |s: &mut AppState| s.arm(i)),
        sized_box(waveform_view(peaks, wave_color, zero))
            .width(WAVEFORM_W.px())
            .height(WAVEFORM_H.px()),
        text_button(expand_label, move |s: &mut AppState| s.toggle_expand(i)),
    ))
    .gap(SP_3);

    let layer_rows: Vec<AnyFlexChild<AppState>> = if expanded {
        track
            .layers
            .iter()
            .enumerate()
            .map(|(li, layer)| {
                let li_u = li as u16;
                let mute_label = if layer.muted { "muted" } else { "on" };
                let gain_label = format!("{:.2}", layer.gain);
                // This layer's own waveform (the point of expanding).
                let layer_pk = state
                    .layer_peaks
                    .get(i)
                    .and_then(|v| v.get(li))
                    .cloned()
                    .unwrap_or_default();
                flex_row((
                    label(format!("L{li}")).text_size(TS_XS),
                    text_button(mute_label, move |s: &mut AppState| {
                        s.toggle_layer_mute(i, li_u)
                    }),
                    text_button("–", move |s: &mut AppState| s.nudge_layer_gain(i, li_u, -0.1)),
                    label(gain_label).text_size(TS_XS).font(mono_family()),
                    text_button("+", move |s: &mut AppState| s.nudge_layer_gain(i, li_u, 0.1)),
                    sized_box(waveform_view(layer_pk, wave_color, zero))
                        .width(WAVEFORM_W.px())
                        .height(LAYER_WAVEFORM_H.px()),
                ))
                .gap(SP_2)
                .into_any_flex()
            })
            .collect()
    } else {
        Vec::new()
    };

    flex_col((header, flex_col(layer_rows).gap(SP_1))).gap(SP_1)
}

/// Deeler/SelectOne strip: arm + a variation-slot row. The active slot
/// is marked; clicking a slot makes it the playing variation.
fn selectone_strip(state: &AppState, i: usize) -> impl WidgetView<AppState> + use<> {
    let track = &state.session.tracks[i];
    let arm_label = format!("{}  {}", if track.armed { "●" } else { "○" }, track.name);
    let active = track.playback_mode.active_layer();

    let slots: Vec<AnyFlexChild<AppState>> = track
        .layers
        .iter()
        .enumerate()
        .map(|(li, _layer)| {
            let li_u = li as u16;
            let is_active = active == Some(li_u);
            let lbl = format!("{}v{}", if is_active { "● " } else { "" }, li + 1);
            text_button(lbl, move |s: &mut AppState| s.select_variation(i, li_u)).into_any_flex()
        })
        .collect();

    flex_row((
        text_button(arm_label, move |s: &mut AppState| s.arm(i)),
        flex_row(slots).gap(SP_2),
    ))
    .gap(SP_3)
}
