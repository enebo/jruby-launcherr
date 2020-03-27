use log::{info, error};
use std::path::{PathBuf, Path};
use std::{env, fs};
use crate::file_helper::find_from_path;
use crate::file_logger;
use core::fmt;
use std::fmt::Formatter;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct LaunchError {
    message: String,
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Launch Options Error: {}", self.message)
    }
}

impl std::error::Error for LaunchError {
    fn description(&self) -> &str {
        &self.message
    }
}

pub fn new(args: Vec<String>) -> Result<LaunchOptions, Box<dyn Error>> {
    let mut options = LaunchOptions::default();

    options.parse(args)?;

    if options.launcher_logfile.is_some() {
        options.setup_logging();
    };

    options.determine_home()?;
    options.determine_java_location()?;
    options.prepare_options()?;

    Ok(options)
}

#[derive(Debug)]
pub struct LaunchOptions {
    fork_java: bool,
    command_only: bool,
    no_boot_classpath: bool,
    pub(crate) nailgun_client: bool,
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
    java_location: Option<PathBuf>,
}

macro_rules! arg_value {
    ($args:expr) => {{
        if $args.peek().is_some() {
            $args.next().to_owned().unwrap()
        } else {
            return Err(Box::new(LaunchError { message: "no extra argument".to_string() }));
        }
    }};
}

impl LaunchOptions {
    pub fn parse(&mut self, args: Vec<String>) -> Result<(), Box<dyn Error>> {
        if let Ok(java_opts) = env::var("JAVA_OPTS") {
            self.java_opts.extend(LaunchOptions::env_as_iter(java_opts))
        }

        if let Ok(jruby_opts) = env::var("JRUBY_OPTS") {
            self.jruby_opts.extend(LaunchOptions::env_as_iter(jruby_opts))
        }

        self.parse_os();

        if let Ok(java_mem) = env::var("JAVA_MEM") {
            self.push_java_opts_arg(java_mem)
        }

        if let Ok(java_stack) = env::var("JAVA_STACK") {
            self.push_java_opts_arg(java_stack)
        }

        let mut args = args.into_iter().peekable();

        self.argv0 = args.next().expect("Impossible to not have argv0");

        while let Some(argument) = args.next() {
            println!("ARG: {}", argument);

            match argument.as_str() {
                "--" => {
                    self.push_program_arg("--");
                    self.program_args.extend(args);
                    break;
                }
                // launcher specific -X self...
                "-Xfork-java" => self.fork_java = true,
                "-Xcommand" => self.command_only = true,
                "-Xnobootclasspath" => self.no_boot_classpath = true,
                "-Xtrace" => self.launcher_logfile = Some(PathBuf::from(arg_value!(args))),
                "-Xbootclass" => self.boot_class = Some(arg_value!(args)),
                "-Xjdkhome" => self.jdk_home = Some(PathBuf::from(arg_value!(args))),
                "-Xcp:p" => self.push_classpath_before(arg_value!(args)),
                "-Xcp:a" => self.push_classpath_after(arg_value!(args)),
                "-Xversion" => return Err(Box::new(LaunchError{ message: "need to fix -Xversion".to_string()})),
                "-Xhelp" | "-X" => {
                    // FIXME: WOT
                    // print_to_console(help)
                    // if self.append_help.isok puts append_help
                    self.push_java_arg("-Djruby.launcher.nopreamble=true");
                    self.push_program_arg("-X");
                }
                "-Xproperties" => self.program_args.push("--properties".to_string()),
                // java options we need to pass to java process itself if we see them
                "-J-cp" | "-J-classpath" => self
                    .classpath
                    .push(PathBuf::from(arg_value!(args))),
                "--server" => self.push_java_arg("-server"),
                "--client" => self.push_java_arg("-client"),
                "--dev" => {
                    self.push_java_arg("-XX:+TieredCompilation");
                    self.push_java_arg("-XX:TieredStopAtLevel=1");
                    self.push_java_arg("-Djruby.compile.mode=OFF");
                    self.push_java_arg("-Djruby.compile.invokedynamic=false");
                }
                "--sample" => self.push_java_arg("-Xprof"),
                "--manage" => {
                    self.push_java_arg("-Dcom.sun.management.jmxremote");
                    self.push_java_arg("-Djruby.management.enabled=true")
                }
                "--headless" => self.push_java_arg("-Djava.awt.headless=true"),
                "--ng" => self.nailgun_client = true,
                "--ng-server" => {
                    self.boot_class = Some("com/martiansoftware/nailgun/NGServer".to_string());
                    self.push_java_arg("-server");
                    self.no_boot_classpath = true;
                }
                "-Jea" => {
                    self.push_java_arg("-ea");
                    self.no_boot_classpath = true;
                    println!("Note: -ea option is specified, there will be no bootclasspath in order to enable assertions")
                }
                _ => {
                    if argument.len() > 2 {
                        let (two, rest) = argument.split_at(2);

                        match two {
                            "-X" if rest.chars().next().unwrap().is_ascii_lowercase() => { // unwrap safe 3+ chars at this point
                                let property = "-Djruby.".to_string() + rest;
                                self.push_java_arg(property.as_str())
                            }
                            "-J" => self.push_java_arg(rest),
                            _ => self.push_program_arg(argument.as_str()),
                        }
                    } else {
                        self.push_program_arg(argument.as_str());
                    }
                }
            }
        }
        println!("launch options = {:?}", self);

        Ok(())
    }

