use std::fmt;

/// Severity of a single linter finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

/// A single problem found in a board, tied to the source line it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub line: usize,
    pub severity: Severity,
    pub message: String,
}

impl Finding {
    fn error(line: usize, message: impl Into<String>) -> Self {
        Finding {
            line,
            severity: Severity::Error,
            message: message.into(),
        }
    }
}

const SIZE: usize = 9;

/// A parsed row: `None` for an empty cell, `Some(1..=9)` for a filled one.
pub type Row = [Option<u8>; SIZE];

/// True for a line that carries no board data: blank, or a `#` comment.
pub fn is_ignorable_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

/// Parses one board row from text. Accepts digits 1-9 for filled cells and
/// '.' or '0' for empty ones. Returns an error message describing what was
/// wrong with the line otherwise.
pub fn parse_row(line: &str) -> Result<Row, String> {
    let trimmed = line.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() != SIZE {
        return Err(format!("expected {} cells, found {}", SIZE, chars.len()));
    }
    let mut row: Row = [None; SIZE];
    for (i, ch) in chars.iter().enumerate() {
        row[i] = match ch {
            '.' | '0' => None,
            '1'..='9' => Some(ch.to_digit(10).unwrap() as u8),
            other => {
                return Err(format!(
                    "invalid character '{}' at position {}",
                    other,
                    i + 1
                ))
            }
        };
    }
    Ok(row)
}

/// Returns the digits that appear more than once in a row, one entry per
/// repeated occurrence beyond the first.
pub fn find_duplicates(row: &Row) -> Vec<u8> {
    let mut seen = [false; SIZE + 1];
    let mut duplicates = Vec::new();
    for cell in row.iter().flatten() {
        let digit = *cell as usize;
        if seen[digit] {
            duplicates.push(*cell);
        } else {
            seen[digit] = true;
        }
    }
    duplicates
}

/// Returns `(row index, column, digit)` for every cell that repeats a digit
/// already seen earlier in the same column. The row index is a position
/// within `rows`, not a source line number, since this works on parsed rows
/// alone.
pub fn find_column_duplicates(rows: &[Row]) -> Vec<(usize, usize, u8)> {
    let mut duplicates = Vec::new();
    for col in 0..SIZE {
        let mut seen = [false; SIZE + 1];
        for (row_idx, row) in rows.iter().enumerate() {
            if let Some(digit) = row[col] {
                let d = digit as usize;
                if seen[d] {
                    duplicates.push((row_idx, col, digit));
                } else {
                    seen[d] = true;
                }
            }
        }
    }
    duplicates
}

/// Returns `(row index, column, digit)` for every cell that repeats a digit
/// already seen earlier in its 3x3 box. Boxes are scanned in reading order
/// (left to right, top to bottom within each box), so "already seen" matches
/// what someone checking a printed board by eye would find first.
pub fn find_box_duplicates(rows: &[Row]) -> Vec<(usize, usize, u8)> {
    let mut duplicates = Vec::new();
    if rows.len() < SIZE {
        return duplicates;
    }
    for box_row in 0..3 {
        for box_col in 0..3 {
            let mut seen = [false; SIZE + 1];
            for r in box_row * 3..box_row * 3 + 3 {
                for c in box_col * 3..box_col * 3 + 3 {
                    if let Some(digit) = rows[r][c] {
                        let d = digit as usize;
                        if seen[d] {
                            duplicates.push((r, c, digit));
                        } else {
                            seen[d] = true;
                        }
                    }
                }
            }
        }
    }
    duplicates
}

/// Lints the text of a sudoku board file and returns every finding, ordered
/// by line number. A file may hold more than one board: a blank line ends
/// the current board and starts the next, so boards can be batch-checked
/// from a single file instead of one file per puzzle. Pure: it does no I/O
/// and only reads the string it is given, which is what keeps it easy to
/// unit test.
pub fn lint(source: &str) -> Vec<Finding> {
    let mut boards: Vec<Vec<(usize, &str)>> = Vec::new();
    let mut current: Vec<(usize, &str)> = Vec::new();

    for (idx, line) in source.lines().enumerate() {
        let line_number = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current.is_empty() {
                boards.push(std::mem::take(&mut current));
            }
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        current.push((line_number, line));
    }
    if !current.is_empty() {
        boards.push(current);
    }

    boards.iter().flat_map(|board| lint_board(board)).collect()
}

