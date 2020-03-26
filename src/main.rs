extern crate log;
extern crate sys_info;

pub mod launch_options;
pub mod file_helper;
pub mod file_logger;

use std::env;
use std::error::Error;
use std::io::{stderr, Write};

fn print_error(mut err: &dyn Error) {
    let _ = writeln!(stderr(), "error: {}", err);
    while let Some(cause) = err.source() {
        let _ = writeln!(stderr(), "caused by: {}", cause);
        err = cause;
    }
}

fn main() {
    let options = launch_options::new(env::args().collect());

    if let Err(err) = options {
        print_error(&err);
        std::process::exit(1);
    }

    let mut options = options.unwrap();
    if options.nailgun_client {
        options.prepend_program_arg("org.jruby.util.NailMain");
        // FIXME: Add nailgun client support?
    }
}




