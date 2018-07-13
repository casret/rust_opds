# Rust OPDS

A project for me to learn more about Rust.  It takes a directory of
comics and will serve an OPDS feed of them.  Works much better if
the comics are tagged by something like [comictagger](https://github.com/davide-romanini/comictagger).

This is not intended to ever be a fully featured comic management or reading app.  It's intended to be
a low resource server to be used with a reader that supports OPDS.

It's in alpha, so feel free to give it a shot and tell me how it goes in the issues.  
You can check out the TODO list if you want to follow along
with my work list or pop something into the issues.  In particular I haven't tried this on windows.

## Building

Install [Rust](https://www.rust-lang.org/en-US/install.html)

```bash
cargo build --release
```

This will build 2 binaries in target/release, rust_opds and import_comicrack.  Right now
it only builds on MacOS, because it's using the bundled version of sqlite on my branch.  But you
can build against stock rusqlite as long as you are linking against SQLite 3.24.0.

## Running

First edit your config.toml to specify where you are putting your comics, and if you want to change where
the sqlite db goes.  Then fire it up:

```bash
RUST_LOG=info target/release/rust_opds
```

Let it scan through your comics, and then hit the server http://localhost:6737 (use a browser for this first step).
Usernames are self provisioning, whatever password you put in the first time is what you need to use going
forward.  Right now it's only used to track the read status for each comic.  Now you should be able to use your
favorite OPDS client to read comics.

If you use comicrack you can pull in those read statuses and metadata using the import_comicrack binary.  First 
configure the read_user that you just created in the config.toml.  You'll have to find your ComicDB.xml file and then:

```bash
target/release/import_comicrack ComicDb.xml
```

Right now it's intended to be run once, but if there is sufficient interest I can make bi-directional.
