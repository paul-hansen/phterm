//! Unicode placeholders for the kitty graphics protocol.
//!
//! An image is given a *virtual placement* (`U=1`), which has a size in
//! cells but no position. The image is then displayed by printing the
//! `U+10EEEE` placeholder character, with the image id encoded in the
//! foreground color, the placement id in the underline color, and the
//! row, column and most significant byte of the image id encoded as
//! combining diacritics. Missing diacritics are inferred from the cell
//! to the left.
//! <https://sw.kovidgoyal.net/kitty/graphics-protocol/#unicode-placeholders>
//!
//! Since the placeholders are just text, we resolve them into image
//! slices attached to the placeholder cells, and re-resolve whenever the
//! text of a line, the virtual placements or the cell size change.
//! The slices are marked as such, so that they can be told apart from
//! images attached by regular placements in the same cells.
use crate::terminalstate::kitty::KittyImageState;
use crate::{Screen, StableRowIndex, TerminalState};
use std::collections::HashMap;
use std::sync::Arc;
use wezterm_cell::color::ColorAttribute;
use wezterm_cell::image::{ImageCell, UNICODE_PLACEHOLDER};
use wezterm_escape_parser::apc::KittyImagePlacement;
use wezterm_surface::change::ImageData;
use wezterm_surface::line::{CellRef, Line};
use wezterm_surface::{SequenceNo, TextureCoordinate};

/// The z-index used for placeholder images. As in kitty, the z-index of
/// the virtual placement is ignored, so that the cursor and text are
/// drawn over the image.
const PLACEHOLDER_Z_INDEX: i32 = -1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VirtualPlacement {
    placement_id: Option<u32>,
    columns: Option<u32>,
    rows: Option<u32>,
}

#[derive(Debug, Default)]
pub(crate) struct VirtualPlacements {
    /// The virtual placements of each image, in the order in which
    /// they were created
    by_image: HashMap<u32, Vec<VirtualPlacement>>,
    /// Set when every line needs to be resolved again, because the
    /// placements or the cell size changed
    needs_full_rescan: bool,
    /// Lines changed after this seqno need to be resolved again
    seqno: SequenceNo,
    /// The stable row index of the top of the primary and alt screens
    /// as of the last update
    top_stable_row: [StableRowIndex; 2],
}

impl VirtualPlacements {
    pub fn has_image(&self, image_id: u32) -> bool {
        self.by_image.contains_key(&image_id)
    }

    pub fn image_ids(&self) -> impl Iterator<Item = &u32> {
        self.by_image.keys()
    }

    fn insert(&mut self, image_id: u32, placement: &KittyImagePlacement) {
        // kitty treats zero values as unset
        let non_zero = |v: Option<u32>| v.filter(|&v| v != 0);
        let placement = VirtualPlacement {
            placement_id: non_zero(placement.placement_id),
            columns: non_zero(placement.columns),
            rows: non_zero(placement.rows),
        };
        let placements = self.by_image.entry(image_id).or_default();
        // Placing an existing placement id again replaces it, but keeps its
        // position in the creation order; otherwise a new placement is made
        match placements
            .iter_mut()
            .find(|p| p.placement_id.is_some() && p.placement_id == placement.placement_id)
        {
            Some(existing) => *existing = placement,
            None => placements.push(placement),
        }
        self.needs_full_rescan = true;
    }

    /// Removes the virtual placements of the image, either all of them,
    /// or only the one with the specified placement id
    pub fn remove(&mut self, image_id: u32, placement_id: Option<u32>) {
        let Some(placements) = self.by_image.get_mut(&image_id) else {
            return;
        };
        match placement_id.filter(|&p| p != 0) {
            Some(placement_id) => placements.retain(|p| p.placement_id != Some(placement_id)),
            None => placements.clear(),
        }
        if placements.is_empty() {
            self.by_image.remove(&image_id);
        }
        self.needs_full_rescan = true;
    }

    /// Finds the virtual placement selected by a placeholder: the one with
    /// the specified id, or else the first one created for the image
    fn find(&self, image_id: u32, placement_id: u32) -> Option<&VirtualPlacement> {
        let placements = self.by_image.get(&image_id)?;
        if placement_id == 0 {
            placements.first()
        } else {
            placements
                .iter()
                .find(|p| p.placement_id == Some(placement_id))
        }
    }
}

