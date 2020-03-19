extern crate log;
extern crate uname;

use std::path::{PathBuf, Path};
use std::fs;
use std::fs::{File, read_link};
use log::{info, warn, error, Metadata, Level, Record, SetLoggerError, LevelFilter};
use std::io::Write;
use std::sync::Mutex;
use std::env;
use std::env::split_paths;
use libc::{access, R_OK};
use std::os::unix::ffi::OsStrExt;
use std::ffi::{CString};
use std::borrow::Borrow;

struct FileLogger {
    file: Mutex<File>,
}

impl FileLogger {
    pub fn init(path: &Path) -> Result<(), SetLoggerError> {
        if path.exists() {
            fs::remove_file(path).expect("Could not remove old log file");
        }
        let logger = FileLogger { file: Mutex::new(File::create(path).unwrap()) };
        log::set_boxed_logger(Box::new(logger))
            .map(|()| log::set_max_level(LevelFilter::Info))
    }
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            write!(self.file.lock().unwrap(), "{}: {}", record.level(), record.args()).expect("Could not write to log file");
        }
    }

    fn flush(&self) {}
}

#[derive(Debug)]
struct LaunchOptions {
    fork_java: bool,
    command_only: bool,
    no_boot_classpath: bool,
    nailgun_client: bool,
    launcher_logfile: Option<PathBuf>,
    boot_class: Option<String>,
    jdk_home: Option<PathBuf>,
    classpath_before: Vec<PathBuf>,
    classpath_after: Vec<PathBuf>,
    classpath: Vec<PathBuf>,
    java_args: Vec<String>, // Note: some other fields will also eventually be java args in final command-line.
    program_args: Vec<String>,
    java_opts: Vec<String>,
    jruby_opts: Vec<String>,
    platform_dir: Option<PathBuf>,
    argv0: String,
}

impl LaunchOptions {
    fn push_java_arg(&mut self, value: &str) {
        self.java_args.push(value.to_string());
    }

    fn prepend_program_arg(&mut self, value: &str) {
        self.program_args.insert(0, value.to_string());
    }

    fn push_program_arg(&mut self, value: &str) {
        self.program_args.push(value.to_string());
    }

    fn push_classpath_before(&mut self, value: String) {
        self.classpath_before.push(PathBuf::from(value));
    }

    fn push_classpath_after(&mut self, value: String) {
        self.classpath_after.push(PathBuf::from(value));
    }

    fn push_jruby_opts_arg(&mut self, value: String) {
        self.jruby_opts.push(value.to_string());
    }

    fn push_java_opts_arg(&mut self, value: String) {
        self.java_opts.push(value.to_string());
    }

}

impl Default for LaunchOptions {
    fn default() -> LaunchOptions {
        LaunchOptions {
            fork_java: false,
            command_only: false,
            no_boot_classpath: false,
            nailgun_client: false,
            launcher_logfile: None,
            boot_class: None,
            jdk_home: None,
            classpath_before: vec![],
            classpath_after: vec![],
            classpath: vec![],
            java_args: vec![],
            program_args: vec![],
            java_opts: vec![],
            jruby_opts: vec![],
            platform_dir: None,
            argv0: "".to_string(),
        }
    }
}

macro_rules! arg_value {
    ($args:expr) => {{
        if $args.peek().is_some() {
            $args.next().to_owned()
        } else {
            return Err(());
        }
    }};
}

fn find_from_path(file: &str) -> Option<PathBuf> {

    if let Ok(paths) = env::var("PATH") {
        for path in split_paths(paths.as_str()) {
            let test = path.join(file);

            if test.exists() {
                return Some(test)
            }
        }
    }

    None
}

