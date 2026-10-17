//! Tests for kitty graphics protocol Unicode placeholders, where an image
//! is created with a virtual placement (`U=1`) and then displayed by
//! printing `U+10EEEE` placeholder characters.
//! <https://sw.kovidgoyal.net/kitty/graphics-protocol/#unicode-placeholders>
//!
//! The tests named `kitty_*` are ported from the `test_unicode_placeholders*`
//! family in kitty's `kitty_tests/graphics.py`. kitty reports one source
//! rectangle per horizontal run of placeholder cells, while wezterm attaches
//! a slice of the image to each individual cell, so the expectations below
//! are kitty's rectangles divided up across the cells of each run.
//! Where kitty's source rectangle extends past the image (because the image
//! is letterboxed within its placement), wezterm instead clips the texture
//! coordinates to the image and shrinks the drawn area with cell padding.

use super::*;
use base64::Engine;
use k9::assert_equal as assert_eq;
use wezterm_cell::image::ImageCell;

const PLACEHOLDER: char = '\u{10EEEE}';

/// Returns the diacritic that encodes `n` as a row, column or image id MSB.
/// This is deliberately independent of the table in the implementation.
fn diacritic(n: usize) -> char {
    match n {
        0 => '\u{305}',
        1 => '\u{30d}',
        2 => '\u{30e}',
        3 => '\u{310}',
        4 => '\u{312}',
        5 => '\u{33d}',
        6 => '\u{33e}',
        7 => '\u{33f}',
        42 => '\u{59c}',
        _ => panic!("no diacritic for {} in this test table", n),
    }
}

/// A placeholder cell carrying the given diacritics, which are, in order:
/// row, column and the most significant byte of the image id.
fn p(diacritics: &[usize]) -> String {
    let mut s = PLACEHOLDER.to_string();
    for &n in diacritics {
        s.push(diacritic(n));
    }
    s
}

fn new_term(rows: usize, cols: usize, cell_w: usize, cell_h: usize, scrollback: usize) -> TestTerm {
    let mut term = TestTerm::new(rows, cols, scrollback);
    term.resize(TerminalSize {
        rows,
        cols,
        pixel_width: cols * cell_w,
        pixel_height: rows * cell_h,
        dpi: 0,
    });
    term
}

/// Transmits an RGBA image of the given dimensions, using the provided
/// keys to control the action, id and placement.
fn transmit(term: &mut TestTerm, keys: &str, width: u32, height: u32) {
    let data = vec![0xffu8; (width * height * 4) as usize];
    let payload = base64::engine::general_purpose::STANDARD.encode(&data);
    term.print(format!(
        "\x1b_G{keys},f=32,s={width},v={height},q=2;{payload}\x1b\\"
    ));
}

/// Creates a placement for an existing image
fn put_ref(term: &mut TestTerm, image_id: u32, keys: &str) {
    term.print(format!("\x1b_Ga=p,i={image_id},q=2,{keys}\x1b\\"));
}

fn delete(term: &mut TestTerm, keys: &str) {
    term.print(format!("\x1b_Ga=d,q=2,{keys}\x1b\\"));
}

#[track_caller]
fn assert_cursor(term: &TestTerm, x: usize, y: i64) {
    let cursor = term.cursor_pos();
    assert_eq!((cursor.x, cursor.y), (x, y), "cursor position");
}

fn round(v: f32) -> f32 {
    (v * 10_000.0).round() / 10_000.0
}

/// The slice of an image that a placeholder cell displays
#[derive(Debug, Clone, Copy, PartialEq)]
struct Slice {
    image_id: u32,
    placement_id: Option<u32>,
    /// Texture coordinates: left, top, right, bottom
    tex: [f32; 4],
    /// Padding: left, top, right, bottom
    padding: [u16; 4],
}

/// A slice of an unnamed placement covering the given texture
/// coordinates and filling its cell
fn slice(image_id: u32, left: f32, top: f32, right: f32, bottom: f32) -> Slice {
    Slice {
        image_id,
        placement_id: None,
        tex: [round(left), round(top), round(right), round(bottom)],
        padding: [0; 4],
    }
}

impl Slice {
    fn placement(mut self, placement_id: u32) -> Self {
        self.placement_id = Some(placement_id);
        self
    }

    fn padding(mut self, left: u16, top: u16, right: u16, bottom: u16) -> Self {
        self.padding = [left, top, right, bottom];
        self
    }
}

fn placeholder_images(cell: &CellRef) -> Vec<ImageCell> {
    cell.attrs()
        .images()
        .unwrap_or_default()
        .into_iter()
        .filter(|im| im.is_unicode_placeholder())
        .collect()
}

