use core::fmt;
use log::{error, info, warn};
use std::error::Error;
use std::ffi::OsString;
use std::fmt::Formatter;
use std::path::PathBuf;
use std::{env, fs};

use crate::environment::Environment;
use crate::file_helper::find_from_path;
use crate::file_logger;
use crate::os_string_ext::OsStringExt;

pub const MAIN_CLASS: &str = "org/jruby/Main";

pub const XSS_DEFAULT: &str = "2048k";
pub const XSS_DEFAULT_OPT: &str = "-Xss2048k";

pub const DEV_MODE_JAVA_OPTIONS: [&str; 4] = [
    "-XX:+TieredCompilation",
    "-XX:TieredStopAtLevel=1",
    "-Djruby.compile.mode=OFF",
    "-Djruby.compile.invokedynamic=false"
];

#[cfg(target_os = "windows")]
pub const JAVA_NAME: &str = "java.exe";

#[cfg(target_os = "windows")]
pub const JAVAW_NAME: &str = "javaw.exe";

#[cfg(target_os = "windows")]
pub const SHELL: &str = "-Djruby.shell=cmd.exe";

#[cfg(not(windows))]
pub const JAVA_NAME: &str = "java";

#[cfg(not(windows))]
pub const SHELL: &str = "-Djruby.shell=/bin/sh";

#[derive(Debug, Clone)]
pub struct LaunchError {
    pub message: &'static str,
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

pub fn new(args: Vec<OsString>) -> Result<LaunchOptions, Box<dyn Error>> {
    let mut options = LaunchOptions::default();
    let env = Environment::from_env(args);

    options.parse(&env)?;

    if options.launcher_logfile.is_some() {
        options.setup_logging();
    };

    let executable = env.determine_jruby_executable(|f| f.exists())?;
    options.platform_dir = Some(executable.ancestors().take(3).collect());
    info!("launch_options = {:?}", options);
    options.determine_java_location(&env)?;
    info!("launch_options = {:?}", options);
    options.prepare_options(&env)?;
    info!("launch_options = {:?}", options);

    Ok(options)
}

#[derive(Debug, Default)]
pub struct LaunchOptions {
    fork_java: bool,
    pub(crate) command_only: bool,
    no_boot_classpath: bool,
    pub(crate) nailgun_client: bool,
    launcher_logfile: Option<PathBuf>,
    boot_class: Option<OsString>,
    jdk_home: Option<PathBuf>,
    classpath_before: Vec<PathBuf>,
    classpath_after: Vec<PathBuf>,
    classpath_explicit: Vec<PathBuf>, // What we passed explicitly to the launcher as a classpath.
    classpath: Vec<PathBuf>,
    java_args: Vec<OsString>, // Note: some other fields will also eventually be java args in final command-line.
    pub(crate) program_args: Vec<OsString>,
    java_opts: Vec<OsString>,
    jruby_opts: Vec<OsString>,
    platform_dir: Option<PathBuf>,
    pub(crate) java_location: Option<PathBuf>,
    xss: Option<OsString>,
    use_module_path: bool,
    boot_classpath: Vec<PathBuf>,
    suppress_console: bool,
}

macro_rules! arg_value {
    ($args:expr) => {{
        if $args.peek().is_some() {
            $args.next().to_owned().unwrap()
        } else {
            return Err(Box::new(LaunchError {
                message: "no extra argument",
            }));
        }
    }};
}

impl LaunchOptions {
    pub fn parse(&mut self, env: &Environment) -> Result<(), Box<dyn Error>> {
        if let Some(java_opts) = &env.java_opts {
            self.java_opts.extend(LaunchOptions::env_as_iter(java_opts))
        }

        if let Some(jruby_opts) = &env.jruby_opts {
            self.jruby_opts.extend(LaunchOptions::env_as_iter(jruby_opts))
        }

        self.parse_os(env);

        if let Some(java_mem) = &env.java_mem {
            self.java_args.push(java_mem.clone());
        }

        if let Some(java_stack) = &env.java_stack {
            self.java_opts.push(java_stack.clone())
        }

        let mut args = env.args.clone().into_iter().peekable();

        args.next().expect("Impossible to not have argv0");

        while let Some(argument) = args.next() {
            match argument.to_string_lossy().into_owned().as_str() {
                "--" => {
                    self.program_args.push(OsString::from("--"));
                    self.program_args.extend(args.clone());
                    break;
                }
                // launcher specific -X self...
                "-Xfork-java" => self.fork_java = true,
                "-Xcommand" => self.command_only = true,
                "-Xnobootclasspath" => self.no_boot_classpath = true,
                "-Xtrace" => self.launcher_logfile = Some(PathBuf::from(arg_value!(args))),
                "-Xbootclass" => self.boot_class = Some(arg_value!(args)),
                "-Xjdkhome" => self.jdk_home = Some(PathBuf::from(arg_value!(args))),
                "-Xcp:p" => self.classpath_before.push(PathBuf::from(arg_value!(args))),
                "-Xcp:a" => self.classpath_after.push(PathBuf::from(arg_value!(args))),
                "-Xversion" => {
                    return Err(Box::new(LaunchError {
                        message: "need to fix -Xversion",
                    }))
                }
                "-Xhelp" | "-X" => {
                    // FIXME: WOT
                    // print_to_console(help)
                    // if self.append_help.isok puts append_help
                    self.java_args.push(OsString::from("-Djruby.launcher.nopreamble=true"));
                    self.program_args.push(OsString::from("-X"));
                }
                "-Xproperties" => self.program_args.push(OsString::from("--properties")),
                // java options we need to pass to java process itself if we see them
                "-J-cp" | "-J-classpath" => self
                    .classpath_explicit
                    .push(PathBuf::from(arg_value!(args))),
                "--server" => self.java_args.push(OsString::from("-server")),
                "--client" => self.java_args.push(OsString::from("-client")),
                "--dev" => {
                    let dev_args = DEV_MODE_JAVA_OPTIONS.iter().map(|e| OsString::from(e)).into_iter();
                    self.java_args.extend(dev_args);
                }
                "--sample" => self.java_args.push(OsString::from("-Xprof")),
                "--manage" => {
                    self.java_args.push(OsString::from("-Dcom.sun.management.jmxremote"));
                    self.java_args.push(OsString::from("-Djruby.management.enabled=true"))
                }
                "--headless" => self.java_args.push(OsString::from("-Djava.awt.headless=true")),
                "--ng" => self.nailgun_client = true,
                "--ng-server" => {
                    self.boot_class = Some(OsString::from("com/martiansoftware/nailgun/NGServer"));
                    self.java_args.push(OsString::from("-server"));
                    self.no_boot_classpath = true;
                }
                "-Jea" => {
                    self.java_args.push(OsString::from("-ea"));
                    self.no_boot_classpath = true;
                    println!("Note: -ea option is specified, there will be no bootclasspath in order to enable assertions")
                }
                _ => {
                    if argument.len() > 2 {
                        let (two, rest) = argument.split_at(2);

                        let two = two.to_str().unwrap();

                        match two {
                            "-X" if rest.starts_with(OsString::from("xss")) => self.xss = Some(argument),
                            "-X" if rest.to_string_lossy().chars().next().unwrap().is_ascii_lowercase() => {
                                // unwrap safe 3+ chars at this point
                                let mut property = OsString::from("-Djruby.".to_string());
                                property.push(rest);
                                self.java_args.push(property)
                            }
                            "-J" => self.java_args.push(rest),
                            _ => self.program_args.push(argument),
                        }
                    } else {
                        self.program_args.push(argument);
                    }
                }
            }
        }
        info!("launch options = {:?}", self);

        Ok(())
    }

