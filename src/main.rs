extern crate log;
extern crate sys_info;

pub mod environment;
pub mod file_helper;
pub mod file_logger;
pub mod launch_options;

use std::env;
use std::error::Error;
use std::io::{stderr, Write};

fn print_error(err: Box<dyn Error>) {
    let mut err = err.as_ref();
    let _ = writeln!(stderr(), "error: {}", err);
    while let Some(cause) = err.source() {
        let _ = writeln!(stderr(), "caused by: {}", cause);
        err = cause;
    }
}

#[cfg(target_os = "windows")]
fn execute(command: String, args: Vec<String>) {
    use std::process::Command;

    // Old launcher spawns command suspended and then disbles parents control-c then resumes after that point
    Command::new(command)
        .args(args)
        .spawn().unwrap()
        .wait().expect("The child to end");
}

#[cfg(not(target_os = "windows"))]
fn execute(command: String,  args: Vec<String>) {
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
    let options = launch_options::new(env::args().collect());

    if let Err(err) = options {
        print_error(err);
        std::process::exit(1);
    }

    let mut options = options.unwrap();
    if options.nailgun_client {
        options.program_args.insert(0, "org.jruby.util.NailMain".to_string());
        if options.command_only {
            println!("{:?}", options.program_args);
        } else {
            execute(options.java_location.clone().unwrap().to_str().unwrap().to_string(), options.command_line());
        }
    } else {
        if options.command_only {
            println!("{:?} {:?}", &options.java_location, options.command_line());
        } else {
            execute(options.java_location.clone().unwrap().to_str().unwrap().to_string(), options.command_line());
        }
    }
}
