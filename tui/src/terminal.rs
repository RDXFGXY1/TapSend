#[cfg(windows)]
use crate::windows_console;
#[cfg(not(windows))]
use std::io::Read;
use std::io::{self, Write};
#[cfg(not(windows))]
use std::process::Command;

pub enum Key {
    View(usize),
    NextView,
    Compose,
    Receive,
    Up,
    Down,
    Enter,
    Back,
    Space,
    Start,
    Hidden,
    Add,
    ToggleTransfers,
    PairInfo,
    Files,
    Devices,
    Quit,
    Other,
    Command,
    Help,
    RefreshPairing,
    Settings,
    #[cfg(windows)]
    ScanCamera,
}

pub enum ModalKey {
    Char(char),
    Up,
    Down,
    Enter,
    Backspace,
    Escape,
    Tab,
    Other,
}

pub struct RawTerminal {
    #[cfg(not(windows))]
    saved_mode: String,
    #[cfg(windows)]
    modes: windows_console::Modes,
}
impl RawTerminal {
    pub fn enter() -> io::Result<Self> {
        #[cfg(windows)]
        let modes = windows_console::enter()?;
        #[cfg(not(windows))]
        let saved_mode = {
            let saved = Command::new("stty")
                .arg("-g")
                .stdin(std::process::Stdio::inherit())
                .output()?;
            if !saved.status.success() {
                return Err(io::Error::other("stdin must be a terminal"));
            }
            let saved = String::from_utf8_lossy(&saved.stdout).trim().to_string();
            let status = Command::new("stty")
                .args(["-echo", "-icanon", "-isig", "min", "0", "time", "2"])
                .status()?;
            if !status.success() {
                return Err(io::Error::other("could not enable terminal raw mode"));
            }
            saved
        };
        print!("\x1b[?1049h\x1b[?25l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[2J");
        io::stdout().flush()?;
        Ok(Self {
            #[cfg(not(windows))]
            saved_mode,
            #[cfg(windows)]
            modes,
        })
    }
    pub fn read_key(&self) -> io::Result<Key> {
        let Some(first) = read_byte(false)? else {
            return Ok(Key::Other);
        };
        Ok(match first {
            b'1'..=b'5' => Key::View((first - b'1') as usize),
            b'0' => Key::View(0),
            b'6' => Key::Receive,
            b'\t' => Key::NextView,
            b'm' | b'M' => Key::Compose,
            b':' | b';' => Key::Command,
            b'?' => Key::Help,
            b'r' | b'R' => Key::RefreshPairing,
            b'g' | b'G' => Key::Settings,
            #[cfg(windows)]
            b'c' | b'C' => Key::ScanCamera,
            b'\n' | b'\r' => Key::Enter,
            b' ' => Key::Space,
            b's' | b'S' => Key::Start,
            b'a' | b'A' => Key::Add,
            b't' | b'T' => Key::ToggleTransfers,
            b'p' | b'P' => Key::PairInfo,
            b'f' | b'F' => Key::Files,
            b'd' | b'D' => Key::Devices,
            b'h' | b'H' => Key::Hidden,
            b'l' => Key::Enter,
            3 | b'q' | b'Q' => Key::Quit,
            8 | 127 => Key::Back,
            b'k' => Key::Up,
            b'j' => Key::Down,
            27 => {
                if let Some(direction) = read_escape_direction()? {
                    match direction {
                        b'A' => Key::Up,
                        b'B' => Key::Down,
                        _ => Key::Other,
                    }
                } else {
                    Key::Other
                }
            }
            _ => Key::Other,
        })
    }

