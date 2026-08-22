pub mod inline;
pub mod lines;

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Markdown,
    Html,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadingEvidence<'a> {
    pub level: u8,
    pub text: &'a str,
    pub explicit_id: Option<&'a str>,
    pub offset: usize,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageEvidence {
    pub kind: ImageKind,
    pub destination: String,
    pub offset: usize,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerEvidence<'a> {
    pub name: &'a str,
    pub argument: Option<&'a str>,
    pub offset: usize,
    pub line: usize,
    pub body_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ScanEvidence<'a> {
    pub headings: Vec<HeadingEvidence<'a>>,
    pub images: Vec<ImageEvidence>,
    pub containers: Vec<ContainerEvidence<'a>>,
    pub has_emphasis: bool,
    pub has_code: bool,
}

pub fn scan_document(body: &str) -> ScanEvidence<'_> {
    lines::scan_lines(body)
}
