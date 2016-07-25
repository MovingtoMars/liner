use unicode_width::UnicodeWidthStr;
use std::io::{self, Write};
use std::iter::FromIterator;
use std::fmt::{self, Write as FmtWrite};

#[derive(Debug,Clone)]
pub enum Action {
    Insert {
        start: usize,
        text: Vec<char>,
    },
    Remove {
        start: usize,
        text: Vec<char>,
    },
}

impl Action {
    pub fn do_on(&self, buf: &mut Buffer) {
        match *self {
            Action::Insert { start, ref text } => buf.insert_raw(start, &text[..]),
            Action::Remove { start, ref text } => {
                buf.remove_raw(start, start + text.len());
            }
        }
    }

    pub fn undo(&self, buf: &mut Buffer) {
        match *self {
            Action::Insert { start, ref text } => {
                buf.remove_raw(start, start + text.len());
            }
            Action::Remove { start, ref text } => buf.insert_raw(start, &text[..]),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Buffer {
    data: Vec<char>,
    actions: Vec<Action>,
    undone_actions: Vec<Action>,
}

impl From<Buffer> for String {
    fn from(buf: Buffer) -> Self {
        String::from_iter(buf.data)
    }
}

impl From<String> for Buffer {
    fn from(s: String) -> Self {
        Buffer::from_iter(s.chars())
    }
}

impl<'a> From<&'a str> for Buffer {
    fn from(s: &'a str) -> Self {
        Buffer::from_iter(s.chars())
    }
}

impl fmt::Display for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for &c in &self.data {
            try!(f.write_char(c));
        }
        Ok(())
    }
}

impl FromIterator<char> for Buffer {
    fn from_iter<T: IntoIterator<Item = char>>(t: T) -> Self {
        Buffer {
            data: t.into_iter().collect(),
            actions: Vec::new(),
            undone_actions: Vec::new(),
        }
    }
}

impl Buffer {
    pub fn new() -> Self {
        Buffer {
            data: Vec::new(),
            actions: Vec::new(),
            undone_actions: Vec::new(),
        }
    }

    pub fn clear_actions(&mut self) {
        self.actions.clear();
        self.undone_actions.clear();
    }

    pub fn undo(&mut self) -> bool {
        match self.actions.pop() {
            None => false,
            Some(act) => {
                act.undo(self);
                self.undone_actions.push(act);
                true
            }
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.undone_actions.pop() {
            None => false,
            Some(act) => {
                act.do_on(self);
                self.actions.push(act);
                true
            }
        }
    }

    pub fn revert(&mut self) -> bool {
        if self.actions.len() == 0 {
            return false;
        }

        while self.undo() {}
        true
    }

    fn push_action(&mut self, act: Action) {
        self.actions.push(act);
        self.undone_actions.clear();
    }

    pub fn num_chars(&self) -> usize {
        self.data.len()
    }

    pub fn num_bytes(&self) -> usize {
        let s: String = self.clone().into();
        s.len()
    }

    pub fn width(&self) -> usize {
        self.range_width(0, self.num_chars())
    }

    pub fn char_before(&self, cursor: usize) -> Option<char> {
        if cursor == 0 {
            None
        } else {
            self.data.get(cursor - 1).cloned()
        }
    }

    pub fn char_after(&self, cursor: usize) -> Option<char> {
        self.data.get(cursor).cloned()
    }

    /// Returns the number of characters removed.
    pub fn remove(&mut self, start: usize, end: usize) -> usize {
        let s = self.remove_raw(start, end);
        let num_removed = s.len();
        let act = Action::Remove {
            start: start,
            text: s,
        };
        self.push_action(act);
        num_removed
    }

    pub fn insert(&mut self, start: usize, text: &[char]) {
        let act = Action::Insert {
            start: start,
            text: text.into(),
        };
        act.do_on(self);
        self.push_action(act);
    }

    pub fn range(&self, start: usize, end: usize) -> String {
        self.data[start..end].iter().cloned().collect()
    }

    pub fn range_chars(&self, start: usize, end: usize) -> Vec<char> {
        self.data[start..end].iter().cloned().collect()
    }

    pub fn range_width(&self, start: usize, end: usize) -> usize {
        self.range(start, end)[..].width()
    }

    pub fn chars(&self) -> ::std::slice::Iter<char> {
        self.data.iter()
    }

    pub fn truncate(&mut self, num: usize) {
        self.data.truncate(num);
    }

    pub fn print<W>(&self, out: &mut W) -> io::Result<()>
        where W: Write
    {
        let string: String = self.data.iter().cloned().collect();
        try!(out.write(string.as_bytes()));

        Ok(())
    }

    fn remove_raw(&mut self, start: usize, end: usize) -> Vec<char> {
        self.data.drain(start..end).collect()
    }

    fn insert_raw(&mut self, start: usize, text: &[char]) {
        for (i, &c) in text.iter().enumerate() {
            self.data.insert(start + i, c)
        }
    }
}
