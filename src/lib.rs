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

/// The two board layouts this linter understands: standard 9x9 with 3x3
/// boxes, and the 16x16 variant with 4x4 boxes. `size` is both the row/column
/// count and the highest digit a cell can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoardKind {
    size: usize,
    box_size: usize,
}

const NINE: BoardKind = BoardKind { size: 9, box_size: 3 };
const SIXTEEN: BoardKind = BoardKind {
    size: 16,
    box_size: 4,
};

/// A parsed row: `None` for an empty cell, `Some(digit)` for a filled one.
/// Its length is the board's size (9 or 16), fixed at parse time rather than
/// at compile time so both variants can share the same code.
pub type Row = Vec<Option<u8>>;

/// True for a line that carries no board data: blank, or a `#` comment.
pub fn is_ignorable_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

/// Parses one board row of `size` cells from text. Accepts digits 1-9 for
/// values up to 9, and beyond that the letters A-G (10-16, case
/// insensitive), for the 16x16 variant. '.' or '0' mark an empty cell.
/// Returns an error message describing what was wrong with the line
/// otherwise.
pub fn parse_row(line: &str, size: usize) -> Result<Row, String> {
    let trimmed = line.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() != size {
        return Err(format!("expected {} cells, found {}", size, chars.len()));
    }
    let mut row: Row = vec![None; size];
    for (i, ch) in chars.iter().enumerate() {
        row[i] = match parse_cell(*ch, size) {
            Some(cell) => cell,
            None => {
                return Err(format!(
                    "invalid character '{}' at position {}",
                    ch,
                    i + 1
                ))
            }
        };
    }
    Ok(row)
}

/// Parses a single cell character. The outer `None` means the character
/// isn't valid at all; `Some(None)` means it's a valid empty-cell marker.
fn parse_cell(ch: char, size: usize) -> Option<Option<u8>> {
    let digit = match ch {
        '.' | '0' => return Some(None),
        '1'..='9' => ch.to_digit(10).unwrap() as u8,
        'A'..='G' => 10 + (ch as u8 - b'A'),
        'a'..='g' => 10 + (ch as u8 - b'a'),
        _ => return None,
    };
    if digit as usize <= size {
        Some(Some(digit))
    } else {
        None
    }
}

/// Renders a digit the way it appears on the board: `1`-`9` as themselves,
/// `10`-`16` as the letters `A`-`G` used by the 16x16 variant.
fn format_digit(digit: u8) -> String {
    if digit <= 9 {
        digit.to_string()
    } else {
        ((b'A' + (digit - 10)) as char).to_string()
    }
}

