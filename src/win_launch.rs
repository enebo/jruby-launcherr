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

use log::error;
use std::ffi::OsStr;
use std::iter::once;
use std::ptr;
use std::os::windows::ffi::OsStrExt;
use crate::launch_options::{JAVA_NAME, JAVAW_NAME};

pub fn quote_vec(vector: Vec<String>) -> String {
    let command_line: Vec<String> = vector.into_iter().map( |mut s| {
        s.insert(0, '"');
        s.push('"');
        s
    }).collect();

    command_line.join(" ")
}

fn is_console_attached() -> bool {
    unsafe {
        GetConsoleWindow() != HWND::default()
    }
}

pub fn execute_with_create_process(mut command: String, args: Vec<String>) -> u32 {
    let si: *mut STARTUPINFOW = &mut STARTUPINFOW::default();
    let pi: *mut PROCESS_INFORMATION = &mut PROCESS_INFORMATION::default();

    // We will run the new process using windows vs console if we are already not
    // running from within a console.
    if !is_console_attached() {
        command = command.replace(JAVA_NAME, JAVAW_NAME);
    }

    let mut command_line = vec![command];
    command_line.extend(args);
    let command_line = quote_vec(command_line);
    let mut command_line_wide: Vec<u16> = OsStr::new(&command_line).encode_wide().chain(once(0)).collect();

    println!("EXECUTING: {}", command_line);
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
            panic!("Could not launch process: {}", &command_line);
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
    use crate::win_launch::execute_with_create_process;

    // FIXME: Decide how to test this?
    fn aaaa_test() {
        let command = String::from("C:/Windows/System32/whoami.exe");
        let args = vec![];
        execute_with_create_process(command, args);
    }
}
