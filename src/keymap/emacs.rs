use std::io::{self, Write};
use termion::event::Key;

use KeyMap;
use Editor;

pub struct Emacs<'a, W: Write> {
    ed: Editor<'a, W>,
}

impl<'a, W: Write> Emacs<'a, W> {
    pub fn new(ed: Editor<'a, W>) -> Self {
        Emacs {
            ed: ed,
        }
    }
}

impl<'a, W: Write> KeyMap<'a, W, Emacs<'a, W>> for Emacs<'a, W> {
    fn handle_key_core(&mut self, key: Key) -> io::Result<()> {
        self.ed.handle_key_emacs(key)
    }

    fn editor(&mut self) ->  &mut Editor<'a, W> {
        &mut self.ed
    }
}

impl<'a, W: Write> From<Emacs<'a, W>> for String {
    fn from(emacs: Emacs<'a, W>) -> String {
        emacs.ed.into()
    }
}
