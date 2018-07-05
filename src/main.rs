extern crate env_logger;
extern crate failure;
extern crate rust_opds;

use failure::Error;

fn main() -> Result<(), Error> {
    env_logger::init();
    rust_opds::run()?;
    Ok(())
}
