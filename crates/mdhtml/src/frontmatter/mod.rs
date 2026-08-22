mod block;
mod flow;
mod scalar;

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Sequence(Vec<Value>),
    Mapping(Vec<(String, Value)>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedFrontMatter<'a> {
    pub front_matter: Value,
    pub raw: &'a str,
    pub body: &'a str,
    pub body_offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontMatterError {
    code: &'static str,
    line: usize,
    column: usize,
    message: String,
}

impl FrontMatterError {
    fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            code: "E-PARSE-01",
            line,
            column,
            message: message.into().replace(['\n', '\r'], " "),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn column(&self) -> usize {
        self.column
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for FrontMatterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for FrontMatterError {}

#[derive(Clone, Copy)]
pub(super) struct Line<'a> {
    pub text: &'a str,
    pub end: usize,
}

#[derive(Clone, Copy)]
pub(super) struct Significant {
    pub index: usize,
    pub column: usize,
}

pub(super) struct Parser<'a> {
    pub source: &'a str,
    pub lines: Vec<Line<'a>>,
    pub index: usize,
}

impl<'a> Parser<'a> {
    fn error(&self, message: impl Into<String>, line: usize, column: usize) -> FrontMatterError {
        FrontMatterError::new(message, line, column)
    }

    pub(super) fn peek(&self) -> Result<Option<Significant>, FrontMatterError> {
        let mut index = self.index;
        while index < self.lines.len() {
            let line = self.lines[index];
            let Some(column) = indentation(line.text, self, index)? else {
                index += 1;
                continue;
            };
            if line.text.as_bytes().get(column) == Some(&b'#') {
                index += 1;
                continue;
            }
            return Ok(Some(Significant { index, column }));
        }
        Ok(None)
    }

    fn parse(mut self) -> Result<ParsedFrontMatter<'a>, FrontMatterError> {
        let front_matter = self.parse_block(0, true)?;
        let closing = self.peek()?.filter(|significant| {
            significant.column == 0 && self.lines[significant.index].text == "---"
        });
        let Some(closing) = closing else {
            let line = self
                .peek()?
                .map_or(self.lines.len(), |value| value.index + 1);
            return Err(self.error("unterminated front matter", line, 1));
        };
        let line = self.lines[closing.index];
        let body_offset = if line.end < self.source.len() {
            line.end + 1
        } else {
            self.source.len()
        };
        Ok(ParsedFrontMatter {
            front_matter: if front_matter_is_null(&front_matter) {
                Value::Mapping(Vec::new())
            } else {
                front_matter
            },
            raw: &self.source[..body_offset],
            body: &self.source[body_offset..],
            body_offset,
        })
    }
}

fn front_matter_is_null(value: &Value) -> bool {
    matches!(value, Value::Null)
}

fn indentation(
    text: &str,
    parser: &Parser<'_>,
    line_index: usize,
) -> Result<Option<usize>, FrontMatterError> {
    for (index, byte) in text.bytes().enumerate() {
        match byte {
            b' ' => continue,
            b'\t' => {
                return Err(parser.error(
                    "tab indentation is not allowed",
                    line_index + 1,
                    index + 1,
                ));
            }
            _ => return Ok(Some(index)),
        }
    }
    Ok(None)
}

pub(super) fn is_space(byte: u8) -> bool {
    byte == b' ' || byte == b'\t'
}

pub(super) fn is_sequence_indicator(text: &str, column: usize) -> bool {
    text.as_bytes().get(column) == Some(&b'-')
        && text
            .as_bytes()
            .get(column + 1)
            .is_none_or(|byte| *byte == b' ')
}

pub fn parse_front_matter(source: &str) -> Result<ParsedFrontMatter<'_>, FrontMatterError> {
    let lines = split_lines(source);
    if lines.first().map(|line| line.text) != Some("---") {
        return Ok(ParsedFrontMatter {
            front_matter: Value::Mapping(Vec::new()),
            raw: "",
            body: source,
            body_offset: 0,
        });
    }
    Parser {
        source,
        lines,
        index: 1,
    }
    .parse()
}

fn split_lines(source: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            lines.push(make_line(source, start, index));
            start = index + 1;
        }
    }
    lines.push(make_line(source, start, source.len()));
    lines
}

fn make_line(source: &str, start: usize, end: usize) -> Line<'_> {
    let mut text_end = end;
    if text_end > start && source.as_bytes()[text_end - 1] == b'\r' {
        text_end -= 1;
    }
    Line {
        text: &source[start..text_end],
        end,
    }
}