    fn determine_java_location(&mut self, env: &Environment) -> Result<(), Box<dyn Error>> {
        let java = if let Some(cmd) = &env.java_cmd {
            info!("Found JAVACMD");
            Some(PathBuf::from(cmd))
        } else if self.jdk_home.is_some() {
            info!("-Xjdkhome was specified");
            Some(
                PathBuf::from(self.jdk_home.as_ref().unwrap())
                    .join("bin")
                    .join(JAVA_NAME),
            )
        } else if let Some(home) = &env.java_home {
            info!("Deriving from JAVA_HOME");
            Some(PathBuf::from(home).join("bin").join(JAVA_NAME))
        } else {
            info!("Trying to find java command on Path");
            find_from_path(JAVA_NAME, &env.path, |f| f.exists())
        };

        // FIXME: Seemingly if not found on path we should probably just exit with an error here.
        self.java_location = java;
        Ok(())
    }

    fn prepare_options(&mut self, env: &Environment) -> Result<(), Box<dyn Error>> {
        let mut java_options: Vec<OsString> = self.java_opts.clone();

        if let Some(jdk_home) = &self.jdk_home {
            java_options.push(OsString::from("-Djdk.home=".to_string() + jdk_home.to_str().unwrap()));
        }

        let platform_dir = self.platform_dir.to_owned().unwrap();

        java_options.push(OsString::from("-Djruby.home=".to_string() + platform_dir.to_str().unwrap()));
        java_options.push(OsString::from("-Djruby.script=jruby"));
        java_options.push(OsString::from(SHELL));

        let mut jni_dir = platform_dir.clone().join("lib").join("jni");

        info!("JNI DIR: {:?}, {}", jni_dir, jni_dir.exists());
        if !jni_dir.exists() {
            jni_dir = platform_dir.clone().join("lib").join("native");
            if !jni_dir.exists() {
                return Err(Box::new(LaunchError {
                    message: "unable to find JNI dir",
                }));
            }
        }

        let os_name = sys_info::os_type().unwrap();
        let entries = fs::read_dir(&jni_dir)
            .unwrap()
            .filter_map(|entry| -> Option<PathBuf> {
                let path = &entry.unwrap().path().to_str().unwrap().to_owned();

                if path.contains(&os_name) {
                    Some(PathBuf::from(path))
                } else {
                    None
                }
            });

        let mut paths: Vec<PathBuf> = vec![jni_dir];
        paths.extend(entries);

        let paths = env::join_paths(paths)?;

        if !paths.is_empty() {
            info!("found paths: {}", paths.to_str().unwrap());
            java_options.push(OsString::from("-Djffi.boot.library.path=".to_string() + paths.to_str().unwrap()));
        }

        if self.xss.is_none() {
            info!("No explicit xss. Defaulting to: {}", XSS_DEFAULT);
            java_options.push(OsString::from(XSS_DEFAULT_OPT));
        }

        if self.jdk_home.is_none() {
            info!("No Java home detected.  Cannot check for JPMS.");
        } else {
            let jmods =
                PathBuf::from(self.jdk_home.as_ref().unwrap().to_str().unwrap()).join("jmods");

            if jmods.exists() {
                info!("jmods directory found in Java home.  Set up module support");
                self.use_module_path = true;
            }
        }

        // construct_boot_classpath
        let lib_dir = PathBuf::from(&platform_dir).join("lib");
        let jruby_complete_jar = lib_dir.clone().join("jruby-complete.jar");
        let jruby_jar = lib_dir.join("jruby.jar");

        if jruby_jar.exists() {
            self.add_to_boot_class_path(jruby_jar, true);
            if jruby_complete_jar.exists() {
                warn!("Both jruby.jar and jruby-complete.jar are present.  Using jruby.jar");
            }
        } else if jruby_complete_jar.exists() {
            self.add_to_boot_class_path(jruby_complete_jar, false);
        } else {
            warn!("No jruby.jar or jruby-complete.jar found.")
            // not in original launcher (maybe verify via CLASSPATH and other potential places they could be set?
        }

        // construct_classpath
        self.add_jars_to_classpath();

        if self.classpath_explicit.is_empty() {
            info!("No explicit classpath passed in....Gettting from ENV");
            if let Some(classpath) = &env.classpath {
                self.classpath.extend(env::split_paths(&classpath));
            }
        } else {
            info!("Explicit classpath..ignoring ENV");
            self.classpath.extend(self.classpath_explicit.to_owned());
        }

        if !self.classpath_after.is_empty() {
            self.classpath.extend(self.classpath_after.to_owned());
        }

        // Extend empty entry for get extra : or ; so that it include CWD as part of classpath
        if !self.classpath.is_empty() {
            self.classpath.push(PathBuf::from(""));
        }

        info!("ClassPath: {:?}", env::join_paths(self.classpath.iter())?);

        if self.boot_class.is_none() {
            self.boot_class = Some(OsString::from(MAIN_CLASS));
        }

        let command_name = self.boot_class.as_ref().unwrap().clone().replace(b'/', b'.');

        info!("CommandName: {:?}", command_name);

        let mut command = OsString::from("-Dsun.java.command=");
        command.push(command_name);
        java_options.push(command);

        if !self.boot_classpath.is_empty() {
            let path = env::join_paths(self.boot_classpath.iter()).unwrap();

            if self.use_module_path {
                let mut module_path = OsString::from("--module-path=");
                module_path.push(path);
                java_options.push(module_path);
            } else {
                let mut boot_class_path = OsString::from("-Xbootclasspath/a:");
                boot_class_path.push(path);
                java_options.push(boot_class_path);
            }
        }

        if self.use_module_path {
            let bin_options_file: PathBuf = ["bin", ".jruby.module_opts"].iter().collect();
            let module_options_file = self.platform_dir.as_ref().unwrap().join(bin_options_file);
            info!("MOF: {:?}", module_options_file);

            if module_options_file.exists() {
                info!(
                    "Found module options file {:?}.  Using that.",
                    module_options_file
                );
                java_options.push(OsString::from("@".to_string() + module_options_file.to_str().unwrap()));
            } else {
                info!("Found no module options file.  Use hard-coded values.");
                java_options.push(OsString::from("--add-opens"));
                java_options.push(OsString::from("java.base/java.io=org.jruby.dist"));
                java_options.push(OsString::from("--add-opens"));
                java_options.push(OsString::from("java.base/java.nio.channels=org.jruby.dist"));
                java_options.push(OsString::from("--add-opens"));
                java_options.push(OsString::from("java.base/sun.nio.ch=org.jruby.dist"));
                java_options.push(OsString::from("--add-opens"));
                java_options.push(OsString::from("java.management/sun.management=org.jruby.dist"));
            }
        }

        let class_path = env::join_paths(self.classpath.iter())?;
        if self.fork_java {
            java_options.push(OsString::from("-cp"));
            java_options.push(class_path);
        } else {
            let mut cp = OsString::from("-Djava.class.path=");
            cp.push(class_path);
            java_options.push(cp);
        }

        info!("JAVA_OPTS = {:?}", java_options);
        self.java_opts = java_options;

        Ok(())
    }

