//! The four pages of the main window.

pub mod acquire;
pub mod analysis;
pub mod settings;
pub mod sweep;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Acquire,
    Sweep,
    Analysis,
    Settings,
}

impl Page {
    pub const ALL: [Self; 4] = [Self::Acquire, Self::Sweep, Self::Analysis, Self::Settings];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Acquire => "1:ACQUIRE",
            Self::Sweep => "2:SWEEP",
            Self::Analysis => "3:ANALYSIS",
            Self::Settings => "4:CONFIG",
        }
    }
}
