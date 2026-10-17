#![cfg(test)]

use super::*;
use crate::hyperlink::{Hyperlink, Rule};
use crate::line::clusterline::ClusteredLine;
use crate::line::storage::CellStorage;
use crate::{SequenceNo, SEQ_ZERO};
use alloc::sync::Arc;
use k9::assert_equal as assert_eq;
use wezterm_cell::{Cell, CellAttributes};

/// There are 4 double-wide graphemes that occupy 2 cells each.
/// When we join the lines, we must preserve the invisible blank
/// that is part of the grapheme otherwise our metrics will be
/// wrong.
/// <https://github.com/wezterm/wezterm/issues/2568>
#[test]
fn append_line() {
    let mut line1: Line = "0123456789".into();
    let line2: Line = "グループaa".into();

    line1.append_line(line2, SEQ_ZERO);

    assert_eq!(line1.len(), 20);
}

#[test]
fn hyperlinks() {
    let text = "❤ 😍🤢 http://example.com \u{1f468}\u{1f3fe}\u{200d}\u{1f9b0} http://example.com";

    let rules = vec![
        Rule::new(r"\b\w+://(?:[\w.-]+)\.[a-z]{2,15}\S*\b", "$0").unwrap(),
        Rule::new(r"\b\w+@[\w-]+(\.[\w-]+)+\b", "mailto:$0").unwrap(),
    ];

    let hyperlink = Arc::new(Hyperlink::new_implicit("http://example.com"));
    let hyperlink_attr = CellAttributes::default()
        .set_hyperlink(Some(hyperlink.clone()))
        .clone();

    let mut line: Line = text.into();
    line.scan_and_create_hyperlinks(&rules);
    assert!(line.has_hyperlink());
    assert_eq!(
        line.coerce_vec_storage().to_vec(),
        vec![
            Cell::new_grapheme("❤", CellAttributes::default(), None),
            Cell::new(' ', CellAttributes::default()), // double width spacer
            Cell::new_grapheme("😍", CellAttributes::default(), None),
            Cell::new(' ', CellAttributes::default()), // double width spacer
            Cell::new_grapheme("🤢", CellAttributes::default(), None),
            Cell::new(' ', CellAttributes::default()), // double width spacer
            Cell::new(' ', CellAttributes::default()),
            Cell::new('h', hyperlink_attr.clone()),
            Cell::new('t', hyperlink_attr.clone()),
            Cell::new('t', hyperlink_attr.clone()),
            Cell::new('p', hyperlink_attr.clone()),
            Cell::new(':', hyperlink_attr.clone()),
            Cell::new('/', hyperlink_attr.clone()),
            Cell::new('/', hyperlink_attr.clone()),
            Cell::new('e', hyperlink_attr.clone()),
            Cell::new('x', hyperlink_attr.clone()),
            Cell::new('a', hyperlink_attr.clone()),
            Cell::new('m', hyperlink_attr.clone()),
            Cell::new('p', hyperlink_attr.clone()),
            Cell::new('l', hyperlink_attr.clone()),
            Cell::new('e', hyperlink_attr.clone()),
            Cell::new('.', hyperlink_attr.clone()),
            Cell::new('c', hyperlink_attr.clone()),
            Cell::new('o', hyperlink_attr.clone()),
            Cell::new('m', hyperlink_attr.clone()),
            Cell::new(' ', CellAttributes::default()),
            Cell::new_grapheme(
                // man: dark skin tone, red hair ZWJ emoji grapheme
                "\u{1f468}\u{1f3fe}\u{200d}\u{1f9b0}",
                CellAttributes::default(),
                None,
            ),
            Cell::new(' ', CellAttributes::default()), // double width spacer
            Cell::new(' ', CellAttributes::default()),
            Cell::new('h', hyperlink_attr.clone()),
            Cell::new('t', hyperlink_attr.clone()),
            Cell::new('t', hyperlink_attr.clone()),
            Cell::new('p', hyperlink_attr.clone()),
            Cell::new(':', hyperlink_attr.clone()),
            Cell::new('/', hyperlink_attr.clone()),
            Cell::new('/', hyperlink_attr.clone()),
            Cell::new('e', hyperlink_attr.clone()),
            Cell::new('x', hyperlink_attr.clone()),
            Cell::new('a', hyperlink_attr.clone()),
            Cell::new('m', hyperlink_attr.clone()),
            Cell::new('p', hyperlink_attr.clone()),
            Cell::new('l', hyperlink_attr.clone()),
            Cell::new('e', hyperlink_attr.clone()),
            Cell::new('.', hyperlink_attr.clone()),
            Cell::new('c', hyperlink_attr.clone()),
            Cell::new('o', hyperlink_attr.clone()),
            Cell::new('m', hyperlink_attr.clone()),
        ]
    );
}

