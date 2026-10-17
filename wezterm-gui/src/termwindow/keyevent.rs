use crate::termwindow::InputMap;
use ::window::{
    DeadKeyStatus, KeyCode, KeyEvent, KeyboardLedStatus, Modifiers, RawKeyEvent, WindowOps,
};
use anyhow::Context;
use config::keyassignment::{KeyAssignment, KeyTableEntry};
use mux::pane::{Pane, PerformAssignmentResult};
use smol::Timer;
use std::sync::Arc;
use std::time::{Duration, Instant};
use termwiz::input::KeyboardEncoding;

#[derive(Debug, Clone)]
pub struct KeyTableStateEntry {
    name: String,
    /// If this activation expires, when it should expire
    expiration: Option<Instant>,
    /// Whether this activation pops itself after recognizing a key press
    one_shot: bool,
    until_unknown: bool,
    prevent_fallback: bool,
    /// The timeout duration; used when updating the expiration
    timeout_milliseconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct KeyTableArgs<'a> {
    pub name: &'a str,
    pub timeout_milliseconds: Option<u64>,
    pub replace_current: bool,
    pub one_shot: bool,
    pub until_unknown: bool,
    pub prevent_fallback: bool,
}

#[derive(Debug, Default, Clone)]
pub struct KeyTableState {
    stack: Vec<KeyTableStateEntry>,
}

impl KeyTableState {
    pub fn activate(&mut self, args: KeyTableArgs) {
        if args.replace_current {
            self.pop();
        }
        self.stack.push(KeyTableStateEntry {
            name: args.name.to_string(),
            expiration: args
                .timeout_milliseconds
                .map(|ms| Instant::now() + Duration::from_millis(ms)),
            one_shot: args.one_shot,
            until_unknown: args.until_unknown,
            prevent_fallback: args.prevent_fallback,
            timeout_milliseconds: args.timeout_milliseconds,
        });
    }

    pub fn pop(&mut self) {
        self.stack.pop();
    }

    pub fn clear_stack(&mut self) {
        self.stack.clear();
    }

    pub fn process_expiration(&mut self) -> bool {
        let should_pop = self
            .stack
            .last()
            .map(|entry| match entry.expiration {
                Some(deadline) => Instant::now() >= deadline,
                None => false,
            })
            .unwrap_or(false);
        if !should_pop {
            return false;
        }
        self.pop();
        true
    }

    /// Returns true if any entries were popped
    pub fn pop_until_unknown(&mut self) -> bool {
        let mut popped = false;
        while self
            .stack
            .last()
            .map(|entry| entry.until_unknown)
            .unwrap_or(false)
        {
            self.pop();
            popped = true;
        }
        popped
    }

    pub fn current_table(&mut self) -> Option<&str> {
        while self.process_expiration() {}
        self.stack.last().map(|entry| entry.name.as_str())
    }

    fn lookup_key(
        &mut self,
        input_map: &InputMap,
        key: &KeyCode,
        mods: Modifiers,
        only_key_bindings: OnlyKeyBindings,
    ) -> Option<(KeyTableEntry, Option<String>)> {
        while self.process_expiration() {}

        let mut pop_count = 0;
        let mut result = None;

        for stack_entry in self.stack.iter_mut().rev() {
            let name = stack_entry.name.as_str();
            if let Some(entry) = input_map.lookup_key(key, mods, Some(name)) {
                if let Some(timeout) = stack_entry.timeout_milliseconds {
                    stack_entry
                        .expiration
                        .replace(Instant::now() + Duration::from_millis(timeout));
                }
                result = Some((entry, Some(name.to_string())));
                break;
            }

            if stack_entry.until_unknown {
                pop_count += 1;
            }

            if stack_entry.prevent_fallback {
                // If we've passed the key-bindings-only phase, then we want
                // to prevent the default action of passing the key through.
                // Prior to that, we mustn't prevent subsequent phases.
                if only_key_bindings == OnlyKeyBindings::No {
                    result = Some((
                        KeyTableEntry {
                            action: KeyAssignment::Nop,
                        },
                        Some(name.to_string()),
                    ));
                }

                // Whether we explicitly map Nop or not, prevent looking
                // in later key tables on the stack.
                break;
            }
        }

        // This is a little bit tricky: until_unknown needs to
        // pop entries if we didn't match, but since we need to
        // make three separate passes to resolve a key using its
        // various physical, mapped and raw forms, we cannot
        // unilaterally pop here without breaking a later pass.
        // It is only safe to pop here if we did match something:
        // in that case we know that we won't make additional
        // passes.
        // It is important that `pop_until_unknown` is called
        // in the final "no keys matched" case to correctly
        // manage that state transition.
        if result.is_some() {
            for _ in 0..pop_count {
                self.pop();
            }
        }

        result
    }

