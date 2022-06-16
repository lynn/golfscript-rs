# golfscript-rs

This is a [GolfScript](http://www.golfscript.com/golfscript/) interpreter in Rust (WIP).

It's about 20× faster than the Ruby interpreter, and currently compatible with about 97% of the GolfScript solutions on [anarchy golf](http://golf.shinh.org/).

Try `cargo run -- --code code --input input`, e.g. `cargo run -- --code '~]{+}*' --input '1 2 3 4'`

