use std::io::{self, Write};
use termion::{self, clear, cursor};
use unicode_width::*;

use Context;
use Buffer;
use event::*;
use util;

/// Represents the position of the cursor relative to words in the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorPosition {
    /// The cursor is in the word with the specified index.
    InWord(usize),

    /// The cursor is on the left edge of the word with the specified index.
    /// For example: `abc |hi`, where `|` is the cursor.
    OnWordLeftEdge(usize),

    /// The cursor is on the right edge of the word with the specified index.
    /// For example: `abc| hi`, where `|` is the cursor.
    OnWordRightEdge(usize),

    /// The cursor is not in contact with any word. Each `Option<usize>` specifies the index of the
    /// closest word to the left and right, respectively, or `None` if there is no word on that side.
    InSpace(Option<usize>, Option<usize>),
}

impl CursorPosition {
    pub fn get(cursor: usize, words: &[(usize, usize)]) -> CursorPosition {
        use CursorPosition::*;

        if words.len() == 0 {
            return InSpace(None, None);
        } else if cursor == words[0].0 {
            return OnWordLeftEdge(0);
        } else if cursor < words[0].0 {
            return InSpace(None, Some(0));
        }

        for (i, &(start, end)) in words.iter().enumerate() {
            if start == cursor {
                return OnWordLeftEdge(i);
            } else if end == cursor {
                return OnWordRightEdge(i);
            } else if start < cursor && cursor < end {
                return InWord(i);
            } else if cursor < start {
                return InSpace(Some(i - 1), Some(i));
            }
        }

        InSpace(Some(words.len() - 1), None)
    }
}

pub struct Editor<'a, W: Write> {
    prompt: String,
    out: W,
    context: &'a mut Context,

    // The location of the cursor. Note that the cursor does not lie on a char, but between chars.
    // So, if `cursor == 0` then the cursor is before the first char,
    // and if `cursor == 1` ten the cursor is after the first char and before the second char.
    cursor: usize,

    // Buffer for the new line (ie. not from editing history)
    new_buf: Buffer,

    // None if we're on the new buffer, else the index of history
    cur_history_loc: Option<usize>,

    // The width of the text in the prompt.
    prompt_width: usize,

    // The line of the cursor relative to the prompt. 1-indexed.
    // So if the cursor is on the same line as the prompt, `term_cursor_line == 1`.
    // If the cursor is on the line below the prompt, `term_cursor_line == 2`.
    term_cursor_line: usize,

    // If this is true, on the next tab we print the completion list.
    show_completions_hint: bool,

    // if set, the cursor will not be allow to move one past the end of the line, this is necessary
    // for Vi's normal mode.
    pub no_eol: bool,
}

macro_rules! cur_buf_mut {
    ($s:expr) => {
        match $s.cur_history_loc {
            Some(i) => &mut $s.context.history[i],
            _ => &mut $s.new_buf,
        }
    }
}

macro_rules! cur_buf {
    ($s:expr) => {
        match $s.cur_history_loc {
            Some(i) => &$s.context.history[i],
            _ => &$s.new_buf,
        }
    }
}

macro_rules! send_event {
    ($handler:expr, $s:expr, $kind:ident, $($args:expr),*) => {
        $handler(Event::new($s, EventKind::$kind($($args),*)))
    };
    ($handler:expr, $s:expr, $kind:ident) => {
        $handler(Event::new($s, EventKind::$kind))
    }
}

impl<'a, W: Write> Editor<'a, W> {
    pub fn new(out: W, prompt: String, context: &'a mut Context) -> io::Result<Self> {
        let prompt_width = util::remove_codes(&prompt[..]).width();

        let mut ed = Editor {
            prompt: prompt,
            cursor: 0,
            out: out,
            new_buf: Buffer::new(),
            cur_history_loc: None,
            context: context,
            show_completions_hint: false,
            prompt_width: prompt_width,
            term_cursor_line: 1,
            no_eol: false,
        };

        try!(ed.print_current_buffer(true));
        Ok(ed)
    }

