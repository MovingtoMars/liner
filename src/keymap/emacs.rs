use std::io::{self, Write};
use termion::event::Key;

use KeyMap;
use Editor;
use event::EventHandler;

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
    fn handle_key(&mut self, key: Key, handler: &mut EventHandler<W>) -> io::Result<bool> {
        self.ed.handle_key(key, handler)
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
