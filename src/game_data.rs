use super::{GameSetting, LogType, GameShowType};
use slog::Logger;
use sloggers::Build;
use sloggers::file::FileLoggerBuilder;
use sloggers::null::NullLoggerBuilder;
use sloggers::terminal::{TerminalLoggerBuilder, Destination};
use vte::Perform;
use std::str;
use std::default::Default;
#[derive(Copy, Clone, Debug)]
struct Cursor {
    x: usize,
    y: usize,
}
impl Cursor {
    fn new(x: usize, y: usize) -> Cursor {
        Cursor { x: x, y: y }
    }
}
impl Default for Cursor {
    fn default() -> Cursor {
        Cursor { x: 0, y: 0 }
    }
}
#[derive(Debug)]
pub struct GameData {
    buf: Vec<Vec<u8>>,
    cur: Cursor,
    height: usize,
    width: usize,
    mode: TermMode,
    logger: Logger,
    show_type: GameShowType,
}


impl GameData {
    pub fn from_setting(s: &GameSetting) -> GameData {
        GameData {
            buf: vec![vec![b' '; s.columns]; s.lines],
            cur: Cursor::default(),
            height: s.lines,
            width: s.lines,
            mode: TermMode::default(),
            logger: match s.debug_log {
                        LogType::File((ref name, level)) => {
                            FileLoggerBuilder::new(&name).level(level).build()
                        }
                        LogType::Stdout(level) => {
                            TerminalLoggerBuilder::new()
                                .destination(Destination::Stdout)
                                .level(level)
                                .build()
                        }
                        LogType::Stderr(level) => {
                            TerminalLoggerBuilder::new()
                                .destination(Destination::Stderr)
                                .level(level)
                                .build()
                        }
                        LogType::None => NullLoggerBuilder {}.build(),
                    }
                    .ok()
                    .unwrap(),
            show_type: s.game_show,
        }
    }
    fn is_cursor_valid(&self) -> bool {
        self.cur.y < self.height && self.cur.x < self.width
    }
    fn assert_cursor(&self) {
        assert!(self.is_cursor_valid(), "Cursor has invalid val!");
    }
    fn add_x(&mut self, num: usize) {
        self.cur.x += num;
        assert!(self.cur.x < self.width);
    }
    fn add_y(&mut self, num: usize) {
        self.cur.x += num;
        assert!(self.cur.y < self.height);
    }
    fn sub_x(&mut self, num: usize) {
        assert!(self.cur.x >= num);
        self.cur.x -= num;
    }
    fn sub_y(&mut self, num: usize) {
        assert!(self.cur.y >= num);
        self.cur.y -= num;
    }
    fn goto_x(&mut self, num: usize) {
        self.cur.x = num;
        assert!(self.cur.x < self.width);
    }
    fn goto_y(&mut self, num: usize) {
        self.cur.y = num;
        assert!(self.cur.y < self.height);
    }
    fn goto(&mut self, c: Cursor) {
        self.cur = c;
    }
    fn clear_scr(&mut self, mode: ClearMode) {}
    fn clear_line(&mut self, mode: LineClearMode) {}
    fn scroll_up(&mut self, num: usize) {}
    fn scroll_down(&mut self, num: usize) {}
}


impl Perform for GameData {
    // draw
    fn print(&mut self, c: char) {
        self.assert_cursor();
        assert!(c.is_ascii(), "Non Ascii char Input!");
        self.buf[self.cur.y][self.cur.x] = c as u8;
        self.cur.x += 1;
    }
    // C0orC1
    fn execute(&mut self, byte: u8) {
        match byte {
            C0::BS => self.cur.x -= 1, // backspace
            C0::CR => self.goto_x(0),
            C0::LF | C0::VT | C0::FF => self.cur.y += 1, // linefeed
            C1::NEL => {
                self.cur.y += 1;
                if self.mode.contains(TermMode::LINE_FEED_NEW_LINE) {
                    self.cur.x = 1;
                }
            }
            _ => debug!(self.logger, "[unhandled] execute byte={:02x}", byte),
        }
    }

