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
    count: u32,
}

impl<'a, W: Write> Vi<'a, W> {
    pub fn new(ed: Editor<'a, W>) -> Self {
        Vi {
            ed: ed,
            mode_stack: ModeStack::with_insert(),
            count: 0,
        }
    }

    /// Get the current mode.
    fn mode(&self) -> Mode {
        self.mode_stack.mode()
    }

    fn set_mode(&mut self, mode: Mode) {
        self.ed.no_eol = mode == Mode::Normal;
        self.mode_stack.push(mode);
    }

    fn pop_mode(&mut self) {
        self.mode_stack.pop();
        self.ed.no_eol = self.mode() == Mode::Normal;
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
                if self.count > 0 {
                    self.count = 0;
                }
                // cursor moves to the left when switching from insert to normal mode
                try!(self.ed.move_cursor_left(1));
                self.pop_mode();
                Ok(())
            }
            Key::Char(c) => self.ed.insert_after_cursor(c),
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
                self.set_mode(Insert);
                Ok(())
            }
            Key::Char('a') => {
                self.set_mode(Insert);
                self.ed.move_cursor_right(1)
            }
            Key::Char('A') => {
                self.set_mode(Insert);
                self.ed.move_cursor_to_end_of_line()
            }
            Key::Char('I') => {
                self.set_mode(Insert);
                self.ed.move_cursor_to_start_of_line()
            }
            Key::Char('h') | Key::Left | Key::Backspace => {
                self.ed.move_cursor_left(1)
            }
            Key::Char('l') | Key::Right | Key::Char(' ') => {
                self.ed.move_cursor_right(1)
            }
            Key::Char('k') => self.ed.move_up(),
            Key::Char('j') => self.ed.move_down(),
            // if count is 0, 0 should move to start of line
            Key::Char('0') if self.count == 0 => {
                self.ed.move_cursor_to_start_of_line()
            }
            Key::Char(i @ '0'...'9') => {
                let i = i.to_digit(10).unwrap();
                // count = count * 10 + i
                self.count = self.count
                    .saturating_mul(10)
                    .saturating_add(i);
                Ok(())
            }
            Key::Char('$') => self.ed.move_cursor_to_end_of_line(),
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
}