fn line_slices(line: &Line) -> Vec<Option<Slice>> {
    let mut slices: Vec<Option<Slice>> = line
        .visible_cells()
        .map(|cell| {
            let images = placeholder_images(&cell);
            assert!(
                images.len() <= 1,
                "cell {} has multiple placeholder images: {:?}",
                cell.cell_index(),
                images
            );
            images.first().map(|im| {
                let (l, t, r, b) = im.padding();
                Slice {
                    image_id: im.image_id().expect("placeholder images have an id"),
                    placement_id: im.placement_id(),
                    tex: [
                        round(*im.top_left().x),
                        round(*im.top_left().y),
                        round(*im.bottom_right().x),
                        round(*im.bottom_right().y),
                    ],
                    padding: [l, t, r, b],
                }
            })
        })
        .collect();
    while let Some(None) = slices.last() {
        slices.pop();
    }
    slices
}

#[track_caller]
fn assert_slices(actual: Vec<Option<Slice>>, expected: &[Option<Slice>], what: &str) {
    let mut expected = expected.to_vec();
    while let Some(None) = expected.last() {
        expected.pop();
    }
    assert_eq!(
        actual, expected,
        "{what}: placeholder slices (left=actual) didn't match (right=expected)"
    );
}

#[track_caller]
fn assert_row(term: &TestTerm, row: usize, expected: &[Option<Slice>]) {
    let lines = term.screen().visible_lines();
    assert_slices(line_slices(&lines[row]), expected, &format!("row {row}"));
}

/// Asserts that no cell in the scrollback or on screen displays a
/// placeholder image
#[track_caller]
fn assert_no_placeholder_images(term: &TestTerm) {
    for (idx, line) in term.screen().all_lines().iter().enumerate() {
        assert_slices(line_slices(line), &[], &format!("phys row {idx}"));
    }
}

/// Returns the slices of each line, across both scrollback and the
/// visible screen, that display any placeholder image
fn all_line_slices(term: &TestTerm) -> Vec<(PhysRowIndex, Vec<Option<Slice>>)> {
    term.screen()
        .all_lines()
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            let slices = line_slices(line);
            if slices.is_empty() {
                None
            } else {
                Some((idx, slices))
            }
        })
        .collect()
}

const IMAGE_B: u32 = (42 << 16) + (43 << 8) + 44;

/// Shared setup for kitty's test_unicode_placeholders: a 10x5 screen with
/// 10x20 pixel cells and two images with 4x2 cell virtual placements
fn kitty_two_images() -> TestTerm {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    transmit(&mut term, &format!("a=T,U=1,i={IMAGE_B},c=4,r=2"), 10, 20);
    term
}

// ---------------------------------------------------------------------------
// Ported from kitty: test_unicode_placeholders

#[test]
fn kitty_virtual_placements_are_invisible() {
    let term = kitty_two_images();
    assert_no_placeholder_images(&term);
}

#[test]
fn kitty_8bit_color_image_id() {
    let mut term = kitty_two_images();
    term.print("\x1b[38;5;42m");
    // These two characters form a single run
    term.print(format!("{}{}", p(&[0, 0]), p(&[0, 1])));
    // These two are separate runs, as the columns are not contiguous
    term.print(format!("{}{}", p(&[0, 0]), p(&[0, 2])));

    // Image 42 is 20x20 and is fit into a 40x40 pixel (4x2 cell) box,
    // so each cell shows a quarter of the width and half of the height.
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.5, 0., 0.75, 0.5)),
        ],
    );

    // Erasing the line removes the images along with the placeholders
    term.print("\x1b[2K");
    assert_no_placeholder_images(&term);
}

#[test]
fn kitty_24bit_color_image_id() {
    let mut term = kitty_two_images();
    // The first image, 1x1
    term.print("\x1b[38;2;0;0;42m");
    term.print(p(&[0, 0]));
    // The second image, 2x1
    term.print("\x1b[38;2;42;43;44m");
    term.print(format!("{}{}", p(&[0, 1]), p(&[0, 2])));

    // The second image is 10x20 in a 40x40 box: it is fit to the height and
    // centered horizontally, so it covers exactly columns 1 and 2 of its
    // placement, and the run therefore spans its full width.
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(IMAGE_B, 0., 0., 0.5, 0.5)),
            Some(slice(IMAGE_B, 0.5, 0., 1.0, 0.5)),
        ],
    );
}

#[test]
fn kitty_implicit_row_and_column() {
    let mut term = kitty_two_images();
    term.print("\x1b[38;5;42m");
    // Full row 0 of the first image, mixing explicit and implicit columns
    term.print(format!("{}{}{}{}", p(&[0, 0]), p(&[0]), p(&[]), p(&[0])));
    // Full row 1 of the first image
    term.print(format!("{}{}{}{}", p(&[1]), p(&[]), p(&[]), p(&[1, 3])));

    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
            Some(slice(42, 0.5, 0., 0.75, 0.5)),
            Some(slice(42, 0.75, 0., 1.0, 0.5)),
            Some(slice(42, 0., 0.5, 0.25, 1.0)),
            Some(slice(42, 0.25, 0.5, 0.5, 1.0)),
            Some(slice(42, 0.5, 0.5, 0.75, 1.0)),
            Some(slice(42, 0.75, 0.5, 1.0, 1.0)),
        ],
    );
}