    /// What directory is the main application (e.g. jruby).
    ///
    fn determine_home(&mut self) -> Result<(), Box<dyn Error>> {
        info!("determining JRuby home");

        if let Ok(java_opts) = env::var("JRUBY_HOME") {
            info!("Found JRUBY_HOME = '{}'", java_opts);

            let dir = PathBuf::from(java_opts);
            let jruby_bin = dir.join("bin");
            if jruby_bin.exists() {
                info!("Success: Found bin directory within JRUBY_HOME");

                self.platform_dir = Some(dir);
                return Ok(())
            } else {
                info!("Cannot find bin within provided JRUBY_HOME {:?}", jruby_bin);
            }
        }

        if let Some(dir) = self.init_platform_dir_os() {
            info!("Success: Found from os magic!");
            self.platform_dir = Some(dir);
            return Ok(())
        }

        let argv0 = Path::new(&self.argv0);
        let mut dir: Option<PathBuf>;

        if argv0.is_absolute() {
            info!("Found absolute path for argv0");
            dir = Some(argv0.to_path_buf());
        } else if argv0.parent().is_some() && env::current_dir().is_ok()  { // relative path (will contain / or \).
            info!("Relative path argv0...combine with CWD");  // FIXME: make cwd Option in LaunchOptions
            dir = Some(env::current_dir()?.join(argv0).to_path_buf());
        } else {
            info!("Try and find argv0 within PATH env");
            dir = find_from_path(argv0.to_str().unwrap());
        }

        if dir.is_none() { // hail mary pass in argv[0].
            info!("Previous attempt failed...just leave argv0 as-is");
            dir = Some(argv0.to_path_buf());
        }

        if !dir.as_ref().unwrap().exists() {
            error!("Failue: '{:?}' does not exist", dir);
            return Err(Box::new(LaunchError { message: "unable to find JRuby home".to_string()}));
        }

        info!("Success found it: '{:?}'", dir);
        // FIXME: We can error here if we end with a path of "/jruby" (which would not sanely happen).
        let parent = dir.unwrap().parent().unwrap().to_path_buf().parent().unwrap().to_path_buf();
        self.platform_dir = Some(parent);
        Ok(())
    }

    fn determine_java_location(&mut self) -> Result<(), Box<dyn Error>> {
        let java = if let Ok(cmd) = env::var("JAVACMD") {
            Some(PathBuf::from(cmd))
        } else if self.jdk_home.is_some() {
            Some(PathBuf::from(self.jdk_home.as_ref().unwrap()).join("bin").join("java"))
        } else if let Ok(home) = env::var("JAVA_HOME") {
            Some(PathBuf::from(home).join("bin").join("java"))
        } else {
            find_from_path("java")
        };

        self.java_location = java;
        Ok(())
    }

