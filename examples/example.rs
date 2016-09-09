extern crate liner;

use liner::Context;

fn main() {
    let mut con = Context::new();

    loop {
        let res = con.read_line("[prompt]$ ", &mut |_| {}).unwrap();

        if res.is_empty() {
            break;
        }

        con.history.push(res.into()).unwrap();
    }
}
