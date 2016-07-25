use std::io::{self, Stdout, stdout, stdin};
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};

use super::*;

/// The default for `Context.word_fn`.
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

pub struct Context {
    pub history: History,
    pub completer: Option<Box<Completer>>,
    pub word_fn: Box<Fn(&Buffer) -> Vec<(usize, usize)>>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            history: History::new(),
            completer: None,
            word_fn: Box::new(get_buffer_words),
        }
    }

    /// Creates an `Editor` and feeds it keypresses from stdin until the line is entered.
    /// The output is stdout.
    /// The returned line has the newline removed.
    /// Before returning, will revert all changes to the history buffers.
    pub fn read_line<P: Into<String>>(&mut self,
                                      prompt: P,
                                      mut handler: &mut EventHandler<RawTerminal<Stdout>>)
                                      -> io::Result<String> {
        let res = {
            let stdin = stdin();

            let stdout = stdout().into_raw_mode().unwrap();
            let mut ed = try!(Editor::new(stdout, prompt.into(), self));

            for c in stdin.keys() {
                if try!(ed.handle_key(c.unwrap(), handler)) {
                    break;
                }
            }

            Ok(ed.into())
        };

        self.revert_all_history();
        res
    }

    pub fn revert_all_history(&mut self) {
        for buf in &mut self.history.buffers {
            buf.revert();
        }
    }
}
