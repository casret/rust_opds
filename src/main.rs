extern crate failure;
extern crate rust_opds;

use failure::Error;

fn main() -> Result<(), Error> {
    rust_opds::run()?;
    Ok(())
}
