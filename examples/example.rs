extern crate liner;

use liner::Context;
use liner::KeyBindings;

fn main() {
    let mut con = Context::new();

    loop {
        let res = con.read_line("[prompt]$ ", &mut |_| {}).unwrap();

        if res.is_empty() {
            break;
        }

        match res.as_str() {
            "emacs" => {
                con.key_bindings = KeyBindings::Emacs;
                println!("emacs mode");
            }
            "vi" =>  {
                con.key_bindings = KeyBindings::Vi;
                println!("vi mode");
            }
            _ => {}
        }

        con.history.push(res.into()).unwrap();
    }
}
