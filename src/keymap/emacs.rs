use std::io::{self, Write};
use termion::event::Key;

use KeyMap;
use Editor;

pub struct Emacs<'a, W: Write> {
    ed: Editor<'a, W>,
}

impl<'a, W: Write> Emacs<'a, W> {
    pub fn new(ed: Editor<'a, W>) -> Self {
        Emacs { ed: ed }
    }

    fn handle_ctrl_key(&mut self, c: char) -> io::Result<()> {
        match c {
            'l' => self.ed.clear(),
            'a' => self.ed.move_cursor_to_start_of_line(),
            'e' => self.ed.move_cursor_to_end_of_line(),
            'b' => self.ed.move_cursor_left(1),
            'f' => self.ed.move_cursor_right(1),
            'd' => self.ed.delete_after_cursor(),
            'p' => self.ed.move_up(),
            'n' => self.ed.move_down(),
            'u' => self.ed.delete_all_before_cursor(),
            'k' => self.ed.delete_all_after_cursor(),
            'w' => self.ed.delete_word_before_cursor(true),
            'x' => {
                try!(self.ed.undo());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn handle_alt_key(&mut self, c: char) -> io::Result<()> {
        match c {
            '<' => self.ed.move_to_start_of_history(),
            '>' => self.ed.move_to_end_of_history(),
            '\x7F' => self.ed.delete_word_before_cursor(true),
            'f' => emacs_move_word(&mut self.ed, EmacsMoveDir::Right),
            'b' => emacs_move_word(&mut self.ed, EmacsMoveDir::Left),
            'r' => {
                try!(self.ed.revert());
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl<'a, W: Write> KeyMap<'a, W, Emacs<'a, W>> for Emacs<'a, W> {
    fn handle_key_core(&mut self, key: Key) -> io::Result<()> {
        match key {
            Key::Char(c) => self.ed.insert_after_cursor(c),
            Key::Alt(c) => self.handle_alt_key(c),
            Key::Ctrl(c) => self.handle_ctrl_key(c),
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

    fn editor_mut(&mut self) ->  &mut Editor<'a, W> {
        &mut self.ed
    }

    fn editor(&self) ->  &Editor<'a, W> {
        &self.ed
    }
}

impl<'a, W: Write> From<Emacs<'a, W>> for String {
    fn from(emacs: Emacs<'a, W>) -> String {
        emacs.ed.into()
    }
}

#[derive(PartialEq, Clone, Copy)]
enum EmacsMoveDir {
    Left,
    Right,
}

impl EmacsMoveDir {
    fn advance(&self, cursor: &mut usize, max: usize) -> bool {
        if *self == EmacsMoveDir::Right && *cursor == max {
            return false;
        }
        
        if *self == EmacsMoveDir::Left && *cursor == 0 {
            return false;
        }

        match *self {
            EmacsMoveDir::Right => *cursor += 1,
            EmacsMoveDir::Left => *cursor -= 1,
        }

        true
    }
}


fn emacs_move_word<W: Write>(ed: &mut Editor<W>, direction: EmacsMoveDir) -> io::Result<()> {
    enum State {
        Word,
        NonWord,
    }

    let mut cursor = ed.cursor();

    {
        let buf = ed.current_buffer();

        let next_char = |cursor| {
            match direction {
                EmacsMoveDir::Right => buf.char_after(cursor),
                EmacsMoveDir::Left => buf.char_before(cursor),
            }
        };

        let mut state = match next_char(cursor) {
            None => return Ok(()),
            Some(c) => {
                match c {
                    c if is_emacs_word(c) => State::Word,
                    _ => State::NonWord,
                }
            },
        };

        // Skip non-words until a word is found, then stop at the character after the word
        while direction.advance(&mut cursor, buf.num_chars()) {
            let c = match next_char(cursor) {
                Some(c) => c,
                _ => break,
            };

            match state {
                State::NonWord => match c {
                    c if is_emacs_word(c) => state = State::Word,
                    _ => {},
                },
                State::Word => match c {
                    c if !is_emacs_word(c) => break,
                    _ => {},
                },
            }
        }
    }

    ed.move_cursor_to(cursor)
}

fn is_emacs_word(c: char) -> bool {
    c.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;
    use termion::event::Key;
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
        where I: Iterator<Item = &'b Key>
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
        let mut map = Emacs::new(ed);
        map.ed.insert_str_after_cursor("done").unwrap();
        assert_eq!(map.ed.cursor(), 4);

        assert!(simulate_keys!(map, [Key::Char('\n')]));

        assert_eq!(map.ed.cursor(), 4);
        assert_eq!(String::from(map), "done");
    }

    #[test]
    fn move_cursor_left() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Emacs::new(ed);
        map.editor_mut().insert_str_after_cursor("let").unwrap();
        assert_eq!(map.ed.cursor(), 3);

        simulate_keys!(map, [Key::Left, Key::Char('f')]);

        assert_eq!(map.ed.cursor(), 3);
        assert_eq!(String::from(map), "left");
    }

    #[test]
    fn move_word() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Emacs::new(ed);
        map.editor_mut().insert_str_after_cursor("abc def ghi").unwrap();
        assert_eq!(map.ed.cursor(), 11);

        simulate_keys!(map, [Key::Alt('b')]);

        // Move to `g`
        assert_eq!(map.ed.cursor(), 8);

        simulate_keys!(map, [Key::Alt('b'), Key::Alt('f')]);

        // Move to the char after `f`
        assert_eq!(map.ed.cursor(), 7);
    }

    #[test]
    fn cursor_movement() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Emacs::new(ed);
        map.ed.insert_str_after_cursor("right").unwrap();
        assert_eq!(map.ed.cursor(), 5);

        simulate_keys!(map, [Key::Left, Key::Left, Key::Right]);

        assert_eq!(map.ed.cursor(), 4);
    }
}