    fn add_jars_to_classpath(&mut self) {
        let lib_dir = self.platform_dir.clone().unwrap().join("lib");

        if !lib_dir.is_dir() {
            // FIXME: This should full on abort
            error!("{:?} is not a directory...skipping!", lib_dir);
            return;
        }

        for entry in fs::read_dir(lib_dir).unwrap().into_iter() {
            let path = &entry.unwrap().path().to_str().unwrap().to_owned();

            if path.ends_with(".jar") {
                self.classpath.push(PathBuf::from(path));
            }
        }
    }

    fn add_to_boot_class_path(&mut self, path: PathBuf, only_if_exists: bool) {
        if self.no_boot_classpath {
            info!("no boot classpath specified so adding to classpath instead");
            self.add_to_class_path(path, only_if_exists);
            return;
        }

        if only_if_exists && !path.exists() {
            info!("boot class path does not exist: {:?}", &path);
            return;
        }

        if self.boot_classpath.contains(&path) {
            info!(
                "{:?} already is within the boot classpath.  Skipping it.",
                path
            );
        } else {
            self.boot_classpath.push(path);
        }
    }

    fn add_to_class_path(&mut self, path: PathBuf, only_if_exists: bool) {
        if only_if_exists && !path.exists() {
            return;
        }

        if self.classpath.contains(&path) {
            info!("{:?} already is within classpath.  Skipping it.", path);
            return;
        }

        if self.boot_classpath.contains(&path) {
            info!(
                "{:?} already is within the boot classpath.  Not adding to classpath.",
                path
            );
        } else {
            self.classpath.push(path);
        }
    }