#[test]
fn double_click_range_bounds() {
    let line: Line = "hello".into();
    let r = line.compute_double_click_range(200, |_| true);
    assert_eq!(r, DoubleClickRange::Range(200..200));
}

#[test]
fn cluster_representation_basic() {
    let line: Line = "hello".into();
    let mut compressed = line.clone();
    compressed.compress_for_scrollback();
    k9::snapshot!(
        &compressed.cells,
        r#"
C(
    ClusteredLine {
        text: "hello",
        is_double_wide: None,
        clusters: [
            Cluster {
                cell_width: 5,
                attrs: CellAttributes {
                    attributes: 0,
                    intensity: Normal,
                    underline: None,
                    blink: None,
                    italic: false,
                    reverse: false,
                    strikethrough: false,
                    invisible: false,
                    wrapped: false,
                    overline: false,
                    semantic_type: Output,
                    foreground: Default,
                    background: Default,
                    fat: None,
                },
            },
        ],
        len: 5,
        last_cell_width: Some(
            1,
        ),
    },
)
"#
    );
    compressed.coerce_vec_storage();
    assert_eq!(line, compressed);
}

#[test]
fn cluster_representation_double_width() {
    let line: Line = "❤ 😍🤢he❤ 😍🤢llo❤ 😍🤢".into();
    let mut compressed = line.clone();
    compressed.compress_for_scrollback();
    k9::snapshot!(
        &compressed.cells,
        r#"
C(
    ClusteredLine {
        text: "❤ 😍🤢he❤ 😍🤢llo❤ 😍🤢",
        is_double_wide: Some(
            FixedBitSet {
                data: [
                    2626580,
                ],
                length: 23,
            },
        ),
        clusters: [
            Cluster {
                cell_width: 23,
                attrs: CellAttributes {
                    attributes: 0,
                    intensity: Normal,
                    underline: None,
                    blink: None,
                    italic: false,
                    reverse: false,
                    strikethrough: false,
                    invisible: false,
                    wrapped: false,
                    overline: false,
                    semantic_type: Output,
                    foreground: Default,
                    background: Default,
                    fat: None,
                },
            },
        ],
        len: 23,
        last_cell_width: Some(
            1,
        ),
    },
)
"#
    );
    compressed.coerce_vec_storage();
    assert_eq!(line, compressed);
}

#[test]
fn cluster_representation_empty() {
    let line = Line::from_cells(vec![], SEQ_ZERO);

    let mut compressed = line.clone();
    compressed.compress_for_scrollback();
    k9::snapshot!(
        &compressed.cells,
        r#"
C(
    ClusteredLine {
        text: "",
        is_double_wide: None,
        clusters: [],
        len: 0,
        last_cell_width: None,
    },
)
"#
    );
    compressed.coerce_vec_storage();
    assert_eq!(line, compressed);
}

