use std::{ffi::c_void, io, mem::MaybeUninit};

type Handle = *mut c_void;
const STD_INPUT_HANDLE: u32 = -10_i32 as u32;
const STD_OUTPUT_HANDLE: u32 = -11_i32 as u32;
const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
const ENABLE_QUICK_EDIT_MODE: u32 = 0x0040;
const ENABLE_EXTENDED_FLAGS: u32 = 0x0080;
const ENABLE_LINE_INPUT: u32 = 0x0002;
const ENABLE_ECHO_INPUT: u32 = 0x0004;
const ENABLE_PROCESSED_OUTPUT: u32 = 0x0001;
const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;
const UTF8_CODE_PAGE: u32 = 65001;

#[repr(C)]
#[derive(Clone, Copy)]
struct Coord {
    x: i16,
    y: i16,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct SmallRect {
    left: i16,
    top: i16,
    right: i16,
    bottom: i16,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct ScreenBufferInfo {
    size: Coord,
    cursor_position: Coord,
    attributes: u16,
    window: SmallRect,
    maximum_window_size: Coord,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(kind: u32) -> Handle;
    fn GetConsoleMode(handle: Handle, mode: *mut u32) -> i32;
    fn SetConsoleMode(handle: Handle, mode: u32) -> i32;
    fn GetConsoleScreenBufferInfo(handle: Handle, info: *mut ScreenBufferInfo) -> i32;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
    fn ReadFile(
        handle: Handle,
        buffer: *mut c_void,
        bytes_to_read: u32,
        bytes_read: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn GetConsoleCP() -> u32;
    fn GetConsoleOutputCP() -> u32;
    fn SetConsoleCP(page: u32) -> i32;
    fn SetConsoleOutputCP(page: u32) -> i32;
}

pub fn read_byte(timeout_ms: u32) -> io::Result<Option<u8>> {
    let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_TIMEOUT: u32 = 258;
    match unsafe { WaitForSingleObject(handle, timeout_ms) } {
        WAIT_TIMEOUT => return Ok(None),
        WAIT_OBJECT_0 => {}
        _ => return Err(io::Error::last_os_error()),
    }
    let mut value = 0_u8;
    let mut read = 0_u32;
    if unsafe {
        ReadFile(
            handle,
            (&mut value as *mut u8).cast(),
            1,
            &mut read,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok((read == 1).then_some(value))
}

pub struct Modes {
    input: u32,
    output: u32,
    input_page: u32,
    output_page: u32,
}

pub fn enter() -> io::Result<Modes> {
    let input_handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    let output_handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    let mut input = 0;
    let mut output = 0;
    if unsafe { GetConsoleMode(input_handle, &mut input) } == 0
        || unsafe { GetConsoleMode(output_handle, &mut output) } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let modes = Modes {
        input,
        output,
        input_page: unsafe { GetConsoleCP() },
        output_page: unsafe { GetConsoleOutputCP() },
    };
    if unsafe {
        SetConsoleMode(
            output_handle,
            output | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if unsafe {
        SetConsoleMode(
            input_handle,
            (input
                & !(ENABLE_LINE_INPUT
                    | ENABLE_ECHO_INPUT
                    | ENABLE_PROCESSED_INPUT
                    | ENABLE_QUICK_EDIT_MODE))
                | ENABLE_VIRTUAL_TERMINAL_INPUT
                | ENABLE_EXTENDED_FLAGS,
        )
    } == 0
    {
        unsafe { SetConsoleMode(output_handle, output) };
        return Err(io::Error::last_os_error());
    }
    unsafe {
        SetConsoleCP(UTF8_CODE_PAGE);
        SetConsoleOutputCP(UTF8_CODE_PAGE);
    }
    Ok(modes)
}

pub fn restore(modes: &Modes) {
    unsafe {
        SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), modes.input);
        SetConsoleMode(GetStdHandle(STD_OUTPUT_HANDLE), modes.output);
        SetConsoleCP(modes.input_page);
        SetConsoleOutputCP(modes.output_page);
    }
}

pub fn terminal_size() -> Option<(usize, usize)> {
    let mut info = MaybeUninit::<ScreenBufferInfo>::uninit();
    if unsafe { GetConsoleScreenBufferInfo(GetStdHandle(STD_OUTPUT_HANDLE), info.as_mut_ptr()) }
        == 0
    {
        return None;
    }
    let window = unsafe { info.assume_init() }.window;
    let width = i32::from(window.right) - i32::from(window.left) + 1;
    let height = i32::from(window.bottom) - i32::from(window.top) + 1;
    (width > 0 && height > 0).then_some((width as usize, height as usize))
}
