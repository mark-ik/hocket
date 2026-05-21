//! Combination grid — the Deeler primary surface.
//!
//! One row per track, one cell per variation slot; the active variation
//! is bracketed. Clicking a cell makes it the playing variation for
//! that track (scheduled at the next bar by the engine). This is the
//! same `select_variation` gesture as the SelectOne strip, transposed
//! into a grid you can read across.

use xilem::style::Style;
use xilem::view::{flex_col, flex_row, label, text_button, AnyFlexChild, FlexExt};
use xilem::WidgetView;

use strophe_widgets::theme::{mono_family, SP_1, SP_2, TS_SM, TS_XS};

use crate::AppState;

pub fn combination_surface(state: &AppState) -> impl WidgetView<AppState> + use<> {
    let rows: Vec<AnyFlexChild<AppState>> = state
        .session
        .tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let active = track.playback_mode.active_layer();
            let mut cells: Vec<AnyFlexChild<AppState>> = Vec::new();
            cells.push(label(track.name.clone()).text_size(TS_XS).into_any_flex());
            for (li, _layer) in track.layers.iter().enumerate() {
                let li_u = li as u16;
                let is_active = active == Some(li_u);
                let lbl = if is_active {
                    format!("[v{}]", li + 1)
                } else {
                    format!(" v{} ", li + 1)
                };
                cells.push(
                    text_button(lbl, move |s: &mut AppState| s.select_variation(i, li_u))
                        .into_any_flex(),
                );
            }
            if track.layers.is_empty() {
                cells.push(label("(no layers)").text_size(TS_XS).into_any_flex());
            }
            flex_row(cells).gap(SP_2).into_any_flex()
        })
        .collect();

    flex_col((
        label("Combination grid").text_size(TS_SM),
        label("pick the active variation per track").text_size(TS_XS).font(mono_family()),
        flex_col(rows).gap(SP_1),
    ))
    .gap(SP_2)
}