#[test]
fn cluster_wrap_last() {
    let mut line: Line = "hello".into();
    line.compress_for_scrollback();
    line.set_last_cell_was_wrapped(true, 1);
    k9::snapshot!(
        line,
        r#"
Line {
    cells: C(
        ClusteredLine {
            text: "hello",
            is_double_wide: None,
            clusters: [
                Cluster {
                    cell_width: 4,
                    attrs: CellAttributes {
                        attributes: 0,
                        intensity: Normal,
                        underline: None,
                        blink: None,
                        italic: false,
                        reverse: false,
                        strikethrough: false,
                        invisible: false,
                        wrapped: false,
                        overline: false,
                        semantic_type: Output,
                        foreground: Default,
                        background: Default,
                        fat: None,
                    },
                },
                Cluster {
                    cell_width: 1,
                    attrs: CellAttributes {
                        attributes: 2048,
                        intensity: Normal,
                        underline: None,
                        blink: None,
                        italic: false,
                        reverse: false,
                        strikethrough: false,
                        invisible: false,
                        wrapped: true,
                        overline: false,
                        semantic_type: Output,
                        foreground: Default,
                        background: Default,
                        fat: None,
                    },
                },
            ],
            len: 5,
            last_cell_width: Some(
                1,
            ),
        },
    ),
    zones: [],
    seqno: 1,
    bits: LineBits(
        0x0,
    ),
    appdata: Mutex {
        data: None,
        poisoned: false,
        ..
    },
}
"#
    );
}

fn bold() -> CellAttributes {
    use wezterm_cell::Intensity;
    let mut attr = CellAttributes::default();
    attr.set_intensity(Intensity::Bold);
    attr
}

#[test]
fn cluster_representation_attributes() {
    let line = Line::from_cells(
        vec![
            Cell::new_grapheme("a", CellAttributes::default(), None),
            Cell::new_grapheme("b", bold(), None),
            Cell::new_grapheme("c", CellAttributes::default(), None),
            Cell::new_grapheme("d", bold(), None),
        ],
        SEQ_ZERO,
    );

    let mut compressed = line.clone();
    compressed.compress_for_scrollback();
    k9::snapshot!(
        &compressed.cells,
        r#"
C(
    ClusteredLine {
        text: "abcd",
        is_double_wide: None,
        clusters: [
            Cluster {
                cell_width: 1,
                attrs: CellAttributes {
                    attributes: 0,
                    intensity: Normal,
                    underline: None,
                    blink: None,
                    italic: false,
                    reverse: false,
                    strikethrough: false,
                    invisible: false,
                    wrapped: false,
                    overline: false,
                    semantic_type: Output,
                    foreground: Default,
                    background: Default,
                    fat: None,
                },
            },
            Cluster {
                cell_width: 1,
                attrs: CellAttributes {
                    attributes: 1,
                    intensity: Bold,
                    underline: None,
                    blink: None,
                    italic: false,
                    reverse: false,
                    strikethrough: false,
                    invisible: false,
                    wrapped: false,
                    overline: false,
                    semantic_type: Output,
                    foreground: Default,
                    background: Default,
                    fat: None,
                },
            },
            Cluster {
                cell_width: 1,
                attrs: CellAttributes {
                    attributes: 0,
                    intensity: Normal,
                    underline: None,
                    blink: None,
                    italic: false,
                    reverse: false,
                    strikethrough: false,
                    invisible: false,
                    wrapped: false,
                    overline: false,
                    semantic_type: Output,
                    foreground: Default,
                    background: Default,
                    fat: None,
                },
            },
            Cluster {
                cell_width: 1,
                attrs: CellAttributes {
                    attributes: 1,
                    intensity: Bold,
                    underline: None,
                    blink: None,
                    italic: false,
                    reverse: false,
                    strikethrough: false,
                    invisible: false,
                    wrapped: false,
                    overline: false,
                    semantic_type: Output,
                    foreground: Default,
                    background: Default,
                    fat: None,
                },
            },
        ],
        len: 4,
        last_cell_width: Some(
            1,
        ),
    },
)
"#
    );
    compressed.coerce_vec_storage();
    assert_eq!(line, compressed);
}

