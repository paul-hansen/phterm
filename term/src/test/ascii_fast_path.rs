//! Printable ASCII is assigned to the screen in runs rather than a grapheme
//! at a time. These tests feed identical output to a terminal with and
//! without that fast path and require identical results.
use super::*;
use crate::terminalstate::performer::DISABLE_ASCII_FAST_PATH;
use k9::assert_equal as assert_eq;

fn run(rows: usize, cols: usize, chunks: &[Vec<u8>], disable_fast_path: bool) -> Terminal {
    DISABLE_ASCII_FAST_PATH.with(|d| d.set(disable_fast_path));
    // Not TestTerm::new, which turns on trace logging; that makes the
    // randomized test take far longer.
    let mut term = Terminal::new(
        TerminalSize {
            rows,
            cols,
            pixel_width: cols * 8,
            pixel_height: rows * 16,
            dpi: 0,
        },
        Arc::new(TestTermConfig { scrollback: 20 }),
        "WezTerm",
        "O_o",
        Box::new(Vec::new()),
    );
    for chunk in chunks {
        term.advance_bytes(chunk);
    }
    DISABLE_ASCII_FAST_PATH.with(|d| d.set(false));
    term
}

fn assert_matches_general_path(rows: usize, cols: usize, chunks: &[Vec<u8>], what: &str) {
    let fast = run(rows, cols, chunks, false);
    let general = run(rows, cols, chunks, true);

    let fast_lines = fast.screen().all_lines();
    let general_lines = general.screen().all_lines();
    assert_eq!(
        fast_lines.len(),
        general_lines.len(),
        "{}: line count",
        what
    );
    for (idx, (f, g)) in fast_lines.iter().zip(general_lines.iter()).enumerate() {
        assert!(
            f == g,
            "{}: line {} differs\nfast:    {:?}\ngeneral: {:?}\n{:#?}\n{:#?}",
            what,
            idx,
            f.as_str(),
            g.as_str(),
            f,
            g
        );
    }
    assert_eq!(fast.cursor_pos(), general.cursor_pos(), "{}: cursor", what);
    assert_eq!(
        fast.current_seqno(),
        general.current_seqno(),
        "{}: seqno",
        what
    );
}

fn check(rows: usize, cols: usize, output: &str) {
    // Append a marker so that any difference in pending wrap state shows
    let output = format!("{output}#");
    assert_matches_general_path(rows, cols, &[output.as_bytes().to_vec()], &output);
}

#[test]
fn ascii_runs_and_wrapping() {
    check(3, 10, "hello");
    // Exactly fills the line, leaving a pending wrap
    check(3, 10, "0123456789");
    check(3, 10, "0123456789\r\nnext");
    // Wraps several times and scrolls into scrollback
    check(
        3,
        10,
        &"the quick brown fox jumps over the lazy dog ".repeat(5),
    );
    // Overwrites existing text
    check(3, 10, "aaaaaaaaaa\x1b[Hbb\x1b[1;5Hcc");
    check(3, 10, "\x1b[1mbold\x1b[0m plain \x1b[31mred");
}

#[test]
fn ascii_runs_without_autowrap() {
    check(3, 10, "\x1b[?7l0123456789abcdef");
    check(3, 10, "\x1b[?7lab\x1b[1;8Hxyzxyz\r\nmore");
}

#[test]
fn ascii_runs_with_margins() {
    // Left/right margins, writing from inside them
    check(4, 10, "\x1b[?69h\x1b[3;7s\x1b[1;3Hinside the margins");
    // Starting beyond the right margin
    check(4, 10, "\x1b[?69h\x1b[3;7s\x1b[1;9Hbeyond");
    // Scroll region
    check(
        5,
        10,
        "\x1b[2;4r\x1b[2;1H1234567890abcdefghij1234567890abcdefghij",
    );
}

#[test]
fn ascii_next_to_other_graphemes() {
    // Combining marks attach to the preceding ASCII letter
    check(3, 10, "cafe\u{301} ok");
    // Prepend characters attach to the following ASCII
    check(3, 10, "x\u{600}1y");
    check(3, 10, "ab中cd😀ef");
    // Overwriting half of a double-width character
    check(3, 10, "中中中\x1b[1;2Hxy");
    check(3, 10, "中中中\x1b[1;1Hxyz");
    // Zero-width characters are elided
    check(3, 10, "a\u{2068}b");
}

