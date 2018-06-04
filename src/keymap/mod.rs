use std::io::{self, Write, ErrorKind};
use termion::event::Key;
use Editor;
use event::*;

pub trait KeyMap<'a, W: Write, T>: From<T> {
    fn handle_key_core(&mut self, key: Key) -> io::Result<()>;
    fn editor(&self) -> &Editor<'a, W>;
    fn editor_mut(&mut self) -> &mut Editor<'a, W>;

    fn handle_key(&mut self, mut key: Key, handler: &mut EventHandler<W>) -> io::Result<bool> {
        let mut done = false;

        handler(Event::new(self.editor_mut(), EventKind::BeforeKey(key)));

        let is_empty = self.editor().current_buffer().is_empty();

        if key == Key::Ctrl('h') {
            // XXX: Might need to change this when remappable keybindings are added.
            key = Key::Backspace;
        }

        match key {
            Key::Ctrl('c') => {
                try!(self.editor_mut().handle_newline());
                return Err(io::Error::new(ErrorKind::Interrupted, "ctrl-c"));
            }
            // if the current buffer is empty, treat ctrl-d as eof
            Key::Ctrl('d') if is_empty => {
                try!(self.editor_mut().handle_newline());
                return Err(io::Error::new(ErrorKind::UnexpectedEof, "ctrl-d"));
            }
            Key::Char('\t') => try!(self.editor_mut().complete(handler)),
            Key::Char('\n') => {
                done = try!(self.editor_mut().handle_newline());
            }
            Key::Ctrl('f') if self.editor().is_currently_showing_autosuggestion() => {
                try!(self.editor_mut().accept_autosuggestion());
            }
            Key::Right if self.editor().is_currently_showing_autosuggestion() &&
                          self.editor().cursor_is_at_end_of_line() => {
                try!(self.editor_mut().accept_autosuggestion());
            }
            _ => {
                try!(self.handle_key_core(key));
                self.editor_mut().skip_completions_hint();
            }
        };

        handler(Event::new(self.editor_mut(), EventKind::AfterKey(key)));

        try!(self.editor_mut().flush());

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

        fn editor_mut(&mut self) ->  &mut Editor<'a, W> {
            &mut self.ed
        }

        fn editor(&self) ->  &Editor<'a, W> {
            &self.ed
        }
    }

    #[test]
    /// when the current buffer is empty, ctrl-d generates and eof error
    fn ctrl_d_empty() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), |s| {String::from(s)}, &mut context).unwrap();
        let mut map = TestKeyMap::new(ed);

        let res = map.handle_key(Ctrl('d'), &mut |_| {});
        assert_eq!(res.is_err(), true);
        assert_eq!(res.err().unwrap().kind(), ErrorKind::UnexpectedEof);
    }

    #[test]
    /// when the current buffer is not empty, ctrl-d should be ignored
    fn ctrl_d_non_empty() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), |s| {String::from(s)}, &mut context).unwrap();
        let mut map = TestKeyMap::new(ed);
        map.ed.insert_str_after_cursor("not empty").unwrap();

        let res = map.handle_key(Ctrl('d'), &mut |_| {});
        assert_eq!(res.is_ok(), true);
    }

    #[test]
    /// ctrl-c should generate an error
    fn ctrl_c() {
        let mut context = Context::new();
        let out = Vec::new();
        let ed = Editor::new(out, "prompt".to_owned(), |s| {String::from(s)}, &mut context).unwrap();
        let mut map = TestKeyMap::new(ed);

        let res = map.handle_key(Ctrl('c'), &mut |_| {});
        assert_eq!(res.is_err(), true);
        assert_eq!(res.err().unwrap().kind(), ErrorKind::Interrupted);
    }
}
