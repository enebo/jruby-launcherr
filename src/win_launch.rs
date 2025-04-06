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
use std::os::windows::ffi::OsStringExt as SysOsStringExt;
use widestring::U16String;

fn rawCommandLine() -> Vec<u16> {
    let ptr = unsafe { GetCommandLineW() };
    let length: usize = unsafe { uaw_wcslen(ptr.0 as *mut u16) };
    let str = unsafe {U16String::from_ptr(ptr.0 as *mut u16, length) };

    str.into_vec()
}

const BACKSLASH: u16 = b'\\' as u16;
const DOUBLE_QUOTE: u16 = b'\"' as u16;
const LEFT_BRACKET: u16 = b'[' as u16;
const LEFT_CURLY: u16 = b'{' as u16;
const NEWLINE: u16 = b'\n' as u16;
const QUESTION: u16 = b'?' as u16;
const SINGLE_QUOTE: u16 = b'\'' as u16;
const SPACE: u16 = b' ' as u16;
const STAR: u16 = b'*' as u16;
const TAB: u16 = b'\t' as u16;

pub fn commandLine(line: Vec<u16>) -> Vec<OsString> {
    let mut slashes = false;
    let mut escape = false;
    let mut quote: u16 = 0 as u16;
    let mut args: Vec<OsString> = vec![];
    let mut start: usize = 0;
    let mut glob: usize = 0;

    for (i, c) in line.iter().enumerate() {
        match *c {
            BACKSLASH => {
                if quote != SINGLE_QUOTE {
                    slashes = true;
                }
            },
            SPACE | TAB | NEWLINE=> {
                if quote == 0 {
                    args.push(OsString::from_wide(&line[start..i]));
                }
            },
            LEFT_BRACKET | LEFT_CURLY | STAR | QUESTION => {
                if quote != SINGLE_QUOTE {
                    glob += 1;
                }
                slashes = false;
            },
            SINGLE_QUOTE | DOUBLE_QUOTE => {
                if !slashes {
                    if quote == 0 {
                        quote = *c;
                    } else if quote == *c {
                        //if quote == DOUBLE_QUOTE && quote == line[i + 1] {
                        //    advance_c();
                        //}
                        quote = 0;
                    }
                }
                escape = true;
                slashes = false;
            },
            _ => slashes = false,
        }
    }

    println!("ARGS: {:?}", args);
    args
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
