use serde::{Deserialize, Serialize};

/// What a conversion could not carry over as it was, in words for the user: every remark names
/// one thing, and whether it still plays.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    #[serde(default)]
    pub remarks: Vec<Remark>,
}

impl Report {
    pub fn new() -> Self {
        Self::default()
    }

    /// Something that plays, but not as the source did.
    pub fn approximated(&mut self, text: impl Into<String>) {
        self.remarks.push(Remark {
            kind: RemarkKind::Approximated,
            text: text.into(),
        });
    }

    /// Something kept under the format's extension, which other programs do not play.
    pub fn unplayed(&mut self, text: impl Into<String>) {
        self.remarks.push(Remark {
            kind: RemarkKind::Unplayed,
            text: text.into(),
        });
    }

    pub fn is_empty(&self) -> bool {
        self.remarks.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remark {
    pub kind: RemarkKind,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemarkKind {
    /// It plays, but not as the source did.
    Approximated,
    /// It is kept, and other programs do not play it.
    Unplayed,
}

/// Where a conversion tells its progress, and is asked whether to stop: a format and [`write`]
/// look at [`Progress::cancelled`] between steps and give [`Error::Cancelled`].
///
/// [`write`]: crate::write
/// [`Error::Cancelled`]: crate::Error::Cancelled
pub trait Progress {
    /// `done` of `total` steps, the one now under way named.
    fn step(&mut self, _done: u64, _total: u64, _what: &str) {}

    /// Whether the conversion is to stop.
    fn cancelled(&self) -> bool {
        false
    }
}

/// A progress nobody watches.
#[derive(Clone, Copy, Debug, Default)]
pub struct Quiet;

impl Progress for Quiet {}
