# sudoku-lint

Sudoku puzzles get passed around as plain text files: nine lines of nine
characters, digits for filled cells and a dot or zero for empty ones. Anyone
who has scraped a batch of these from a website or hand-typed one from a
newspaper knows how easy it is to end up with a row that's eight characters
long, a stray letter where a digit belongs, or the same digit twice in a row
before the puzzle has even been looked at by a solver. Those problems are
easy to catch by eye one file at a time and easy to miss across a folder of
them.

`sudoku-lint` reads a board file and reports every problem it finds, one
line per finding, in the usual `file:line: severity: message` shape so it
reads like a compiler error and works with editors that jump to a location
from that format.

## Board format

```
# comment lines and blank lines are ignored
534678912
672195348
198342567
859761423
426853791
713924856
961537284
287419635
345286179
```

Nine rows are expected, each exactly nine characters: `1`-`9` for a filled
cell, `.` or `0` for an empty one.

A board whose first row is 16 characters wide is treated as a 16x16 variant
with 4x4 boxes instead. Cells above 9 are written as the letters `A`-`G`
(`A` is 10, `G` is 16, case insensitive):

```
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
```

A file can hold more than one board. A blank line ends the current board and
starts the next, so a whole batch of puzzles pulled from the same source can
be checked in one pass:

```
534678912
672195348
198342567
859761423
426853791
713924856
961537284
287419635
345286179

100000000
020000000
...
```

Each board is checked on its own; a problem in one does not affect line
numbers or findings in another.

## Usage

```
$ cargo run -- board.txt
board.txt:1: error: duplicate digit '3' in row
```

A clean board produces no output and the program exits with status 0.

Pass `--json` to get findings as a JSON array instead, for piping into
another tool:

```
$ cargo run -- --json board.txt
[{"line":1,"severity":"error","message":"duplicate digit '3' in row"}]
```

## Library

The linting logic lives in `src/lib.rs` and has no side effects: `lint`
takes the text of a board and returns a `Vec<Finding>`, so it can be tested
directly against string literals without touching the filesystem.

```rust
let findings = sudoku_lint::lint(board_text);
for finding in &findings {
    println!("{}: {}", finding.line, finding.message);
}
```

## What it checks today

- a file may hold multiple boards, separated by blank lines, each checked on
  its own
- a board is treated as 9x9 (3x3 boxes) unless its first row is 16
  characters wide, in which case it's checked as the 16x16 variant (4x4
  boxes) instead
- each board has exactly nine rows for the 9x9 variant, sixteen for the
  16x16 one (ignoring `#` comments)
- each row matches that width
- each character is a valid digit for the board's size (`1`-`9`, plus
  `A`-`G` for 16x16), a `.`, or a `0`
- no digit repeats within a single row
- no digit repeats within a single column
- no digit repeats within a single box
- a structurally clean board has at least one valid solution, checked by
  backtracking