#[test]
fn kitty_reset_erases_images_but_keeps_virtual_placements() {
    let mut term = kitty_two_images();
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}", p(&[0, 0]), p(&[0, 1])));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
        ],
    );

    // RIS clears the screen, and with it the placeholder images
    term.print("\x1bc");
    assert_no_placeholder_images(&term);

    // but, as in kitty, virtual placements are not tied to a screen
    // position and survive the reset
    term.print("\x1b[38;5;42m");
    term.print(p(&[1, 3]));
    assert_row(&term, 0, &[Some(slice(42, 0.75, 0.5, 1.0, 1.0))]);
}

// ---------------------------------------------------------------------------
// Ported from kitty: test_unicode_placeholders_3rd_combining_char

const IMAGE_C: u32 = (42 << 24) + 43;

fn kitty_msb_images() -> TestTerm {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    transmit(&mut term, &format!("a=T,U=1,i={IMAGE_C},c=4,r=1"), 20, 10);
    term
}

#[test]
fn kitty_msb_missing_image() {
    let mut term = kitty_msb_images();
    // Without the third diacritic this refers to id 43, which does not exist
    term.print("\x1b[38;2;0;0;43m");
    term.print(format!("{}{}{}{}", p(&[0]), p(&[]), p(&[]), p(&[])));
    assert_no_placeholder_images(&term);
}

#[test]
fn kitty_msb_24bit_color() {
    let mut term = kitty_msb_images();
    // id=42, with an explicit (but unnecessary) zero MSB
    term.print("\x1b[38;2;0;0;42m");
    term.print(format!("{}{}", p(&[0, 0, 0]), p(&[0, 1, 0])));
    // The second image, using the MSB diacritic for 42
    term.print("\x1b[38;2;0;0;43m");
    term.print(format!("{}{}", p(&[0, 0, 42]), p(&[0, 1, 42])));
    // Continue the run using implicit columns and MSB
    term.print(format!("{}{}", p(&[0]), p(&[])));

    // IMAGE_C is 20x10 in a 40x20 box, so it fills its placement
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
            Some(slice(IMAGE_C, 0., 0., 0.25, 1.0)),
            Some(slice(IMAGE_C, 0.25, 0., 0.5, 1.0)),
            Some(slice(IMAGE_C, 0.5, 0., 0.75, 1.0)),
            Some(slice(IMAGE_C, 0.75, 0., 1.0, 1.0)),
        ],
    );
}

#[test]
fn kitty_msb_8bit_color() {
    let mut term = kitty_msb_images();
    // In 8-bit color mode, the third diacritic gives us 16 bits of id
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}", p(&[0, 0, 0]), p(&[])));
    term.print("\x1b[38;5;43m");
    term.print(format!("{}{}{}{}", p(&[0, 0, 42]), p(&[]), p(&[0]), p(&[])));

    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
            Some(slice(IMAGE_C, 0., 0., 0.25, 1.0)),
            Some(slice(IMAGE_C, 0.25, 0., 0.5, 1.0)),
            Some(slice(IMAGE_C, 0.5, 0., 0.75, 1.0)),
            Some(slice(IMAGE_C, 0.75, 0., 1.0, 1.0)),
        ],
    );
}

// ---------------------------------------------------------------------------
// Ported from kitty: test_unicode_placeholders_multiple_placements

#[test]
fn kitty_multiple_placements_via_underline_color() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,p=1,c=1,r=1", 20, 20);
    put_ref(&mut term, 42, "U=1,p=22,c=2,r=1");
    put_ref(&mut term, 42, "U=1,p=44,c=4,r=2");
    assert_no_placeholder_images(&term);

    // Draw the first row of each placement
    term.print("\x1b[38;5;42m");
    term.print("\x1b[58;5;1m");
    term.print(p(&[0]));
    term.print("\x1b[58;5;22m");
    term.print(format!("{}{}", p(&[0]), p(&[0])));
    term.print("\x1b[58;5;44m");
    term.print(format!("{}{}{}{}", p(&[0]), p(&[0]), p(&[0]), p(&[0])));

    assert_row(
        &term,
        0,
        &[
            // A 20x20 image in a single 10x20 cell is fit to the width and
            // centered vertically: it occupies pixels 5..15 of the cell.
            // (kitty expresses this as a source rect extending to 1.5)
            Some(slice(42, 0., 0., 1., 1.).placement(1).padding(0, 5, 0, 10)),
            // In a 20x20 box, the image fits exactly
            Some(slice(42, 0., 0., 0.5, 1.).placement(22)),
            Some(slice(42, 0.5, 0., 1., 1.).placement(22)),
            // The first row of the 4x2 placement
            Some(slice(42, 0., 0., 0.25, 0.5).placement(44)),
            Some(slice(42, 0.25, 0., 0.5, 0.5).placement(44)),
            Some(slice(42, 0.5, 0., 0.75, 0.5).placement(44)),
            Some(slice(42, 0.75, 0., 1., 0.5).placement(44)),
        ],
    );
}

