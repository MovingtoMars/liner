use std::borrow::Cow;
use unicode_width::*;

pub fn width<S: AsRef<str>>(s: S) -> usize {
    remove_codes(s.as_ref()).width()
}

pub fn find_longest_common_prefix<T: Clone + Eq>(among: &[Vec<T>]) -> Option<Vec<T>> {
    if among.len() == 0 {
        return None;
    } else if among.len() == 1 {
        return Some(among[0].clone());
    }

    for s in among {
        if s.len() == 0 {
            return None;
        }
    }

    let shortest_word = among.iter().min_by_key(|x| x.len()).unwrap();

    let mut end = shortest_word.len();
    while end > 0 {
        let prefix = &shortest_word[..end];

        let mut failed = false;
        for s in among {
            if !s.starts_with(prefix) {
                failed = true;
                break;
            }
        }

        if !failed {
            return Some(prefix.into());
        }

        end -= 1;
    }

    None
}

pub enum AnsiState {
    Norm,
    Esc,
    Csi,
    Osc,
}

pub fn remove_codes(input: &str) -> Cow<str> {
    if input.contains('\x1B') {
        let mut clean = String::new();

        let mut s = AnsiState::Norm;
        for c in input.chars() {
            match s {
                AnsiState::Norm => match c {
                    '\x1B' => s = AnsiState::Esc,
                    _ => clean.push(c),
                },
                AnsiState::Esc => match c {
                    '[' => s = AnsiState::Csi,
                    ']' => s = AnsiState::Osc,
                    _ => s = AnsiState::Norm,
                },
                AnsiState::Csi => match c {
                    'A' ... 'Z' | 'a' ... 'z' => s = AnsiState::Norm,
                    _ => (),
                },
                AnsiState::Osc => match c {
                    '\x07' => s = AnsiState::Norm,
                    _ => (),
                }
            }
        }

        Cow::Owned(clean)
    } else {
        Cow::Borrowed(input)
    }
}
