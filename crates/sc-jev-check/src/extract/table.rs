//! `table-column` extractor.
//!
//! Cells split on unescaped `|` only. `\|` is one literal pipe inside a cell.
//! A subject's span covers the trimmed cell in the source line, so it is the
//! cell's own bytes rather than the whole row.

use sc_agent_types::{InputSpec, JevError, RepoRoot, Rev, Span, Subject};

use super::Extracted;
use super::Extractor;
use super::private::Sealed;
use super::read::read_repo_text;

/// Reads one markdown table column.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TableColumnExtractor;

impl Sealed for TableColumnExtractor {}

impl Extractor for TableColumnExtractor {
    fn kind(&self) -> &'static str {
        "table-column"
    }

    fn extract(
        &self,
        repo: &RepoRoot,
        rev: &Rev,
        input: &InputSpec,
    ) -> Result<Vec<Subject>, JevError> {
        Ok(extract_table(repo, rev, input)?.subjects)
    }
}

pub(crate) fn extract_table(
    repo: &RepoRoot,
    rev: &Rev,
    input: &InputSpec,
) -> Result<Extracted, JevError> {
    let InputSpec::TableColumn { path, column } = input else {
        return Err(JevError::ConfigInvalid {
            detail: "table-column extractor received a different input kind".into(),
        });
    };
    let text = read_repo_text(repo, rev, path)?;
    let subjects = column_subjects(&text, path, column)?;
    if subjects.is_empty() {
        return Err(JevError::ExtractionEmpty);
    }
    Ok(Extracted {
        subjects,
        document: None,
    })
}

fn column_subjects(text: &str, path: &str, column: &str) -> Result<Vec<Subject>, JevError> {
    let mut offset = 0usize;
    let mut line_no = 1u32;
    let mut rows = Vec::new();
    for raw in text.split_inclusive('\n') {
        let body = raw.trim_end_matches(['\n', '\r']);
        rows.push((line_no, offset, body));
        offset += raw.len();
        line_no = line_no.saturating_add(1);
    }

    let mut index = 0;
    while index < rows.len() {
        let (_, _, line) = rows[index];
        if !line.contains('|') {
            index += 1;
            continue;
        }
        let header = split_row(line);
        if header.iter().all(|cell| is_delimiter(&cell.text)) {
            index += 1;
            continue;
        }
        if let Some(column_index) = header.iter().position(|cell| cell.text == column) {
            return data_subjects(&rows[index + 1..], path, column, column_index);
        }
        index += 1;
    }
    Err(JevError::InputInvalid {
        detail: format!("column {column} was not found in {path}"),
    })
}

fn data_subjects(
    rows: &[(u32, usize, &str)],
    path: &str,
    column: &str,
    column_index: usize,
) -> Result<Vec<Subject>, JevError> {
    let mut subjects = Vec::new();
    for (line_no, offset, line) in rows {
        if line.is_empty() || !line.contains('|') {
            break;
        }
        let cells = split_row(line);
        if cells.iter().all(|cell| is_delimiter(&cell.text)) {
            continue;
        }
        let Some(cell) = cells.get(column_index) else {
            return Err(JevError::InputInvalid {
                detail: format!("{path}:{line_no} is missing column {column}"),
            });
        };
        if cell.text.is_empty() {
            continue;
        }
        let start_byte = byte_offset(offset + cell.start, path)?;
        let end_byte = byte_offset(offset + cell.end, path)?;
        subjects.push(Subject {
            id: format!("{path}:{line_no}"),
            capture: "column".into(),
            text: cell.text.clone(),
            span: Span {
                start_line: *line_no,
                end_line: *line_no,
                start_byte,
                end_byte,
            },
        });
    }
    Ok(subjects)
}

/// One table cell: unescaped, trimmed text and its byte range in the line.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cell {
    text: String,
    /// First byte of the trimmed cell in the line.
    start: usize,
    /// Byte one past the trimmed cell in the line.
    end: usize,
}

fn byte_offset(offset: usize, path: &str) -> Result<u32, JevError> {
    u32::try_from(offset).map_err(|_| JevError::InputInvalid {
        detail: format!("{path} is too large for a u32 byte span"),
    })
}

