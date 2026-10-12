//! Bounded parsing plus public-ABI construction of an owned multipart snapshot.

mod parse;
mod values;

pub(crate) use parse::parse;
pub(crate) use values::build;

// Private snapshot tags shared with Multipart.result() in multipart.vut.
// They are framework data, not a new ABI status or binary representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub(crate) enum Issue {
    NotMultipart = 1,
    MissingBoundary = 2,
    InvalidBoundary = 3,
    Malformed = 4,
    InvalidText = 5,
    InvalidDisposition = 6,
    TooManyParts = 7,
}

pub(crate) struct Upload {
    name: String,
    filename: Option<String>,
    content_type: Option<String>,
    bytes: multer::bytes::Bytes,
}

#[derive(Default)]
pub(crate) struct Form {
    issue: Option<Issue>,
    fields: Vec<(String, String)>,
    files: Vec<Upload>,
}

impl Form {
    fn failed(issue: Issue) -> Self {
        Self {
            issue: Some(issue),
            ..Self::default()
        }
    }
}