// ---------------------------------------------------------------------------
// Ported from kitty: test_unicode_placeholders_scroll

/// The image row `n` of an 8 row image, one cell wide
fn eighth(n: usize) -> Option<Slice> {
    Some(slice(42, 0., 0.125 * n as f32, 1., 0.125 * (n + 1) as f32))
}

#[test]
fn kitty_scroll_region() {
    let mut term = new_term(8, 10, 5, 10, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=1,r=8", 5, 80);
    term.print("\x1b[38;5;42m");
    for row in 0..8 {
        term.cup(0, row as isize);
        term.print(p(&[row]));
    }
    // Each line displays a part of the image
    for row in 0..8 {
        assert_row(&term, row, &[eighth(row)]);
    }

    // Set margins to lines 3..=6 (1-based) and scroll that region up by two:
    // lines 3 and 4 are erased and lines 5 and 6 move up.
    term.set_scroll_region(2, 5);
    term.cup(0, 5);
    term.print("\x1bD\x1bD");
    let expect = [
        eighth(0),
        eighth(1),
        eighth(4),
        eighth(5),
        None,
        None,
        eighth(6),
        eighth(7),
    ];
    for (row, expected) in expect.iter().enumerate() {
        assert_row(&term, row, &[*expected]);
    }

    // Now scroll the region down three lines: line 5 (originally image
    // row 4) moves to the bottom of the region and line 6 is erased
    term.cup(0, 2);
    term.print("\x1bM\x1bM\x1bM");
    let expect = [
        eighth(0),
        eighth(1),
        None,
        None,
        None,
        eighth(4),
        eighth(6),
        eighth(7),
    ];
    for (row, expected) in expect.iter().enumerate() {
        assert_row(&term, row, &[*expected]);
    }
}

// ---------------------------------------------------------------------------
// Ported from kitty: test_unicode_placeholders_reverse_scroll

#[test]
fn kitty_reverse_scroll_does_not_resurrect_images() {
    let mut term = new_term(4, 10, 5, 10, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=1,r=1", 5, 10);

    // Via reverse index
    term.print("\x1b[38;5;42m");
    term.cup(0, 3);
    term.print(p(&[0]));
    assert_row(&term, 3, &[Some(slice(42, 0., 0., 1., 1.))]);
    term.cup(0, 0);
    term.print("\x1bM");
    assert_no_placeholder_images(&term);
    term.cup(0, 3);
    term.print("\x1bD");
    assert_no_placeholder_images(&term);

    // Same via CSI T
    term.print("\x1bc");
    term.print("\x1b[38;5;42m");
    term.cup(0, 3);
    term.print(p(&[0]));
    assert_row(&term, 3, &[Some(slice(42, 0., 0., 1., 1.))]);
    term.print("\x1b[2T");
    assert_no_placeholder_images(&term);
    term.cup(0, 3);
    term.print("\x1bD\x1bD");
    assert_no_placeholder_images(&term);
}

// ---------------------------------------------------------------------------
// Adapted from kitty: test_unicode_placeholders_history_margins and
// test_unicode_placeholders_history_fill. In kitty, images for lines in the
// history are only resolved when the view is scrolled back; in wezterm they
// are attached to the cells and so they travel into the scrollback with them.

fn draw_two_row_placeholders(term: &mut TestTerm) {
    term.print("\x1b[38;5;42m");
    term.cup(0, 2);
    term.print(p(&[0]));
    term.cup(0, 3);
    term.print(p(&[1]));
    term.print("\x1b[39m");
}

fn index_plain_lines(term: &mut TestTerm, row: isize, n: usize) {
    term.cup(0, row);
    for _ in 0..n {
        term.print("plain\x1bD\r");
    }
}

#[test]
fn kitty_history_keeps_images() {
    let mut term = new_term(6, 10, 5, 10, 20);
    transmit(&mut term, "a=T,U=1,i=42,c=1,r=2", 5, 20);
    draw_two_row_placeholders(&mut term);
    index_plain_lines(&mut term, 5, 8);

    let scrollback_rows = term.screen().scrollback_rows() - term.screen().physical_rows;
    let found = all_line_slices(&term);
    assert_eq!(
        found
            .iter()
            .map(|(_, slices)| slices.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![Some(slice(42, 0., 0., 1., 0.5))],
            vec![Some(slice(42, 0., 0.5, 1., 1.))],
        ]
    );
    for (idx, _) in &found {
        assert!(*idx < scrollback_rows, "line {} is in the scrollback", idx);
    }

    // A reverse index at the top of the screen must not affect the
    // lines in the history
    term.cup(0, 0);
    term.print("\x1bM");
    assert_eq!(all_line_slices(&term), found);
}

// ---------------------------------------------------------------------------
// Additional coverage beyond kitty's own tests

#[test]
fn virtual_placement_does_not_move_cursor_or_attach_to_cells() {
    let mut term = new_term(5, 10, 10, 20, 0);
    term.cup(3, 1);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    assert_cursor(&term, 3, 1);
    put_ref(&mut term, 42, "U=1,p=5,c=2,r=2");
    assert_cursor(&term, 3, 1);
    for line in term.screen().all_lines() {
        for cell in line.visible_cells() {
            assert!(cell.attrs().images().is_none(), "no images on screen");
        }
    }
}

#[test]
fn kitty_style_zero_valued_keys_mean_unset() {
    // kitty's test helpers send every key, using zero for "unset"
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(
        &mut term,
        "a=T,i=42,z=0,c=4,r=2,x=0,y=0,w=0,h=0,X=0,Y=0,p=0,C=0,U=1,P=0,Q=0,H=0,V=0",
        20,
        20,
    );
    assert_cursor(&term, 0, 0);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}", p(&[0, 0]), p(&[])));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
        ],
    );
}