#[test]
fn cluster_append() {
    let mut cl = ClusteredLine::new();
    cl.append(Cell::new_grapheme("h", CellAttributes::default(), None));
    cl.append(Cell::new_grapheme("e", CellAttributes::default(), None));
    cl.append(Cell::new_grapheme("l", bold(), None));
    cl.append(Cell::new_grapheme("l", CellAttributes::default(), None));
    cl.append(Cell::new_grapheme("o", CellAttributes::default(), None));
    k9::snapshot!(
        cl,
        r#"
ClusteredLine {
    text: "hello",
    is_double_wide: None,
    clusters: [
        Cluster {
            cell_width: 2,
            attrs: CellAttributes {
                attributes: 0,
                intensity: Normal,
                underline: None,
                blink: None,
                italic: false,
                reverse: false,
                strikethrough: false,
                invisible: false,
                wrapped: false,
                overline: false,
                semantic_type: Output,
                foreground: Default,
                background: Default,
                fat: None,
            },
        },
        Cluster {
            cell_width: 1,
            attrs: CellAttributes {
                attributes: 1,
                intensity: Bold,
                underline: None,
                blink: None,
                italic: false,
                reverse: false,
                strikethrough: false,
                invisible: false,
                wrapped: false,
                overline: false,
                semantic_type: Output,
                foreground: Default,
                background: Default,
                fat: None,
            },
        },
        Cluster {
            cell_width: 2,
            attrs: CellAttributes {
                attributes: 0,
                intensity: Normal,
                underline: None,
                blink: None,
                italic: false,
                reverse: false,
                strikethrough: false,
                invisible: false,
                wrapped: false,
                overline: false,
                semantic_type: Output,
                foreground: Default,
                background: Default,
                fat: None,
            },
        },
    ],
    len: 5,
    last_cell_width: Some(
        1,
    ),
}
"#
    );
}

#[test]
fn cluster_line_new() {
    let mut line = Line::new(1);
    line.set_cell(
        0,
        Cell::new_grapheme("h", CellAttributes::default(), None),
        1,
    );
    line.set_cell(
        1,
        Cell::new_grapheme("e", CellAttributes::default(), None),
        2,
    );
    line.set_cell(2, Cell::new_grapheme("l", bold(), None), 3);
    line.set_cell(
        3,
        Cell::new_grapheme("l", CellAttributes::default(), None),
        4,
    );
    line.set_cell(
        4,
        Cell::new_grapheme("o", CellAttributes::default(), None),
        5,
    );
    k9::snapshot!(
        line,
        r#"
Line {
    cells: C(
        ClusteredLine {
            text: "hello",
            is_double_wide: None,
            clusters: [
                Cluster {
                    cell_width: 2,
                    attrs: CellAttributes {
                        attributes: 0,
                        intensity: Normal,
                        underline: None,
                        blink: None,
                        italic: false,
                        reverse: false,
                        strikethrough: false,
                        invisible: false,
                        wrapped: false,
                        overline: false,
                        semantic_type: Output,
                        foreground: Default,
                        background: Default,
                        fat: None,
                    },
                },
                Cluster {
                    cell_width: 1,
                    attrs: CellAttributes {
                        attributes: 1,
                        intensity: Bold,
                        underline: None,
                        blink: None,
                        italic: false,
                        reverse: false,
                        strikethrough: false,
                        invisible: false,
                        wrapped: false,
                        overline: false,
                        semantic_type: Output,
                        foreground: Default,
                        background: Default,
                        fat: None,
                    },
                },
                Cluster {
                    cell_width: 2,
                    attrs: CellAttributes {
                        attributes: 0,
                        intensity: Normal,
                        underline: None,
                        blink: None,
                        italic: false,
                        reverse: false,
                        strikethrough: false,
                        invisible: false,
                        wrapped: false,
                        overline: false,
                        semantic_type: Output,
                        foreground: Default,
                        background: Default,
                        fat: None,
                    },
                },
            ],
            len: 5,
            last_cell_width: Some(
                1,
            ),
        },
    ),
    zones: [],
    seqno: 5,
    bits: LineBits(
        0x0,
    ),
    appdata: Mutex {
        data: None,
        poisoned: false,
        ..
    },
}
"#
    );
}