/// The diacritics used to encode row, column and image id MSB values
/// for Unicode placeholders, in value order: the diacritic at index `n`
/// encodes the value `n`.
/// Taken from kitty's `gen/rowcolumn-diacritics.txt`.
const ROW_COLUMN_DIACRITICS: [char; 297] = [
    '\u{0305}',
    '\u{030d}',
    '\u{030e}',
    '\u{0310}',
    '\u{0312}',
    '\u{033d}',
    '\u{033e}',
    '\u{033f}',
    '\u{0346}',
    '\u{034a}',
    '\u{034b}',
    '\u{034c}',
    '\u{0350}',
    '\u{0351}',
    '\u{0352}',
    '\u{0357}',
    '\u{035b}',
    '\u{0363}',
    '\u{0364}',
    '\u{0365}',
    '\u{0366}',
    '\u{0367}',
    '\u{0368}',
    '\u{0369}',
    '\u{036a}',
    '\u{036b}',
    '\u{036c}',
    '\u{036d}',
    '\u{036e}',
    '\u{036f}',
    '\u{0483}',
    '\u{0484}',
    '\u{0485}',
    '\u{0486}',
    '\u{0487}',
    '\u{0592}',
    '\u{0593}',
    '\u{0594}',
    '\u{0595}',
    '\u{0597}',
    '\u{0598}',
    '\u{0599}',
    '\u{059c}',
    '\u{059d}',
    '\u{059e}',
    '\u{059f}',
    '\u{05a0}',
    '\u{05a1}',
    '\u{05a8}',
    '\u{05a9}',
    '\u{05ab}',
    '\u{05ac}',
    '\u{05af}',
    '\u{05c4}',
    '\u{0610}',
    '\u{0611}',
    '\u{0612}',
    '\u{0613}',
    '\u{0614}',
    '\u{0615}',
    '\u{0616}',
    '\u{0617}',
    '\u{0657}',
    '\u{0658}',
    '\u{0659}',
    '\u{065a}',
    '\u{065b}',
    '\u{065d}',
    '\u{065e}',
    '\u{06d6}',
    '\u{06d7}',
    '\u{06d8}',
    '\u{06d9}',
    '\u{06da}',
    '\u{06db}',
    '\u{06dc}',
    '\u{06df}',
    '\u{06e0}',
    '\u{06e1}',
    '\u{06e2}',
    '\u{06e4}',
    '\u{06e7}',
    '\u{06e8}',
    '\u{06eb}',
    '\u{06ec}',
    '\u{0730}',
    '\u{0732}',
    '\u{0733}',
    '\u{0735}',
    '\u{0736}',
    '\u{073a}',
    '\u{073d}',
    '\u{073f}',
    '\u{0740}',
    '\u{0741}',
    '\u{0743}',
    '\u{0745}',
    '\u{0747}',
    '\u{0749}',
    '\u{074a}',
    '\u{07eb}',
    '\u{07ec}',
    '\u{07ed}',
    '\u{07ee}',
    '\u{07ef}',
    '\u{07f0}',
    '\u{07f1}',
    '\u{07f3}',
    '\u{0816}',
    '\u{0817}',
    '\u{0818}',
    '\u{0819}',
    '\u{081b}',
    '\u{081c}',
    '\u{081d}',
    '\u{081e}',
    '\u{081f}',
    '\u{0820}',
    '\u{0821}',
    '\u{0822}',
    '\u{0823}',
    '\u{0825}',
    '\u{0826}',
    '\u{0827}',
    '\u{0829}',
    '\u{082a}',
    '\u{082b}',
    '\u{082c}',
    '\u{082d}',
    '\u{0951}',
    '\u{0953}',
    '\u{0954}',
    '\u{0f82}',
    '\u{0f83}',
    '\u{0f86}',
    '\u{0f87}',
    '\u{135d}',
    '\u{135e}',
    '\u{135f}',
    '\u{17dd}',
    '\u{193a}',
    '\u{1a17}',
    '\u{1a75}',
    '\u{1a76}',
    '\u{1a77}',
    '\u{1a78}',
    '\u{1a79}',
    '\u{1a7a}',
    '\u{1a7b}',
    '\u{1a7c}',
    '\u{1b6b}',
    '\u{1b6d}',
    '\u{1b6e}',
    '\u{1b6f}',
    '\u{1b70}',
    '\u{1b71}',
    '\u{1b72}',
    '\u{1b73}',
    '\u{1cd0}',
    '\u{1cd1}',
    '\u{1cd2}',
    '\u{1cda}',
    '\u{1cdb}',
    '\u{1ce0}',
    '\u{1dc0}',
    '\u{1dc1}',
    '\u{1dc3}',
    '\u{1dc4}',
    '\u{1dc5}',
    '\u{1dc6}',
    '\u{1dc7}',
    '\u{1dc8}',
    '\u{1dc9}',
    '\u{1dcb}',
    '\u{1dcc}',
    '\u{1dd1}',
    '\u{1dd2}',
    '\u{1dd3}',
    '\u{1dd4}',
    '\u{1dd5}',
    '\u{1dd6}',
    '\u{1dd7}',
    '\u{1dd8}',
    '\u{1dd9}',
    '\u{1dda}',
    '\u{1ddb}',
    '\u{1ddc}',
    '\u{1ddd}',
    '\u{1dde}',
    '\u{1ddf}',
    '\u{1de0}',
    '\u{1de1}',
    '\u{1de2}',
    '\u{1de3}',
    '\u{1de4}',
    '\u{1de5}',
    '\u{1de6}',
    '\u{1dfe}',
    '\u{20d0}',
    '\u{20d1}',
    '\u{20d4}',
    '\u{20d5}',
    '\u{20d6}',
    '\u{20d7}',
    '\u{20db}',
    '\u{20dc}',
    '\u{20e1}',
    '\u{20e7}',
    '\u{20e9}',
    '\u{20f0}',
    '\u{2cef}',
    '\u{2cf0}',
    '\u{2cf1}',
    '\u{2de0}',
    '\u{2de1}',
    '\u{2de2}',
    '\u{2de3}',
    '\u{2de4}',
    '\u{2de5}',
    '\u{2de6}',
    '\u{2de7}',
    '\u{2de8}',
    '\u{2de9}',
    '\u{2dea}',
    '\u{2deb}',
    '\u{2dec}',
    '\u{2ded}',
    '\u{2dee}',
    '\u{2def}',
    '\u{2df0}',
    '\u{2df1}',
    '\u{2df2}',
    '\u{2df3}',
    '\u{2df4}',
    '\u{2df5}',
    '\u{2df6}',
    '\u{2df7}',
    '\u{2df8}',
    '\u{2df9}',
    '\u{2dfa}',
    '\u{2dfb}',
    '\u{2dfc}',
    '\u{2dfd}',
    '\u{2dfe}',
    '\u{2dff}',
    '\u{a66f}',
    '\u{a67c}',
    '\u{a67d}',
    '\u{a6f0}',
    '\u{a6f1}',
    '\u{a8e0}',
    '\u{a8e1}',
    '\u{a8e2}',
    '\u{a8e3}',
    '\u{a8e4}',
    '\u{a8e5}',
    '\u{a8e6}',
    '\u{a8e7}',
    '\u{a8e8}',
    '\u{a8e9}',
    '\u{a8ea}',
    '\u{a8eb}',
    '\u{a8ec}',
    '\u{a8ed}',
    '\u{a8ee}',
    '\u{a8ef}',
    '\u{a8f0}',
    '\u{a8f1}',
    '\u{aab0}',
    '\u{aab2}',
    '\u{aab3}',
    '\u{aab7}',
    '\u{aab8}',
    '\u{aabe}',
    '\u{aabf}',
    '\u{aac1}',
    '\u{fe20}',
    '\u{fe21}',
    '\u{fe22}',
    '\u{fe23}',
    '\u{fe24}',
    '\u{fe25}',
    '\u{fe26}',
    '\u{10a0f}',
    '\u{10a38}',
    '\u{1d185}',
    '\u{1d186}',
    '\u{1d187}',
    '\u{1d188}',
    '\u{1d189}',
    '\u{1d1aa}',
    '\u{1d1ab}',
    '\u{1d1ac}',
    '\u{1d1ad}',
    '\u{1d242}',
    '\u{1d243}',
    '\u{1d244}',
];