#[test]
fn placeholder_text_is_preserved() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}", p(&[0, 0]), p(&[0, 1])));
    // Each placeholder occupies a single cell
    assert_cursor(&term, 2, 0);
    let lines = term.screen().visible_lines();
    assert_eq!(
        lines[0].as_str().to_string(),
        format!("{}{}", p(&[0, 0]), p(&[0, 1]))
    );
}

#[test]
fn placeholder_images_are_drawn_below_text() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2,z=5", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(p(&[0, 0]));
    let lines = term.screen().visible_lines();
    let cell = lines[0].visible_cells().next().unwrap();
    let images = placeholder_images(&cell);
    assert_eq!(images.len(), 1);
    // As in kitty, the z-index of the virtual placement is ignored
    // and the cursor is drawn on top of the image
    assert_eq!(images[0].z_index(), -1);
}

#[test]
fn virtual_placement_created_after_placeholders() {
    let mut term = new_term(5, 10, 10, 20, 0);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}", p(&[0, 0]), p(&[])));
    assert_no_placeholder_images(&term);

    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
        ],
    );
}

#[test]
fn virtual_placement_resolves_lines_in_scrollback() {
    let mut term = new_term(6, 10, 5, 10, 20);
    draw_two_row_placeholders(&mut term);
    index_plain_lines(&mut term, 5, 8);
    assert_no_placeholder_images(&term);

    transmit(&mut term, "a=T,U=1,i=42,c=1,r=2", 5, 20);
    let scrollback_rows = term.screen().scrollback_rows() - term.screen().physical_rows;
    let found = all_line_slices(&term);
    assert_eq!(
        found
            .iter()
            .map(|(_, slices)| slices.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![Some(slice(42, 0., 0., 1., 0.5))],
            vec![Some(slice(42, 0., 0.5, 1., 1.))],
        ]
    );
    for (idx, _) in &found {
        assert!(*idx < scrollback_rows, "line {} is in the scrollback", idx);
    }
}

#[test]
fn placeholders_on_alt_screen() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[?1049h");
    term.print("\x1b[38;5;42m");
    term.print(p(&[0, 0]));
    assert_row(&term, 0, &[Some(slice(42, 0., 0., 0.25, 0.5))]);
    term.print("\x1b[?1049l");
    assert_no_placeholder_images(&term);
}

#[test]
fn msb_is_inherited_by_implicit_cells() {
    // The example from the kitty documentation: 33554474 = 42 + (2 << 24)
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=33554474,c=4,r=2", 20, 20);
    // An image with the low 24 bits only must not be chosen
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}{}", p(&[0, 0, 2]), p(&[]), p(&[0])));
    assert_row(
        &term,
        0,
        &[
            Some(slice(33554474, 0., 0., 0.25, 0.5)),
            Some(slice(33554474, 0.25, 0., 0.5, 0.5)),
            Some(slice(33554474, 0.5, 0., 0.75, 0.5)),
        ],
    );
}

#[test]
fn default_foreground_is_plain_text() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print(format!("{}{}", p(&[0, 0]), p(&[0, 1])));
    assert_no_placeholder_images(&term);
}

#[test]
fn unknown_placement_id_shows_nothing() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,p=1,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m\x1b[58;5;9m");
    term.print(p(&[0, 0]));
    assert_no_placeholder_images(&term);
}

#[test]
fn non_virtual_placement_is_not_used_by_placeholders() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=t,i=42", 20, 20);
    // A regular placement of the image elsewhere on the screen
    term.cup(0, 3);
    put_ref(&mut term, 42, "p=1,c=2,r=1");
    term.cup(0, 0);
    term.print("\x1b[38;5;42m");
    term.print(p(&[0, 0]));
    assert_no_placeholder_images(&term);
    term.print("\x1b[58;5;1m");
    term.print(p(&[0, 0]));
    assert_no_placeholder_images(&term);
}

