#![no_std]
#![no_main]

use core::convert::Infallible;
use embedded_io::Read;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::usb_serial_jtag::{UsbSerialJtag, UsbSerialJtagRx, UsbSerialJtagTx};
use esp_hal::{main, Blocking};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

const MAGIC_WORD: &[u8; 6] = b"DMC53:";
const SELF_ID: &[u8; 3] = b"000";

const BUFFER_SIZE: usize = 512;

const COMMAND_WIDTH: usize = 4;

const MAX_BINARY_LEN: usize = 1024;

/// Maimum number of attempts to get a meaningful response
const MAX_ATTEMPTS: u8 = 3;

/// Instruction set
mod is {
    pub const STOP: &[u8] = b"STOP";
    pub const IDENTITY: &[u8] = b"IDFY";
    pub const WAIT: &[u8] = b"WAIT";
    pub const DATA: &[u8] = b"DATA";
    pub const DATA_END: &[u8] = b"DEND";
    pub const ATTN: &[u8] = b"ATTN";
    pub const FINI: &[u8] = b"FINI";

    pub enum Instruction {
        Stop,
        Identity,
        Wait,
        Data,
        Dend,
        Attn,
        Finish,
    }

    impl Instruction {
        pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
            match bytes {
                STOP => Some(Self::Stop),
                IDENTITY => Some(Self::Identity),
                WAIT => Some(Self::Wait),
                DATA => Some(Self::Data),
                DATA_END => Some(Self::Dend),
                ATTN => Some(Self::Attn),
                FINI => Some(Self::Finish),
                _ => None,
            }
        }
    }
}

mod msgs {
    pub const RDY: &[u8] = b"RDY";
    pub const BYE: &[u8] = b"BYE";
    pub const END: &[u8] = b"END";

    pub enum Message {
        Ready,
        Bye,
        End,
    }

    impl Message {
        pub fn to_bytes(&self) -> &[u8] {
            match self {
                Message::Ready => RDY,
                Message::Bye => BYE,
                Message::End => END,
            }
        }
    }
}

use msgs::Message;

struct UsbChannel<'a> {
    rx: UsbSerialJtagRx<'a, Blocking>,
    tx: UsbSerialJtagTx<'a, Blocking>,
    buffer: [u8; BUFFER_SIZE],
    buf_pos: usize, // number of meaningful bytes in the buffer
}

impl<'a> UsbChannel<'a> {
    fn write(&mut self, msg: &[u8]) {
        self.tx.write(msg).expect("Error while writing");
    }

    fn write_buffer(&mut self) {
        let this = self as *mut Self;
        unsafe {
            // (*this).write(&(*this).buffer[..(*this).buf_pos]);
            (*this).write(&(&(*this).buffer)[..(*this).buf_pos]);
        }
    }

    fn read(&mut self, buff: &mut [u8]) -> usize {
        self.rx.read(buff).expect("Error while reading")
    }

    fn read_buffer(&mut self) {
        let this = self as *mut Self;
        unsafe {
            let s = (*this).read(&mut (*this).buffer);
            self.buf_pos = s;
        }
    }

    fn read_byte(&mut self) -> Result<u8, nb::Error<Infallible>> {
        return self.rx.read_byte();
    }

    fn announce(&mut self) {
        //-> usize {
        let delay = Delay::new();
        //        loop {
        self.write(MAGIC_WORD);
        self.write(SELF_ID);
        self.write(b" Ready_");

        // if let Ok(waiting_byte) = self.read_byte() {
        //     self.buffer[0] = waiting_byte;
        //     self.buf_pos = self.rx.read(&mut self.buffer[1..]).unwrap() + 1;
        //     return self.buf_pos;
        //}
        delay.delay_millis(500_u32);
        //}
    }

    fn repeat_rcv(&mut self) {
        self.write(b"RCV:( ");
        self.write_buffer();
        self.write(b") ");
    }

    fn buffer_is_equal(&self, arr: &[u8]) -> bool {
        if arr.len() != self.buf_pos {
            return false;
        }
        let mut matched = true;
        for ind in 0..self.buf_pos {
            matched &= self.buffer[ind] == arr[ind];
        }
        return matched;
    }

    fn find_line(&self) -> Option<usize> {
        for i in 0..self.buf_pos {
            if self.buffer[i] == b'\n' {
                return Some(i + 1);
            }
        }
        None
    }

    fn consume(&mut self, n: usize) {
        if n >= self.buf_pos {
            self.buf_pos = 0;
            return;
        }

        let remaining = self.buf_pos - n;

        for i in 0..remaining {
            self.buffer[i] = self.buffer[n + i];
        }

        self.buf_pos = remaining;
    }
}

enum WaitState {
    Line,
    Binary { remaining: usize },
}

enum Mode {
    Main,
    Interactive,
    Wait(WaitState),
}