    // Note: Assumes launcher_logfile is Some.
    fn setup_logging(&mut self) {
        let path = self.launcher_logfile.as_ref().unwrap().to_str().unwrap();
        let path = match path {
            "__stdout__" => None,
            _ => Some(PathBuf::from(path)),
        };

        let result = file_logger::init(path);
        if result.is_err() {
            panic!("Cannot resolve file logger: {:?}", result)
        }
        result.ok();
    }

    pub fn command_line(&self) -> Vec<OsString> {
        let mut command_line = self.java_opts.clone();

        command_line.push(self.boot_class.clone().unwrap());
        command_line.extend(self.program_args.clone());
        command_line
    }

    fn env_as_iter(value: &OsString) -> Vec<OsString> {
        // FIXME: Some off quote removal but only for first/last char of string
        value.split_ascii_whitespace().collect()
    }

    #[cfg(unix)]
    fn parse_os(&mut self, env: &Environment) {
        if cfg!(target_os="macos") {
            if let None = env.java_encoding {
                self.java_opts.push("-Dfile.encoding=UTF-8".to_string());
            }
        } else {
            // FIXME: old launcher still checked this on macos but problems in check_urandom not compiling on macos
            self.check_urandom()
        }
    }

    #[cfg(target_os = "windows")]
    fn parse_os(&mut self, _env: &Environment) {
        // no checks
    }

    // Force OpenJDK-based JVMs to use /dev/urandom for random number generation
    // See https://github.com/jruby/jruby/issues/4685 among others.
    #[cfg(any(unix))]
    fn check_urandom(&mut self) {
        use libc::{access, R_OK};
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::path::Path;

        let path = CString::new(Path::new("/dev/urandom").as_os_str().as_bytes()).unwrap();

        unsafe {
            // OpenJDK tries really hard to prevent you from using urandom.
            // See https://bugs.openjdk.java.net/browse/JDK-6202721
            // Non-file URL causes fallback to slow threaded SeedGenerator.
            // See https://bz.apache.org/bugzilla/show_bug.cgi?id=56139
            if access(path.as_ptr() as *const i8, R_OK) == 0 {
                self.java_opts.push("-Djava.security.egd=file:/dev/urandom".to_string());
            }
        }
    }
}