#[test]
fn placement_id_via_24bit_underline_color() {
    let mut term = new_term(5, 10, 10, 20, 0);
    let placement_id = (1 << 16) + (2 << 8) + 3;
    transmit(
        &mut term,
        &format!("a=T,U=1,i=42,p={placement_id},c=1,r=1"),
        20,
        20,
    );
    put_ref(&mut term, 42, "U=1,p=3,c=4,r=2");
    term.print("\x1b[38;5;42m\x1b[58;2;1;2;3m");
    term.print(p(&[0, 0]));
    assert_row(
        &term,
        0,
        &[Some(
            slice(42, 0., 0., 1., 1.)
                .placement(placement_id)
                .padding(0, 5, 0, 10),
        )],
    );
}

#[test]
fn invalid_diacritic_is_treated_as_missing() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    // U+0300 is not one of the row/column diacritics, so the row is
    // unknown (defaulting to 0) while the column is still 1
    term.print(format!("{PLACEHOLDER}\u{300}{}", diacritic(1)));
    assert_row(&term, 0, &[Some(slice(42, 0.25, 0., 0.5, 0.5))]);
}

#[test]
fn run_is_broken_by_other_text() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}x{}", p(&[0, 0]), p(&[])));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            None,
            Some(slice(42, 0., 0., 0.25, 0.5)),
        ],
    );
}

#[test]
fn run_is_broken_by_image_id_change() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    transmit(&mut term, "a=T,U=1,i=43,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(p(&[0, 0]));
    term.print("\x1b[38;5;43m");
    term.print(p(&[]));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(43, 0., 0., 0.25, 0.5)),
        ],
    );
}

#[test]
fn run_is_broken_by_placement_id_change() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,p=1,c=4,r=2", 20, 20);
    put_ref(&mut term, 42, "U=1,p=2,c=4,r=2");
    term.print("\x1b[38;5;42m\x1b[58;5;1m");
    term.print(p(&[0, 0]));
    term.print("\x1b[58;5;2m");
    term.print(p(&[]));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5).placement(1)),
            Some(slice(42, 0., 0., 0.25, 0.5).placement(2)),
        ],
    );
}

#[test]
fn run_is_broken_by_explicit_row_change() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}{}", p(&[0, 0]), p(&[1]), p(&[])));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0., 0.5, 0.25, 1.)),
            Some(slice(42, 0.25, 0.5, 0.5, 1.)),
        ],
    );
}

#[test]
fn runs_do_not_continue_across_lines() {
    let mut term = new_term(5, 4, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    // Six cells on a 4 column screen: the last two wrap onto the next
    // line, where inference starts over from column 0
    term.print(format!(
        "{}{}{}{}{}{}",
        p(&[0, 0]),
        p(&[]),
        p(&[]),
        p(&[]),
        p(&[]),
        p(&[])
    ));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
            Some(slice(42, 0.5, 0., 0.75, 0.5)),
            Some(slice(42, 0.75, 0., 1., 0.5)),
        ],
    );
    assert_row(
        &term,
        1,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
        ],
    );
}

#[test]
fn overwriting_a_cell_reinfers_the_rest_of_the_run() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}{}", p(&[0, 0]), p(&[]), p(&[])));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
            Some(slice(42, 0.5, 0., 0.75, 0.5)),
        ],
    );

    // The second cell now starts a new run at column 0
    term.cup(0, 0);
    term.print("x");
    assert_row(
        &term,
        0,
        &[
            None,
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
        ],
    );
}

#[test]
fn inserting_a_cell_reinfers_the_rest_of_the_run() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}{}", p(&[0, 0]), p(&[]), p(&[])));
    term.cup(1, 0);
    // ICH: insert a blank cell
    term.print("\x1b[@");
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            None,
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
        ],
    );

    // DCH: delete it again, rejoining the run
    term.print("\x1b[P");
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
            Some(slice(42, 0.5, 0., 0.75, 0.5)),
        ],
    );
}

#[test]
fn natural_size_when_columns_and_rows_are_omitted() {
    let mut term = new_term(5, 10, 10, 20, 0);
    // 25x30 pixels needs 3x2 cells of 10x20 pixels: a 30x40 pixel box.
    // The image is fit to the width (scale 1.2, 36 pixels tall) and
    // centered vertically, leaving 2 pixels above and below.
    transmit(&mut term, "a=T,U=1,i=42", 25, 30);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}{}", p(&[0, 0]), p(&[]), p(&[])));
    term.cup(0, 1);
    term.print(format!("{}{}{}", p(&[1, 0]), p(&[]), p(&[])));
    let third = 1. / 3.;
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., third, 0.5).padding(0, 2, 0, 2)),
            Some(slice(42, third, 0., 2. * third, 0.5).padding(0, 2, 0, 2)),
            Some(slice(42, 2. * third, 0., 1., 0.5).padding(0, 2, 0, 2)),
        ],
    );
    assert_row(
        &term,
        1,
        &[
            Some(slice(42, 0., 0.5, third, 1.).padding(0, 0, 0, 2)),
            Some(slice(42, third, 0.5, 2. * third, 1.).padding(0, 0, 0, 2)),
            Some(slice(42, 2. * third, 0.5, 1., 1.).padding(0, 0, 0, 2)),
        ],
    );
}

