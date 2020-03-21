extern crate log;
extern crate sys_info;

pub mod launch_options;
pub mod file_helper;
pub mod file_logger;

use std::path::PathBuf;
use std::env;
use file_helper::find_from_path;

fn main() {
    let mut options = launch_options::new(env::args().collect()).ok().unwrap();

    if options.nailgun_client {
        options.prepend_program_arg("org.jruby.util.NailMain");
        // FIXME: Add nailgun client support?
    }

    let java = if let Ok(cmd) = env::var("JAVACMD") {
        Some(PathBuf::from(cmd))
    } else if options.jdk_home.is_some() {
        Some(PathBuf::from(options.jdk_home.unwrap()).join("bin").join("java"))
    } else if let Ok(home) = env::var("JAVA_HOME") {
        Some(PathBuf::from(home).join("bin").join("java"))
    } else {
        find_from_path("java")
    };

    if java.is_none() {
        println!("No `java' executable found");
        return ();
    }

    println!("JAVA IS {:?}" , java)
}




