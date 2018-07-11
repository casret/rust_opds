# Rust OPDS

A project for me to learn more about Rust.  It takes a directory of
comics and will serve an OPDS feed of them on port 3000.  Works much better if
the comics are tagged by something like [comictagger](https://github.com/davide-romanini/comictagger).

This is not intended to ever be a fully featured comic management or reading app.  It's intended to be
a low resource server to be used with a reader that supports OPDS.

It's in alpha, so feel free to give it a shot and tell me how it goes in the issues.  
You can check out the TODO list if you want to follow along
with my work list or pop something into the issues.  I'm not yet distributing binaries so you'll have to
install rust (stable should be fine), edit the config.toml and do a cargo run.

Username and passwords are self provisioning, whatever you put in the first time is what you need to use going
forward.  It's meant to seperate the read tags.