/// Returns the 1-based value encoded by a diacritic, or 0 if it is
/// not one of the row/column diacritics.
fn diacritic_to_num(c: char) -> u32 {
    match ROW_COLUMN_DIACRITICS.binary_search(&c) {
        Ok(idx) => idx as u32 + 1,
        Err(_) => 0,
    }
}

/// The lower 24 bits of an image or placement id are encoded in a color
fn color_to_id(color: ColorAttribute) -> u32 {
    match color {
        ColorAttribute::TrueColorWithPaletteFallback(c, _)
        | ColorAttribute::TrueColorWithDefaultFallback(c) => {
            let component = |v: f32| (v * 255.).round() as u32 & 0xff;
            (component(c.0) << 16) | (component(c.1) << 8) | component(c.2)
        }
        ColorAttribute::PaletteIndex(idx) => idx as u32,
        ColorAttribute::Default => 0,
    }
}

/// The information encoded in a placeholder cell.
/// row, col and msb are 1-based, with 0 meaning that it was not specified.
#[derive(Debug, Clone, Copy)]
struct PlaceholderCell {
    image_id_low: u32,
    placement_id: u32,
    row: u32,
    col: u32,
    msb: u32,
}

impl PlaceholderCell {
    fn parse(cell: &CellRef) -> Option<Self> {
        let mut chars = cell.str().chars();
        if chars.next()? != UNICODE_PLACEHOLDER {
            return None;
        }
        let mut next_num = || chars.next().map(diacritic_to_num).unwrap_or(0);
        let row = next_num();
        let col = next_num();
        let msb = next_num();
        let attrs = cell.attrs();
        Some(Self {
            image_id_low: color_to_id(attrs.foreground()),
            placement_id: color_to_id(attrs.underline_color()),
            row,
            col,
            msb,
        })
    }