    fn prepare_options(&mut self) -> Result<(), Box<dyn Error>> {
        let mut java_options: Vec<String> = vec![];

        if let Some(jdk_home) = &self.jdk_home {
            java_options.push("-Djdk.home=".to_string() + jdk_home.to_str().unwrap());
        }

        let platform_dir= self.platform_dir.to_owned().unwrap();

        java_options.push("-Djruby.home=".to_string() + platform_dir.to_str().unwrap());
        java_options.push("-Djruby.script=jruby".to_string());

        if cfg!(target_os = "windows") {
            java_options.push("-Djruby.shell=cmd.exe".to_string());
        } else {
            java_options.push("-Djruby.shell=/bin/sh".to_string());
        }

        let mut jni_dir = platform_dir.clone().join("lib").join("jni");

        println!("JNI DIR: {:?}, {}", jni_dir, jni_dir.exists());
        if !jni_dir.exists() {
            jni_dir = platform_dir.clone().join("lib").join("native");
            if !jni_dir.exists() {
                return Err(Box::new(LaunchError { message: "unable to find JNI dir".to_string() }))
            }
        }

        // FIXME: I believe else path will also work on windows so no more hard-coding
        let _ffi_option = "-Djffi.boot.library.path=".to_string();
        let os_name = sys_info::os_type().unwrap();

        println!("SYSINFO: {} {}", sys_info::os_release().expect("Whoa need to handle this without dying"), os_name);

        for entry in fs::read_dir(jni_dir).unwrap().into_iter() {
            let entry = entry.unwrap();

            if entry.path().to_str().unwrap().contains(&os_name) {
                println!("FOUND!!!!");
            }
            println!("ENTRY: {:?}", entry);
        }

        Ok(())
    }

    // Note: Assumes launcher_logfile is Some.
    fn setup_logging(&mut self) {
        let path = self.launcher_logfile.as_ref().unwrap().to_str().unwrap();
        let path = match path {
            "__stdout__" => None,
            _ => Some(PathBuf::from(path))
        };
        println!("LOG IS {:?}", path);
        let result = file_logger::init(path);
        if result.is_err() {
            panic!("PANICK LOCGGGG: {:?}", result)
        }
        result.ok();
    }

    #[cfg(target_os="macos")]
    fn init_platform_dir_os(&mut self) -> Option<PathBuf> {
        extern "C" {
            fn _NSGetExecutablePath(buf: *mut libc::c_char, bufsize: *mut u32) -> libc::c_int;
        }
        unsafe {
            let mut size: u32 = 0;
            if _NSGetExecutablePath(ptr::null_mut(), &mut size) == 0 {
                return None;
            }

            let mut size = size as usize;
            let mut buf: Vec<u8> = Vec::with_capacity(size);
            if _NSGetExecutablePath(buf.as_mut_ptr() as *mut i8, &mut size) != 0 {
                return None;
            }
            buf.set_len(size - 1); // -1 since \0 is not saved in rust
            PathBuf::from(OsString::from_vec(v));
        }
    }

    #[cfg(target_os="linux")]
    fn init_platform_dir_os(&mut self) -> Option<PathBuf> {
        use std::fs::read_link;
        if let Ok(path) = read_link(Path::new("/proc/self/exe")) {
            Some(path)
        } else {
            None
        }
    }

    #[cfg(target_os="windows")]
    fn init_platform_dir_os(&mut self) -> Option<PathBuf> {
        //FIXME: need VirtualQuery and GetModuleFileName
        None
    }

    fn env_as_iter(value: String) -> Vec<String> {
        // FIXME: Some off quote removal but only for first/last char of string
        value.split_ascii_whitespace().map(|a| a.to_string()).collect()
    }

    #[cfg(target_os="macos")]
    fn parse_os(&mut self) {
        if let None = env::var("JAVA_ENCODING") {
            self.push_java_opts_arg("-Dfile.encoding=UTF-8");
        }

        check_urandom(options)
    }

    #[cfg(target_os="linux")]
    fn parse_os(&mut self) {
        self.check_urandom()
    }

    #[cfg(target_os="windows")]
    fn parse_os(&mut self) {
        // no checks
    }

    // Force OpenJDK-based JVMs to use /dev/urandom for random number generation
    // See https://github.com/jruby/jruby/issues/4685 among others.
    #[cfg(target_os="linux")]
    fn check_urandom(&mut self) {
        use libc::{access, R_OK};
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let path = CString::new(Path::new("/dev/urandom").as_os_str().as_bytes()).unwrap();

        unsafe {
            // OpenJDK tries really hard to prevent you from using urandom.
            // See https://bugs.openjdk.java.net/browse/JDK-6202721
            // Non-file URL causes fallback to slow threaded SeedGenerator.
            // See https://bz.apache.org/bugzilla/show_bug.cgi?id=56139
            if access(path.as_ptr() as *const i8, R_OK) == 0 {
                self.push_java_opts_arg("-Djava.security.egd=file:/dev/urandom".to_string());
            }
        }
    }

    fn push_java_arg(&mut self, value: &str) {
        self.java_args.push(value.to_string());
    }

    pub(crate) fn prepend_program_arg(&mut self, value: &str) {
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
            java_location: None,
        }
    }
}