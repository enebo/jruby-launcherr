use std::env::split_paths;
use std::ffi::OsString;
use std::path::PathBuf;
use log::info;

pub(crate) fn find_from_path<T>(file: &str, path: &Option<OsString>, test: T) -> Option<PathBuf> where
    T: Fn(&PathBuf) -> bool {
    if let Some(paths) = path {
        info!("find_from_path({})", &file);
        for path in split_paths(paths) {
            let test_path = path.join(file);
            info!("find_from_path Testing:   {:?}", &test_path);

            if test(&test_path) {
                return Some(test_path)
            }
        }
    }

    None
}