    /// Returns true if this cell can continue the run of `prev`, the
    /// placeholder cell to its left: the ids must match and any values
    /// that are specified must be consistent with following `prev`.
    fn continues(&self, prev: &Self) -> bool {
        self.image_id_low == prev.image_id_low
            && self.placement_id == prev.placement_id
            && (self.row == 0 || self.row == prev.row)
            && (self.col == 0 || self.col == prev.col + 1)
            && (self.msb == 0 || self.msb == prev.msb)
    }

    fn image_id(&self) -> u32 {
        self.image_id_low | (self.msb.saturating_sub(1) << 24)
    }
}

/// Returns the fully specified placeholder cells of a line, following the
/// inference rules from the spec (and the algorithm used by kitty) to fill
/// in values from the cell to the left.
fn placeholder_cells(line: &Line) -> Vec<(usize, PlaceholderCell)> {
    let mut result: Vec<(usize, PlaceholderCell)> = vec![];
    let mut prev: Option<PlaceholderCell> = None;
    for cell in line.visible_cells() {
        let Some(mut cur) = PlaceholderCell::parse(&cell) else {
            prev = None;
            continue;
        };
        match prev {
            Some(prev) if cur.continues(&prev) => {
                cur.row = prev.row;
                cur.col = prev.col + 1;
                cur.msb = prev.msb;
            }
            _ => {
                cur.row = cur.row.max(1);
                cur.col = cur.col.max(1);
                cur.msb = cur.msb.max(1);
            }
        }
        result.push((cell.cell_index(), cur));
        prev = Some(cur);
    }
    result
}

/// A virtual placement resolved against its image data and the cell size
struct ResolvedPlacement {
    image_id: u32,
    placement_id: Option<u32>,
    data: Arc<ImageData>,
    cell_width: f64,
    cell_height: f64,
    /// The image rectangle within the placement box, in pixels:
    /// left, top, width, height
    image_rect: (f64, f64, f64, f64),
}

