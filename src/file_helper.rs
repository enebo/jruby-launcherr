use std::env;
use std::env::split_paths;
use std::path::PathBuf;

pub(crate) fn find_from_path(file: &str) -> Option<PathBuf> {
    if let Ok(paths) = env::var("PATH") {
        for path in split_paths(paths.as_str()) {
            let test = path.join(file);

            if test.exists() {
                return Some(test);
            }
        }
    }

    None
}
