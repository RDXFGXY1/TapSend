use std::{
    io::{self, Read, Write},
    process::Command,
};

pub enum Key {
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

pub struct RawTerminal;
impl RawTerminal {
    pub fn enter() -> io::Result<Self> {
        let status = Command::new("stty")
            .args(["-echo", "-icanon", "min", "0", "time", "2"])
            .status()?;
        if !status.success() {
            return Err(io::Error::other("could not enable terminal raw mode"));
        }
        print!("\x1b[?1049h\x1b[?25l\x1b[2J");
        io::stdout().flush()?;
        Ok(Self)
    }
    pub fn read_key(&self) -> io::Result<Key> {
        let mut first = [0_u8; 1];
        if io::stdin().read(&mut first)? == 0 {
            return Ok(Key::Other);
        }
        Ok(match first[0] {
            b':' | b';' => Key::Command,
            b'?' => Key::Help,
            b'r' | b'R' => Key::RefreshPairing,
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
            b'q' | b'Q' => Key::Quit,
            8 | 127 => Key::Back,
            b'k' => Key::Up,
            b'j' => Key::Down,
            27 => {
                let mut tail = [0_u8; 2];
                if io::stdin().read_exact(&mut tail).is_ok() && tail[0] == b'[' {
                    match tail[1] {
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
        let mut first = [0_u8; 1];
        if io::stdin().read(&mut first)? == 0 {
            return Ok(ModalKey::Other);
        }
        Ok(match first[0] {
            b'\n' | b'\r' => ModalKey::Enter,
            b'\t' => ModalKey::Tab,
            8 | 127 => ModalKey::Backspace,
            27 => {
                let mut tail = [0_u8; 2];
                if io::stdin().read_exact(&mut tail).is_ok() && tail[0] == b'[' {
                    match tail[1] {
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
                    if io::stdin().read_exact(&mut bytes[1..=extra]).is_ok() {
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
impl Drop for RawTerminal {
    fn drop(&mut self) {
        print!("\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = io::stdout().flush();
        let _ = Command::new("stty").arg("sane").status();
    }
}