impl ResolvedPlacement {
    fn new(
        state: &KittyImageState,
        image_id: u32,
        placement_id: u32,
        cell_width: usize,
        cell_height: usize,
    ) -> Option<Self> {
        if image_id == 0 || cell_width == 0 || cell_height == 0 {
            return None;
        }
        let placement = state.virtual_placements.find(image_id, placement_id)?;
        let data = Arc::clone(state.id_to_data.get(&image_id)?);
        let (width, height) = data.data().dimensions().ok()?;
        if width == 0 || height == 0 {
            return None;
        }

        // If the number of columns or rows is not set, compute them so that
        // the image is as close as possible to its natural size
        let columns = placement
            .columns
            .unwrap_or_else(|| (width + cell_width as u32 - 1) / cell_width as u32)
            as u64;
        let rows = placement
            .rows
            .unwrap_or_else(|| (height + cell_height as u32 - 1) / cell_height as u32)
            as u64;

        let (cw, ch) = (cell_width as f64, cell_height as f64);
        let (box_width, box_height) = (columns as f64 * cw, rows as f64 * ch);
        let (w, h) = (width as f64, height as f64);

        // Fit the image to the box, preserving its aspect ratio, and center
        // it along the other axis
        let image_rect = if width as u64 * rows * cell_height as u64
            > height as u64 * columns * cell_width as u64
        {
            let scale = box_width / w;
            (0., (box_height - h * scale) / 2., box_width, h * scale)
        } else {
            let scale = box_height / h;
            ((box_width - w * scale) / 2., 0., w * scale, box_height)
        };

        Some(Self {
            image_id,
            placement_id: placement.placement_id,
            data,
            cell_width: cw,
            cell_height: ch,
            image_rect,
        })
    }

    /// Computes the slice of the image displayed in the cell at the given
    /// 0-based row and column of the placement, if any
    fn slice(&self, row: u32, col: u32) -> Option<ImageCell> {
        let (cw, ch) = (self.cell_width, self.cell_height);
        let (img_left, img_top, img_width, img_height) = self.image_rect;
        let cell_left = col as f64 * cw;
        let cell_top = row as f64 * ch;

        // The part of the cell covered by the image
        let left = cell_left.max(img_left);
        let right = (cell_left + cw).min(img_left + img_width);
        let top = cell_top.max(img_top);
        let bottom = (cell_top + ch).min(img_top + img_height);

        // and its position within the cell, in whole pixels
        let pad_left = (left - cell_left).round();
        let pad_top = (top - cell_top).round();
        let width = (right - cell_left).round() - pad_left;
        let height = (bottom - cell_top).round() - pad_top;
        if width <= 0. || height <= 0. {
            return None;
        }

        let tex = |x: f64, y: f64| {
            TextureCoordinate::new_f32(
                ((x - img_left) / img_width) as f32,
                ((y - img_top) / img_height) as f32,
            )
        };

        Some(
            ImageCell::with_z_index(
                tex(left, top),
                tex(right, bottom),
                Arc::clone(&self.data),
                PLACEHOLDER_Z_INDEX,
                pad_left as u16,
                pad_top as u16,
                (cw - width) as u16,
                (ch - height) as u16,
                Some(self.image_id),
                self.placement_id,
            )
            .with_unicode_placeholder(),
        )
    }
}

/// Resolves the placeholder cells of lines into image slices
struct Resolver<'a> {
    state: &'a KittyImageState,
    cell_width: usize,
    cell_height: usize,
    placements: HashMap<(u32, u32), Option<ResolvedPlacement>>,
}

impl<'a> Resolver<'a> {
    fn resolve_line(&mut self, line: &mut Line, seqno: SequenceNo) {
        // Only placeholder cells can hold placeholder images, so lines
        // without placeholders never need updating
        let cells = placeholder_cells(line);
        if cells.is_empty() {
            return;
        }

        let mut changes = vec![];
        {
            let current: HashMap<usize, ImageCell> = line
                .visible_cells()
                .filter_map(|cell| {
                    let image = cell.attrs().unicode_placeholder_image()?.clone();
                    Some((cell.cell_index(), image))
                })
                .collect();
            for (idx, cell) in cells {
                let image_id = cell.image_id();
                let (state, cell_width, cell_height) =
                    (self.state, self.cell_width, self.cell_height);
                let slice = self
                    .placements
                    .entry((image_id, cell.placement_id))
                    .or_insert_with(|| {
                        ResolvedPlacement::new(
                            state,
                            image_id,
                            cell.placement_id,
                            cell_width,
                            cell_height,
                        )
                    })
                    .as_ref()
                    .and_then(|placement| placement.slice(cell.row - 1, cell.col - 1));
                if current.get(&idx) != slice.as_ref() {
                    changes.push((idx, slice));
                }
            }
        }

        if changes.is_empty() {
            return;
        }
        let cells = line.cells_mut_for_attr_changes_only();
        for (idx, slice) in changes {
            if let Some(cell) = cells.get_mut(idx) {
                cell.attrs_mut()
                    .set_unicode_placeholder_image(slice.map(Box::new));
            }
        }
        line.update_last_change_seqno(seqno);
    }
}

