extern crate rust_opds;

extern crate failure;

use failure::Error;

fn main() -> Result<(), Error> {
    rust_opds::scan_dir("/Users/casret/comics")
}
