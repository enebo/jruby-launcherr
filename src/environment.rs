use std::env;
use std::path::PathBuf;
use log::{error, info};
use crate::launch_options::LaunchError;
use crate::file_helper::find_from_path;
use std::error::Error;
use std::ffi::OsString;
use process_path::get_executable_path;

/// Represents a wrapper around accessing the actual OS environment.
///
/// We have this struct so that we can test without needing to actually
/// set environment variables.  This implementation will eagerly get
/// all envs whether they are needed ot not but the time for that
/// is small and it gains testability.
///
pub struct Environment {
    pub args: Vec<OsString>,
    pub classpath: Option<OsString>,
    pub current_dir: Option<PathBuf>,
    pub java_cmd: Option<OsString>,
    pub java_encoding: Option<OsString>,
    pub java_home: Option<OsString>,
    pub java_mem: Option<OsString>,
    pub java_opts: Option<OsString>,
    pub java_stack: Option<OsString>,
    pub jruby_opts: Option<OsString>,
    pub jruby_home: Option<OsString>,
    pub path: Option<OsString>,
}

impl Environment {
    pub(crate) fn from_env(args: Vec<OsString>) -> Self {
        Self {
            args,
            classpath: env::var_os("CLASSPATH"),
            current_dir: env::current_dir().ok(),
            java_cmd: env::var_os("JAVACMD"),
            java_encoding: env::var_os("JAVA_ENCODING"),
            java_home: env::var_os("JAVA_HOME"),
            java_mem: env::var_os("JAVA_MEM"),
            java_opts: env::var_os("JAVA_OPTS"),
            java_stack: env::var_os("JAVA_STACK"),
            jruby_opts: env::var_os("JRUBY_OPTS"),
            jruby_home: env::var_os("JRUBY_HOME"),
            path: env::var_os("PATH"),
        }
    }

    pub(crate) fn argv0(&self) -> PathBuf {
        let argv0 = self.args.iter().next().unwrap();
        let path = PathBuf::from(argv0);

        if cfg!(target_os = "windows") && path.extension().is_none() {
            return path.with_extension(".exe");
        }

        path
    }

    /// What directory is the main application (e.g. jruby)?
    ///
    pub(crate) fn determine_jruby_executable<T>(&self, exist_test: T) -> Result<PathBuf, Box<dyn Error>> where
        T: Fn(&PathBuf) -> bool + Copy {
        info!("determining JRuby home");

        if let Some(java_opts) = &self.jruby_home {
            let dir = PathBuf::from(java_opts);

            info!("Found JRUBY_HOME = '{:?}'", &dir);

            let jruby_bin = dir.join("bin");
            if exist_test(&jruby_bin) {
                info!("Success: Found bin directory within JRUBY_HOME");

                return Ok(dir);
            } else {
                info!("Cannot find bin within provided JRUBY_HOME {:?}", &jruby_bin);
            }
        }

        if let Some(dir) = get_executable_path() {
            info!("Success: Found from os magic! {:?}", &dir);
            return Ok(dir);
        }

        let dir = self.derive_home_from_argv0(&self.argv0(), &self.path, exist_test);

        if !exist_test(&dir) {
            error!("Failue: '{:?}' does not exist", &dir);
            return Err(Box::new(LaunchError {
                message: "unable to find JRuby home",
            }));
        }

        info!("Success found it: '{:?}'", &dir);
        Ok(dir)
    }

    /// Return a possible JRUBY install home based on liklihood.
    ///  1. assume absolute path is launched from project dir
    ///  2. CWD + relative path
    ///  3. Find in PATH + relative path
    ///  4. Go for broke...just return ARGV0 value itself.
    fn derive_home_from_argv0<T>(&self, argv0: &PathBuf, path: &Option<OsString>, test: T) -> PathBuf where
        T: Fn(&PathBuf) -> bool {
        if argv0.is_absolute() {
            info!("Found absolute path for argv0");
            return argv0.to_path_buf();
        }

        let parent = argv0.parent();
        if parent.is_some()
            && !parent.unwrap().as_os_str().is_empty()
            && self.current_dir.is_some()
            && test(&self.current_dir.as_ref().unwrap().join(argv0)) {
            // relative path (will contain / or \).
            info!("Relative path argv0...combine with CWD");
            self.current_dir.as_ref().unwrap().join(argv0)
        } else {
            info!("Try and find argv0 within PATH env");
            if let Some(dir) = find_from_path(argv0.to_str().unwrap(), path, test) {
                dir
            } else {
                info!("Not found in PATH...just leave argv0 as-is");
                argv0.to_path_buf()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::environment::Environment;
    use std::path::{MAIN_SEPARATOR, PathBuf};

    fn empty_env() -> Environment {
        Environment {
            args: vec![],
            classpath: None,
            current_dir: None,
            java_cmd: None,
            java_encoding: None,
            java_home: None,
            java_mem: None,
            java_opts: None,
            java_stack: None,
            jruby_opts: None,
            jruby_home: None,
            path: None,
        }
    }

    #[test]
    fn test_determine_jruby_home() {
        let mut env = empty_env();
        let traditional_home: PathBuf = [MAIN_SEPARATOR.to_string().as_str(), "home", "user", "jruby"].iter().collect();
        let absolute: PathBuf = [MAIN_SEPARATOR.to_string().as_str(), "home", "user", "jruby", "bin", "jruby"].iter().collect();
        let argv0 = &absolute;
        let test = |f: &PathBuf| f.exists();

        env.jruby_home = Some(traditional_home.into_os_string());

        assert_eq!(env.derive_home_from_argv0(&argv0, &None, test).as_os_str(), &absolute);
    }

    #[test]
    fn test_jruby_home_argv0() {
        let mut env = empty_env();
        let absolute: PathBuf = [MAIN_SEPARATOR.to_string().as_str(), "home", "user", "jruby", "bin", "jruby"].iter().collect();
        let argv0 = &absolute;
        let test = |f: &PathBuf| f.exists();

        assert_eq!(env.derive_home_from_argv0(&argv0, &None, test).as_os_str(), &absolute);

        let argv0: PathBuf = ["bin", "jruby"].iter().collect();
        let traditional_home: PathBuf = [MAIN_SEPARATOR.to_string().as_str(), "home", "user", "jruby"].iter().collect();
        env.current_dir = Some(traditional_home.clone());
        let absolute_test = |f: &PathBuf| f == &absolute;

        assert_eq!(env.derive_home_from_argv0(&argv0, &None, absolute_test).as_os_str(), &absolute);

        env.current_dir = None;

        assert_eq!(env.derive_home_from_argv0(&argv0, &None, test).as_os_str(), &argv0);

        let test_home = absolute.clone();
        let path = Some(traditional_home.into_os_string());
        let path_test = |t: &PathBuf| t == &test_home;

        assert_eq!(env.derive_home_from_argv0(&argv0, &path, path_test).as_os_str(), &absolute);
    }

    #[test]
    fn test_jruby_home_argv0_windows_specific() {
        let env = empty_env();
        let absolute: PathBuf = [r"\\frogger\", "home", "user", "jruby", "bin", "jruby"].iter().collect();
        let argv0 = &absolute;
        let test = |f: &PathBuf| f.exists();

        assert_eq!(env.derive_home_from_argv0(&argv0, &None, test).as_os_str(), &absolute);

    }
}