fn split_row(line: &str) -> Vec<Cell> {
    let mut raw = Vec::new();
    let mut cell_start = 0;
    let mut chars = line.char_indices().peekable();
    while let Some((at, ch)) = chars.next() {
        if ch == '\\' && chars.peek().is_some_and(|(_, next)| *next == '|') {
            chars.next();
            continue;
        }
        if ch == '|' {
            raw.push((cell_start, at));
            cell_start = at + 1;
        }
    }
    raw.push((cell_start, line.len()));
    let mut cells: Vec<Cell> = raw
        .into_iter()
        .map(|(start, end)| trimmed_cell(line, start, end))
        .collect();
    if line.trim_start().starts_with('|') && cells.first().is_some_and(|cell| cell.text.is_empty())
    {
        cells.remove(0);
    }
    if line.trim_end().ends_with('|') && cells.last().is_some_and(|cell| cell.text.is_empty()) {
        cells.pop();
    }
    cells
}

fn trimmed_cell(line: &str, start: usize, end: usize) -> Cell {
    let slice = &line[start..end];
    let leading = slice.len() - slice.trim_start().len();
    let trimmed = slice.trim();
    let start = start + leading;
    Cell {
        text: trimmed.replace("\\|", "|"),
        start,
        end: start + trimmed.len(),
    }
}

fn is_delimiter(cell: &str) -> bool {
    let bytes = cell.as_bytes();
    if bytes.len() < 3 {
        return false;
    }
    let start = usize::from(bytes[0] == b':');
    let end = if bytes[bytes.len() - 1] == b':' {
        bytes.len() - 1
    } else {
        bytes.len()
    };
    end > start && bytes[start..end].iter().all(|byte| *byte == b'-') && end - start >= 3
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::test_support::fixture_root;
    use sc_agent_types::InputSpec;

    #[test]
    fn extractors_table_column_keeps_escaped_pipes() {
        let repo = RepoRoot::new(fixture_root());
        let subjects = TableColumnExtractor
            .extract(
                &repo,
                &Rev::worktree(),
                &InputSpec::TableColumn {
                    path: "docs/decisions.md".into(),
                    column: "Choice".into(),
                },
            )
            .expect("column");
        let texts: Vec<&str> = subjects
            .iter()
            .map(|subject| subject.text.as_str())
            .collect();
        assert_eq!(texts, ["ship | hold", "drop"]);
        assert_eq!(subjects[0].id, "docs/decisions.md:3");
        assert_eq!(subjects[1].id, "docs/decisions.md:4");
    }

    #[test]
    fn extractors_table_column_does_not_split_escaped_code_pipes() {
        let dir = crate::extract::test_support::temp_dir("table");
        std::fs::write(
            dir.path().join("decisions.md"),
            "| Choice |\n| --- |\n| `a\\|b` |\n",
        )
        .expect("write");
        let subjects = TableColumnExtractor
            .extract(
                &RepoRoot::new(dir.path()),
                &Rev::worktree(),
                &InputSpec::TableColumn {
                    path: "decisions.md".into(),
                    column: "Choice".into(),
                },
            )
            .expect("column");
        assert_eq!(subjects.len(), 1);
        assert_eq!(subjects[0].text, "`a|b`");
    }

    #[test]
    fn table_subject_span_covers_the_cell_not_the_row() {
        let dir = crate::extract::test_support::temp_dir("table-span");
        let text = "| Id | Choice |\n| --- | --- |\n| 1 | ship it |\n";
        std::fs::write(dir.path().join("t.md"), text).expect("write");
        let subjects = TableColumnExtractor
            .extract(
                &RepoRoot::new(dir.path()),
                &Rev::worktree(),
                &InputSpec::TableColumn {
                    path: "t.md".into(),
                    column: "Choice".into(),
                },
            )
            .expect("column");
        let span = &subjects[0].span;
        let start = usize::try_from(span.start_byte).expect("start");
        let end = usize::try_from(span.end_byte).expect("end");
        assert_eq!(&text[start..end], "ship it");
        assert_eq!(subjects[0].text.len(), end - start);
    }
}
