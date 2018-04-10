use std::io::{self, stdin, stdout, Stdout, Write};
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};

use super::*;
use keymap;

/// The default for `Context.word_divider_fn`.
pub fn get_buffer_words(buf: &Buffer) -> Vec<(usize, usize)> {
    let mut res = Vec::new();

    let mut word_start = None;
    let mut just_had_backslash = false;

    for (i, &c) in buf.chars().enumerate() {
        if c == '\\' {
            just_had_backslash = true;
            continue;
        }

        if let Some(start) = word_start {
            if c == ' ' && !just_had_backslash {
                res.push((start, i));
                word_start = None;
            }
        } else {
            if c != ' ' {
                word_start = Some(i);
            }
        }

        just_had_backslash = false;
    }

    if let Some(start) = word_start {
        res.push((start, buf.num_chars()));
    }

    res
}

/// The key bindings to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyBindings {
    Vi,
    Emacs,
}

pub struct Context {
    pub history: History,
    pub completer: Option<Box<Completer>>,
    pub word_divider_fn: Box<Fn(&Buffer) -> Vec<(usize, usize)>>,
    pub key_bindings: KeyBindings,
}

impl Context {
    pub fn new() -> Self {
        Context {
            history: History::new(),
            completer: None,
            word_divider_fn: Box::new(get_buffer_words),
            key_bindings: KeyBindings::Emacs,
        }
    }

    /// Creates an `Editor` and feeds it keypresses from stdin until the line is entered.
    /// The output is stdout.
    /// The returned line has the newline removed.
    /// Before returning, will revert all changes to the history buffers.
    pub fn read_line<P: Into<String>>(
        &mut self,
        prompt: P,
        handler: &mut EventHandler<RawTerminal<Stdout>>,
    ) -> io::Result<String> {
        self.read_line_with_init_buffer(prompt, handler, Buffer::new())
    }

    /// Same as `Context.read_line()`, but passes the provided initial buffer to the editor.
    ///
    /// ```no_run
    /// use liner::Context;
    /// let mut context = Context::new();
    /// let line =
    ///     context.read_line_with_init_buffer("[prompt]$ ",
    ///                                        &mut |_| {},
    ///                                        "some initial buffer");
    /// ```
    pub fn read_line_with_init_buffer<P: Into<String>, B: Into<Buffer>>(
        &mut self,
        prompt: P,
        handler: &mut EventHandler<RawTerminal<Stdout>>,
        buffer: B,
    ) -> io::Result<String> {
        let res = {
            let stdout = stdout().into_raw_mode().unwrap();
            let ed = try!(Editor::new_with_init_buffer(stdout, prompt, self, buffer));
            match self.key_bindings {
                KeyBindings::Emacs => Self::handle_keys(keymap::Emacs::new(ed), handler),
                KeyBindings::Vi => Self::handle_keys(keymap::Vi::new(ed), handler),
            }
        };

        self.revert_all_history();
        res
    }

    fn handle_keys<'a, T, W: Write, M: KeyMap<'a, W, T>>(
        mut keymap: M,
        handler: &mut EventHandler<W>,
    ) -> io::Result<String>
    where
        String: From<M>,
    {
        let stdin = stdin();
        for c in stdin.keys() {
            if try!(keymap.handle_key(c.unwrap(), handler)) {
                break;
            }
        }

        Ok(keymap.into())
    }

    pub fn revert_all_history(&mut self) {
        for buf in &mut self.history.buffers {
            buf.revert();
        }
    }
}