    fn csi_dispatch(&mut self, args: &[i64], intermediates: &[u8], _ignore: bool, action: char) {
        let private = intermediates.get(0).map(|b| *b == b'?').unwrap_or(false);
        macro_rules! unhandled {
            () => {{
                debug!(self.logger, "[unhandled! (CSI)] action={:?}, args={:?}, intermediates={:?}",
                             action, args, intermediates);
                return;
            }}
        }
        let args_or =
            |id: usize, default: i64| -> i64 { if id >= args.len() { default } else { args[id] } };
        trace!(self.logger, "[trace! (CSI)] action: {}", action);
        match action {
            '@' => {
                let num = args_or(0, 1);

            }
            'A' => self.sub_y(args_or(0, 1) as usize),
            'B' | 'e' => self.add_y(args_or(0, 1) as usize),
            'C' | 'a' => self.add_x(args_or(0, 1) as usize),
            'D' => self.sub_x(args_or(0, 1) as usize),
            'E' => {
                self.add_y(args_or(0, 1) as usize);
                self.goto_x(0);
            }
            'F' => {
                self.sub_y(args_or(0, 1) as usize);
                self.goto_x(0);
            }
            'G' | '`' => self.goto_x(args_or(0, 1) as usize - 1),
            'H' | 'f' => {
                let y = args_or(0, 1) as usize - 1;
                let x = args_or(1, 1) as usize - 1;
                self.goto(Cursor::new(x, y));
            }
            'J' => {
                let mode = match args_or(0, 1) {
                    0 => ClearMode::Below,
                    1 => ClearMode::Above,
                    2 => ClearMode::All,
                    3 => ClearMode::Saved,
                    _ => unhandled!(),
                };
                self.clear_scr(mode);
            }
            'K' => {
                let mode = match args_or(0, 1) {
                    0 => LineClearMode::Right,
                    1 => LineClearMode::Left,
                    2 => LineClearMode::All,
                    _ => unhandled!(),
                };
                self.clear_line(mode);
            }
            'S' => self.scroll_up(args_or(0, 1) as usize),
            'T' => self.scroll_down(args_or(0, 1) as usize),
            'L' => {}
            _ => {}
        }
    }
    fn esc_dispatch(&mut self, params: &[i64], intermediates: &[u8], ignore: bool, byte: u8) {}
    // unsupported now
    fn osc_dispatch(&mut self, params: &[&[u8]]) {
        debug!(
            self.logger,
            "[ignored! (osc_dispatch)]: {}",
            str::from_utf8(params[0]).unwrap()
        );
    }
    fn hook(&mut self, params: &[i64], intermediates: &[u8], ignore: bool) {
        debug!(
            self.logger,
            "[unhandled! (hook)] params={:?}, ints: {:?}, ignore: {:?}",
            params,
            intermediates,
            ignore
        );
    }
    fn put(&mut self, byte: u8) {
        debug!(self.logger, "[unhandled! (put)] byte={:?}", byte);
    }
    fn unhook(&mut self) {
        debug!(self.logger, "[unhandled! (unhook)]");
    }
}


// below, from awesome https://github.com/jwilm/alacritty. Many thanks!
bitflags! {
    pub struct TermMode: u16 {
        const SHOW_CURSOR         = 0b000000000001;
        const APP_CURSOR          = 0b000000000010;
        const APP_KEYPAD          = 0b000000000100;
        const MOUSE_REPORT_CLICK  = 0b000000001000;
        const BRACKETED_PASTE     = 0b000000010000;
        const SGR_MOUSE           = 0b000000100000;
        const MOUSE_MOTION        = 0b000001000000;
        const LINE_WRAP           = 0b000010000000;
        const LINE_FEED_NEW_LINE  = 0b000100000000;
        const ORIGIN              = 0b001000000000;
        const INSERT              = 0b010000000000;
        const FOCUS_IN_OUT        = 0b100000000000;
        const ANY                 = 0b111111111111;
        const NONE                = 0;
    }
}

impl Default for TermMode {
    fn default() -> TermMode {
        TermMode::SHOW_CURSOR | TermMode::LINE_WRAP
    }
}


