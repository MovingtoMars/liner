use std::{mem, cmp};
use std::io::{self, Write};
use termion::event::Key;

use KeyMap;
use Editor;

/// The editing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Insert,
    Normal,
}

struct ModeStack(Vec<Mode>);

impl ModeStack {
    fn with_insert() -> Self {
        ModeStack(vec![Mode::Insert])
    }

    /// Get the current mode.
    ///
    /// If the stack is empty, we are in normal mode.
    fn mode(&self) -> Mode {
        self.0.last()
            .map(|&m| m)
            .unwrap_or(Mode::Normal)
    }

    /// Push the given mode on to the stack.
    fn push(&mut self, m: Mode) {
        self.0.push(m)
    }

    fn pop(&mut self) -> Mode {
        self.0.pop()
            .unwrap_or(Mode::Normal)
    }
}

pub struct Vi<'a, W: Write> {
    ed: Editor<'a, W>,
    mode_stack: ModeStack,
    last_command: Vec<Key>,
    last_insert: Option<Key>,
    count: u32,
    last_count: u32,
    movement_reset: bool,
}

impl<'a, W: Write> Vi<'a, W> {
    pub fn new(mut ed: Editor<'a, W>) -> Self {
        // since we start in insert mode, we need to start an undo group
        ed.current_buffer_mut().start_undo_group();

        Vi {
            ed: ed,
            mode_stack: ModeStack::with_insert(),
            last_command: Vec::new(),
            // we start vi in insert mode
            last_insert: Some(Key::Char('i')),
            count: 0,
            last_count: 0,
            movement_reset: false,
        }
    }

    /// Get the current mode.
    fn mode(&self) -> Mode {
        self.mode_stack.mode()
    }

    fn set_mode(&mut self, mode: Mode) {
        use self::Mode::*;

        self.ed.no_eol = mode == Normal;
        self.movement_reset = mode != Insert;
        self.mode_stack.push(mode);

        if mode == Insert {
            self.last_count = 0;
            self.last_command.clear();
            self.ed.current_buffer_mut().start_undo_group();
        }
    }

    fn pop_mode_after_movement(&mut self) -> io::Result<()> {
        use self::Mode::*;

        self.ed.no_eol = self.mode() == Mode::Normal;
        self.movement_reset = self.mode() != Mode::Insert;

        // in normal mode, count goes back to 0 after movement
        if self.mode_stack.pop() == Normal {
            self.count = 0;
        }

        Ok(())
    }

    fn pop_mode(&mut self) {
        use self::Mode::*;

        let last_mode = self.mode_stack.pop();
        self.ed.no_eol = self.mode() == Normal;
        self.movement_reset = self.mode() != Insert;

        if last_mode == Insert {
            self.ed.current_buffer_mut().end_undo_group();
        }
    }

    /// When doing a move, 0 should behave the same as 1 as far as the count goes.
    fn move_count(&mut self) -> usize {
        match self.count {
            0 => 1,
            _ => self.count as usize,
        }
    }

    /// Get the current count or the number of remaining chars in the buffer.
    fn move_count_left(&mut self) -> usize {
        cmp::min(self.ed.cursor(), self.move_count())
    }

    /// Get the current count or the number of remaining chars in the buffer.
    fn move_count_right(&mut self) -> usize {
        cmp::min(self.ed.current_buffer().num_chars() - self.ed.cursor(), self.move_count())
    }

    fn repeat(&mut self) -> io::Result<()> {
        self.last_count = self.count;
        let keys = mem::replace(&mut self.last_command, Vec::new());

        if let Some(insert_key) = self.last_insert {
            // enter insert mode if necessary
            try!(self.handle_key_core(insert_key));
        }

        for k in keys.iter() {
            try!(self.handle_key_core(*k));
        }

        if self.last_insert.is_some() {
            // leave insert mode
            try!(self.handle_key_core(Key::Esc));
        }

        // restore the last command
        mem::replace(&mut self.last_command, keys);

        Ok(())
    }

