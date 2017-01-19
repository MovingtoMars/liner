use std::io::{self, Write};
use termion::event::Key;
use Editor;
use event::*;

pub trait KeyMap<'a, W: Write, T>: From<T> {
    fn handle_key_core(&mut self, key: Key) -> io::Result<()>;
    fn editor(&mut self) -> &mut Editor<'a, W>;

    fn handle_key(&mut self, key: Key, handler: &mut EventHandler<W>) -> io::Result<bool> {
        let mut done = false;

        handler(Event::new(self.editor(), EventKind::BeforeKey(key)));

        match key {
            Key::Char('\t') => try!(self.editor().complete(handler)),
            Key::Char('\n') => {
                try!(self.editor().handle_newline());
                done = true;
            }
            _ => {
                try!(self.handle_key_core(key));
                self.editor().skip_completions_hint();
            }
        };

        handler(Event::new(self.editor(), EventKind::AfterKey(key)));

        try!(self.editor().flush());

        Ok(done)
    }
}

pub mod emacs;
pub use emacs::Emacs;