/// Writes `text` at `idx` both a byte at a time via set_cell_grapheme and
/// as a run via set_cells_printable_ascii, and asserts identical results,
/// including which storage representation the line ends up using.
fn assert_ascii_run_matches_per_cell(line: &Line, idx: usize, text: &str, attr: &CellAttributes) {
    let mut per_cell = line.clone();
    for (i, b) in text.bytes().enumerate() {
        let s = [b];
        let s = core::str::from_utf8(&s).unwrap();
        per_cell.set_cell_grapheme(idx + i, s, 1, attr.clone(), 2);
    }

    let mut batched = line.clone();
    batched.set_cells_printable_ascii(idx, text.as_bytes(), attr, 2);

    assert_eq!(batched.as_str(), per_cell.as_str(), "{text:?} at {idx}");
    assert_eq!(batched, per_cell, "{text:?} at {idx}");
}

fn clustered(text: &str) -> Line {
    let mut line: Line = text.into();
    line.compress_for_scrollback();
    assert!(matches!(line.cells, CellStorage::C(_)));
    line
}

#[test]
fn ascii_run_appends_to_empty_line() {
    let blank = CellAttributes::blank();
    let bold = CellAttributes::default()
        .set_intensity(wezterm_cell::Intensity::Bold)
        .clone();
    for line in [Line::new(1), clustered("")] {
        assert_ascii_run_matches_per_cell(&line, 0, "hello", &blank);
        // Blank spaces past the end are left implicit, so the line stays short
        assert_ascii_run_matches_per_cell(&line, 5, "  hi  ", &blank);
        assert_ascii_run_matches_per_cell(&line, 3, "   ", &blank);
        // ...but not if they carry attributes
        assert_ascii_run_matches_per_cell(&line, 5, "  hi  ", &bold);
    }
}

#[test]
fn ascii_run_overwrites_existing_cells() {
    let bold = CellAttributes::default()
        .set_intensity(wezterm_cell::Intensity::Bold)
        .clone();
    let vec_line: Line = "hello world".into();
    let cluster_line = clustered("hello world");
    for line in [&vec_line, &cluster_line] {
        assert_ascii_run_matches_per_cell(line, 0, "HELLO", &bold);
        assert_ascii_run_matches_per_cell(line, 3, "XY", &bold);
        // Starts inside the line and extends past its end
        assert_ascii_run_matches_per_cell(line, 8, "12345", &bold);
        // Appends exactly at the end
        assert_ascii_run_matches_per_cell(line, 11, "!!", &bold);
        // Leaves a gap after the end
        assert_ascii_run_matches_per_cell(line, 14, "far", &bold);
    }
}

#[test]
fn ascii_run_next_to_double_width_cells() {
    let attr = CellAttributes::default();
    // 'a', '中' + its spacer, 'b'
    let vec_line: Line = "a中b".into();
    let cluster_line = clustered("a中b");
    for line in [&vec_line, &cluster_line] {
        // Starting on the spacer nerfs the double-width cell before it
        assert_ascii_run_matches_per_cell(line, 2, "Z", &attr);
        assert_ascii_run_matches_per_cell(line, 2, "ZZZ", &attr);
        // Ending on the double-width cell itself
        assert_ascii_run_matches_per_cell(line, 0, "xy", &attr);
        // Covering both halves
        assert_ascii_run_matches_per_cell(line, 1, "xy", &attr);
    }
}

