use std::io::{self, Write, ErrorKind};
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
            Key::Ctrl('c') => {
                try!(self.editor().handle_newline());
                return Err(io::Error::new(ErrorKind::Interrupted, "ctrl-c"));
            }
            Key::Char('\t') => try!(self.editor().complete(handler)),
            Key::Char('\n') => {
                try!(self.editor().handle_newline());
                done = true;
            }
            Key::Ctrl('f') => {
                try!(self.editor().accept_autosuggestion());
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

pub mod vi;
pub use vi::Vi;

pub mod emacs;
pub use emacs::Emacs;

#[cfg(test)]
mod tests {
    use super::*;
    use termion::event::Key::*;
    use std::io::ErrorKind;
    use Context;

    struct TestKeyMap<'a, W: Write> {
        ed: Editor<'a, W>,
    }

    impl<'a, W: Write> TestKeyMap<'a, W> {
        pub fn new(ed: Editor<'a, W>) -> Self {
            TestKeyMap {
                ed: ed,
            }
        }
    }

    impl<'a, W: Write> KeyMap<'a, W, TestKeyMap<'a, W>> for TestKeyMap<'a, W> {
        fn handle_key_core(&mut self, _: Key) -> io::Result<()> {
            Ok(())
        }

        fn editor(&mut self) ->  &mut Editor<'a, W> {
            &mut self.ed
        }
    }

    #[test]
    /// ctrl-c should generate an error
    fn ctrl_c() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), &mut context).unwrap();
        let mut map = TestKeyMap::new(ed);

        let res = map.handle_key(Ctrl('c'), &mut |_| {});
        assert_eq!(res.is_err(), true);
        assert_eq!(res.err().unwrap().kind(), ErrorKind::Interrupted);
    }
}