fn main() {
    let mut options = parse(env::args().collect()).ok().unwrap();

    if options.launcher_logfile.is_some() {
        FileLogger::init(options.launcher_logfile.as_ref().unwrap()).expect("Unable to initialize logger");
    }

    init_platform_dir(&mut options);

    if options.nailgun_client {
        options.prepend_program_arg("org.jruby.util.NailMain");
        // FIXME: Add nailgun client support?
    }

    prepare_options(&options);

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

fn env_as_iter(value: String) -> Vec<String> {
    // FIXME: Some off quote removal but only for first/last char of string
    value.split_ascii_whitespace().map(|a| a.to_string()).collect()
}

// Force OpenJDK-based JVMs to use /dev/urandom for random number generation
// See https://github.com/jruby/jruby/issues/4685 among others.
fn check_urandom(options: &mut LaunchOptions) {
    unsafe {
        let path = CString::new(Path::new("/dev/urandom").as_os_str().as_bytes()).unwrap();

        // OpenJDK tries really hard to prevent you from using urandom.
        // See https://bugs.openjdk.java.net/browse/JDK-6202721
        // Non-file URL causes fallback to slow threaded SeedGenerator.
        // See https://bz.apache.org/bugzilla/show_bug.cgi?id=56139
        if access(path.as_ptr() as *const i8, R_OK) == 0 {
            options.push_java_opts_arg("-Djava.security.egd=file:/dev/urandom".to_string());
        }
    }
}

#[cfg(target_os="macos")]
fn parse_os(options: &LaunchOptions) {
    if let None = env::var("JAVA_ENCODING") {
        options.push_java_opts_arg("-Dfile.encoding=UTF-8");
    }

    check_urandom(options)
}

// FIXME: Maybe make linux unix for broader coverage?

#[cfg(target_os="linux")]
fn parse_os(options: &mut LaunchOptions) {
    check_urandom(options)
}

#[cfg(target_os="windows")]
fn parse_os(options: LaunchOptions) {
    // no checks
}

fn prepare_options(options: &LaunchOptions) {
    let mut java_options: Vec<String> = vec![];

    if options.jdk_home.is_some() {
        java_options.push("-Djdk.home=".to_string() + options.jdk_home.to_owned().unwrap().to_str().unwrap());
    }

    let platform_dir= options.platform_dir.to_owned().unwrap();

    java_options.push("-Djruby.home=".to_string() + platform_dir.to_str().unwrap());
    java_options.push("-Djruby.script=jruby".to_string());

    if cfg!(target_os = "windows") {
        java_options.push("-Djruby.shell=cmd.exe".to_string());
    } else {
        java_options.push("-Djruby.shell=/bin/sh".to_string());
    }

    let mut jni_dir = platform_dir.clone();
    jni_dir.push("lib");
    jni_dir.push("jni");

    println!("JNI DIR: {:?}, {}", jni_dir, jni_dir.exists());
     if !jni_dir.exists() {
         let mut old_jni_dir = platform_dir.clone();
         old_jni_dir.push("lib");
         old_jni_dir.push("native");
         jni_dir = old_jni_dir
         // FIXME: Old launcher does not verify the old version at all.
    }

    // FIXME: I believe else path will also work on windows so no more hard-coding
    let mut ffi_option = "-Djffi.boot.library.path=".to_string();
    if cfg!(target_os = "windows") {
        ffi_option.push_str(jni_dir.to_str().unwrap());
        ffi_option.push_str(";");
        let mut path = jni_dir.clone();
        path.push("i386-Windows");
        ffi_option.push_str(path.to_str().unwrap());
        ffi_option.push_str(";");
        let mut path = jni_dir.clone();
        path.push("x86_64-Windows");
        ffi_option.push_str(path.to_str().unwrap());
    } else {
        // FIXME: old launcher adds all linux dirs as ffi dirs vs only the one which matches specific machine arch???
        let info = uname::uname().unwrap();
        let mut sysinfo = info.machine;
        sysinfo.push_str("-");
        sysinfo.push_str(info.sysname.as_str());

        println!("SYSINFO: {}", sysinfo);
        for entry in fs::read_dir(jni_dir).unwrap().into_iter() {
            let entry = entry.unwrap();

            if entry.path().to_str().unwrap().contains(&sysinfo) {
                println!("FOUND!!!!");
            }
            println!("ENTRY: {:?}", entry);
        }
    }





}

#[cfg(target_os="macos")]
fn init_platform_dir_os(options: &LaunchOptions) {
    extern "C" {
        fn _NSGetExecutablePath(buf: *mut libc::c_char, bufsize: *mut u32) -> libc::c_int;
    }
    unsafe {
        let mut size: u32 = 0;
        if _NSGetExecutablePath(ptr::null_mut(), &mut size) == 0 {
            return;
        }

        let mut size = size as usize;
        let mut buf: Vec<u8> = Vec::with_capacity(size);
        if _NSGetExecutablePath(buf.as_mut_ptr() as *mut i8, &mut size) != 0 {
            return;
        }
        buf.set_len(size - 1); // -1 since \0 is not saved in rust
        options.platform_dir = Some(PathBuf::from(OsString::from_vec(v)));
    }
}

#[cfg(target_os="linux")]
fn init_platform_dir_os(options: &mut LaunchOptions) {
    let path = read_link(Path::new("/proc/self/exe"));

    if path.is_ok() {
        options.platform_dir = path.ok();
    }
}

#[cfg(target_os="windows")]
fn init_platform_dir_os(options: &LaunchOptions) {
    //FIXME: need VirtualQuery and GetModuleFileName

}

fn init_platform_dir(options: &mut LaunchOptions) {
    let mut platform_dir: Option<PathBuf> = None;

    if let Ok(java_opts) = env::var("JRUBY_HOME") {
        let mut path = PathBuf::from(java_opts);

        path.push("bin");
        path.push("jruby");
        platform_dir = Some(path);
    }

    if platform_dir.is_none() {
        init_platform_dir_os(options);
    }

    if platform_dir.is_none() {
        let argv0 = Path::new(&options.argv0);

        if argv0.is_absolute() {
            platform_dir = Some(argv0.to_path_buf());
        } else if argv0.parent().is_some() { // relative path (will contain / or \).
            let mut path = env::current_dir().unwrap();

            path.push(argv0);
            platform_dir = Some(path.to_path_buf());
        } else {
            platform_dir = find_from_path(argv0.to_str().unwrap());
        }

        if platform_dir.is_none() { // hail mary pass in argv[0].
            platform_dir = Some(argv0.to_path_buf());
        }
    }

    if !platform_dir.as_ref().unwrap().exists() {
        error!("Platform dir '{:?}' does not exist", platform_dir)
    } else {
        info!("Platform dir '{:?}' does exist", platform_dir)
    }

    // FIXME: we should just exist when we find it and not have to exist at bottom and then wonder back up from bin/jruby...
    let parent = platform_dir.unwrap().parent().unwrap().to_path_buf().parent().unwrap().to_path_buf();
    options.platform_dir = Some(parent);
}

fn parse(args: Vec<String>) -> Result<LaunchOptions, ()> {
    let mut options = LaunchOptions::default();

    if let Ok(java_opts) = env::var("JAVA_OPTS") {
        options.java_opts.extend(env_as_iter(java_opts))
    }

    if let Ok(jruby_opts) = env::var("JRUBY_OPTS") {
        options.jruby_opts.extend(env_as_iter(jruby_opts))
    }

    parse_os(&mut options);

    if let Ok(java_mem) = env::var("JAVA_MEM") {
        options.push_java_opts_arg(java_mem)
    }

    if let Ok(java_stack) = env::var("JAVA_STACK") {
        options.push_java_opts_arg(java_stack)
    }

    let mut args = args.into_iter().peekable();

    options.argv0 = args.next().unwrap(); // exe

    while let Some(argument) = args.next() {
        println!("ARG: {}", argument);

        match argument.as_str() {
            "--" => {
                options.push_program_arg("--");
                options.program_args.extend(args);
                break;
            }
            // launcher specific -X options...
            "-Xfork-java" => options.fork_java = true,
            "-Xcommand" => options.command_only = true,
            "-Xnobootclasspath" => options.no_boot_classpath = true,
            "-Xtrace" => options.launcher_logfile = Some(PathBuf::from(arg_value!(args).unwrap())),
            "-Xbootclass" => options.boot_class = arg_value!(args),
            "-Xjdkhome" => options.jdk_home = Some(PathBuf::from(arg_value!(args).unwrap())),
            "-Xcp:p" => options.push_classpath_before(arg_value!(args).unwrap()),
            "-Xcp:a" => options.push_classpath_after(arg_value!(args).unwrap()),
            "-Xversion" => return Err(()), // FIXME: Should print out version of launcher and exit
            "-Xhelp" | "-X" => {
                // FIXME: WOT
                // print_to_console(help)
                // if options.append_help.isok puts append_help
                options.push_java_arg("-Djruby.launcher.nopreamble=true");
                options.push_program_arg("-X");
            }
            "-Xproperties" => options.program_args.push("--properties".to_string()),
            // java options we need to pass to java process itself if we see them
            "-J-cp" | "-J-classpath" => options
                .classpath
                .push(PathBuf::from(arg_value!(args).unwrap())),
            "--server" => options.push_java_arg("-server"),
            "--client" => options.push_java_arg("-client"),
            "--dev" => {
                options.push_java_arg("-XX:+TieredCompilation");
                options.push_java_arg("-XX:TieredStopAtLevel=1");
                options.push_java_arg("-Djruby.compile.mode=OFF");
                options.push_java_arg("-Djruby.compile.invokedynamic=false");
            }
            "--sample" => options.push_java_arg("-Xprof"),
            "--manage" => {
                options.push_java_arg("-Dcom.sun.management.jmxremote");
                options.push_java_arg("-Djruby.management.enabled=true")
            }
            "--headless" => options.push_java_arg("-Djava.awt.headless=true"),
            "--ng" => options.nailgun_client = true,
            "--ng-server" => {
                options.boot_class = Some("com/martiansoftware/nailgun/NGServer".to_string());
                options.push_java_arg("-server");
                options.no_boot_classpath = true;
            }
            "-Jea" => {
                options.push_java_arg("-ea");
                options.no_boot_classpath = true;
                println!("Note: -ea option is specified, there will be no bootclasspath in order to enable assertions")
            }
            _ => {
                if argument.len() > 2 {
                    let (two, rest) = argument.split_at(2);
                    match two {
                        "-X" if rest.chars().next().unwrap().is_ascii_lowercase() => {
                            let property = "-Djruby.".to_string() + rest;
                            options.push_java_arg(property.as_str())
                        }
                        "-J" => options.push_java_arg(rest),
                        _ => options.push_program_arg(argument.as_str()),
                    }
                } else {
                    options.push_program_arg(argument.as_str());
                }
            }
        }
    }

    println!("launch options = {:?}", options);

    Ok(options)
}
