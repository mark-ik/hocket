//! View surfaces for the Strophe app.
//!
//! A persistent [`transport`] bar sits above whichever [`Surface`] is
//! active. Each surface is a builder that takes `&AppState` and emits a
//! `WidgetView<AppState>`; the action closures call the helper methods
//! on [`crate::AppState`]. Strophe-specific composition lives here (not
//! in `strophe-widgets`) because it reads `AppState` — `strophe-widgets`
//! stays for data-parameterized, state-agnostic widgets.

pub mod combination;
pub mod settings;
pub mod tracks;
pub mod transport;

use xilem::core::one_of::OneOf3;
use xilem::style::Style;
use xilem::view::flex_col;
use xilem::WidgetView;

use strophe_widgets::theme::SP_3;

use crate::AppState;

/// Which top-level surface shows below the transport bar.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Surface {
    /// The track column — profile-aware strips.
    #[default]
    Tracks,
    /// The Deeler combination grid (tracks × variation slots).
    Combination,
    /// Session settings (profile, tempo, …).
    Settings,
}

impl Surface {
    pub fn label(self) -> &'static str {
        match self {
            Self::Tracks => "Tracks",
            Self::Combination => "Combination",
            Self::Settings => "Settings",
        }
    }
}

/// The whole app body: persistent transport bar + the active surface.
pub fn app_shell(state: &AppState) -> impl WidgetView<AppState> + use<> {
    let surface = match state.surface {
        Surface::Tracks => OneOf3::A(tracks::tracks_surface(state)),
        Surface::Combination => OneOf3::B(combination::combination_surface(state)),
        Surface::Settings => OneOf3::C(settings::settings_surface(state)),
    };
    flex_col((transport::transport(state), surface)).gap(SP_3)
}