    fn handle_key_common(&mut self, key: Key) -> io::Result<()> {
        match key {
            Key::Ctrl('l') => self.ed.clear(),
            Key::Left => self.ed.move_cursor_left(1),
            Key::Right => self.ed.move_cursor_right(1),
            Key::Up => self.ed.move_up(),
            Key::Down => self.ed.move_down(),
            Key::Home => self.ed.move_cursor_to_start_of_line(),
            Key::End => self.ed.move_cursor_to_end_of_line(),
            Key::Backspace => self.ed.delete_before_cursor(),
            Key::Delete => self.ed.delete_after_cursor(),
            Key::Null => Ok(()),
            _ => Ok(()),
        }
    }

    fn handle_key_insert(&mut self, key: Key) -> io::Result<()> {
        match key {
            Key::Esc => {
                // perform any repeats
                if self.count > 0 {
                    self.last_count = self.count;
                    for _ in 1..self.count {
                        let keys = mem::replace(&mut self.last_command, Vec::new());
                        for k in keys.into_iter() {
                            try!(self.handle_key_core(k));
                        }
                    }
                    self.count = 0;
                }
                // cursor moves to the left when switching from insert to normal mode
                try!(self.ed.move_cursor_left(1));
                self.pop_mode();
                Ok(())
            }
            Key::Char(c) => {
                if self.movement_reset {
                    self.ed.current_buffer_mut().end_undo_group();
                    self.ed.current_buffer_mut().start_undo_group();
                    self.last_command.clear();
                    self.movement_reset = false;
                    // vim behaves as if this was 'i'
                    self.last_insert = Some(Key::Char('i'));
                }
                self.last_command.push(key);
                self.ed.insert_after_cursor(c)
            }
            // delete and backspace need to be included in the command buffer
            Key::Backspace | Key::Delete => {
                if self.movement_reset {
                    self.ed.current_buffer_mut().end_undo_group();
                    self.ed.current_buffer_mut().start_undo_group();
                    self.last_command.clear();
                    self.movement_reset = false;
                    // vim behaves as if this was 'i'
                    self.last_insert = Some(Key::Char('i'));
                }
                self.last_command.push(key);
                self.handle_key_common(key)
            }
            // if this is a movement while in insert mode, reset the repeat count
            Key::Left | Key::Right | Key::Home | Key::End => {
                self.count = 0;
                self.movement_reset = true;
                self.handle_key_common(key)
            }
            // up and down require even more special handling
            Key::Up => {
                self.count = 0;
                self.movement_reset = true;
                self.ed.current_buffer_mut().end_undo_group();
                try!(self.ed.move_up());
                self.ed.current_buffer_mut().start_undo_group();
                Ok(())
            }
            Key::Down => {
                self.count = 0;
                self.movement_reset = true;
                self.ed.current_buffer_mut().end_undo_group();
                try!(self.ed.move_down());
                self.ed.current_buffer_mut().start_undo_group();
                Ok(())
            }
            _ => self.handle_key_common(key),
        }
    }