/// Terminal modes
#[derive(Debug, Eq, PartialEq)]
#[allow(dead_code)]
enum ModeInt {
    /// ?1
    CursorKeys = 1,
    /// Select 80 or 132 columns per page
    ///
    /// CSI ? 3 h -> set 132 column font
    /// CSI ? 3 l -> reset 80 column font
    ///
    /// Additionally,
    ///
    /// * set margins to default positions
    /// * erases all data in page memory
    /// * resets DECLRMM to unavailable
    /// * clears data from the status line (if set to host-writable)
    DECCOLM = 3,
    /// IRM Insert Mode
    ///
    /// NB should be part of non-private mode enum
    ///
    /// * `CSI 4 h` change to insert mode
    /// * `CSI 4 l` reset to replacement mode
    Insert = 4,
    /// ?6
    Origin = 6,
    /// ?7
    LineWrap = 7,
    /// ?12
    BlinkingCursor = 12,
    /// 20
    ///
    /// NB This is actually a private mode. We should consider adding a second
    /// enumeration for public/private modesets.
    LineFeedNewLine = 20,
    /// ?25
    ShowCursor = 25,
    /// ?1000
    ReportMouseClicks = 1000,
    /// ?1002
    ReportMouseMotion = 1002,
    /// ?1004
    ReportFocusInOut = 1004,
    /// ?1006
    SgrMouse = 1006,
    /// ?1049
    SwapScreenAndSetRestoreCursor = 1049,
    /// ?2004
    BracketedPaste = 2004,
}

impl ModeInt {
    /// Create mode from a primitive
    ///
    /// TODO lots of unhandled values..
    fn from_primitive(private: bool, num: i64) -> Option<ModeInt> {
        if private {
            Some(match num {
                1 => ModeInt::CursorKeys,
                3 => ModeInt::DECCOLM,
                6 => ModeInt::Origin,
                7 => ModeInt::LineWrap,
                12 => ModeInt::BlinkingCursor,
                25 => ModeInt::ShowCursor,
                1000 => ModeInt::ReportMouseClicks,
                1002 => ModeInt::ReportMouseMotion,
                1004 => ModeInt::ReportFocusInOut,
                1006 => ModeInt::SgrMouse,
                1049 => ModeInt::SwapScreenAndSetRestoreCursor,
                2004 => ModeInt::BracketedPaste,
                _ => return None,
            })
        } else {
            Some(match num {
                4 => ModeInt::Insert,
                20 => ModeInt::LineFeedNewLine,
                _ => return None,
            })
        }
    }
}

/// Mode for clearing line
///
/// Relative to cursor
#[derive(Debug, Clone, Copy)]
enum LineClearMode {
    /// Clear right of cursor
    Right,
    /// Clear left of cursor
    Left,
    /// Clear entire line
    All,
}

/// Mode for clearing terminal
///
/// Relative to cursor
#[derive(Debug, Clone, Copy)]
enum ClearMode {
    /// Clear below cursor
    Below,
    /// Clear above cursor
    Above,
    /// Clear entire terminal
    All,
    /// Clear 'saved' lines (scrollback)
    Saved,
}
/// C0 set of 7-bit control characters (from ANSI X3.4-1977).
#[allow(non_snake_case, dead_code)]
mod C0 {
    /// Null filler, terminal should ignore this character
    pub const NUL: u8 = 0x00;
    /// Start of Header
    pub const SOH: u8 = 0x01;
    /// Start of Text, implied end of header
    pub const STX: u8 = 0x02;
    /// End of Text, causes some terminal to respond with ACK or NAK
    pub const ETX: u8 = 0x03;
    /// End of Transmission
    pub const EOT: u8 = 0x04;
    /// Enquiry, causes terminal to send ANSWER-BACK ID
    pub const ENQ: u8 = 0x05;
    /// Acknowledge, usually sent by terminal in response to ETX
    pub const ACK: u8 = 0x06;
    /// Bell, triggers the bell, buzzer, or beeper on the terminal
    pub const BEL: u8 = 0x07;
    /// Backspace, can be used to define overstruck characters
    pub const BS: u8 = 0x08;
    /// Horizontal Tabulation, move to next predetermined position
    pub const HT: u8 = 0x09;
    /// Linefeed, move to same position on next line (see also NL)
    pub const LF: u8 = 0x0A;
    /// Vertical Tabulation, move to next predetermined line
    pub const VT: u8 = 0x0B;
    /// Form Feed, move to next form or page
    pub const FF: u8 = 0x0C;
    /// Carriage Return, move to first character of current line
    pub const CR: u8 = 0x0D;
    /// Shift Out, switch to G1 (other half of character set)
    pub const SO: u8 = 0x0E;
    /// Shift In, switch to G0 (normal half of character set)
    pub const SI: u8 = 0x0F;
    /// Data Link Escape, interpret next control character specially
    pub const DLE: u8 = 0x10;
    /// (DC1) Terminal is allowed to resume transmitting
    pub const XON: u8 = 0x11;
    /// Device Control 2, causes ASR-33 to activate paper-tape reader
    pub const DC2: u8 = 0x12;
    /// (DC2) Terminal must pause and refrain from transmitting
    pub const XOFF: u8 = 0x13;
    /// Device Control 4, causes ASR-33 to deactivate paper-tape reader
    pub const DC4: u8 = 0x14;
    /// Negative Acknowledge, used sometimes with ETX and ACK
    pub const NAK: u8 = 0x15;
    /// Synchronous Idle, used to maintain timing in Sync communication
    pub const SYN: u8 = 0x16;
    /// End of Transmission block
    pub const ETB: u8 = 0x17;
    /// Cancel (makes VT100 abort current escape sequence if any)
    pub const CAN: u8 = 0x18;
    /// End of Medium
    pub const EM: u8 = 0x19;
    /// Substitute (VT100 uses this to display parity errors)
    pub const SUB: u8 = 0x1A;
    /// Prefix to an escape sequence
    pub const ESC: u8 = 0x1B;
    /// File Separator
    pub const FS: u8 = 0x1C;
    /// Group Separator
    pub const GS: u8 = 0x1D;
    /// Record Separator (sent by VT132 in block-transfer mode)
    pub const RS: u8 = 0x1E;
    /// Unit Separator
    pub const US: u8 = 0x1F;
    /// Delete, should be ignored by terminal
    pub const DEL: u8 = 0x7f;
}


