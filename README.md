# liner
A Rust library offering readline-like functionality.

[CONTRIBUTING.md](/CONTRIBUTING.md)

[![Build Status](https://travis-ci.org/MovingtoMars/liner.svg)](https://travis-ci.org/MovingtoMars/liner)

## Basic Usage
Currently requires a nightly Rust compiler.

See `src/main.rs` for a more advanced example.

```rust
extern crate liner;

use liner::Context;

fn main() {
    let mut con = Context::new();

    loop {
        let res = con.read_line("[prompt]$ ", &mut |_| {}).unwrap();

        if res.is_empty() {
            break;
        }

        con.history.push(res.into());
    }
}
```

## License
MIT licensed. See the `LICENSE` file.
