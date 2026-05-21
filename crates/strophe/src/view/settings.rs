//! Session settings surface.
//!
//! The configurability goal lives here. For now: switch the whole
//! session between the looper-pedal and Deeler profiles, and read back
//! the session's transport/track summary. Per-setting controls (track
//! count, master clock, count-in, click) land here as the model grows
//! the fields to back them.

use xilem::style::Style;
use xilem::view::{flex_col, flex_row, label, text_button};
use xilem::WidgetView;

use strophe_widgets::theme::{mono_family, SP_2, SP_3, TS_SM, TS_XS};

use crate::AppState;

pub fn settings_surface(state: &AppState) -> impl WidgetView<AppState> + use<> {
    let deeler = state.is_deeler();
    let profile_line = format!(
        "Profile: {}",
        if deeler {
            "Deeler (select-one variations)"
        } else {
            "Looper (summed overdub)"
        }
    );
    let session_line = format!(
        "{} tracks · {} bars/phrase · {} BPM · {}/{}",
        state.session.tracks.len(),
        state.session.bars_per_phrase,
        state.session.bpm as u32,
        state.session.time_signature.numerator,
        state.session.time_signature.denominator,
    );

    flex_col((
        label("Session settings").text_size(TS_SM),
        label(profile_line).text_size(TS_XS),
        flex_row((
            text_button("Looper profile", |s: &mut AppState| s.switch_profile(false)),
            text_button("Deeler profile", |s: &mut AppState| s.switch_profile(true)),
        ))
        .gap(SP_3),
        label(session_line).text_size(TS_XS).font(mono_family()),
        label("switching profile starts a fresh session").text_size(TS_XS),
    ))
    .gap(SP_2)
}