/// C1 set of 8-bit control characters (from ANSI X3.64-1979)
///
/// 0x80 (@), 0x81 (A), 0x82 (B), 0x83 (C) are reserved
/// 0x98 (X), 0x99 (Y) are reserved
/// 0x9a (Z) is 'reserved', but causes DEC terminals to respond with DA codes
#[allow(non_snake_case, dead_code)]
mod C1 {
    /// Reserved
    pub const PAD: u8 = 0x80;
    /// Reserved
    pub const HOP: u8 = 0x81;
    /// Reserved
    pub const BPH: u8 = 0x82;
    /// Reserved
    pub const NBH: u8 = 0x83;
    /// Index, moves down one line same column regardless of NL
    pub const IND: u8 = 0x84;
    /// New line, moves done one line and to first column (CR+LF)
    pub const NEL: u8 = 0x85;
    /// Start of Selected Area to be sent to auxiliary output device
    pub const SSA: u8 = 0x86;
    /// End of Selected Area to be sent to auxiliary output device
    pub const ESA: u8 = 0x87;
    /// Horizontal Tabulation Set at current position
    pub const HTS: u8 = 0x88;
    /// Hor Tab Justify, moves string to next tab position
    pub const HTJ: u8 = 0x89;
    /// Vertical Tabulation Set at current line
    pub const VTS: u8 = 0x8A;
    /// Partial Line Down (subscript)
    pub const PLD: u8 = 0x8B;
    /// Partial Line Up (superscript)
    pub const PLU: u8 = 0x8C;
    /// Reverse Index, go up one line, reverse scroll if necessary
    pub const RI: u8 = 0x8D;
    /// Single Shift to G2
    pub const SS2: u8 = 0x8E;
    /// Single Shift to G3 (VT100 uses this for sending PF keys)
    pub const SS3: u8 = 0x8F;
    /// Device Control String, terminated by ST (VT125 enters graphics)
    pub const DCS: u8 = 0x90;
    /// Private Use 1
    pub const PU1: u8 = 0x91;
    /// Private Use 2
    pub const PU2: u8 = 0x92;
    /// Set Transmit State
    pub const STS: u8 = 0x93;
    /// Cancel character, ignore previous character
    pub const CCH: u8 = 0x94;
    /// Message Waiting, turns on an indicator on the terminal
    pub const MW: u8 = 0x95;
    /// Start of Protected Area
    pub const SPA: u8 = 0x96;
    /// End of Protected Area
    pub const EPA: u8 = 0x97;
    /// SOS
    pub const SOS: u8 = 0x98;
    /// SGCI
    pub const SGCI: u8 = 0x99;
    /// DECID - Identify Terminal
    pub const DECID: u8 = 0x9a;
    /// Control Sequence Introducer
    pub const CSI: u8 = 0x9B;
    /// String Terminator (VT125 exits graphics)
    pub const ST: u8 = 0x9C;
    /// Operating System Command (reprograms intelligent terminal)
    pub const OSC: u8 = 0x9D;
    /// Privacy Message (password verification), terminated by ST
    pub const PM: u8 = 0x9E;
    /// Application Program Command (to word processor), term by ST
    pub const APC: u8 = 0x9F;
}