    /// Returns true if a one-shot entry was popped
    pub fn did_process_key(&mut self) -> bool {
        let should_pop = self
            .stack
            .last()
            .map(|entry| entry.one_shot)
            .unwrap_or(false);
        if should_pop {
            self.pop();
        }
        should_pop
    }
}

#[derive(Debug)]
pub enum Key {
    Code(::termwiz::input::KeyCode),
    Composed(String),
    None,
}

/// Whether a key that was just forwarded to a pane needs an immediate
/// repaint. Repainting before the pane has responded renders an unchanged
/// frame, and the echo then has to wait for the next frame slot (the next
/// compositor frame callback on Wayland, 1/max_fps on X11) to be shown.
/// Panes whose response arrives as output get repainted by the resulting
/// PaneOutput notification, so they only need one here when GUI-side
/// state such as the viewport or the key table stack changed.
fn key_input_needs_repaint(pane_renders_via_output: bool, gui_state_changed: bool) -> bool {
    gui_state_changed || !pane_renders_via_output
}

/// Stores a newly advised dead key/composition status. Returns true if it
/// changed, and so the composing text shown in the window needs a repaint.
pub fn update_dead_key_status(current: &mut DeadKeyStatus, status: DeadKeyStatus) -> bool {
    if *current == status {
        return false;
    }
    *current = status;
    true
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum OnlyKeyBindings {
    Yes,
    No,
}

impl super::TermWindow {
    fn encode_win32_input(&self, pane: &Arc<dyn Pane>, key: &KeyEvent) -> Option<String> {
        if !self.config.allow_win32_input_mode
            || pane.get_keyboard_encoding() != KeyboardEncoding::Win32
        {
            return None;
        }
        key.encode_win32_input_mode()
    }

    fn encode_kitty_input(&self, pane: &Arc<dyn Pane>, key: &KeyEvent) -> Option<String> {
        if !self.config.enable_kitty_keyboard {
            return None;
        }
        if let KeyboardEncoding::Kitty(flags) = pane.get_keyboard_encoding() {
            Some(key.encode_kitty(flags))
        } else {
            None
        }
    }

    fn lookup_key(
        &mut self,
        pane: &Arc<dyn Pane>,
        keycode: &KeyCode,
        mods: Modifiers,
        only_key_bindings: OnlyKeyBindings,
    ) -> Option<(KeyTableEntry, Option<String>)> {
        if let Some(overlay) = self.pane_state(pane.pane_id()).overlay.as_mut() {
            if let Some((entry, table_name)) = overlay.key_table_state.lookup_key(
                &self.input_map,
                keycode,
                mods,
                only_key_bindings,
            ) {
                return Some((entry, table_name.map(|s| s.to_string())));
            }
        }
        if let Some((entry, table_name)) =
            self.key_table_state
                .lookup_key(&self.input_map, keycode, mods, only_key_bindings)
        {
            return Some((entry, table_name.map(|s| s.to_string())));
        }
        self.input_map
            .lookup_key(keycode, mods, None)
            .map(|entry| (entry, None))
    }

    fn process_key(
        &mut self,
        pane: &Arc<dyn Pane>,
        context: &dyn WindowOps,
        keycode: &KeyCode,
        raw_modifiers: Modifiers,
        leader_active: bool,
        leader_mod: Modifiers,
        only_key_bindings: OnlyKeyBindings,
        is_down: bool,
        key_event: Option<&KeyEvent>,
    ) -> bool {
        if is_down && !leader_active {
            // Check to see if this key-press is the leader activating
            if let Some(duration) = self.input_map.is_leader(&keycode, raw_modifiers) {
                // Yes; record its expiration
                let target = std::time::Instant::now() + duration;
                self.leader_is_down.replace(target);
                self.update_title();
                // schedule an invalidation so that the cursor or status
                // area will be repainted at the right time
                if let Some(window) = self.window.clone() {
                    promise::spawn::spawn(async move {
                        Timer::at(target).await;
                        window.invalidate();
                    })
                    .detach();
                }
                return true;
            }
        }

        if is_down {
            if only_key_bindings == OnlyKeyBindings::No {
                if let Some(modal) = self.get_modal() {
                    if let Key::Code(term_key) = self.win_key_code_to_termwiz_key_code(keycode) {
                        match modal.key_down(term_key, raw_modifiers.remove_positional_mods(), self)
                        {
                            Ok(true) => return true,
                            Ok(false) => {}
                            Err(err) => {
                                log::error!("Error dispatching key to modal: {err:#}");
                                return true;
                            }
                        }
                    }
                }
            }

            if let Some((entry, table_name)) = self.lookup_key(
                pane,
                &keycode,
                raw_modifiers | leader_mod,
                only_key_bindings,
            ) {
                if self.config.debug_key_events {
                    log::info!(
                        "{}{:?} {:?} -> perform {:?}",
                        match table_name {
                            Some(name) => format!("table:{} ", name),
                            None => String::new(),
                        },
                        keycode,
                        raw_modifiers | leader_mod,
                        entry.action,
                    );
                }

                self.key_table_state.did_process_key();
                let handled = match self.perform_key_assignment(&pane, &entry.action) {
                    Ok(PerformAssignmentResult::Handled) => true,
                    Err(_) => true,
                    Ok(_) => false,
                };

                if handled {
                    context.invalidate();

                    if leader_active {
                        // A successful leader key-lookup cancels the leader
                        // virtual modifier state
                        self.leader_done();
                    }

                    return true;
                }
            }
        }

        // While the leader modifier is active, only registered
        // keybindings are recognized.
        let only_key_bindings = match (only_key_bindings, leader_active) {
            (OnlyKeyBindings::Yes, _) => OnlyKeyBindings::Yes,
            (_, true) => OnlyKeyBindings::Yes,
            _ => OnlyKeyBindings::No,
        };

        if only_key_bindings == OnlyKeyBindings::No {
            let config = &self.config;

            // This is a bit ugly.
            // Not all of our platforms report LEFT|RIGHT ALT; most report just ALT.
            // For those that do distinguish between them we want to respect the left vs.
            // right settings for the compose behavior.
            // Otherwise, if the event didn't include left vs. right then we want to
            // respect the generic compose behavior.
            let bypass_compose =
                    // Left ALT and they disabled compose
                    (raw_modifiers.contains(Modifiers::LEFT_ALT)
                    && !config.send_composed_key_when_left_alt_is_pressed)
                    // Right ALT and they disabled compose
                    || (raw_modifiers.contains(Modifiers::RIGHT_ALT)
                        && !config.send_composed_key_when_right_alt_is_pressed)
                    // Generic ALT and they disabled generic compose
                    || (!raw_modifiers.contains(Modifiers::RIGHT_ALT)
                        && !raw_modifiers.contains(Modifiers::LEFT_ALT)
                        && raw_modifiers.contains(Modifiers::ALT)
                        && !(config.send_composed_key_when_left_alt_is_pressed
                             || config.send_composed_key_when_right_alt_is_pressed));

            if bypass_compose {
                if let Key::Code(term_key) = self.win_key_code_to_termwiz_key_code(keycode) {
                    let tw_raw_modifiers = raw_modifiers;

                    let mut did_encode = false;
                    if let Some(key_event) = key_event {
                        if let Some(encoded) = self.encode_win32_input(&pane, &key_event) {
                            if self.config.debug_key_events {
                                log::info!("win32: Encoded input as {:?}", encoded);
                            }
                            pane.writer()
                                .write_all(encoded.as_bytes())
                                .context("sending win32-input-mode encoded data")
                                .ok();
                            did_encode = true;
                        } else if let Some(encoded) = self.encode_kitty_input(&pane, &key_event) {
                            if self.config.debug_key_events {
                                log::info!("kitty: Encoded input as {:?}", encoded);
                            }
                            pane.writer()
                                .write_all(encoded.as_bytes())
                                .context("sending kitty encoded data")
                                .ok();
                            did_encode = true;
                        }
                    };
                    if !did_encode {
                        if self.config.debug_key_events {
                            log::info!(
                                "{:?} {:?} -> send to pane {:?} {:?}",
                                keycode,
                                raw_modifiers,
                                term_key,
                                tw_raw_modifiers
                            );
                        }

                        did_encode = if is_down {
                            pane.key_down(term_key, tw_raw_modifiers)
                        } else {
                            pane.key_up(term_key, tw_raw_modifiers)
                        }
                        .is_ok();
                    };

                    if did_encode {
                        if is_down
                            && !keycode.is_modifier()
                            && self.pane_state(pane.pane_id()).overlay.is_none()
                        {
                            self.maybe_scroll_to_bottom_for_input(&pane);
                        }
                        if is_down
                            && self.config.hide_mouse_cursor_when_typing
                            && !keycode.is_modifier()
                        {
                            context.set_cursor(None);
                        }
                        if !keycode.is_modifier() {
                            context.invalidate();
                        }

                        return true;
                    }
                }
            }
        }

        false
    }

    pub fn raw_key_event_impl(&mut self, key: RawKeyEvent, context: &dyn WindowOps) {
        // The leader key is a kind of modal modifier key.
        // It is allowed to be active for up to the leader timeout duration,
        // after which it auto-deactivates.
        let (leader_active, leader_mod) = if self.leader_is_active_mut() {
            // Currently active
            (true, Modifiers::LEADER)
        } else {
            (false, Modifiers::NONE)
        };

        if self.config.debug_key_events {
            log::info!(
                "key_event {:?} {}",
                key,
                if leader_active { "LEADER" } else { "" }
            );
        } else {
            log::trace!(
                "key_event {:?} {}",
                key,
                if leader_active { "LEADER" } else { "" }
            );
        }

        let modifier_and_leds = (key.modifiers, key.leds);
        if self.current_modifier_and_leds != modifier_and_leds {
            self.current_modifier_and_leds = modifier_and_leds;
            self.schedule_next_status_update();
        }

        let pane = match self.get_active_pane_or_overlay() {
            Some(pane) => pane,
            None => return,
        };

        // First, try to match raw physical key
        let phys_key = match &key.key {
            phys @ KeyCode::Physical(_) => Some(phys.clone()),
            _ => key.phys_code.map(KeyCode::Physical),
        };

        if let Some(phys_key) = &phys_key {
            if self.process_key(
                &pane,
                context,
                &phys_key,
                key.modifiers,
                leader_active,
                leader_mod,
                OnlyKeyBindings::Yes,
                key.key_is_down,
                None,
            ) {
                key.set_handled();
                return;
            }
        }

        // Then try the raw code
        let raw_key = match &key.key {
            raw @ KeyCode::RawCode(_) => raw.clone(),
            _ => KeyCode::RawCode(key.raw_code),
        };
        if self.process_key(
            &pane,
            context,
            &raw_key,
            key.modifiers,
            leader_active,
            leader_mod,
            OnlyKeyBindings::Yes,
            key.key_is_down,
            None,
        ) {
            key.set_handled();
            return;
        }

        if phys_key.as_ref() == Some(&key.key) || raw_key == key.key {
            // We already matched against whatever key.key is, so no need
            // to do it again below
            return;
        }

        if self.process_key(
            &pane,
            context,
            &key.key,
            key.modifiers,
            leader_active,
            leader_mod,
            OnlyKeyBindings::Yes,
            key.key_is_down,
            None,
        ) {
            key.set_handled();
        }
    }

    pub fn current_modifier_and_led_state(&self) -> (Modifiers, KeyboardLedStatus) {
        self.current_modifier_and_leds
    }

    pub fn leader_is_active(&self) -> bool {
        match self.leader_is_down.as_ref() {
            Some(expiry) if *expiry > std::time::Instant::now() => {
                self.update_next_frame_time(Some(*expiry));
                true
            }
            Some(_) => false,
            None => false,
        }
    }

    pub fn leader_is_active_mut(&mut self) -> bool {
        match self.leader_is_down.as_ref() {
            Some(expiry) if *expiry > std::time::Instant::now() => {
                self.update_next_frame_time(Some(*expiry));
                true
            }
            Some(_) => {
                self.leader_done();
                false
            }
            None => false,
        }
    }

    pub fn current_key_table_name(&mut self) -> Option<String> {
        let mut name = None;

        if let Some(pane) = self.get_active_pane_or_overlay() {
            if let Some(overlay) = self.pane_state(pane.pane_id()).overlay.as_mut() {
                name = overlay
                    .key_table_state
                    .current_table()
                    .map(|s| s.to_string());

                if let Some(entry) = overlay.key_table_state.stack.last() {
                    if let Some(expiry) = entry.expiration {
                        self.update_next_frame_time(Some(expiry));
                    }
                }
            }
        }
        if name.is_none() {
            name = self.key_table_state.current_table().map(|s| s.to_string());
        }
        if let Some(entry) = self.key_table_state.stack.last() {
            if let Some(expiry) = entry.expiration {
                self.update_next_frame_time(Some(expiry));
            }
        }
        name
    }

    pub fn composition_status(&self) -> &DeadKeyStatus {
        &self.dead_key_status
    }

    fn leader_done(&mut self) {
        self.leader_is_down.take();
        self.update_title();
        if let Some(window) = &self.window {
            window.invalidate();
        }
    }

    pub fn key_event_impl(&mut self, window_key: KeyEvent, context: &dyn WindowOps) {
        let pane = match self.get_active_pane_or_overlay() {
            Some(pane) => pane,
            None => return,
        };

        // The leader key is a kind of modal modifier key.
        // It is allowed to be active for up to the leader timeout duration,
        // after which it auto-deactivates.
        let (leader_active, leader_mod) = if self.leader_is_active_mut() {
            // Currently active
            (true, Modifiers::LEADER)
        } else {
            (false, Modifiers::NONE)
        };

        if self.config.debug_key_events {
            log::info!(
                "key_event {:?} {}",
                window_key,
                if leader_active { "LEADER" } else { "" }
            );
        } else {
            log::trace!(
                "key_event {:?} {}",
                window_key,
                if leader_active { "LEADER" } else { "" }
            );
        }

        let modifiers = window_key.modifiers;

        if self.process_key(
            &pane,
            context,
            &window_key.key,
            window_key.modifiers,
            leader_active,
            leader_mod,
            OnlyKeyBindings::No,
            window_key.key_is_down,
            Some(&window_key),
        ) {
            return;
        }

        // If we get here, then none of the keys matched
        // any key table rules. Therefore, we should pop all `until_unknown`
        // entries from the stack.
        // Tracks GUI-side state changed by this key, which, unlike the
        // pane's response, won't trigger a repaint on its own.
        let mut gui_state_changed = false;
        if window_key.key_is_down {
            gui_state_changed |= self.key_table_state.pop_until_unknown();
        }

        let key = self.win_key_code_to_termwiz_key_code(&window_key.key);

        match key {
            Key::Code(key) => {
                if window_key.key_is_down && !key.is_modifier() {
                    if leader_active {
                        // Leader was pressed and this non-modifier keypress isn't
                        // a registered key binding; swallow this event and cancel
                        // the leader modifier.
                        self.leader_done();
                        return;
                    }
                    gui_state_changed |= self.key_table_state.did_process_key();
                }

                if let Some(modal) = self.get_modal() {
                    if window_key.key_is_down {
                        modal.key_down(key, modifiers, self).ok();
                    }
                    return;
                }

                let res = if let Some(encoded) = self.encode_win32_input(&pane, &window_key) {
                    if self.config.debug_key_events {
                        log::info!("win32: Encoded input as {:?}", encoded);
                    }
                    pane.writer()
                        .write_all(encoded.as_bytes())
                        .context("sending win32-input-mode encoded data")
                } else if let Some(encoded) = self.encode_kitty_input(&pane, &window_key) {
                    if self.config.debug_key_events {
                        log::info!("kitty: Encoded input as {:?}", encoded);
                    }
                    pane.writer()
                        .write_all(encoded.as_bytes())
                        .context("sending kitty encoded data")
                } else {
                    if self.config.debug_key_events {
                        log::info!(
                            "send to pane {} key={:?} mods={:?}",
                            if window_key.key_is_down { "DOWN" } else { "UP" },
                            key,
                            modifiers
                        );
                    }

                    if window_key.key_is_down {
                        pane.key_down(key, modifiers)
                    } else {
                        pane.key_up(key, modifiers)
                    }
                };

                if res.is_ok() {
                    if window_key.key_is_down
                        && !key.is_modifier()
                        && self.pane_state(pane.pane_id()).overlay.is_none()
                    {
                        gui_state_changed |= self.maybe_scroll_to_bottom_for_input(&pane);
                    }
                    if window_key.key_is_down
                        && self.config.hide_mouse_cursor_when_typing
                        && !key.is_modifier()
                    {
                        context.set_cursor(None);
                    }
                    if !key.is_modifier()
                        && key_input_needs_repaint(
                            pane.key_input_renders_via_output(),
                            gui_state_changed,
                        )
                    {
                        context.invalidate();
                    }
                }
            }
            Key::Composed(s) => {
                if !window_key.key_is_down {
                    return;
                }
                if leader_active {
                    // Leader was pressed and this non-modifier keypress isn't
                    // a registered key binding; swallow this event and cancel
                    // the leader modifier.
                    self.leader_done();
                    return;
                }
                gui_state_changed |= self.key_table_state.did_process_key();
                if self.config.debug_key_events {
                    log::info!("send to pane string={:?}", s);
                }
                if let Err(err) = pane.send_composed_text(&s) {
                    log::error!("failed to send composed text to pane: {err:#}");
                }
                gui_state_changed |= self.maybe_scroll_to_bottom_for_input(&pane);
                if key_input_needs_repaint(pane.key_input_renders_via_output(), gui_state_changed) {
                    context.invalidate();
                }
            }
            Key::None => {}
        }
    }

    pub fn win_key_code_to_termwiz_key_code(&self, key: &::window::KeyCode) -> Key {
        use ::termwiz::input::KeyCode as KC;
        use ::window::KeyCode as WK;

        let code = match key {
            // TODO: consider eliminating these codes from termwiz::input::KeyCode
            WK::Char('\r') => KC::Enter,
            WK::Char('\t') => KC::Tab,
            WK::Char('\u{08}') => {
                if self.config.swap_backspace_and_delete {
                    KC::Delete
                } else {
                    KC::Backspace
                }
            }
            WK::Char('\u{7f}') => {
                if self.config.swap_backspace_and_delete {
                    KC::Backspace
                } else {
                    KC::Delete
                }
            }
            WK::Char('\u{1b}') => KC::Escape,
            WK::RawCode(_) => return Key::None,
            WK::Physical(phys) => {
                return self.win_key_code_to_termwiz_key_code(&phys.to_key_code())
            }

            WK::Char(c) => KC::Char(*c),
            WK::Composed(ref s) => {
                let mut chars = s.chars();
                if let Some(first_char) = chars.next() {
                    if chars.next().is_none() {
                        // Was just a single char after all
                        return self.win_key_code_to_termwiz_key_code(&WK::Char(first_char));
                    }
                }
                return Key::Composed(s.to_owned());
            }
            WK::Function(f) => KC::Function(*f),
            WK::LeftArrow => KC::LeftArrow,
            WK::RightArrow => KC::RightArrow,
            WK::UpArrow => KC::UpArrow,
            WK::DownArrow => KC::DownArrow,
            WK::Home => KC::Home,
            WK::End => KC::End,
            WK::PageUp => KC::PageUp,
            WK::PageDown => KC::PageDown,
            WK::Insert => KC::Insert,
            WK::Hyper => KC::Hyper,
            WK::Super => KC::Super,
            WK::Meta => KC::Meta,
            WK::Cancel => KC::Cancel,
            WK::Clear => KC::Clear,
            WK::Shift => KC::Shift,
            WK::LeftShift => KC::LeftShift,
            WK::RightShift => KC::RightShift,
            WK::Control => KC::Control,
            WK::LeftControl => KC::LeftControl,
            WK::RightControl => KC::RightControl,
            WK::Alt => KC::Alt,
            WK::LeftAlt => KC::LeftAlt,
            WK::RightAlt => KC::RightAlt,
            WK::Pause => KC::Pause,
            WK::CapsLock => KC::CapsLock,
            WK::VoidSymbol => return Key::None,
            WK::Select => KC::Select,
            WK::Print => KC::Print,
            WK::Execute => KC::Execute,
            WK::PrintScreen => KC::PrintScreen,
            WK::Help => KC::Help,
            WK::LeftWindows => KC::LeftWindows,
            WK::RightWindows => KC::RightWindows,
            WK::Sleep => KC::Sleep,
            WK::Multiply => KC::Multiply,
            WK::Applications => KC::Applications,
            WK::Add => KC::Add,
            WK::Numpad(0) => KC::Numpad0,
            WK::Numpad(1) => KC::Numpad1,
            WK::Numpad(2) => KC::Numpad2,
            WK::Numpad(3) => KC::Numpad3,
            WK::Numpad(4) => KC::Numpad4,
            WK::Numpad(5) => KC::Numpad5,
            WK::Numpad(6) => KC::Numpad6,
            WK::Numpad(7) => KC::Numpad7,
            WK::Numpad(8) => KC::Numpad8,
            WK::Numpad(9) => KC::Numpad9,
            WK::Numpad(_) => return Key::None,
            WK::Separator => KC::Separator,
            WK::Subtract => KC::Subtract,
            WK::Decimal => KC::Decimal,
            WK::Divide => KC::Divide,
            WK::NumLock => KC::NumLock,
            WK::ScrollLock => KC::ScrollLock,
            WK::Copy => KC::Copy,
            WK::Cut => KC::Cut,
            WK::Paste => KC::Paste,
            WK::BrowserBack => KC::BrowserBack,
            WK::BrowserForward => KC::BrowserForward,
            WK::BrowserRefresh => KC::BrowserRefresh,
            WK::BrowserStop => KC::BrowserStop,
            WK::BrowserSearch => KC::BrowserSearch,
            WK::BrowserFavorites => KC::BrowserFavorites,
            WK::BrowserHome => KC::BrowserHome,
            WK::VolumeMute => KC::VolumeMute,
            WK::VolumeDown => KC::VolumeDown,
            WK::VolumeUp => KC::VolumeUp,
            WK::MediaNextTrack => KC::MediaNextTrack,
            WK::MediaPrevTrack => KC::MediaPrevTrack,
            WK::MediaStop => KC::MediaStop,
            WK::MediaPlayPause => KC::MediaPlayPause,
            WK::ApplicationLeftArrow => KC::ApplicationLeftArrow,
            WK::ApplicationRightArrow => KC::ApplicationRightArrow,
            WK::ApplicationUpArrow => KC::ApplicationUpArrow,
            WK::ApplicationDownArrow => KC::ApplicationDownArrow,
            WK::KeyPadHome => KC::KeyPadHome,
            WK::KeyPadEnd => KC::KeyPadEnd,
            WK::KeyPadBegin => KC::KeyPadBegin,
            WK::KeyPadPageUp => KC::KeyPadPageUp,
            WK::KeyPadPageDown => KC::KeyPadPageDown,
        };
        Key::Code(code)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::termwindow::scroll_to_bottom_for_input;

    fn table(one_shot: bool, until_unknown: bool) -> KeyTableArgs<'static> {
        KeyTableArgs {
            name: "test",
            timeout_milliseconds: None,
            replace_current: false,
            one_shot,
            until_unknown,
            prevent_fallback: false,
        }
    }

    #[test]
    fn local_pane_key_input_waits_for_output() {
        // The echo's PaneOutput notification repaints; repainting here
        // would only delay it.
        assert!(!key_input_needs_repaint(true, false));
    }

    #[test]
    fn local_pane_key_input_repaints_for_gui_changes() {
        // eg: scrolled back to the bottom, or a key table popped
        assert!(key_input_needs_repaint(true, true));
    }

    #[test]
    fn synchronously_rendering_pane_always_repaints() {
        // eg: copy mode search editing or the client pane's predictive echo
        assert!(key_input_needs_repaint(false, false));
        assert!(key_input_needs_repaint(false, true));
    }

    #[test]
    fn unchanged_dead_key_status_does_not_repaint() {
        // Wayland advises None after every text-input `done`
        let mut status = DeadKeyStatus::None;
        assert!(!update_dead_key_status(&mut status, DeadKeyStatus::None));

        let mut status = DeadKeyStatus::Composing("e".to_string());
        assert!(!update_dead_key_status(
            &mut status,
            DeadKeyStatus::Composing("e".to_string())
        ));
    }

    #[test]
    fn changed_dead_key_status_repaints() {
        let mut status = DeadKeyStatus::None;
        assert!(update_dead_key_status(
            &mut status,
            DeadKeyStatus::Composing("´".to_string())
        ));
        assert_eq!(status, DeadKeyStatus::Composing("´".to_string()));

        // Composition text grows
        assert!(update_dead_key_status(
            &mut status,
            DeadKeyStatus::Composing("にほ".to_string())
        ));
        assert_eq!(status, DeadKeyStatus::Composing("にほ".to_string()));

        // Composition ends: the placeholder must be erased
        assert!(update_dead_key_status(&mut status, DeadKeyStatus::None));
        assert_eq!(status, DeadKeyStatus::None);
    }

    #[test]
    fn scroll_to_bottom_reports_viewport_change() {
        let mut viewport = Some(-10);
        assert!(scroll_to_bottom_for_input(&mut viewport, true));
        assert_eq!(viewport, None);

        // Already at the bottom: nothing to repaint
        assert!(!scroll_to_bottom_for_input(&mut viewport, true));
        assert_eq!(viewport, None);
    }

    #[test]
    fn scroll_to_bottom_disabled_keeps_viewport() {
        let mut viewport = Some(-10);
        assert!(!scroll_to_bottom_for_input(&mut viewport, false));
        assert_eq!(viewport, Some(-10));
    }

    #[test]
    fn did_process_key_reports_one_shot_pop() {
        let mut state = KeyTableState::default();
        assert!(!state.did_process_key());

        state.activate(table(false, false));
        assert!(!state.did_process_key());
        assert_eq!(state.current_table(), Some("test"));

        state.activate(table(true, false));
        assert!(state.did_process_key());
        assert_eq!(state.stack.len(), 1);
    }

    #[test]
    fn pop_until_unknown_reports_pops() {
        let mut state = KeyTableState::default();
        assert!(!state.pop_until_unknown());

        state.activate(table(false, false));
        assert!(!state.pop_until_unknown());
        assert_eq!(state.stack.len(), 1);

        state.activate(table(false, true));
        state.activate(table(false, true));
        assert!(state.pop_until_unknown());
        assert_eq!(state.stack.len(), 1);
    }
}
