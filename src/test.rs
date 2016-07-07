use super::*;
use context;

fn assert_cursor_pos(s: &str, cursor: usize, expected_pos: CursorPosition) {
    let buf = Buffer::from(s.to_owned());
    let words = context::get_buffer_words(&buf);
    let pos = CursorPosition::get(cursor, &words[..]);
    assert!(expected_pos == pos,
            format!("buffer: {:?}, cursor: {}, expected pos: {:?}, pos: {:?}",
                    s,
                    cursor,
                    expected_pos,
                    pos));
}

#[test]
fn test_get_cursor_position() {
    use CursorPosition::*;

    let tests = &[("hi", 0, OnWordLeftEdge(0)),
                  ("hi", 1, InWord(0)),
                  ("hi", 2, OnWordRightEdge(0)),
                  ("abc  abc", 4, InSpace(Some(0), Some(1))),
                  ("abc  abc", 5, OnWordLeftEdge(1)),
                  ("abc  abc", 6, InWord(1)),
                  ("abc  abc", 8, OnWordRightEdge(1)),
                  (" a", 0, InSpace(None, Some(0))),
                  ("a ", 2, InSpace(Some(0), None))];

    for t in tests {
        assert_cursor_pos(t.0, t.1, t.2);
    }
}

fn assert_buffer_actions(start: &str, expected: &str, actions: &[Action]) {
    let mut buf = Buffer::from(start.to_owned());
    for a in actions {
        a.do_on(&mut buf);
    }

    assert_eq!(expected, String::from(buf));
}

#[test]
fn test_buffer_actions() {
    assert_buffer_actions("",
                          "h",
                          &[Action::Insert {
                                start: 0,
                                text: "hi".chars().collect(),
                            },
                            Action::Remove {
                                start: 1,
                                text: ".".chars().collect(),
                            }]);
}
