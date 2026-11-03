//! `table-column` extractor.
//!
//! Cells split on unescaped `|` only. `\|` is one literal pipe inside a cell.

use sc_agent_types::{InputSpec, JevError, RepoRoot, Rev, Span, Subject};

use super::Extracted;
use super::Extractor;
use super::private::Sealed;
use super::read::read_repo_text;

/// Reads one markdown table column.
#[derive(Debug, Clone, Copy)]
pub struct TableColumnExtractor;

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
        if header.iter().all(|cell| is_delimiter(cell)) {
            index += 1;
            continue;
        }
        if let Some(column_index) = header.iter().position(|cell| cell == column) {
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
        if cells.iter().all(|cell| is_delimiter(cell)) {
            continue;
        }
        let Some(cell) = cells.get(column_index) else {
            return Err(JevError::InputInvalid {
                detail: format!("{path}:{line_no} is missing column {column}"),
            });
        };
        if cell.is_empty() {
            continue;
        }
        let start_byte = u32::try_from(*offset).unwrap_or(u32::MAX);
        let end_byte = start_byte.saturating_add(u32::try_from(line.len()).unwrap_or(0));
        subjects.push(Subject {
            id: format!("{path}:{line_no}"),
            capture: "column".into(),
            text: (*cell).clone(),
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

fn split_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'|') {
            cell.push('|');
            chars.next();
            continue;
        }
        if ch == '|' {
            cells.push(std::mem::take(&mut cell));
            continue;
        }
        cell.push(ch);
    }
    cells.push(cell);
    let mut cells: Vec<String> = cells
        .into_iter()
        .map(|cell| cell.trim().to_owned())
        .collect();
    if line.trim_start().starts_with('|') && cells.first().is_some_and(String::is_empty) {
        cells.remove(0);
    }
    if line.trim_end().ends_with('|') && cells.last().is_some_and(String::is_empty) {
        cells.pop();
    }
    cells
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
}
