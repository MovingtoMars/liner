use std::io::Write;
use termion::event::Key;
use Editor;

pub type EventHandler<'a, W> = FnMut(Event<W>) + 'a;

pub struct Event<'a, 'out: 'a, W: Write + 'a> {
    pub editor: &'a mut Editor<'out, W>,
    pub kind: EventKind,
}

impl<'a, 'out: 'a, W: Write + 'a> Event<'a, 'out, W> {
    pub fn new(editor: &'a mut Editor<'out, W>, kind: EventKind) -> Self {
        Event {
            editor: editor,
            kind: kind,
        }
    }
}

#[derive(Debug)]
pub enum EventKind {
    /// Sent before handling a keypress.
    BeforeKey(Key),
    /// Sent after handling a keypress.
    AfterKey(Key),
    /// Sent in `Editor.complete()`, before processing the completion.
    BeforeComplete,
}