#[test]
fn ascii_run_hyperlinks() {
    let rules = vec![Rule::new(r"\b\w+://(?:[\w.-]+)\.[a-z]{2,15}\S*\b", "$0").unwrap()];
    let mut implicit: Line = "see http://example.com here".into();
    implicit.scan_and_create_hyperlinks(&rules);
    assert!(implicit.has_hyperlink());
    // Overwriting part of the line invalidates the implicit links
    assert_ascii_run_matches_per_cell(&implicit, 0, "SEE", &CellAttributes::default());

    let explicit = CellAttributes::default()
        .set_hyperlink(Some(Arc::new(Hyperlink::new("https://wezterm.org"))))
        .clone();
    assert_ascii_run_matches_per_cell(&Line::new(1), 0, "link", &explicit);
    assert_ascii_run_matches_per_cell(&"plain text".into(), 2, "link", &explicit);
}

#[test]
fn resize_and_clear_matches_for_both_storages() {
    let attr = CellAttributes::default()
        .set_background(wezterm_cell::color::ColorAttribute::PaletteIndex(4))
        .clone();
    for width in [0, 5, 11, 20] {
        let mut vec_line: Line = "hello wörld 中".into();
        let mut cluster_line = clustered("hello wörld 中");
        vec_line.resize_and_clear(width, 3, attr.clone());
        cluster_line.resize_and_clear(width, 3, attr.clone());
        assert_eq!(cluster_line, vec_line, "width {width}");
        assert_eq!(cluster_line.len(), width);
        assert!(cluster_line
            .visible_cells()
            .all(|c| c.str() == " " && *c.attrs() == attr));
    }
}

/// fill_range as it was implemented before it was batched
fn fill_range_per_cell(
    line: &mut Line,
    cols: core::ops::Range<usize>,
    cell: &Cell,
    seqno: SequenceNo,
) {
    if line.len() == 0 && *cell == Cell::blank() {
        return;
    }
    for x in cols {
        line.set_cell_clearing_image_placements(x, cell.clone(), seqno);
    }
    line.prune_trailing_blanks(seqno);
}

fn assert_fill_range_matches_per_cell(line: &Line, cols: core::ops::Range<usize>, cell: &Cell) {
    let mut per_cell = line.clone();
    fill_range_per_cell(&mut per_cell, cols.clone(), cell, 2);
    let mut batched = line.clone();
    batched.fill_range(cols.clone(), cell, 2);
    assert_eq!(
        batched.as_str(),
        per_cell.as_str(),
        "{cols:?} with {cell:?}"
    );
    assert_eq!(batched, per_cell, "{cols:?} with {cell:?}");
}

#[test]
fn fill_range_matches_per_cell() {
    let blank = Cell::blank();
    let colored = Cell::blank_with_attrs(
        CellAttributes::default()
            .set_background(wezterm_cell::color::ColorAttribute::PaletteIndex(4))
            .clone(),
    );
    let linked = Cell::new(
        'x',
        CellAttributes::default()
            .set_hyperlink(Some(Arc::new(Hyperlink::new("https://wezterm.org"))))
            .clone(),
    );
    let wide = Cell::new_grapheme("中", CellAttributes::default(), None);

    let mut implicit: Line = "see http://example.com here".into();
    implicit.scan_and_create_hyperlinks(&[Rule::new(r"\b\w+://\S+", "$0").unwrap()]);

    let lines = [
        Line::new(1),
        clustered(""),
        "hello world".into(),
        clustered("hello world"),
        "a中b中".into(),
        clustered("a中b中"),
        implicit,
    ];
    for line in &lines {
        for cell in [&blank, &colored, &linked, &wide] {
            for cols in [0..0, 0..3, 2..5, 3..11, 5..20, 11..15, 13..16, 2..3, 1..2] {
                assert_fill_range_matches_per_cell(line, cols, cell);
            }
        }
    }
}
