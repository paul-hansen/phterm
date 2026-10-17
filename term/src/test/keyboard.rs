//! The GUI skips repainting after forwarding a key to a local pane
//! (see `Pane::key_input_renders_via_output`), relying on the echo to
//! arrive as pty output. That's only correct if key input never changes
//! the terminal model itself.
use super::*;
use k9::assert_equal as assert_eq;

#[derive(Debug)]
struct KeyboardTestConfig;
impl TerminalConfiguration for KeyboardTestConfig {
    fn color_palette(&self) -> ColorPalette {
        ColorPalette::default()
    }

    fn enable_kitty_keyboard(&self) -> bool {
        true
    }
}

#[derive(Clone, Default)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl SharedWriter {
    /// The terminal writes via a background thread, so wait for the
    /// output to stop growing and return its length.
    fn settled_len(&self) -> usize {
        let mut len = self.0.lock().unwrap().len();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(20));
            let now = self.0.lock().unwrap().len();
            if now == len {
                return len;
            }
            len = now;
        }
    }
}

fn new_term(writer: &SharedWriter) -> Terminal {
    Terminal::new(
        TerminalSize {
            rows: 3,
            cols: 10,
            pixel_width: 80,
            pixel_height: 48,
            dpi: 0,
        },
        Arc::new(KeyboardTestConfig),
        "WezTerm",
        "O_o",
        Box::new(writer.clone()),
    )
}

/// Sends a variety of keys and asserts that they produced pty output
/// without touching the screen, cursor or sequence number.
fn assert_keys_only_write_to_pty(term: &mut Terminal, writer: &SharedWriter, mode: &str) {
    let seqno = term.current_seqno();
    let cursor = term.cursor_pos();
    let lines = term.screen().visible_lines();
    let sent_before = writer.settled_len();

    for key in [
        KeyCode::Char('a'),
        KeyCode::Char('A'),
        KeyCode::Enter,
        KeyCode::Backspace,
        KeyCode::Tab,
        KeyCode::Escape,
        KeyCode::UpArrow,
        KeyCode::LeftArrow,
        KeyCode::Function(1),
        KeyCode::PageDown,
    ] {
        for mods in [KeyModifiers::NONE, KeyModifiers::CTRL, KeyModifiers::ALT] {
            term.key_down(key, mods).unwrap();
            term.key_up(key, mods).unwrap();
        }
    }

    assert!(
        writer.settled_len() > sent_before,
        "{}: keys should have been sent to the pty",
        mode
    );
    assert_eq!(term.current_seqno(), seqno, "{}: seqno changed", mode);
    assert_eq!(term.cursor_pos(), cursor, "{}: cursor changed", mode);
    assert_eq!(
        term.screen().visible_lines(),
        lines,
        "{}: screen contents changed",
        mode
    );
}

#[test]
fn key_input_does_not_change_terminal_state() {
    let writer = SharedWriter::default();
    let mut term = new_term(&writer);
    term.advance_bytes("hello");
    assert_keys_only_write_to_pty(&mut term, &writer, "default");

    // DECCKM: application cursor keys
    term.advance_bytes("\x1b[?1h");
    assert_keys_only_write_to_pty(&mut term, &writer, "application cursor keys");

    // LNM: newline mode changes what Enter sends
    term.advance_bytes("\x1b[20h");
    assert_keys_only_write_to_pty(&mut term, &writer, "newline mode");

    // Kitty keyboard protocol: disambiguate, report key up events and
    // report all keys as escape codes, so key_up sends bytes too.
    term.advance_bytes("\x1b[>11u");
    assert_keys_only_write_to_pty(&mut term, &writer, "kitty keyboard");

    // The alternate screen keeps its own keyboard stack
    term.advance_bytes("\x1b[?1049h");
    term.advance_bytes("\x1b[>1u");
    assert_keys_only_write_to_pty(&mut term, &writer, "alt screen kitty keyboard");
}
