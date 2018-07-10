extern crate env_logger;
extern crate failure;
extern crate rust_opds;
extern crate toml;

use failure::Error;
use rust_opds::Config;
use std::fs::File;
use std::io::Read;

fn main() -> Result<(), Error> {
    let mut file = File::open("config.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let config: Config = toml::from_str(&contents)?;
    print!("{:?}", config);
    env_logger::init();
    rust_opds::run(config)?;
    Ok(())
}
