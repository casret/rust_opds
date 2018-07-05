extern crate failure;
extern crate rust_opds;
extern crate env_logger;

use failure::Error;

fn main() -> Result<(), Error> {
    env_logger::init();
    rust_opds::run()?;
    Ok(())
}