#[test]
fn rows_from_natural_size_when_only_columns_given() {
    let mut term = new_term(5, 10, 10, 20, 0);
    // The rows are computed from the natural image height (1 row), giving a
    // 40x20 box: the 20x20 image is centered, covering only columns 1 and 2
    transmit(&mut term, "a=T,U=1,i=42,c=4", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}{}{}", p(&[0, 0]), p(&[]), p(&[]), p(&[])));
    assert_row(
        &term,
        0,
        &[
            None,
            Some(slice(42, 0., 0., 0.5, 1.)),
            Some(slice(42, 0.5, 0., 1., 1.)),
            None,
        ],
    );
}

#[test]
fn letterboxed_image_partially_covers_cells() {
    let mut term = new_term(5, 10, 10, 20, 0);
    // A 14x20 image in a 20x20 box is centered horizontally,
    // covering pixels 3..17
    transmit(&mut term, "a=T,U=1,i=42,c=2,r=1", 14, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}", p(&[0, 0]), p(&[])));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.5, 1.).padding(3, 0, 3, 0)),
            Some(slice(42, 0.5, 0., 1., 1.).padding(0, 0, 3, 0)),
        ],
    );
}

#[test]
fn cells_outside_the_placement_are_empty() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=2,r=1", 20, 20);
    term.print("\x1b[38;5;42m");
    // Column 2 is beyond the 2 column placement
    term.print(format!("{}{}{}", p(&[0, 0]), p(&[]), p(&[])));
    // Row 1 is beyond the 1 row placement
    term.cup(0, 1);
    term.print(p(&[1, 0]));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.5, 1.)),
            Some(slice(42, 0.5, 0., 1., 1.)),
            None,
        ],
    );
    assert_row(&term, 1, &[]);
}

#[test]
fn delete_by_image_id_removes_virtual_placements() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}", p(&[0, 0]), p(&[])));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
        ],
    );

    delete(&mut term, "d=i,i=42");
    assert_no_placeholder_images(&term);
    // The virtual placement is gone, so new placeholders show nothing
    term.print(p(&[0, 0]));
    assert_no_placeholder_images(&term);

    // but the image data is retained and a new placement can be made
    put_ref(&mut term, 42, "U=1,c=4,r=2");
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
            Some(slice(42, 0., 0., 0.25, 0.5)),
        ],
    );
}

#[test]
fn delete_by_image_id_and_free_data() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(p(&[0, 0]));
    assert_row(&term, 0, &[Some(slice(42, 0., 0., 0.25, 0.5))]);

    delete(&mut term, "d=I,i=42");
    assert_no_placeholder_images(&term);
    // The data was freed, so no placement can be made
    put_ref(&mut term, 42, "U=1,c=4,r=2");
    assert_no_placeholder_images(&term);
}

#[test]
fn delete_by_placement_id_only_affects_that_placement() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,p=1,c=4,r=2", 20, 20);
    put_ref(&mut term, 42, "U=1,p=2,c=4,r=2");
    term.print("\x1b[38;5;42m\x1b[58;5;1m");
    term.print(p(&[0, 0]));
    term.print("\x1b[58;5;2m");
    term.print(p(&[0, 0]));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5).placement(1)),
            Some(slice(42, 0., 0., 0.25, 0.5).placement(2)),
        ],
    );

    delete(&mut term, "d=i,i=42,p=1");
    assert_row(
        &term,
        0,
        &[None, Some(slice(42, 0., 0., 0.25, 0.5).placement(2))],
    );
}

#[test]
fn delete_all_does_not_affect_virtual_placements() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(p(&[0, 0]));
    let expected = [Some(slice(42, 0., 0., 0.25, 0.5))];
    assert_row(&term, 0, &expected);

    // "a" and "A" never affect virtual placements, nor the images
    // displayed on top of placeholders
    delete(&mut term, "d=a");
    assert_row(&term, 0, &expected);
    delete(&mut term, "d=A");
    assert_row(&term, 0, &expected);

    // and the image data is retained for the virtual placement
    term.print(p(&[0, 1]));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5)),
            Some(slice(42, 0.25, 0., 0.5, 0.5)),
        ],
    );
}

#[test]
fn retransmitting_an_image_removes_its_virtual_placements() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(p(&[0, 0]));
    assert_row(&term, 0, &[Some(slice(42, 0., 0., 0.25, 0.5))]);

    // Replacing the image deletes its placements, as in kitty
    transmit(&mut term, "a=t,i=42", 20, 10);
    assert_no_placeholder_images(&term);

    // A new virtual placement uses the new image data:
    // a 20x10 image fits a 4x1 placement exactly
    put_ref(&mut term, 42, "U=1,c=4,r=1");
    assert_row(&term, 0, &[Some(slice(42, 0., 0., 0.25, 1.))]);
}