/// Returns the digits that appear more than once in a row, one entry per
/// repeated occurrence beyond the first.
pub fn find_duplicates(row: &Row) -> Vec<u8> {
    let mut seen = vec![false; row.len() + 1];
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
    let size = match rows.first() {
        Some(row) => row.len(),
        None => return duplicates,
    };
    for col in 0..size {
        let mut seen = vec![false; size + 1];
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
/// already seen earlier in its `box_size`x`box_size` box. Boxes are scanned
/// in reading order (left to right, top to bottom within each box), so
/// "already seen" matches what someone checking a printed board by eye would
/// find first.
pub fn find_box_duplicates(rows: &[Row], box_size: usize) -> Vec<(usize, usize, u8)> {
    let mut duplicates = Vec::new();
    let size = box_size * box_size;
    if rows.len() < size {
        return duplicates;
    }
    for box_row in 0..box_size {
        for box_col in 0..box_size {
            let mut seen = vec![false; size + 1];
            for r in box_row * box_size..box_row * box_size + box_size {
                for c in box_col * box_size..box_col * box_size + box_size {
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

/// True if a complete, structurally valid board has at least one solution.
/// Empty cells (`None`) are filled in by backtracking; a board that is
/// already full just gets checked as-is. Callers are expected to only run
/// this once row/column/box duplicate checks have already passed, since a
/// board with a duplicate can never solve and isn't worth the search.
pub fn is_solvable(rows: &[Row], box_size: usize) -> bool {
    let size = box_size * box_size;
    if rows.len() != size {
        return false;
    }
    let mut grid = vec![vec![0u8; size]; size];
    for (r, row) in rows.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            grid[r][c] = cell.unwrap_or(0);
        }
    }
    solve(&mut grid, size, box_size)
}

fn solve(grid: &mut [Vec<u8>], size: usize, box_size: usize) -> bool {
    for r in 0..size {
        for c in 0..size {
            if grid[r][c] != 0 {
                continue;
            }
            for digit in 1..=size as u8 {
                if is_safe(grid, r, c, digit, size, box_size) {
                    grid[r][c] = digit;
                    if solve(grid, size, box_size) {
                        return true;
                    }
                    grid[r][c] = 0;
                }
            }
            return false;
        }
    }
    true
}

fn is_safe(grid: &[Vec<u8>], row: usize, col: usize, digit: u8, size: usize, box_size: usize) -> bool {
    for i in 0..size {
        if grid[row][i] == digit || grid[i][col] == digit {
            return false;
        }
    }
    let box_row = (row / box_size) * box_size;
    let box_col = (col / box_size) * box_size;
    for r in box_row..box_row + box_size {
        for c in box_col..box_col + box_size {
            if grid[r][c] == digit {
                return false;
            }
        }
    }
    true
}

/// Renders findings as a JSON array, one object per finding with `line`,
/// `severity`, and `message` fields. Kept here rather than in `main.rs` so it
/// stays covered by the same string-in, string-out tests as the rest of the
/// linter instead of only being exercised by running the binary.
pub fn findings_to_json(findings: &[Finding]) -> String {
    let mut out = String::from("[");
    for (i, finding) in findings.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"line\":{},\"severity\":\"{}\",\"message\":\"{}\"}}",
            finding.line,
            finding.severity,
            escape_json_string(&finding.message)
        ));
    }
    out.push(']');
    out
}

fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
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

/// Guesses which board layout a board's lines use from the width of its
/// first line: 16 characters means the 16x16 variant, anything else
/// (including a malformed first line) falls back to standard 9x9 so a typo
/// in that line is reported the same way it always has been.
fn detect_board_kind(lines: &[(usize, &str)]) -> BoardKind {
    match lines.first() {
        Some((_, line)) if line.trim().chars().count() == SIXTEEN.size => SIXTEEN,
        _ => NINE,
    }
}

/// Lints a single board's non-ignorable lines, given as `(source line
/// number, text)` pairs.
fn lint_board(lines: &[(usize, &str)]) -> Vec<Finding> {
    let mut findings = Vec::new();
    let kind = detect_board_kind(lines);
    let mut rows: Vec<(usize, Row)> = Vec::new();

    for &(line_number, line) in lines {
        if rows.len() == kind.size {
            findings.push(Finding::error(
                line_number,
                format!("unexpected extra row, board already has {}", kind.size),
            ));
            continue;
        }

        match parse_row(line, kind.size) {
            Ok(row) => {
                for digit in find_duplicates(&row) {
                    findings.push(Finding::error(
                        line_number,
                        format!("duplicate digit '{}' in row", format_digit(digit)),
                    ));
                }
                rows.push((line_number, row));
            }
            Err(message) => findings.push(Finding::error(line_number, message)),
        }
    }

    if rows.len() < kind.size {
        let last_line = lines.last().map_or(1, |(line_number, _)| *line_number);
        findings.push(Finding::error(
            last_line,
            format!("expected {} rows, found {}", kind.size, rows.len()),
        ));
    }

    let parsed_rows: Vec<Row> = rows.iter().map(|(_, row)| row.clone()).collect();
    for (row_idx, col, digit) in find_column_duplicates(&parsed_rows) {
        let line_number = rows[row_idx].0;
        findings.push(Finding::error(
            line_number,
            format!(
                "duplicate digit '{}' in column {}",
                format_digit(digit),
                col + 1
            ),
        ));
    }

    for (row_idx, col, digit) in find_box_duplicates(&parsed_rows, kind.box_size) {
        let line_number = rows[row_idx].0;
        findings.push(Finding::error(
            line_number,
            format!(
                "duplicate digit '{}' in {size}x{size} box at row {}, column {}",
                format_digit(digit),
                row_idx + 1,
                col + 1,
                size = kind.box_size
            ),
        ));
    }

    if findings.is_empty() && !is_solvable(&parsed_rows, kind.box_size) {
        let last_line = lines.last().map_or(1, |(line_number, _)| *line_number);
        findings.push(Finding::error(last_line, "board has no valid solution"));
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
        let row = parse_row("1.34.6789", 9).unwrap();
        assert_eq!(row[0], Some(1));
        assert_eq!(row[1], None);
        assert_eq!(row[8], Some(9));
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(parse_row("12345", 9).is_err());
    }

    #[test]
    fn rejects_invalid_character() {
        assert!(parse_row("12345678x", 9).is_err());
    }

    #[test]
    fn rejects_letters_over_9_cells() {
        assert!(parse_row("12345678A", 9).is_err());
    }

    #[test]
    fn parses_a_valid_16x16_row_with_letters() {
        let row = parse_row("123456789ABCDEFG", 16).unwrap();
        assert_eq!(row[8], Some(9));
        assert_eq!(row[9], Some(10));
        assert_eq!(row[15], Some(16));
    }

    #[test]
    fn parses_lowercase_letters_in_a_16x16_row() {
        let row = parse_row("g...............", 16).unwrap();
        assert_eq!(row[0], Some(16));
    }

    #[test]
    fn finds_duplicate_digits() {
        let row = parse_row("11345678.", 9).unwrap();
        assert_eq!(find_duplicates(&row), vec![1]);
    }

    #[test]
    fn no_duplicates_in_a_clean_row() {
        let row = parse_row("123456789", 9).unwrap();
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
        .map(|line| parse_row(line, 9).unwrap())
        .collect();
        assert_eq!(find_column_duplicates(&rows), vec![(1, 0, 1)]);
    }

    #[test]
    fn no_column_duplicates_in_a_clean_board() {
        let rows: Vec<Row> = CLEAN_BOARD
            .lines()
            .map(|line| parse_row(line, 9).unwrap())
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
        .map(|line| parse_row(line, 9).unwrap())
        .collect();
        assert_eq!(find_box_duplicates(&rows, 3), vec![(1, 2, 1)]);
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
        .map(|line| parse_row(line, 9).unwrap())
        .collect();
        assert!(find_box_duplicates(&rows, 3).is_empty());
    }

    #[test]
    fn no_box_duplicates_in_a_clean_board() {
        let rows: Vec<Row> = CLEAN_BOARD
            .lines()
            .map(|line| parse_row(line, 9).unwrap())
            .collect();
        assert!(find_box_duplicates(&rows, 3).is_empty());
    }

    #[test]
    fn incomplete_board_has_no_box_duplicates() {
        let rows: Vec<Row> = vec![parse_row("1........", 9).unwrap()];
        assert!(find_box_duplicates(&rows, 3).is_empty());
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
    fn a_solved_board_is_solvable() {
        let rows: Vec<Row> = CLEAN_BOARD
            .lines()
            .map(|line| parse_row(line, 9).unwrap())
            .collect();
        assert!(is_solvable(&rows, 3));
    }

    #[test]
    fn an_empty_board_is_solvable() {
        let empty_row = parse_row(".........", 9).unwrap();
        let rows: Vec<Row> = vec![empty_row; 9];
        assert!(is_solvable(&rows, 3));
    }

    // Row 0 fills cols 0-7 with 1-8, leaving only col 8 open, which needs a
    // 9 to complete the row. Row 1 places a 9 in that same column, so the
    // one open cell in row 0 can never take the digit it needs: no given
    // conflicts with another, but no completion exists either.
    const UNSOLVABLE_BOARD: &str = "\
12345678.
........9
.........
.........
.........
.........
.........
.........
.........
";

    #[test]
    fn a_board_with_no_solution_is_rejected() {
        let rows: Vec<Row> = UNSOLVABLE_BOARD
            .lines()
            .map(|line| parse_row(line, 9).unwrap())
            .collect();
        assert!(!is_solvable(&rows, 3));
    }

    #[test]
    fn lint_flags_an_unsolvable_board() {
        let findings = lint(UNSOLVABLE_BOARD);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 9);
        assert!(findings[0].message.contains("no valid solution"));
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

    // A valid 16x16 solution generated by rotating a 1..16 base row by
    // box_size*(r%box_size) + r/box_size for each row r, the standard
    // construction for a solved Sudoku grid of any box size.
    const CLEAN_BOARD_16: &str = "\
123456789ABCDEFG
56789ABCDEFG1234
9ABCDEFG12345678
DEFG123456789ABC
23456789ABCDEFG1
6789ABCDEFG12345
ABCDEFG123456789
EFG123456789ABCD
3456789ABCDEFG12
789ABCDEFG123456
BCDEFG123456789A
FG123456789ABCDE
456789ABCDEFG123
89ABCDEFG1234567
CDEFG123456789AB
G123456789ABCDEF
";

    #[test]
    fn clean_16x16_board_has_no_findings() {
        assert!(lint(CLEAN_BOARD_16).is_empty());
    }

    #[test]
    fn flags_duplicate_row_digit_in_a_16x16_board() {
        // The two 'G's sit four columns apart, in different 4x4 boxes, so
        // only the row check should trip.
        let first_row = format!("G{}G{}", ".".repeat(3), ".".repeat(11));
        let blank_row = ".".repeat(16);
        let board = format!("{}\n{}\n", first_row, vec![blank_row; 15].join("\n"));
        let findings = lint(&board);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 1);
        assert!(findings[0].message.contains("duplicate digit 'G' in row"));
    }

    #[test]
    fn flags_box_duplicate_in_a_16x16_board() {
        // Second row's third cell repeats the '1' in the first row's first
        // cell; both sit in the top-left 4x4 box but in different rows and
        // columns, so only the box check should trip.
        let first_row = format!("1{}", ".".repeat(15));
        let second_row = format!("..1{}", ".".repeat(13));
        let blank_row = ".".repeat(16);
        let board = format!(
            "{}\n{}\n{}\n",
            first_row,
            second_row,
            vec![blank_row; 14].join("\n")
        );
        let findings = lint(&board);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 2);
        assert!(findings[0]
            .message
            .contains("duplicate digit '1' in 4x4 box at row 2, column 3"));
    }

    #[test]
    fn renders_empty_findings_as_empty_json_array() {
        assert_eq!(findings_to_json(&[]), "[]");
    }

    #[test]
    fn renders_a_finding_as_a_json_object() {
        let findings = vec![Finding::error(1, "duplicate digit '3' in row")];
        assert_eq!(
            findings_to_json(&findings),
            "[{\"line\":1,\"severity\":\"error\",\"message\":\"duplicate digit '3' in row\"}]"
        );
    }

    #[test]
    fn joins_multiple_findings_with_a_comma() {
        let findings = vec![Finding::error(1, "a"), Finding::error(2, "b")];
        assert_eq!(
            findings_to_json(&findings),
            "[{\"line\":1,\"severity\":\"error\",\"message\":\"a\"},\
             {\"line\":2,\"severity\":\"error\",\"message\":\"b\"}]"
        );
    }

    #[test]
    fn escapes_quotes_and_backslashes_in_messages() {
        let findings = vec![Finding::error(1, "invalid character '\"' near \\here")];
        assert_eq!(
            findings_to_json(&findings),
            "[{\"line\":1,\"severity\":\"error\",\"message\":\"invalid character '\\\"' near \\\\here\"}]"
        );
    }
}
