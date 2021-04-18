use bindings::Windows::Win32::SystemServices::BOOL;
use bindings::Windows::Win32::SystemServices::CreateProcessW;
use bindings::Windows::Win32::SystemServices::GetCommandLineW;
use bindings::Windows::Win32::SystemServices::GetConsoleWindow;
use bindings::Windows::Win32::SystemServices::GetExitCodeProcess;
use bindings::Windows::Win32::SystemServices::PROCESS_INFORMATION;
use bindings::Windows::Win32::SystemServices::PWSTR;
use bindings::Windows::Win32::SystemServices::ResumeThread;
use bindings::Windows::Win32::SystemServices::SetConsoleCtrlHandler;
use bindings::Windows::Win32::SystemServices::STARTUPINFOW;
use bindings::Windows::Win32::SystemServices::WaitForSingleObject;
use bindings::Windows::Win32::WindowsAndMessaging::HWND;
use bindings::Windows::Win32::WindowsProgramming::CloseHandle;
use bindings::Windows::Win32::WindowsProgramming::INFINITE;
use bindings::Windows::Win32::WindowsProgramming::PROCESS_CREATION_FLAGS;
use bindings::Windows::Win32::WindowsProgramming::uaw_wcslen;
use bindings::Windows::Win32::Debug::GetLastError;

use log::{error, info};
use std::ffi::{OsStr, OsString};
use std::iter::once;
use std::ptr;
use std::os::windows::ffi::OsStrExt;
use crate::launch_options::{JAVA_NAME, JAVAW_NAME};
use crate::os_string_ext::OsStringExt;
use widestring::U16String;

pub(crate) fn rawCommandLine() -> OsString {
    let ptr = unsafe { GetCommandLineW() };
    let length: usize = unsafe { uaw_wcslen(ptr.0 as *mut u16) };
    let str = unsafe {U16String::from_ptr(ptr.0 as *mut u16, length) };

    str.to_os_string()
}

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
    let mut c = PWSTR::default();
    c.0 = command_line_wide.as_mut_ptr();

    info!("EXECUTING: {:?}", command_line);
    unsafe {
        if !CreateProcessW(PWSTR::default(),
                          c,
                          ptr::null_mut(),
                          ptr::null_mut(),
                          BOOL::from(true),
                          PROCESS_CREATION_FLAGS::CREATE_SUSPENDED,
                          ptr::null_mut(),
                          PWSTR::default(),
                          si,
                          pi).as_bool() {
            panic!("Could not launch process: {:?}", &command_line);
        }

        if !SetConsoleCtrlHandler(None, BOOL::from(true)).as_bool() {
            error!("Could not set up console control handlers {}", GetLastError());
        }

        let pi = &*pi;
        ResumeThread(pi.hThread);
        WaitForSingleObject(pi.hProcess, INFINITE);
        let ret_code: *mut u32 = &mut 0;
        GetExitCodeProcess(pi.hProcess, ret_code);
        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);
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