    pub fn read_modal_key(&self) -> io::Result<ModalKey> {
        let Some(first) = read_byte(false)? else {
            return Ok(ModalKey::Other);
        };
        Ok(match first {
            3 => ModalKey::Escape,
            b'\n' | b'\r' => ModalKey::Enter,
            b'\t' => ModalKey::Tab,
            8 | 127 => ModalKey::Backspace,
            27 => {
                if let Some(direction) = read_escape_direction()? {
                    match direction {
                        b'A' => ModalKey::Up,
                        b'B' => ModalKey::Down,
                        _ => ModalKey::Other,
                    }
                } else {
                    ModalKey::Escape
                }
            }
            byte if byte.is_ascii_graphic() || byte == b' ' => ModalKey::Char(byte as char),
            byte if byte >= 0xC2 => {
                let extra = match byte {
                    0xC2..=0xDF => 1,
                    0xE0..=0xEF => 2,
                    0xF0..=0xF4 => 3,
                    _ => 0,
                };
                if extra == 0 {
                    ModalKey::Other
                } else {
                    let mut bytes = [0_u8; 4];
                    bytes[0] = byte;
                    let mut complete = true;
                    for slot in &mut bytes[1..=extra] {
                        if let Some(value) = read_byte(true)? {
                            *slot = value;
                        } else {
                            complete = false;
                            break;
                        }
                    }
                    if complete {
                        std::str::from_utf8(&bytes[..=extra])
                            .ok()
                            .and_then(|value| value.chars().next())
                            .map(ModalKey::Char)
                            .unwrap_or(ModalKey::Other)
                    } else {
                        ModalKey::Other
                    }
                }
            }
            _ => ModalKey::Other,
        })
    }
}

fn read_byte(timeout: bool) -> io::Result<Option<u8>> {
    #[cfg(windows)]
    {
        windows_console::read_byte(if timeout { 100 } else { 200 })
    }
    #[cfg(not(windows))]
    {
        let _ = timeout;
        let mut byte = [0_u8; 1];
        Ok((io::stdin().read(&mut byte)? == 1).then_some(byte[0]))
    }
}

fn read_escape_direction() -> io::Result<Option<u8>> {
    escape_sequence(|| read_byte(true))
}

// Consume the entire CSI/SS3 report: mouse coordinates and modifiers are not keys.
fn escape_sequence(mut next: impl FnMut() -> io::Result<Option<u8>>) -> io::Result<Option<u8>> {
    let Some(prefix) = next()? else {
        return Ok(None);
    };
    if prefix != b'[' && prefix != b'O' {
        return Ok(Some(0));
    }
    let mut parameters = Vec::new();
    for _ in 0..128 {
        let Some(byte) = next()? else {
            return Ok(Some(0));
        };
        if (0x40..=0x7e).contains(&byte) {
            // Legacy X10 mouse reports have three bytes after CSI M.
            if byte == b'M' && parameters.is_empty() && prefix == b'[' {
                for _ in 0..3 {
                    let _ = next()?;
                }
            }
            return Ok(Some(
                if matches!(byte, b'A' | b'B' | b'C' | b'D')
                    && parameters
                        .iter()
                        .all(|b: &u8| b.is_ascii_digit() || *b == b';')
                {
                    byte
                } else {
                    0
                },
            ));
        }
        parameters.push(byte);
    }
    Ok(Some(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consumes_mouse_reports_without_leaking_command_characters() {
        let mut input = b"[<0;120;35Mq".iter().copied();
        assert_eq!(escape_sequence(|| Ok(input.next())).unwrap(), Some(0));
        assert_eq!(input.next(), Some(b'q'));
        let mut input = b"[1;5Aq".iter().copied();
        assert_eq!(escape_sequence(|| Ok(input.next())).unwrap(), Some(b'A'));
        assert_eq!(input.next(), Some(b'q'));
    }
    #[test]
    fn consumes_legacy_mouse_and_distinguishes_escape() {
        let mut input = b"[M !!q".iter().copied();
        assert_eq!(escape_sequence(|| Ok(input.next())).unwrap(), Some(0));
        assert_eq!(input.next(), Some(b'q'));
        assert_eq!(escape_sequence(|| Ok(None)).unwrap(), None);
    }
}
impl Drop for RawTerminal {
    fn drop(&mut self) {
        print!("\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = io::stdout().flush();
        #[cfg(windows)]
        windows_console::restore(&self.modes);
        #[cfg(not(windows))]
        let _ = Command::new("stty").arg(&self.saved_mode).status();
    }
}