    fn handle_key_normal(&mut self, key: Key) -> io::Result<()> {
        use self::Mode::*;

        match key {
            Key::Esc => {
                self.count = 0;
                Ok(())
            }
            Key::Char('i') => {
                self.last_insert = Some(key);
                self.set_mode(Insert);
                Ok(())
            }
            Key::Char('a') => {
                self.last_insert = Some(key);
                self.set_mode(Insert);
                self.ed.move_cursor_right(1)
            }
            Key::Char('A') => {
                self.last_insert = Some(key);
                self.set_mode(Insert);
                self.ed.move_cursor_to_end_of_line()
            }
            Key::Char('I') => {
                self.last_insert = Some(key);
                self.set_mode(Insert);
                self.ed.move_cursor_to_start_of_line()
            }
            Key::Char('.') => {
                // repeat the last command
                self.count = match (self.count, self.last_count) {
                    // if both count and last_count are zero, use 1
                    (0, 0) => 1,
                    // if count is zero, use last_count
                    (0, _) => self.last_count,
                    // otherwise use count
                    (_, _) => self.count,
                };
                self.repeat()
            }
            Key::Char('h') | Key::Left | Key::Backspace => {
                let count = self.move_count_left();
                try!(self.ed.move_cursor_left(count));
                self.pop_mode_after_movement()
            }
            Key::Char('l') | Key::Right | Key::Char(' ') => {
                let count = self.move_count_right();
                try!(self.ed.move_cursor_right(count));
                self.pop_mode_after_movement()
            }
            Key::Char('k') | Key::Up =>  {
                try!(self.ed.move_up());
                self.pop_mode_after_movement()
            }
            Key::Char('j') | Key::Down => {
                try!(self.ed.move_down());
                self.pop_mode_after_movement()
            }
            // if count is 0, 0 should move to start of line
            Key::Char('0') if self.count == 0 => {
                try!(self.ed.move_cursor_to_start_of_line());
                self.pop_mode_after_movement()
            }
            Key::Char(i @ '0'...'9') => {
                let i = i.to_digit(10).unwrap();
                // count = count * 10 + i
                self.count = self.count
                    .saturating_mul(10)
                    .saturating_add(i);
                Ok(())
            }
            Key::Char('$') => {
                try!(self.ed.move_cursor_to_end_of_line());
                self.pop_mode_after_movement()
            }
            Key::Char('u') => {
                let count = self.move_count();
                self.count = 0;
                for _ in 0..count {
                    let did = try!(self.ed.undo());
                    if !did {
                        break;
                    }
                }
                Ok(())
            }
            Key::Ctrl('r') => {
                let count = self.move_count();
                self.count = 0;
                for _ in 0..count {
                    let did = try!(self.ed.redo());
                    if !did {
                        break;
                    }
                }
                Ok(())
            }
            _ => self.handle_key_common(key),
        }
    }

}

impl<'a, W: Write> KeyMap<'a, W, Vi<'a, W>> for Vi<'a, W> {
    fn handle_key_core(&mut self, key: Key) -> io::Result<()> {
        match self.mode() {
            Mode::Normal => self.handle_key_normal(key),
            Mode::Insert => self.handle_key_insert(key),
        }
    }

    fn editor(&mut self) ->  &mut Editor<'a, W> {
        &mut self.ed
    }
}

impl<'a, W: Write> From<Vi<'a, W>> for String {
    fn from(vi: Vi<'a, W>) -> String {
        vi.ed.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use termion::event::Key;
    use termion::event::Key::*;
    use Buffer;
    use Context;
    use Editor;
    use KeyMap;
    use std::io::Write;

    macro_rules! simulate_keys {
        ($keymap:ident, $keys:expr) => {{
            simulate_keys(&mut $keymap, $keys.into_iter())
        }}
    }

    fn simulate_keys<'a, 'b, W: Write, T, M: KeyMap<'a, W, T>, I>(keymap: &mut M, keys: I) -> bool
        where I: Iterator<Item=&'b Key>
    {
        for k in keys {
            if keymap.handle_key(*k, &mut |_| {}).unwrap() {
                return true;
            }
        }

        false
    }

    #[test]
    fn enter_is_done() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);
        map.ed.insert_str_after_cursor("done").unwrap();
        assert_eq!(map.ed.cursor(), 4);

        assert!(simulate_keys!(map, [
            Char('\n'),
        ]));

