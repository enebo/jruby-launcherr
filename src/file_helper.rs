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

#[cfg(target_os = "macos")]
pub(crate) fn init_platform_dir_os() -> Option<PathBuf> {
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

#[cfg(any(unix))]
pub(crate) fn init_platform_dir_os() -> Option<PathBuf> {
    use std::fs::read_link;
    if let Ok(path) = read_link(Path::new("/proc/self/exe")) {
        Some(path)
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn init_platform_dir_os() -> Option<PathBuf> {
    //FIXME: need VirtualQuery and GetModuleFileName
    None
}
