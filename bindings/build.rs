fn main() {
    windows::build!(
        windows::win32::system_services::CreateProcessW
        windows::win32::system_services::GetConsoleWindow
        windows::win32::system_services::GetExitCodeProcess
        windows::win32::system_services::INFINITE
        windows::win32::system_services::PROCESS_INFORMATION
        windows::win32::system_services::ResumeThread
        windows::win32::system_services::SetConsoleCtrlHandler
        windows::win32::system_services::STARTUPINFOW
        windows::win32::system_services::WaitForSingleObject
        windows::win32::windows_and_messaging::HWND
        windows::win32::windows_programming::CloseHandle
        windows::win32::windows_programming::PROCESS_CREATION_FLAGS
        windows::win32::debug::GetLastError
    );
}