        assert_eq!(map.ed.cursor(), 4);
        assert_eq!(String::from(map), "done");
    }

    #[test]
    fn move_cursor_left() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);
        map.editor().insert_str_after_cursor("let").unwrap();
        assert_eq!(map.ed.cursor(), 3);

        simulate_keys!(map, [
            Left,
            Char('f'),
        ]);

        assert_eq!(map.ed.cursor(), 3);
        assert_eq!(String::from(map), "left");
    }

    #[test]
    fn cursor_movement() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);
        map.ed.insert_str_after_cursor("right").unwrap();
        assert_eq!(map.ed.cursor(), 5);

        simulate_keys!(map, [
            Left,
            Left,
            Right,
        ]);

        assert_eq!(map.ed.cursor(), 4);
    }

    #[test]
    fn vi_initial_insert() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Char('i'),
            Char('n'),
            Char('s'),
            Char('e'),
            Char('r'),
            Char('t'),
        ]);

        assert_eq!(map.ed.cursor(), 6);
        assert_eq!(String::from(map), "insert");
    }

    #[test]
    fn vi_left_right_movement() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);
        map.ed.insert_str_after_cursor("data").unwrap();
        assert_eq!(map.ed.cursor(), 4);

        simulate_keys!(map, [Left]);
        assert_eq!(map.ed.cursor(), 3);
        simulate_keys!(map, [Right]);
        assert_eq!(map.ed.cursor(), 4);

        // switching from insert mode moves the cursor left
        simulate_keys!(map, [Esc, Left]);
        assert_eq!(map.ed.cursor(), 2);
        simulate_keys!(map, [Right]);
        assert_eq!(map.ed.cursor(), 3);

        simulate_keys!(map, [Char('h')]);
        assert_eq!(map.ed.cursor(), 2);
        simulate_keys!(map, [Char('l')]);
        assert_eq!(map.ed.cursor(), 3);
    }

    #[test]
    /// Shouldn't be able to move past the last char in vi normal mode
    fn vi_no_eol() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);
        map.ed.insert_str_after_cursor("data").unwrap();
        assert_eq!(map.ed.cursor(), 4);

        simulate_keys!(map, [Esc]);
        assert_eq!(map.ed.cursor(), 3);

        simulate_keys!(map, [Right, Right]);
        assert_eq!(map.ed.cursor(), 3);

        // in insert mode, we can move past the last char, but no further
        simulate_keys!(map, [Char('i'), Right, Right]);
        assert_eq!(map.ed.cursor(), 4);
    }

    #[test]
    /// Cursor moves left when exiting insert mode.
    fn vi_switch_from_insert() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);
        map.ed.insert_str_after_cursor("data").unwrap();
        assert_eq!(map.ed.cursor(), 4);

        simulate_keys!(map, [Esc]);
        assert_eq!(map.ed.cursor(), 3);

        simulate_keys!(map, [
            Char('i'),
            Esc,
            Char('i'),
            Esc,
            Char('i'),
            Esc,
            Char('i'),
            Esc,
        ]);
        assert_eq!(map.ed.cursor(), 0);
    }

    #[test]
    fn vi_normal_history_cursor_eol() {
        let mut context = Context::new();
        context.history.push("history".into()).unwrap();
        context.history.push("history".into()).unwrap();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);
        map.ed.insert_str_after_cursor("data").unwrap();
        assert_eq!(map.ed.cursor(), 4);

        simulate_keys!(map, [Up]);
        assert_eq!(map.ed.cursor(), 7);

        // in normal mode, make sure we don't end up past the last char
        simulate_keys!(map, [Esc, Up]);
        assert_eq!(map.ed.cursor(), 6);
    }

    #[test]
    /// make sure our count is accurate
    fn vi_count() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Esc,
        ]);
        assert_eq!(map.count, 0);

        simulate_keys!(map, [
            Char('1'),
        ]);
        assert_eq!(map.count, 1);

        simulate_keys!(map, [
            Char('1'),
        ]);
        assert_eq!(map.count, 11);

        // switching to insert mode and back to edit mode should reset the count
        simulate_keys!(map, [
            Char('i'),
            Esc,
        ]);
        assert_eq!(map.count, 0);

        assert_eq!(String::from(map), "");
    }

    #[test]
    /// make sure large counts don't overflow
    fn vi_count_overflow() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        // make sure large counts don't overflow our u32
        simulate_keys!(map, [
            Esc,
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
            Char('9'), Char('9'), Char('9'), Char('9'), Char('9'),
        ]);
        assert_eq!(String::from(map), "");
    }

    #[test]
    /// make sure large counts ending in zero don't overflow
    fn vi_count_overflow_zero() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        // make sure large counts don't overflow our u32
        simulate_keys!(map, [
            Esc,
            Char('1'), Char('0'), Char('0'), Char('0'), Char('0'),
            Char('0'), Char('0'), Char('0'), Char('0'), Char('0'),
            Char('0'), Char('0'), Char('0'), Char('0'), Char('0'),
            Char('0'), Char('0'), Char('0'), Char('0'), Char('0'),
            Char('0'), Char('0'), Char('0'), Char('0'), Char('0'),
            Char('0'), Char('0'), Char('0'), Char('0'), Char('0'),
            Char('0'), Char('0'), Char('0'), Char('0'), Char('0'),
            Char('0'), Char('0'), Char('0'), Char('0'), Char('0'),
            Char('0'), Char('0'), Char('0'), Char('0'), Char('0'),
            Char('0'), Char('0'), Char('0'), Char('0'), Char('0'),
            Char('0'), Char('0'), Char('0'), Char('0'), Char('0'),
            Char('0'), Char('0'), Char('0'), Char('0'), Char('0'),
        ]);
        assert_eq!(String::from(map), "");
    }

    #[test]
    /// Esc should cancel the count
    fn vi_count_cancel() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Esc,
            Char('1'),
            Char('0'),
            Esc,
        ]);
        assert_eq!(map.count, 0);
        assert_eq!(String::from(map), "");
    }

    #[test]
    /// test insert with a count
    fn vi_count_simple() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Esc,
            Char('3'),
            Char('i'),
            Char('t'),
            Char('h'),
            Char('i'),
            Char('s'),
            Esc,
        ]);
        assert_eq!(String::from(map), "thisthisthis");
    }

    #[test]
    /// test dot command
    fn vi_dot_command() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Char('i'),
            Char('f'),
            Esc,
            Char('.'),
            Char('.'),
        ]);
        assert_eq!(String::from(map), "iiifff");
    }

    #[test]
    /// test dot command with repeat
    fn vi_dot_command_repeat() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Char('i'),
            Char('f'),
            Esc,
            Char('3'),
            Char('.'),
        ]);
        assert_eq!(String::from(map), "iifififf");
    }

    #[test]
    /// test dot command with repeat
    fn vi_dot_command_repeat_multiple() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Char('i'),
            Char('f'),
            Esc,
            Char('3'),
            Char('.'),
            Char('.'),
        ]);
        assert_eq!(String::from(map), "iififiifififff");
    }

    #[test]
    /// test dot command with append
    fn vi_dot_command_append() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Esc,
            Char('a'),
            Char('i'),
            Char('f'),
            Esc,
            Char('.'),
            Char('.'),
        ]);
        assert_eq!(String::from(map), "ififif");
    }

    #[test]
    /// test dot command with append and repeat
    fn vi_dot_command_append_repeat() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Esc,
            Char('a'),
            Char('i'),
            Char('f'),
            Esc,
            Char('3'),
            Char('.'),
        ]);
        assert_eq!(String::from(map), "ifififif");
    }

    #[test]
    /// test dot command with movement
    fn vi_dot_command_movement() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Esc,
            Char('a'),
            Char('d'),
            Char('t'),
            Char(' '),
            Left,
            Left,
            Char('a'),
            Esc,
            Right,
            Right,
            Char('.'),
        ]);
        assert_eq!(String::from(map), "data ");
    }

    #[test]
    /// test move_count function
    fn move_count() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        assert_eq!(map.move_count(), 1);
        map.count = 1;
        assert_eq!(map.move_count(), 1);
        map.count = 99;
        assert_eq!(map.move_count(), 99);
    }

    #[test]
    /// make sure the count is reset if movement occurs
    fn vi_count_movement_reset() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Esc,
            Char('3'),
            Char('i'),
            Char('t'),
            Char('h'),
            Char('i'),
            Char('s'),
            Left,
            Esc,
        ]);
        assert_eq!(String::from(map), "this");
    }

    #[test]
    /// test movement with counts
    fn movement_with_count() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);
        map.ed.insert_str_after_cursor("right").unwrap();
        assert_eq!(map.ed.cursor(), 5);

        simulate_keys!(map, [
            Esc,
            Char('3'),
            Left,
        ]);

        assert_eq!(map.ed.cursor(), 1);
    }

    #[test]
    /// test movement with counts, then insert (count should be reset before insert)
    fn movement_with_count_then_insert() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);
        map.ed.insert_str_after_cursor("right").unwrap();
        assert_eq!(map.ed.cursor(), 5);

        simulate_keys!(map, [
            Esc,
            Char('3'),
            Left,
            Char('i'),
            Char(' '),
            Esc,
        ]);
        assert_eq!(String::from(map), "r ight");
    }

    #[test]
    /// verify our move count
    fn move_count_right() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);
        map.ed.insert_str_after_cursor("replace").unwrap();
        assert_eq!(map.ed.cursor(), 7);
        assert_eq!(map.move_count_right(), 0);
        map.count = 10;
        assert_eq!(map.move_count_right(), 0);

        map.count = 0;
        simulate_keys!(map, [
            Esc,
            Left,
        ]);
        assert_eq!(map.move_count_right(), 1);
    }

    #[test]
    /// verify our move count
    fn move_count_left() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);
        map.ed.insert_str_after_cursor("replace").unwrap();
        assert_eq!(map.ed.cursor(), 7);
        assert_eq!(map.move_count_left(), 1);
        map.count = 10;
        assert_eq!(map.move_count_left(), 7);

        map.count = 0;
        simulate_keys!(map, [
            Esc,
            Char('0'),
        ]);
        assert_eq!(map.move_count_left(), 0);
    }

    #[test]
    /// test undo in groups
    fn undo_insert() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Char('i'),
            Char('n'),
            Char('s'),
            Char('e'),
            Char('r'),
            Char('t'),
            Esc,
            Char('u'),
        ]);
        assert_eq!(String::from(map), "");
    }

    #[test]
    /// test undo in groups
    fn undo_insert2() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Esc,
            Char('i'),
            Char('i'),
            Char('n'),
            Char('s'),
            Char('e'),
            Char('r'),
            Char('t'),
            Esc,
            Char('u'),
        ]);
        assert_eq!(String::from(map), "");
    }

    #[test]
    /// test undo in groups
    fn undo_insert_with_history() {
        let mut context = Context::new();
        context.history.push(Buffer::from("")).unwrap();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Esc,
            Char('i'),
            Char('i'),
            Char('n'),
            Char('s'),
            Char('e'),
            Char('r'),
            Char('t'),
            Up,
            Char('h'),
            Char('i'),
            Char('s'),
            Char('t'),
            Char('o'),
            Char('r'),
            Char('y'),
            Down,
            Char(' '),
            Char('t'),
            Char('e'),
            Char('x'),
            Char('t'),
            Esc,
            Char('u'),
        ]);
        assert_eq!(String::from(map), "insert");
    }

    #[test]
    /// test undo in groups
    fn undo_insert_with_history2() {
        let mut context = Context::new();
        context.history.push(Buffer::from("")).unwrap();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Esc,
            Char('i'),
            Char('i'),
            Char('n'),
            Char('s'),
            Char('e'),
            Char('r'),
            Char('t'),
            Up,
            Esc,
            Down,
            Char('u'),
        ]);
        assert_eq!(String::from(map), "");
    }

    #[test]
    /// test undo in groups
    fn undo_insert_with_movement_reset() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Esc,
            Char('i'),
            Char('i'),
            Char('n'),
            Char('s'),
            Char('e'),
            Char('r'),
            Char('t'),
            // movement reset will get triggered here
            Left,
            Right,
            Char(' '),
            Char('t'),
            Char('e'),
            Char('x'),
            Char('t'),
            Esc,
            Char('u'),
        ]);
        assert_eq!(String::from(map), "insert");
    }

    #[test]
    /// test undo in groups
    fn undo_insert_with_count() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Char('i'),
            Char('n'),
            Char('s'),
            Char('e'),
            Char('r'),
            Char('t'),
            Esc,
            Char('3'),
            Char('i'),
            Char('i'),
            Char('n'),
            Char('s'),
            Char('e'),
            Char('r'),
            Char('t'),
            Esc,
            Char('u'),
        ]);
        assert_eq!(String::from(map), "insert");
    }

    #[test]
    /// test undo in groups
    fn undo_insert_with_repeat() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Vi::new(ed);

        simulate_keys!(map, [
            Char('i'),
            Char('n'),
            Char('s'),
            Char('e'),
            Char('r'),
            Char('t'),
            Esc,
            Char('3'),
            Char('.'),
            Esc,
            Char('u'),
        ]);
        assert_eq!(String::from(map), "insert");
    }
}
