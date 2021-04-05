use bindings::windows::win32::system_services::CreateProcessW;
use bindings::windows::win32::system_services::GetConsoleWindow;
use bindings::windows::win32::system_services::GetExitCodeProcess;
use bindings::windows::win32::system_services::INFINITE;
use bindings::windows::win32::system_services::PROCESS_INFORMATION;
use bindings::windows::win32::system_services::ResumeThread;
use bindings::windows::win32::system_services::SetConsoleCtrlHandler;
use bindings::windows::win32::system_services::STARTUPINFOW;
use bindings::windows::win32::system_services::WaitForSingleObject;
use bindings::windows::win32::windows_and_messaging::HWND;
use bindings::windows::win32::windows_programming::CloseHandle;
use bindings::windows::win32::windows_programming::PROCESS_CREATION_FLAGS;
use bindings::windows::win32::debug::GetLastError;

use log::{error, info};
use std::ffi::{OsStr, OsString};
use std::iter::once;
use std::ptr;
use std::os::windows::ffi::OsStrExt;
use crate::launch_options::{JAVA_NAME, JAVAW_NAME};
use crate::os_string_ext::OsStringExt;

pub fn join(vector: Vec<OsString>, delimeter: &str) -> OsString {
    let mut new_string = OsString::new();

    for (i, arg) in vector.iter().enumerate() {
        if i != 0 {
            new_string.push(delimeter);
        }

        new_string.push(arg);
    }

    new_string
}

pub fn quote_os_string(string: OsString) -> OsString {
    let mut new_string = OsString::with_capacity(string.len() + 2);

    new_string.push("\"");
    new_string.push(string);
    new_string.push("\"");
    new_string
}

pub fn quote_vec(vector: Vec<OsString>) -> OsString {
    let command_line: Vec<OsString> = vector.into_iter().map(quote_os_string).collect();
    join(command_line," ")
}

fn is_console_attached() -> bool {
    unsafe {
        GetConsoleWindow() != HWND::default()
    }
}

pub fn execute_with_create_process(mut command: OsString, args: Vec<OsString>) -> u32 {
    let si: *mut STARTUPINFOW = &mut STARTUPINFOW::default();
    let pi: *mut PROCESS_INFORMATION = &mut PROCESS_INFORMATION::default();

    // We will run the new process using windows vs console if we are already not
    // running from within a console.
    if !is_console_attached() {
        command = command.replace_str(&OsString::from(JAVA_NAME), &OsString::from(JAVAW_NAME));
    }

    let mut command_line = vec![command];
    command_line.extend(args);
    let command_line = quote_vec(command_line);
    let mut command_line_wide: Vec<u16> = OsStr::new(&command_line).encode_wide().chain(once(0)).collect();

    info!("EXECUTING: {:?}", command_line);
    unsafe {
        if CreateProcessW(ptr::null_mut(),
                          command_line_wide.as_mut_ptr(),
                          ptr::null_mut(),
                          ptr::null_mut(),
                          bindings::windows::BOOL::from(true),
                          PROCESS_CREATION_FLAGS::CREATE_SUSPENDED,
                          ptr::null_mut(),
                          ptr::null_mut(),
                          si,
                          pi).is_err() {
            panic!("Could not launch process: {:?}", &command_line);
        }

        if SetConsoleCtrlHandler(None, bindings::windows::BOOL::from(true)).is_err() {
            error!("Could not set up console control handlers {}", GetLastError());
        }

        let pi = &*pi;
        ResumeThread(pi.h_thread);
        WaitForSingleObject(pi.h_process, INFINITE);
        let ret_code: *mut u32 = &mut 0;
        GetExitCodeProcess(pi.h_process, ret_code);
        CloseHandle(pi.h_process);
        CloseHandle(pi.h_thread);
        (*ret_code).clone()
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use crate::win_launch::execute_with_create_process;

    // FIXME: Decide how to test this?
    fn aaaa_test() {
        let command = OsString::from("C:/Windows/System32/whoami.exe");
        let args = vec![];
        execute_with_create_process(command, args);
    }
}
