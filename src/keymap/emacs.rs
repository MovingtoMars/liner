use std::io::{self, Write};
use termion::event::Key;

use KeyMap;
use Editor;
use CursorPosition;

pub struct Emacs<'a, W: Write> {
    ed: Editor<'a, W>,
    last_arg_fetch_index: Option<usize>,
}

impl<'a, W: Write> Emacs<'a, W> {
    pub fn new(ed: Editor<'a, W>) -> Self {
        Emacs { ed, last_arg_fetch_index: None }
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
            '.' => self.handle_last_arg_fetch(),
            _ => Ok(()),
        }
    }

    fn handle_last_arg_fetch(&mut self) -> io::Result<()> {
        // Empty history means no last arg to fetch.
        if self.ed.context().history.len() == 0 {
            return Ok(());
        }

        let history_index = match self.last_arg_fetch_index {
            Some(0) => return Ok(()),
            Some(x) => x - 1,
            None => self.ed.current_history_location().unwrap_or(self.ed.context().history.len() - 1),
        };

        // If did a last arg fetch just before this, we need to delete it so it can be replaced by
        // this last arg fetch.
        if self.last_arg_fetch_index.is_some() {
            let buffer_len = self.ed.current_buffer().num_chars();
            if let Some(last_arg_len) = self.ed.current_buffer().last_arg().map(|x| x.len()) {
                self.ed.delete_until(buffer_len - last_arg_len)?;
            }
        }

        // Actually insert it
        let buf = self.ed.context().history[history_index].clone();
        if let Some(last_arg) = buf.last_arg() {
            self.ed.insert_chars_after_cursor(last_arg)?;
        }

        // Edit the index in case the user does a last arg fetch again.
        self.last_arg_fetch_index = Some(history_index);

        Ok(())
    }
}

impl<'a, W: Write> KeyMap<'a, W, Emacs<'a, W>> for Emacs<'a, W> {
    fn handle_key_core(&mut self, key: Key) -> io::Result<()> {
        match key {
            Key::Alt('.') => {},
            _ => self.last_arg_fetch_index = None,
        }

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

fn emacs_move_word<W: Write>(ed: &mut Editor<W>, direction: EmacsMoveDir) -> io::Result<()> {
    let (words, pos) = ed.get_words_and_cursor_position();

    let word_index = match pos {
        CursorPosition::InWord(i) => {
            Some(i)
        },
        CursorPosition::OnWordLeftEdge(mut i) => {
            if i > 0 && direction == EmacsMoveDir::Left {
                i -= 1;
            }
            Some(i)
        },
        CursorPosition::OnWordRightEdge(mut i) => {
            if i < words.len() - 1 && direction == EmacsMoveDir::Right {
                i += 1;
            }
            Some(i)
        },
        CursorPosition::InSpace(left, right) => {
            match direction {
                EmacsMoveDir::Left => left,
                EmacsMoveDir::Right => right,
            }
        },
    };

    match word_index {
        None => Ok(()),
        Some(i) => {
            let (start, end) = words[i];

            let new_cursor_pos = match direction {
                EmacsMoveDir::Left => start,
                EmacsMoveDir::Right => end,
            };

            ed.move_cursor_to(new_cursor_pos)
        }
    }
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

    #[test]
    /// ctrl-h should act as backspace
    fn ctrl_h() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = Emacs::new(ed);
        map.ed.insert_str_after_cursor("not empty").unwrap();

        let res = map.handle_key(Key::Ctrl('h'), &mut |_| {});
        assert_eq!(res.is_ok(), true);
        assert_eq!(map.ed.current_buffer().to_string(), "not empt".to_string());
    }
}