enum WaitCommand<'a> {
    Text(&'a [u8]),
    Data(&'a [u8]),
    Binr { len: usize },
    Dend,
}

enum ParseError {
    Empty,
    UnknownCommand,
    MissingArgument,
    InvalidNumber,
    NumberOutOfRange,
}

// enum ProtocolError {
//     Incomplete,
//     InvalidInstruction,
//     InvalidWaitCommand,
// }

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let delay = Delay::new();

    let (rx, tx) = UsbSerialJtag::new(peripherals.USB_DEVICE).split();
    let mut comm_device = UsbChannel {
        rx,
        tx,
        buffer: [0_u8; BUFFER_SIZE],
        buf_pos: 0,
    };

    let mut mode = Mode::Main;
    _ = comm_device.announce();

    loop {
        match mode {
            Mode::Main => {
                comm_device.read_buffer();

                if comm_device.buf_pos == COMMAND_WIDTH {
                    match is::Instruction::from_bytes(&comm_device.buffer[..COMMAND_WIDTH]) {
                        Some(is::Instruction::Attn) => {
                            //comm_device.write(b"\nINFO: ATTN processed\n");
                            comm_device.write(Message::Ready.to_bytes());
                            comm_device.buf_pos = 0;
                            mode = Mode::Interactive;
                        }

                        Some(_) => {
                            // A valid instruction, but not the one that should
                            // enter interactive mode.
                            comm_device.write(b"\nERR: Command ignored\n");
                            comm_device.write(b"\nERR: ATTN for interactive mode\n");
                            comm_device.repeat_rcv();
                            comm_device.buf_pos = 0;
                        }

                        None => {
                            comm_device.repeat_rcv();
                            comm_device.buf_pos = 0;
                        }
                    }
                }
            }

            Mode::Interactive => {
                comm_device.read_buffer();
                if comm_device.buf_pos < COMMAND_WIDTH {
                    comm_device.tx.flush_tx().unwrap();
                    //comm_device.write(b"INFO: passed\n");
                    // Process???
                    continue;
                }

                let inst = is::Instruction::from_bytes(&comm_device.buffer[..COMMAND_WIDTH]);
                //comm_device.write(b"RX: inst\n");
                match inst {
                    Some(is::Instruction::Identity) => {
                        //comm_device.tx.flush_tx().unwrap();
                        comm_device.write(MAGIC_WORD);
                        comm_device.write(SELF_ID);
                        comm_device.buf_pos = 0;
                    }

                    Some(is::Instruction::Finish) => {
                        comm_device.write(Message::Bye.to_bytes());
                        comm_device.buf_pos = 0;
                        delay.delay_millis(200_u32);

                        mode = Mode::Main;
                    }

                    Some(_) | None => {
                        comm_device.write(b"ERR: Unrecognized instruction");
                        comm_device.buf_pos = 0;

                        // Default: Stay in interactive mode
                        mode = Mode::Interactive;
                    }
                }
            }

            Mode::Wait(WaitState::Line) => {
                comm_device.read_buffer();

                if let Some(line_len) = comm_device.find_line() {
                    let line = &comm_device.buffer[..line_len];

                    match parse_wait_line(line) {
                        Ok(WaitCommand::Text(text)) => {
                            //handle_text(text);
                            //comm_device.write(text);
                            comm_device.write(b"\nOK\n");
                        }

                        Ok(WaitCommand::Data(tokens)) => match handle_data_decimal(tokens) {
                            Ok(()) => comm_device.write(b"OK\n"),
                            Err(ParseError::NumberOutOfRange) => {
                                comm_device.write(b"ERR: OUT_OF_RANGE\n")
                            }
                            Err(_) => comm_device.write(b"ERR: BAD NUMBER"),
                        },

                        Ok(WaitCommand::Binr { len }) => {
                            if len > MAX_BINARY_LEN {
                                comm_device.write(b"ERR TOO_LARGE\n");
                            } else {
                                comm_device.write(b"READY BINR\n");
                                mode = Mode::Wait(WaitState::Binary { remaining: len });
                            }
                        }

                        Ok(WaitCommand::Dend) => {
                            comm_device.write(b"OK: DEND");
                            mode = Mode::Interactive;
                        }

                        Err(ParseError::Empty) => {
                            // ignore empty lines.
                        }

                        Err(ParseError::UnknownCommand) => {
                            comm_device.write(b"ERR: Unknown command\n");
                        }

                        Err(_) => {
                            comm_device.write(b"ERR: Invalid WAIT command");
                            // Abort back to interactive:
                            mode = Mode::Interactive;
                        }
                    }
                }
            }

            Mode::Wait(WaitState::Binary { remaining }) => {
                comm_device.read_buffer();

                let available = comm_device.buf_pos;
                let take = core::cmp::min(available, remaining);

                if take > 0 {
                    //handle_binary_bytes(&comm_device.buffer[..take]);
                    comm_device.consume(take);
                }

                let new_remaining = remaining - take;

                if new_remaining == 0 {
                    comm_device.write(b"OK BINR\n");
                    mode = Mode::Wait(WaitState::Line);
                } else {
                    mode = Mode::Wait(WaitState::Binary {
                        remaining: new_remaining,
                    });
                }
            } //
        }
    }
}

