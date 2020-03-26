extern crate log;
extern crate sys_info;

pub mod launch_options;
pub mod file_helper;
pub mod file_logger;

use std::env;

fn main() {
    let options = launch_options::new(env::args().collect());

    if options.is_err() {
        println!("Problem parsing options: {:?}", options);
        panic!("Whoops");
    }

    let mut options = options.unwrap();
    if options.nailgun_client {
        options.prepend_program_arg("org.jruby.util.NailMain");
        // FIXME: Add nailgun client support?
    }
}




