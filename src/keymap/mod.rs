use std::io::{self, Write};
use termion::event::Key;
use Editor;
use event::EventHandler;

pub trait KeyMap<'a, W: Write, T> : From<T> {
    fn handle_key(&mut self, key: Key, handler: &mut EventHandler<W>) -> io::Result<bool>;
    fn editor(&mut self) -> &mut Editor<'a, W>;
}

pub mod emacs;
pub use emacs::Emacs;