#[test]
fn replacing_a_named_virtual_placement_updates_its_geometry() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,p=7,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}{}{}", p(&[0, 0]), p(&[]), p(&[]), p(&[])));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.25, 0.5).placement(7)),
            Some(slice(42, 0.25, 0., 0.5, 0.5).placement(7)),
            Some(slice(42, 0.5, 0., 0.75, 0.5).placement(7)),
            Some(slice(42, 0.75, 0., 1., 0.5).placement(7)),
        ],
    );

    put_ref(&mut term, 42, "U=1,p=7,c=2,r=1");
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.5, 1.).placement(7)),
            Some(slice(42, 0.5, 0., 1., 1.).placement(7)),
        ],
    );
}

#[test]
fn first_virtual_placement_is_used_without_placement_id() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=t,i=42", 20, 20);
    put_ref(&mut term, 42, "U=1,c=4,r=2");
    put_ref(&mut term, 42, "U=1,c=1,r=1");
    put_ref(&mut term, 42, "U=1,p=3,c=2,r=1");
    term.print("\x1b[38;5;42m");
    term.print(p(&[0, 0]));
    assert_row(&term, 0, &[Some(slice(42, 0., 0., 0.25, 0.5))]);
}

#[test]
fn resize_recomputes_for_the_new_cell_size() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=2,r=1", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}", p(&[0, 0]), p(&[])));
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.5, 1.)),
            Some(slice(42, 0.5, 0., 1., 1.)),
        ],
    );

    // With 20x20 cells, the box is 40x20 and the image covers 10..30
    term.resize(TerminalSize {
        rows: 5,
        cols: 10,
        pixel_width: 200,
        pixel_height: 100,
        dpi: 0,
    });
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.5, 1.).padding(10, 0, 10, 0)),
            Some(slice(42, 0.5, 0., 1., 1.).padding(0, 0, 10, 0)),
        ],
    );
}

#[test]
fn no_pixel_dimensions_shows_nothing_until_resized() {
    let mut term = new_term(5, 10, 0, 0, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=2,r=1", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}", p(&[0, 0]), p(&[])));
    assert_no_placeholder_images(&term);

    term.resize(TerminalSize {
        rows: 5,
        cols: 10,
        pixel_width: 100,
        pixel_height: 100,
        dpi: 0,
    });
    assert_row(
        &term,
        0,
        &[
            Some(slice(42, 0., 0., 0.5, 1.)),
            Some(slice(42, 0.5, 0., 1., 1.)),
        ],
    );
}

#[test]
fn regular_placement_over_placeholders_is_kept() {
    let mut term = new_term(5, 10, 10, 20, 0);
    transmit(&mut term, "a=T,U=1,i=42,c=4,r=2", 20, 20);
    term.print("\x1b[38;5;42m");
    term.print(format!("{}{}", p(&[0, 0]), p(&[])));

    // Place image 43 normally on top of the placeholder cells
    term.cup(0, 0);
    transmit(&mut term, "a=T,i=43,C=1,c=2,r=1", 20, 20);
    // Then modify the line to force the placeholders to be resolved again
    term.cup(5, 0);
    term.print("x");

    let lines = term.screen().visible_lines();
    for cell in lines[0].visible_cells().take(2) {
        let images = cell.attrs().images().unwrap_or_default();
        assert_eq!(
            images
                .iter()
                .map(|im| (im.image_id(), im.is_unicode_placeholder()))
                .collect::<Vec<_>>(),
            vec![(Some(42), true), (Some(43), false)],
            "cell {}",
            cell.cell_index()
        );
    }
}

#[test]
fn virtual_placement_with_chunked_transmission() {
    let mut term = new_term(5, 10, 10, 20, 0);
    let data = vec![0xffu8; 20 * 20 * 4];
    let (first, second) = data.split_at(800);
    let encode = |d: &[u8]| base64::engine::general_purpose::STANDARD.encode(d);
    term.print(format!(
        "\x1b_Ga=T,U=1,i=42,c=4,r=2,f=32,s=20,v=20,q=2,m=1;{}\x1b\\",
        encode(first)
    ));
    term.print(format!("\x1b_Gm=0;{}\x1b\\", encode(second)));
    assert_cursor(&term, 0, 0);
    term.print("\x1b[38;5;42m");
    term.print(p(&[1, 3]));
    assert_row(&term, 0, &[Some(slice(42, 0.75, 0.5, 1., 1.))]);
}

#[test]
fn virtual_placement_via_image_number() {
    let mut term = new_term(5, 10, 10, 20, 0);
    // The image is assigned the next free id, 1
    transmit(&mut term, "a=T,U=1,I=7,c=4,r=2", 20, 20);
    put_ref(&mut term, 1, "U=1,p=2,c=2,r=1");
    term.print("\x1b[38;5;1m");
    term.print(p(&[0, 0]));
    term.print("\x1b[58;5;2m");
    term.print(p(&[0, 0]));
    assert_row(
        &term,
        0,
        &[
            Some(slice(1, 0., 0., 0.25, 0.5)),
            Some(slice(1, 0., 0., 0.5, 1.).placement(2)),
        ],
    );
}