impl TerminalState {
    pub(crate) fn kitty_add_virtual_placement(
        &mut self,
        image_id: u32,
        placement: &KittyImagePlacement,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.kitty_img.id_to_data.contains_key(&image_id),
            "no matching image id {} in id_to_data for virtual placement",
            image_id
        );
        self.kitty_img
            .virtual_placements
            .insert(image_id, placement);
        Ok(())
    }

    /// Request that all lines be resolved again on the next update
    pub(crate) fn kitty_placeholders_invalidate(&mut self) {
        self.kitty_img.virtual_placements.needs_full_rescan = true;
    }

    /// Resolves the placeholders of the lines that changed since the
    /// last update, or of all lines if the placements or cell size changed.
    pub(crate) fn kitty_update_placeholders(&mut self) {
        let seqno = self.seqno;
        let placements = &mut self.kitty_img.virtual_placements;
        let since = std::mem::replace(&mut placements.seqno, seqno);
        let full_rescan = std::mem::take(&mut placements.needs_full_rescan);
        let mut top_stable_row = placements.top_stable_row;

        // If there are no virtual placements, then there can be no
        // placeholder images left after the last full rescan
        if full_rescan || !placements.by_image.is_empty() {
            let screen = &self.screen.screen;
            let mut resolver = Resolver {
                state: &self.kitty_img,
                cell_width: self.pixel_width / screen.physical_cols.max(1),
                cell_height: self.pixel_height / screen.physical_rows.max(1),
                placements: HashMap::new(),
            };

            let screens = [&mut self.screen.screen, &mut self.screen.alt_screen];
            for (screen, last_top) in IntoIterator::into_iter(screens).zip(top_stable_row.iter()) {
                let phys_range = if full_rescan {
                    0..screen.scrollback_rows()
                } else {
                    changed_line_range(screen, *last_top)
                };
                for idx in phys_range {
                    let line = screen.line_mut(idx);
                    if full_rescan || line.changed_since(since) {
                        resolver.resolve_line(line, seqno);
                    }
                }
            }
        }

        for (top, screen) in top_stable_row
            .iter_mut()
            .zip([&self.screen.screen, &self.screen.alt_screen])
        {
            *top = screen.visible_row_to_stable_row(0);
        }
        self.kitty_img.virtual_placements.top_stable_row = top_stable_row;
    }
}

/// Returns the range of lines that may have changed since the top of the
/// screen was at `last_top`: the lines that are visible now, and those
/// that have scrolled into the scrollback since then.
fn changed_line_range(screen: &Screen, last_top: StableRowIndex) -> std::ops::Range<usize> {
    let top = screen.visible_row_to_stable_row(0);
    let start = last_top.min(top);
    screen.stable_range(&(start..top + screen.physical_rows as StableRowIndex))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn diacritics_table() {
        assert_eq!(ROW_COLUMN_DIACRITICS.len(), 297);
        // Must be sorted for the binary search in diacritic_to_num
        assert!(ROW_COLUMN_DIACRITICS.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(diacritic_to_num('\u{305}'), 1);
        assert_eq!(diacritic_to_num('\u{30d}'), 2);
        assert_eq!(diacritic_to_num('\u{59c}'), 43);
        assert_eq!(diacritic_to_num('\u{1d244}'), 297);
        // Excluded from the table because it has a decomposition mapping
        assert_eq!(diacritic_to_num('\u{300}'), 0);
        assert_eq!(diacritic_to_num('a'), 0);
    }

    #[test]
    fn colors_to_ids() {
        use wezterm_cell::color::SrgbaTuple;
        assert_eq!(color_to_id(ColorAttribute::Default), 0);
        assert_eq!(color_to_id(ColorAttribute::PaletteIndex(42)), 42);
        for (r, g, b) in [(0u8, 0u8, 42u8), (42, 43, 44), (255, 255, 255), (1, 2, 3)] {
            let c = SrgbaTuple(r as f32 / 255., g as f32 / 255., b as f32 / 255., 1.);
            let id = ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
            assert_eq!(
                color_to_id(ColorAttribute::TrueColorWithDefaultFallback(c)),
                id
            );
            assert_eq!(
                color_to_id(ColorAttribute::TrueColorWithPaletteFallback(c, 1)),
                id
            );
        }
    }
}
