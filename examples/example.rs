extern crate liner;

use liner::Context;
use liner::KeyBindings;
use std::io::ErrorKind;

fn main() {
    let mut con = Context::new();

    loop {
        let res = con.read_line("[prompt]$ ", &mut |_| {});

        match res {
            Ok(res) => {
                match res.as_str() {
                    "emacs" => {
                        con.key_bindings = KeyBindings::Emacs;
                        println!("emacs mode");
                    }
                    "vi" =>  {
                        con.key_bindings = KeyBindings::Vi;
                        println!("vi mode");
                    }
                    "exit" =>  {
                        println!("exiting...");
                        break;
                    }
                    _ => {}
                }

                con.history.push(res.into()).unwrap();
            }
            Err(e) => {
                match e.kind() {
                    // ctrl-c pressed
                    ErrorKind::Interrupted => {}
                    // ctrl-d pressed
                    ErrorKind::UnexpectedEof => {
                        println!("exiting...");
                        break;
                    }
                    _ => panic!("error: {:?}", e),
                }
            }
        }
    }
}