/// Lints a single board's non-ignorable lines, given as `(source line
/// number, text)` pairs.
fn lint_board(lines: &[(usize, &str)]) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut rows: Vec<(usize, Row)> = Vec::new();

    for &(line_number, line) in lines {
        if rows.len() == SIZE {
            findings.push(Finding::error(
                line_number,
                "unexpected extra row, board already has 9",
            ));
            continue;
        }

        match parse_row(line) {
            Ok(row) => {
                for digit in find_duplicates(&row) {
                    findings.push(Finding::error(
                        line_number,
                        format!("duplicate digit '{}' in row", digit),
                    ));
                }
                rows.push((line_number, row));
            }
            Err(message) => findings.push(Finding::error(line_number, message)),
        }
    }

    if rows.len() < SIZE {
        let last_line = lines.last().map_or(1, |(line_number, _)| *line_number);
        findings.push(Finding::error(
            last_line,
            format!("expected {} rows, found {}", SIZE, rows.len()),
        ));
    }

    let parsed_rows: Vec<Row> = rows.iter().map(|(_, row)| *row).collect();
    for (row_idx, col, digit) in find_column_duplicates(&parsed_rows) {
        let line_number = rows[row_idx].0;
        findings.push(Finding::error(
            line_number,
            format!("duplicate digit '{}' in column {}", digit, col + 1),
        ));
    }

    for (row_idx, col, digit) in find_box_duplicates(&parsed_rows) {
        let line_number = rows[row_idx].0;
        findings.push(Finding::error(
            line_number,
            format!(
                "duplicate digit '{}' in 3x3 box at row {}, column {}",
                digit,
                row_idx + 1,
                col + 1
            ),
        ));
    }

    // Row findings and the "wrong row count" finding are already produced in
    // line order as the board is scanned top to bottom; column and box
    // findings are appended afterward and need folding back in to keep that
    // guarantee.
    findings.sort_by_key(|f| f.line);
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_valid_row() {
        let row = parse_row("1.34.6789").unwrap();
        assert_eq!(row[0], Some(1));
        assert_eq!(row[1], None);
        assert_eq!(row[8], Some(9));
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(parse_row("12345").is_err());
    }

    #[test]
    fn rejects_invalid_character() {
        assert!(parse_row("12345678x").is_err());
    }

    #[test]
    fn finds_duplicate_digits() {
        let row = parse_row("11345678.").unwrap();
        assert_eq!(find_duplicates(&row), vec![1]);
    }

    #[test]
    fn no_duplicates_in_a_clean_row() {
        let row = parse_row("123456789").unwrap();
        assert!(find_duplicates(&row).is_empty());
    }

    #[test]
    fn finds_column_duplicates_across_rows() {
        let rows: Vec<Row> = [
            "1........",
            "1........",
            ".........",
            ".........",
            ".........",
            ".........",
            ".........",
            ".........",
            ".........",
        ]
        .iter()
        .map(|line| parse_row(line).unwrap())
        .collect();
        assert_eq!(find_column_duplicates(&rows), vec![(1, 0, 1)]);
    }

    #[test]
    fn no_column_duplicates_in_a_clean_board() {
        let rows: Vec<Row> = CLEAN_BOARD
            .lines()
            .map(|line| parse_row(line).unwrap())
            .collect();
        assert!(find_column_duplicates(&rows).is_empty());
    }

    #[test]
    fn finds_box_duplicates_within_the_same_box() {
        let rows: Vec<Row> = [
            "1........",
            "..1......",
            ".........",
            ".........",
            ".........",
            ".........",
            ".........",
            ".........",
            ".........",
        ]
        .iter()
        .map(|line| parse_row(line).unwrap())
        .collect();
        assert_eq!(find_box_duplicates(&rows), vec![(1, 2, 1)]);
    }

    #[test]
    fn same_digit_in_different_boxes_is_not_a_box_duplicate() {
        let rows: Vec<Row> = [
            "1........",
            "...1.....",
            ".........",
            ".........",
            ".........",
            ".........",
            ".........",
            ".........",
            ".........",
        ]
        .iter()
        .map(|line| parse_row(line).unwrap())
        .collect();
        assert!(find_box_duplicates(&rows).is_empty());
    }

    #[test]
    fn no_box_duplicates_in_a_clean_board() {
        let rows: Vec<Row> = CLEAN_BOARD
            .lines()
            .map(|line| parse_row(line).unwrap())
            .collect();
        assert!(find_box_duplicates(&rows).is_empty());
    }

    #[test]
    fn incomplete_board_has_no_box_duplicates() {
        let rows: Vec<Row> = vec![parse_row("1........").unwrap()];
        assert!(find_box_duplicates(&rows).is_empty());
    }

    const CLEAN_BOARD: &str = "\
534678912
672195348
198342567
859761423
426853791
713924856
961537284
287419635
345286179
";

    #[test]
    fn clean_board_has_no_findings() {
        assert!(lint(CLEAN_BOARD).is_empty());
    }

    #[test]
    fn flags_duplicate_and_reports_its_line() {
        // Changing this one cell duplicates '3' in row 1, in its 3x3 box
        // (against the '3' now sitting next to it), and, since the board is
        // a complete valid solution, in column 1 against row 9.
        let board = CLEAN_BOARD.replacen("534678912", "334678912", 1);
        let findings = lint(&board);
        assert_eq!(findings.len(), 3);
        assert_eq!(findings[0].line, 1);
        assert!(findings[0].message.contains("duplicate digit '3' in row"));
        assert_eq!(findings[1].line, 1);
        assert!(findings[1].message.contains("duplicate digit '3' in 3x3 box"));
        assert_eq!(findings[2].line, 9);
        assert!(findings[2].message.contains("duplicate digit '3' in column"));
    }

    #[test]
    fn flags_column_duplicate_without_row_duplicate() {
        // The two 1s share a column but sit in different 3x3 boxes, so this
        // should trip only the column check.
        let board = "\
1........
.........
.........
1........
.........
.........
.........
.........
.........
";
        let findings = lint(board);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 4);
        assert!(findings[0].message.contains("duplicate digit '1' in column 1"));
    }

    #[test]
    fn flags_box_duplicate_without_row_or_column_duplicate() {
        let board = "\
1........
..1......
.........
.........
.........
.........
.........
.........
.........
";
        let findings = lint(board);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 2);
        assert!(findings[0]
            .message
            .contains("duplicate digit '1' in 3x3 box at row 2, column 3"));
    }

    #[test]
    fn flags_missing_rows() {
        let findings = lint("123456789\n");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 1);
        assert!(findings[0].message.contains("expected 9 rows, found 1"));
    }

    #[test]
    fn ignores_blank_and_comment_lines() {
        let board = format!("# a puzzle\n\n{}", CLEAN_BOARD);
        assert!(lint(&board).is_empty());
    }

    #[test]
    fn lints_each_board_in_a_multi_board_file() {
        // Second board repeats '3' in the first row.
        let bad_second_board = CLEAN_BOARD.replacen("534678912", "334678912", 1);
        let file = format!("{}\n{}", CLEAN_BOARD, bad_second_board);
        let findings = lint(&file);
        assert_eq!(findings.len(), 3);
        // CLEAN_BOARD is 9 lines plus its trailing newline, so the second
        // board's first row lands on line 11 (line 10 is the blank separator).
        assert_eq!(findings[0].line, 11);
        assert!(findings[0].message.contains("duplicate digit '3' in row"));
        assert_eq!(findings[1].line, 11);
        assert!(findings[1].message.contains("duplicate digit '3' in 3x3 box"));
        assert_eq!(findings[2].line, 19);
        assert!(findings[2].message.contains("duplicate digit '3' in column"));
    }

    #[test]
    fn a_short_board_does_not_swallow_the_next_one_in_the_file() {
        let file = format!("123456789\n\n{}", CLEAN_BOARD);
        let findings = lint(&file);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 1);
        assert!(findings[0].message.contains("expected 9 rows, found 1"));
    }
}