    pub fn get_words_and_cursor_position(&self) -> (Vec<(usize, usize)>, CursorPosition) {
        let word_fn = &self.context.word_fn;
        let words = word_fn(cur_buf!(self));
        let pos = CursorPosition::get(self.cursor, &words);
        (words, pos)
    }

    pub fn set_prompt(&mut self, prompt: String) {
        self.prompt_width = util::remove_codes(&prompt[..]).width();
        self.prompt = prompt;
    }

    pub fn context(&mut self) -> &mut Context {
        self.context
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn handle_newline(&mut self) -> io::Result<()> {
        try!(self.print_current_buffer(true));
        try!(self.out.write(b"\r\n"));
        self.show_completions_hint = false;
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }

    /// Attempts to undo an action on the current buffer.
    ///
    /// Returns `Ok(true)` if an action was undone.
    /// Returns `Ok(false)` if there was no action to undo.
    pub fn undo(&mut self) -> io::Result<bool> {
        let did = cur_buf_mut!(self).undo();
        try!(self.print_current_buffer(did));
        Ok(did)
    }

    pub fn redo(&mut self) -> io::Result<bool> {
        let did = cur_buf_mut!(self).redo();
        try!(self.print_current_buffer(did));
        Ok(did)
    }

    pub fn revert(&mut self) -> io::Result<bool> {
        let did = cur_buf_mut!(self).revert();
        try!(self.print_current_buffer(did));
        Ok(did)
    }

    fn print_completion_list(&mut self, completions: &[String]) -> io::Result<()> {
        use std::cmp::max;

        let (w, _) = try!(termion::terminal_size());

        // XXX wide character support
        let max_word_size = completions.iter().fold(1, |m, x| max(m, x.chars().count()));
        let cols = max(1, (w as usize / (max_word_size)));
        let col_width = 2 + w as usize / cols;
        let cols = max(1, w as usize / col_width);

        let mut i = 0;
        for com in completions {
            if i == cols {
                try!(write!(self.out, "\r\n"));
                i = 0;
            } else if i > cols {
                unreachable!()
            }

            try!(write!(self.out, "{:<1$}", com, col_width));

            i += 1;
        }

        self.term_cursor_line = 1;

        Ok(())
    }

    pub fn skip_completions_hint(&mut self) {
        self.show_completions_hint = false;
    }

    pub fn complete(&mut self, handler: &mut EventHandler<W>) -> io::Result<()> {
        send_event!(handler, self, BeforeComplete);

        let (word, completions) = {
            let word_range = self.get_word_before_cursor(false);
            let buf = cur_buf_mut!(self);

            let word = match word_range {
                Some((start, end)) => buf.range(start, end),
                None => "".into(),
            };

            if let Some(ref completer) = self.context.completer {
                let mut completions = completer.completions(word.as_ref());
                completions.sort();
                (word, completions)
            } else {
                return Ok(());
            }
        };

        if completions.len() == 0 {
            // Do nothing.
            self.show_completions_hint = false;
            Ok(())
        } else if completions.len() == 1 {
            self.show_completions_hint = false;
            try!(self.delete_word_before_cursor(false));
            self.insert_str_after_cursor(completions[0].as_ref())
        } else {
            let common_prefix =
                util::find_longest_common_prefix(&completions.iter()
                    .map(|x| x.chars().collect())
                    .collect::<Vec<Vec<char>>>()[..]);

            if let Some(p) = common_prefix {
                let s = p.iter()
                    .cloned()
                    .collect::<String>();

                if s.len() > word.len() && s.starts_with(&word[..]) {
                    try!(self.delete_word_before_cursor(false));
                    return self.insert_str_after_cursor(s.as_ref());
                }
            }

            if self.show_completions_hint {
                try!(write!(self.out, "\r\n"));
                try!(self.print_completion_list(&completions[..]));
                try!(write!(self.out, "\r\n"));
                try!(self.print_current_buffer(false));

                self.show_completions_hint = false;
            } else {
                self.show_completions_hint = true;
            }
            Ok(())
        }
    }

    fn get_word_before_cursor(&self, ignore_space_before_cursor: bool) -> Option<(usize, usize)> {
        let (words, pos) = self.get_words_and_cursor_position();
        match pos {
            CursorPosition::InWord(i) => Some(words[i]),
            CursorPosition::InSpace(Some(i), _) => {
                if ignore_space_before_cursor {
                    Some(words[i])
                } else {
                    None
                }
            }
            CursorPosition::InSpace(None, _) => None,
            CursorPosition::OnWordLeftEdge(i) => {
                if ignore_space_before_cursor && i > 0 {
                    Some(words[i - 1])
                } else {
                    None
                }
            }
            CursorPosition::OnWordRightEdge(i) => Some(words[i]),
        }
    }

    /// Deletes the word preceding the cursor.
    /// If `ignore_space_before_cursor` is true and there is space directly before the cursor,
    /// this method ignores that space until it finds a word.
    /// If `ignore_space_before_cursor` is false and there is space directly before the cursor,
    /// nothing is deleted.
    pub fn delete_word_before_cursor(&mut self,
                                     ignore_space_before_cursor: bool)
                                     -> io::Result<()> {
        if let Some((start, _)) = self.get_word_before_cursor(ignore_space_before_cursor) {
            let moved = cur_buf_mut!(self).remove(start, self.cursor);
            self.cursor -= moved;
        }
        self.print_current_buffer(false)
    }

    /// Clears the screen then prints the prompt and current buffer.
    pub fn clear(&mut self) -> io::Result<()> {
        try!(write!(self.out, "{}{}", clear::All, cursor::Goto(1, 1)));
        self.term_cursor_line = 1;
        self.print_current_buffer(false)
    }

    /// Move up (backwards) in history.
    pub fn move_up(&mut self) -> io::Result<()> {
        if let Some(i) = self.cur_history_loc {
            if i > 0 {
                self.cur_history_loc = Some(i - 1);
            } else {
                return self.print_current_buffer(false);
            }
        } else {
            if self.context.history.len() > 0 {
                self.cur_history_loc = Some(self.context.history.len() - 1);
            } else {
                return self.print_current_buffer(false);
            }
        }

        self.print_current_buffer(true)
    }

    /// Move down (forwards) in history, or to the new buffer if we reach the end of history.
    pub fn move_down(&mut self) -> io::Result<()> {
        if let Some(i) = self.cur_history_loc {
            if i < self.context.history.len() - 1 {
                self.cur_history_loc = Some(i + 1);
            } else {
                self.cur_history_loc = None;
            }
            self.print_current_buffer(true)
        } else {
            self.print_current_buffer(false)
        }
    }

    /// Moves to the start of history (ie. the earliest history entry).
    pub fn move_to_start_of_history(&mut self) -> io::Result<()> {
        if self.context.history.len() > 0 {
            self.cur_history_loc = Some(0);
            self.print_current_buffer(true)
        } else {
            self.cur_history_loc = None;
            self.print_current_buffer(false)
        }
    }

    /// Moves to the end of history (ie. the new buffer).
    pub fn move_to_end_of_history(&mut self) -> io::Result<()> {
        if self.cur_history_loc.is_some() {
            self.cur_history_loc = None;
            self.print_current_buffer(true)
        } else {
            self.print_current_buffer(false)
        }
    }

    /// Inserts a string directly after the cursor, moving the cursor to the right.
    ///
    /// Note: it is more efficient to call `insert_chars_after_cursor()` directly.
    pub fn insert_str_after_cursor(&mut self, s: &str) -> io::Result<()> {
        self.insert_chars_after_cursor(&s.chars().collect::<Vec<char>>()[..])
    }

    /// Inserts a character directly after the cursor, moving the cursor to the right.
    pub fn insert_after_cursor(&mut self, c: char) -> io::Result<()> {
        self.insert_chars_after_cursor(&[c])
    }

    /// Inserts characters directly after the cursor, moving the cursor to the right.
    pub fn insert_chars_after_cursor(&mut self, cs: &[char]) -> io::Result<()> {
        {
            let buf = cur_buf_mut!(self);
            buf.insert(self.cursor, cs);
        }

        self.cursor += cs.len();
        self.print_current_buffer(false)
    }

    /// Deletes the character directly before the cursor, moving the cursor to the left.
    /// If the cursor is at the start of the line, nothing happens.
    pub fn delete_before_cursor(&mut self) -> io::Result<()> {
        if self.cursor > 0 {
            let buf = cur_buf_mut!(self);
            buf.remove(self.cursor - 1, self.cursor);
            self.cursor -= 1;
        }

        self.print_current_buffer(false)
    }

    /// Deletes the character directly after the cursor. The cursor does not move.
    /// If the cursor is at the end of the line, nothing happens.
    pub fn delete_after_cursor(&mut self) -> io::Result<()> {
        {
            let buf = cur_buf_mut!(self);

            if self.cursor < buf.num_chars() {
                buf.remove(self.cursor, self.cursor + 1);
            }
        }
        self.print_current_buffer(false)
    }

    /// Deletes every character preceding the cursor until the beginning of the line.
    pub fn delete_all_before_cursor(&mut self) -> io::Result<()> {
        cur_buf_mut!(self).remove(0, self.cursor);
        self.cursor = 0;
        self.print_current_buffer(false)
    }

    /// Deletes every character after the cursor until the end of the line.
    pub fn delete_all_after_cursor(&mut self) -> io::Result<()> {
        {
            let buf = cur_buf_mut!(self);
            buf.truncate(self.cursor);
        }
        self.print_current_buffer(false)
    }

    /// Moves the cursor to the left by `count` characters.
    /// The cursor will not go past the start of the buffer.
    pub fn move_cursor_left(&mut self, mut count: usize) -> io::Result<()> {
        if count > self.cursor {
            count = self.cursor;
        }

        self.cursor -= count;

        self.print_current_buffer(false)
    }

    /// Moves the cursor to the right by `count` characters.
    /// The cursor will not go past the end of the buffer.
    pub fn move_cursor_right(&mut self, mut count: usize) -> io::Result<()> {
        {
            let buf = cur_buf!(self);

            if count > buf.num_chars() - self.cursor {
                count = buf.num_chars() - self.cursor;
            }

            self.cursor += count;
        }

        self.print_current_buffer(false)
    }

    /// Moves the cursor to `pos`. If `pos` is past the end of the buffer, it will be clamped.
    pub fn move_cursor_to(&mut self, pos: usize) -> io::Result<()> {
        self.cursor = pos;
        let buf_len = cur_buf!(self).num_chars();
        if self.cursor > buf_len {
            self.cursor = buf_len;
        }
        self.print_current_buffer(false)
    }

    /// Moves the cursor to the start of the line.
    pub fn move_cursor_to_start_of_line(&mut self) -> io::Result<()> {
        self.cursor = 0;
        self.print_current_buffer(false)
    }

    /// Moves the cursor to the end of the line.
    pub fn move_cursor_to_end_of_line(&mut self) -> io::Result<()> {
        self.cursor = cur_buf!(self).num_chars();
        self.print_current_buffer(false)
    }

    ///  Returns a reference to the current buffer being edited.
    /// This may be the new buffer or a buffer from history.
    pub fn current_buffer(&self) -> &Buffer {
        cur_buf!(self)
    }

    ///  Returns a mutable reference to the current buffer being edited.
    /// This may be the new buffer or a buffer from history.
    pub fn current_buffer_mut(&mut self) -> &mut Buffer {
        cur_buf_mut!(self)
    }

    /// Deletes the displayed prompt and buffer, replacing them with the current prompt and buffer
    pub fn print_current_buffer(&mut self, move_cursor_to_end_of_line: bool) -> io::Result<()> {
        let buf = cur_buf!(self);
        let buf_width = buf.width();
        let new_prompt_and_buffer_width = buf_width + self.prompt_width;

        let (w, _) =
            // when testing hardcode terminal size values
            if cfg!(test) { (80, 24) }
            // otherwise pull values from termion
            else { try!(termion::terminal_size()) };
        let w = w as usize;
        let new_num_lines = (new_prompt_and_buffer_width + w) / w;

        // Move the term cursor to the same line as the prompt.
        if self.term_cursor_line > 1 {
            try!(write!(self.out, "{}", cursor::Up(self.term_cursor_line as u16 - 1)));
        }
        // Move the cursor to the start of the line then clear everything after. Write the prompt
        try!(write!(self.out, "\r{}{}", clear::AfterCursor, self.prompt));

        try!(buf.print(&mut self.out));
        if new_prompt_and_buffer_width % w == 0 {
            // at the end of the line, move the cursor down a line
            try!(write!(self.out, "\r\n"));
        }

        let buf_num_chars = buf.num_chars();
        if move_cursor_to_end_of_line {
            self.cursor = buf_num_chars;
        } else {
            if buf_num_chars < self.cursor {
                self.cursor = buf_num_chars;
            }
        }

        // can't move past the last character in vi normal mode
        if self.no_eol {
            if self.cursor >= 1 && self.cursor == buf_num_chars {
                self.cursor -= 1;
            }
        }

        self.term_cursor_line = (self.prompt_width + buf.range_width(0, self.cursor) + w) / w;

        if !move_cursor_to_end_of_line || self.no_eol {
            // The term cursor is now on the bottom line. We may need to move the term cursor up
            // to the line where the true cursor is.
            let cursor_line_diff = new_num_lines as isize - self.term_cursor_line as isize;
            if cursor_line_diff > 0 {
                try!(write!(self.out, "{}", cursor::Up(cursor_line_diff as u16)));
            } else if cursor_line_diff < 0 {
                unreachable!();
            }

            // Now that we are on the right line, we must move the term cursor left or right
            // to match the true cursor.
            let cursor_col_diff = buf_width as isize - buf.range_width(0, self.cursor) as isize -
                                  cursor_line_diff * w as isize;
            if cursor_col_diff > 0 {
                try!(write!(self.out, "{}", cursor::Left(cursor_col_diff as u16)));
            } else if cursor_col_diff < 0 {
                try!(write!(self.out, "{}", cursor::Right((-cursor_col_diff) as u16)));
            }
        }

        self.out.flush()
    }
}

impl<'a, W: Write> From<Editor<'a, W>> for String {
    fn from(ed: Editor<'a, W>) -> String {
        match ed.cur_history_loc {
                Some(i) => ed.context.history[i].clone(),
                _ => ed.new_buf,
            }
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Context;

    #[test]
    /// test undoing delete_all_after_cursor
    fn delete_all_after_cursor_undo() {
        let mut context = Context::new();
        let out = Vec::new();
        let mut ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        ed.insert_str_after_cursor("delete all of this").unwrap();
        ed.move_cursor_to_start_of_line().unwrap();
        ed.delete_all_after_cursor().unwrap();
        ed.undo().unwrap();
        assert_eq!(String::from(ed), "delete all of this");
    }

    #[test]
    fn move_cursor_left() {
        let mut context = Context::new();
        let out = Vec::new();
        let mut ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        ed.insert_str_after_cursor("let").unwrap();
        assert_eq!(ed.cursor, 3);

        ed.move_cursor_left(1).unwrap();
        assert_eq!(ed.cursor, 2);

        ed.insert_after_cursor('f').unwrap();
        assert_eq!(ed.cursor, 3);
        assert_eq!(String::from(ed), "left");
    }

    #[test]
    fn cursor_movement() {
        let mut context = Context::new();
        let out = Vec::new();
        let mut ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        ed.insert_str_after_cursor("right").unwrap();
        assert_eq!(ed.cursor, 5);

        ed.move_cursor_left(2).unwrap();
        ed.move_cursor_right(1).unwrap();
        assert_eq!(ed.cursor, 4);
    }
}
