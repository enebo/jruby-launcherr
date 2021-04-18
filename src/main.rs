extern crate log;
extern crate sys_info;

pub mod environment;
pub mod file_helper;
pub mod file_logger;
pub mod launch_options;
#[cfg(windows)] pub mod win_launch;
pub mod os_string_ext;

use std::env;
use std::error::Error;
use std::io::{stderr, Write};
use std::ffi::OsString;

//const IS_SIXTY_FOUR: bool = cfg!(target_pointer_width = "64");

fn print_error(err: Box<dyn Error>) {
    let mut err = err.as_ref();
    let _ = writeln!(stderr(), "error: {}", err);
    while let Some(cause) = err.source() {
        let _ = writeln!(stderr(), "caused by: {}", cause);
        err = cause;
    }
}

#[cfg(target_os = "windows")]
fn execute(command: OsString, args: Vec<OsString>) {
    use win_launch::execute_with_create_process;

    let ret_code = execute_with_create_process(command, args);
    if ret_code != 0 {
        std::process::exit(ret_code as i32);
    }
}

#[cfg(not(target_os = "windows"))]
fn execute(command: OsString,  args: Vec<OsString>) {
    use std::ffi::CString;
    use nix::unistd::execv;

    let command = CString::new(command.as_str()).unwrap();

    let cstrings: Vec<_> = args.iter()
        .map(|arg| CString::new(arg.as_str()).unwrap())
        .collect();

    let argv: Vec<_> = cstrings.iter()
        .map(|arg| arg.as_c_str())
        .collect();

    execv(command.as_c_str(), argv.as_slice()).expect("What should we do here?");
}

fn main() {
    let options = launch_options::new(env::args_os().collect());

    if let Err(err) = options {
        print_error(err);
        std::process::exit(1);
    }

    let mut options = options.unwrap();
    if options.nailgun_client {
        options.program_args.insert(0, OsString::from("org.jruby.util.NailMain"));
    }

    if options.command_only {
        println!("{:?}", options.program_args);
    } else {
        execute(options.java_location.clone().unwrap().into_os_string(), options.command_line());
    }
}
