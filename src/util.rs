use std::borrow::Cow;

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

fn is_ascii_letter(c: char) -> bool {
    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
}

fn can_finish_code(c: char) -> bool {
    if c == 'P' {
        // dsc code
        return false;
    }

    c == '?' || is_ascii_letter(c)
}

pub fn remove_codes(input: &str) -> Cow<str> {
    if input.contains('\x1B') {
        Cow::Owned(input.split('\x1B')
            .flat_map(|x| x.chars().skip_while(|&c| !can_finish_code(c)).skip(1))
            .collect())
    } else {
        Cow::Borrowed(input)
    }
}