#[test]
fn ascii_with_charsets_and_insert_mode() {
    // DEC line drawing remaps lowercase ASCII
    check(3, 10, "\x1b(0lqqk\x1b(Bplain");
    check(3, 10, "\x1b(A#uk\x1b(B#us");
    // Shift out to G1
    check(3, 10, "\x1b)0ab\x0elqk\x0fcd");
    // Insert mode shifts existing cells
    check(3, 10, "abcdef\x1b[1;3H\x1b[4hXY\x1b[4lZ");
}

#[test]
fn ascii_with_hyperlinks() {
    check(
        3,
        20,
        "\x1b]8;;https://wezterm.org\x1b\\link\x1b]8;;\x1b\\ text",
    );
}

/// Small deterministic PRNG so that failures are reproducible
struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn random_output(rng: &mut XorShift, rows: usize, cols: usize) -> Vec<u8> {
    const ASCII: &[u8] =
        b"abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789 #`{|}~!@";
    let mut out = String::new();
    for _ in 0..rng.below(60) + 20 {
        match rng.below(20) {
            0..=7 => {
                for _ in 0..rng.below(40) + 1 {
                    out.push(ASCII[rng.below(ASCII.len() as u64) as usize] as char);
                }
            }
            8 => out.push_str("\r\n"),
            9 => out.push_str(match rng.below(3) {
                0 => "\r",
                1 => "\n",
                _ => "\x08",
            }),
            10 => out.push_str(&format!(
                "\x1b[{};{}H",
                rng.below(rows as u64 + 1) + 1,
                rng.below(cols as u64 + 2) + 1
            )),
            11 => out.push_str(&format!(
                "\x1b[{}m",
                [0, 1, 4, 7, 31, 42][rng.below(6) as usize]
            )),
            12 => out.push_str(
                ["中", "😀", "e\u{301}", "\u{600}1", "\u{2068}", "\t"][rng.below(6) as usize],
            ),
            13 => out.push_str(
                ["\x1b(0", "\x1b(B", "\x1b(A", "\x1b)0", "\x0e", "\x0f"][rng.below(6) as usize],
            ),
            14 => out.push_str(["\x1b[4h", "\x1b[4l"][rng.below(2) as usize]),
            15 => out.push_str(["\x1b[?7l", "\x1b[?7h"][rng.below(2) as usize]),
            16 => {
                let left = rng.below(cols as u64) + 1;
                let right = left + rng.below(cols as u64 - left + 1);
                out.push_str(&format!("\x1b[?69h\x1b[{left};{right}s"));
            }
            17 => out.push_str("\x1b[?69l"),
            18 => out.push_str(["\x1b[K", "\x1b[2;4r", "\x1b[r"][rng.below(3) as usize]),
            _ => out.push_str(
                ["\x1b]8;;https://wezterm.org\x1b\\", "\x1b]8;;\x1b\\"][rng.below(2) as usize],
            ),
        }
    }
    out.push('#');
    out.into_bytes()
}

#[test]
fn ascii_fast_path_matches_general_path_on_random_output() {
    for seed in 1..=400u64 {
        let mut rng = XorShift(seed.wrapping_mul(0x9e3779b97f4a7c15) | 1);
        let (rows, cols) = (rng.below(6) as usize + 2, rng.below(15) as usize + 3);
        let output = random_output(&mut rng, rows, cols);

        // Feed it in random chunk sizes, as reads from a pty would arrive
        let mut chunks = vec![];
        let mut rest = &output[..];
        while !rest.is_empty() {
            let n = (rng.below(64) as usize + 1).min(rest.len());
            chunks.push(rest[..n].to_vec());
            rest = &rest[n..];
        }

        let what = format!(
            "seed {seed} ({rows}x{cols}) {:?}",
            String::from_utf8_lossy(&output)
        );
        assert_matches_general_path(rows, cols, &chunks, &what);
    }
}