fn parse_wait_line(line: &[u8]) -> Result<WaitCommand<'_>, ParseError> {
    let line = trim_crlf(line);
    let line = trim_ascii_spaces(line);

    if line.is_empty() {
        return Err(ParseError::Empty);
    }

    let (cmd, rest) = split_once_space(line);
    let rest = trim_ascii_spaces(rest);

    if ascii_eq(cmd, b"DEND") {
        return Ok(WaitCommand::Dend);
    }

    if ascii_eq(cmd, b"TEXT") {
        return Ok(WaitCommand::Text(rest));
    }

    if ascii_eq(cmd, b"DATA") {
        if rest.is_empty() {
            return Err(ParseError::MissingArgument);
        }

        return Ok(WaitCommand::Data(rest));
    }

    if ascii_eq(cmd, b"BINR") {
        if rest.is_empty() {
            return Err(ParseError::MissingArgument);
        }

        let len = parse_usize_dec(rest)?;
        return Ok(WaitCommand::Binr { len });
    }

    Err(ParseError::UnknownCommand)
}

// Helpers

fn trim_crlf(mut s: &[u8]) -> &[u8] {
    while let Some((&last, rest)) = s.split_last() {
        if last == b'\n' || last == b'\r' {
            s = rest;
        } else {
            break;
        }
    }

    s
}

fn trim_ascii_spaces(mut s: &[u8]) -> &[u8] {
    while let Some((&first, rest)) = s.split_first() {
        if first == b' ' || first == b'\t' {
            s = rest;
        } else {
            break;
        }
    }

    while let Some((&last, rest)) = s.split_last() {
        if last == b' ' || last == b'\t' {
            s = rest;
        } else {
            break;
        }
    }

    s
}

fn split_once_space(s: &[u8]) -> (&[u8], &[u8]) {
    for i in 0..s.len() {
        if s[i] == b' ' || s[i] == b'\t' {
            return (&s[..i], &s[i + 1..]);
        }
    }

    (s, &[])
}

fn ascii_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    for i in 0..a.len() {
        if a[i].to_ascii_uppercase() != b[i].to_ascii_uppercase() {
            return false;
        }
    }

    true
}

// Dec parsing
fn parse_usize_dec(s: &[u8]) -> Result<usize, ParseError> {
    let s = trim_ascii_spaces(s);

    if s.is_empty() {
        return Err(ParseError::InvalidNumber);
    }

    let mut value: usize = 0;

    for &b in s {
        if !(b'0'..=b'9').contains(&b) {
            return Err(ParseError::InvalidNumber);
        }

        let digit = (b - b'0') as usize;

        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(digit))
            .ok_or(ParseError::NumberOutOfRange)?;
    }

    Ok(value)
}

fn parse_u8_dec(s: &[u8]) -> Result<u8, ParseError> {
    let value = parse_usize_dec(s)?;

    if value > 255 {
        return Err(ParseError::NumberOutOfRange);
    }

    Ok(value as u8)
}

// process tokens
fn for_each_token<F>(mut s: &[u8], mut f: F) -> Result<(), ParseError>
//fn for_each_token<F>(mut s: &[u8], mut f: F) -> F
where
    F: FnMut(&[u8]) -> Result<(), ParseError>,
{
    loop {
        s = trim_ascii_spaces(s);

        if s.is_empty() {
            return Ok(());
        }

        let mut end = s.len();

        for i in 0..end {
            //s.len() {
            if s[i] == b' ' || s[i] == b'\t' {
                end = i;
                break;
            }
        }

        let token = &s[..end];

        f(token)?;

        s = &s[end..];
    }
}

// Example of processing

fn handle_data_decimal(tokens: &[u8]) -> Result<(), ParseError> {
    for_each_token(tokens, |tok| {
        let byte = parse_u8_dec(tok)?;

        //handle_one_data_byte(byte);

        Ok(())
    })
}

// WAIT mode is line-oriented except during BINR transfers.

// Commands:

// 1. TEXT <payload>\n
//    Sends text payload to the device.
//    Payload is all bytes after the command separator up to, but not including,
//    the line ending.

// 2. DATA <b0> <b1> ... <bn>\n
//    Sends decimal byte values.
//    Each value must be in range 0..=255.

// 3. BINR <length>\n
//    Requests binary receive of exactly <length> bytes.
//    The device replies READY BINR\n if the length is acceptable.
//    The sender then transmits exactly <length> raw bytes.
//    The device replies OK BINR\n after all bytes are received.
//    Example: BINR 12\n
//    lafuyew40879689

// 4. DEND\n
//    Ends WAIT mode and returns to interactive mode.
