use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Storage::FileSystem::{FILE_TYPE_CHAR, GetFileType, WriteFile};
use windows_sys::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AllocConsole, AttachConsole, GetStdHandle, STD_ERROR_HANDLE,
    STD_OUTPUT_HANDLE, WriteConsoleW,
};

fn write(handle_id: u32, text: &str) {
    let handle = unsafe { GetStdHandle(handle_id) };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return;
    }

    let mut text = text.to_string();
    if !text.ends_with('\n') {
        text.push('\n');
    }

    let file_type = unsafe { GetFileType(handle) };
    if file_type == FILE_TYPE_CHAR {
        let mut console_text = String::with_capacity(text.len() + 8);
        let mut last_was_cr = false;
        for ch in text.chars() {
            if ch == '\n' && !last_was_cr {
                console_text.push('\r');
            }
            console_text.push(ch);
            last_was_cr = ch == '\r';
        }
        let wide: Vec<u16> = console_text.encode_utf16().collect();
        let mut written = 0u32;
        unsafe {
            WriteConsoleW(
                handle,
                wide.as_ptr(),
                wide.len() as u32,
                &mut written,
                std::ptr::null_mut(),
            );
        }
    } else {
        let mut written = 0u32;
        unsafe {
            WriteFile(
                handle,
                text.as_ptr(),
                text.len() as u32,
                &mut written,
                std::ptr::null_mut(),
            );
        }
    }
}

/// Attach to the invoking console so a `windows_subsystem = "windows"`
/// binary can still behave like a CLI when arguments are supplied.
pub fn attach() {
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
            // Launched from Explorer/shortcut: create a console for the CLI
            // output rather than silently swallowing it.
            AllocConsole();
        }
    }
}

pub fn out(text: &str) {
    write(STD_OUTPUT_HANDLE, text);
}

pub fn err(text: &str) {
    write(STD_ERROR_HANDLE, text);
}
