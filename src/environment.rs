use std::env;
use std::path::PathBuf;

/// Represents a wrapper around accessing the actual OS environment.
///
/// We have this struct so that we can test without needing to actually
/// set environment variables.  This implementation will eagerly get
/// all envs whether they are needed ot not but the time for that
/// is small and it gains testability.
///
pub struct Environment {
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
    pub(crate) fn from_env() -> Self {
        Self {
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
}
