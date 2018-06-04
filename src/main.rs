extern crate liner;
extern crate termion;
extern crate regex;

use std::mem::replace;
use std::borrow::Cow;
use std::env::{args, current_dir};
use std::io;

use liner::{Context, CursorPosition, Event, EventKind, FilenameCompleter};
use termion::color;
use regex::Regex;

fn highlight_dodo(s: &'static str) -> Cow<'a, str> {
    let reg_exp = Regex::new("(?P<k>dodo)").unwrap();
    let format = format!("{}$k{}", color::Fg(color::Red), color::Fg(color::Reset));
    reg_exp.replace_all(s, format.as_str()).into()
}

fn main<'a>() {
    let mut con = Context::new();

    let history_file = args().nth(1);
    match history_file {
        Some(ref file_name) => println!("History file: {}", file_name),
        None => println!("No history file"),
    }

    con.history.set_file_name(history_file);
    if con.history.file_name().is_some() {
        con.history.load_history().unwrap();
    }

    let h = |s: &str| highlight_dodo(s);

    loop {
        let res = con.read_line("[prompt]$ ",
                                highlight_dodo,
                                &mut |Event { editor, kind }| {
            if let EventKind::BeforeComplete = kind {
                let (_, pos) = editor.get_words_and_cursor_position();

                // Figure out of we are completing a command (the first word) or a filename.
                let filename = match pos {
                    CursorPosition::InWord(i) => i > 0,
                    CursorPosition::InSpace(Some(_), _) => true,
                    CursorPosition::InSpace(None, _) => false,
                    CursorPosition::OnWordLeftEdge(i) => i >= 1,
                    CursorPosition::OnWordRightEdge(i) => i >= 1,
                };

                if filename {
                    let completer = FilenameCompleter::new(Some(current_dir().unwrap()));
                    replace(&mut editor.context().completer, Some(Box::new(completer)));
                } else {
                    replace(&mut editor.context().completer, None);
                }
            }
        });

        match res {
            Ok(res) => {
                match res.as_str() {
                    "emacs" => {
                        con.key_bindings = liner::KeyBindings::Emacs;
                        println!("emacs mode");
                    }
                    "vi" => {
                        con.key_bindings = liner::KeyBindings::Vi;
                        println!("vi mode");
                    }
                    "exit" | "" => {
                        println!("exiting...");
                        break;
                    }
                    _ => {}
                }

                if res.is_empty() {
                    break;
                }

                con.history.push(res.into()).unwrap();
            }
            Err(e) => {
                match e.kind() {
                    // ctrl-c pressed
                    io::ErrorKind::Interrupted => {}
                    // ctrl-d pressed
                    io::ErrorKind::UnexpectedEof => {
                        println!("exiting...");
                        break;
                    }
                    _ => {
                        // Ensure that all writes to the history file
                        // are written before exiting.
                        con.history.commit_history();
                        panic!("error: {:?}", e)
                    },
                }
            }
        }

    }

    // Ensure that all writes to the history file are written before exiting.
    con.history.commit_history();
}