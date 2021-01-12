use std::env;
use std::path::{PathBuf, Path};
use log::{error, info};
use crate::launch_options::LaunchError;
use crate::file_helper::{find_from_path, init_platform_dir_os};
use std::error::Error;

/// Represents a wrapper around accessing the actual OS environment.
///
/// We have this struct so that we can test without needing to actually
/// set environment variables.  This implementation will eagerly get
/// all envs whether they are needed ot not but the time for that
/// is small and it gains testability.
///
pub struct Environment {
    pub args: Vec<String>,
    pub classpath: Option<String>,
    pub current_dir: Option<PathBuf>,
    pub java_cmd: Option<String>,
    pub java_encoding: Option<String>,
    pub java_home: Option<String>,
    pub java_mem: Option<String>,
    pub java_opts: Option<String>,
    pub java_stack: Option<String>,
    pub jruby_opts: Option<String>,
    pub jruby_home: Option<String>,
}

impl Environment {
    pub(crate) fn from_env(args: Vec<String>) -> Self {
        Self {
            args,
            classpath: env::var("CLASSPATH").ok(),
            current_dir: env::current_dir().ok(),
            java_cmd: env::var("JAVACMD").ok(),
            java_encoding: env::var("JAVA_ENCODING").ok(),
            java_home: env::var("JAVA_HOME").ok(),
            java_mem: env::var("JAVA_MEM").ok(),
            java_opts: env::var("JAVA_OPTS").ok(),
            java_stack: env::var("JAVA_STACK").ok(),
            jruby_opts: env::var("JRUBY_OPTS").ok(),
            jruby_home: env::var("JRUBY_HOME").ok(),
        }
    }

    pub(crate) fn argv0(&self) -> &Path {
        Path::new(self.args.iter().next().unwrap())
    }

    /// What directory is the main application (e.g. jruby)?
    ///
    pub(crate) fn determine_jruby_home(&self) -> Result<PathBuf, Box<dyn Error>> {
        info!("determining JRuby home");

        if let Some(java_opts) = &self.jruby_home {
            info!("Found JRUBY_HOME = '{}'", java_opts);

            let dir = PathBuf::from(java_opts);
            let jruby_bin = dir.join("bin");
            if jruby_bin.exists() {
                info!("Success: Found bin directory within JRUBY_HOME");

                return Ok(dir);
            } else {
                info!("Cannot find bin within provided JRUBY_HOME {:?}", jruby_bin);
            }
        }

        if let Some(dir) = init_platform_dir_os() {
            info!("Success: Found from os magic!");
            return Ok(dir);
        }

        let dir = self.derive_home_from_argv0(self.argv0());

        if !dir.exists() {
            error!("Failue: '{:?}' does not exist", &dir);
            return Err(Box::new(LaunchError {
                message: "unable to find JRuby home",
            }));
        }

        info!("Success found it: '{:?}'", &dir);
        Ok(dir.ancestors().take(3).collect())
    }

    /// Return a possible JRUBY install home based on liklihood.
    ///  1. assume absolute path is launched from project dir
    ///  2. CWD + relative path
    ///  3. Find in PATH + relative path
    ///  4. Go for broke...just return ARGV0 value itself.
    fn derive_home_from_argv0(&self, argv0: &Path) -> PathBuf {
        if argv0.is_absolute() {
            info!("Found absolute path for argv0");
            argv0.to_path_buf()
        } else if argv0.parent().is_some() && self.current_dir.is_some() {
            // relative path (will contain / or \).
            info!("Relative path argv0...combine with CWD");
            self.current_dir.as_ref().unwrap().clone().join(argv0)
        } else {
            info!("Try and find argv0 within PATH env");
            if let Some(dir) = find_from_path(argv0.to_str().unwrap()) {
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

        }
    }

    #[test]
    fn test_jruby_home() {
        let mut env = empty_env();

        env.jruby_home = Some(String::from("ddddd"));

    }
}