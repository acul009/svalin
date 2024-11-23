use std::{
    borrow::Cow,
    str::Chars,
    sync::{Arc, Mutex},
};
use thiserror::Error;

use iced::widget::{column, shader::wgpu::naga::front::wgsl::ParseError, text};

use crate::Element;

pub struct AnsiGrid {
    cursor_x: usize,
    cursor_y: usize,
    width: usize,
    height: usize,
    grid: Vec<Symbol>,
    foreground_color: String,
    background_color: String,
    state: ParserState,
    characters_parsed: Arc<Mutex<usize>>,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    c: char,
    foreground_color: String,
    background_color: String,
}

pub enum ParserState {
    Write,
    Escaped,
    Cursor(Vec<usize>, String),
    PrivateMode(String),
    OsMode(Vec<usize>, String),
}

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("not yet implemented")]
    Todo(char),
    #[error("invalid number of arguments for command {0}: {1}")]
    InvalidNumberOfArguments(char, usize),
    #[error("invalid argument for command {0}: {1}")]
    InvalidArgument(char, usize),
    #[error("invalid mode selection: {0}")]
    InvalidMode(usize),
    #[error("unexpected character")]
    UnexpectedCharacter(char),
}

type ParseResult = Result<(), ParserError>;

#[derive(Debug)]
pub struct ParseTrace {
    error: ParserError,
    characters_parsed: usize,
}

impl AnsiGrid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            cursor_x: 0,
            cursor_y: 0,
            width,
            height,
            grid: vec![
                Symbol {
                    c: ' ',
                    foreground_color: String::new(),
                    background_color: String::new()
                };
                width * height
            ],
            foreground_color: String::new(),
            background_color: String::new(),
            state: ParserState::Write,
            characters_parsed: Arc::new(Mutex::new(0)),
        }
    }

    pub fn parse(&mut self, input: &str) -> Result<(), ParseTrace> {
        let counter = self.characters_parsed.clone();
        let mut chars = input.chars().map(move |c| {
            *counter.lock().unwrap() += 1;
            c
        });

        while let Some(c) = chars.next() {
            let result = match &mut self.state {
                ParserState::Write => self.write(c),
                ParserState::Escaped => self.escaped(c),
                ParserState::Cursor(stack, num) => match c {
                    '0'..='9' => {
                        num.push(c);
                        Ok(())
                    }
                    ';' => {
                        stack.push(num.parse().unwrap_or(0));
                        *num = String::new();
                        Ok(())
                    }
                    c => {
                        if !num.is_empty() {
                            stack.push(num.parse().unwrap_or(0));
                            *num = String::new();
                        }
                        self.cursor(c)
                    }
                },
                ParserState::OsMode(stack, num) => match c {
                    '0'..='9' => {
                        num.push(c);
                        Ok(())
                    }
                    ';' => {
                        stack.push(num.parse().unwrap_or(0));
                        *num = String::new();
                        Ok(())
                    }
                    _c => {
                        if !num.is_empty() {
                            stack.push(num.parse().unwrap_or(0));
                            *num = String::new();
                        }

                        // Todo: implement OS mode
                        Ok(())
                    }
                },
                ParserState::PrivateMode(num) => match c {
                    '0'..='9' => {
                        num.push(c);
                        Ok(())
                    }
                    'h' => {
                        if num.is_empty() {
                            Err(ParserError::InvalidNumberOfArguments('h', 0))
                        } else {
                            // Todo: implement private modes
                            Ok(())
                        }
                    }
                    c => Err(ParserError::UnexpectedCharacter(c)),
                },
            };
            if let Err(err) = result {
                return Err(ParseTrace {
                    error: err,
                    characters_parsed: *self.characters_parsed.lock().unwrap(),
                });
            }
        }

        Ok(())
    }

    fn write(&mut self, c: char) -> ParseResult {
        match c {
            '\x1b' => {
                self.state = ParserState::Escaped;
                return Ok(());
            }
            '\n' => {
                self.cursor_x = 0;
                self.cursor_y += 1;
                self.fix_cursor_bounds();
            }
            '\r' => self.cursor_x = 0,
            '\t' => {
                self.cursor_x += 8;
                self.fix_cursor_bounds();
            }
            _ => {
                if !c.is_control() {
                    self.grid[self.cursor_y * self.width + self.cursor_x] = Symbol {
                        c,
                        foreground_color: self.foreground_color.clone(),
                        background_color: self.background_color.clone(),
                    };
                    self.cursor_x += 1;
                    self.fix_cursor_bounds();
                } else {
                    return Err(ParserError::Todo(c));
                }
            }
        }
        Ok(())
    }

    pub fn fix_cursor_bounds(&mut self) {
        if self.cursor_x >= self.width {
            self.cursor_x -= self.width;
            self.cursor_y += 1;
        }
        if self.cursor_y >= self.height {
            self.push_row();
            self.cursor_y -= 1;
        }
    }

    pub fn push_row(&mut self) {
        // copy each row to the next in reverse order to avoid losing data
        for y in 0..(self.height - 1) {
            for x in 0..self.width {
                self.grid[y * self.width + x] = self.grid[(y + 1) * self.width + x].clone();
            }
        }
    }

    fn escaped(&mut self, c: char) -> ParseResult {
        match c {
            '[' => self.state = ParserState::Cursor(Vec::new(), String::new()),
            ']' => self.state = ParserState::OsMode(Vec::new(), String::new()),
            _ => return Err(ParserError::Todo(c)),
        }
        Ok(())
    }

    /// Some helpful specs can be found here:
    ///
    /// https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
    fn cursor(&mut self, c: char) -> ParseResult {
        match &mut self.state {
            ParserState::Cursor(stack, _) => match c {
                'A' => match stack.len() {
                    0 => {
                        self.cursor_y = self.cursor_y.saturating_sub(1);
                    }
                    1 => {
                        self.cursor_y = self.cursor_y.saturating_sub(stack[0]);
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'B' => match stack.len() {
                    0 => {
                        self.cursor_y = (self.cursor_y + 1) % self.height;
                    }
                    1 => {
                        self.cursor_y = (self.cursor_y + stack[0]) % self.height;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'C' => match stack.len() {
                    0 => {
                        self.cursor_x = (self.cursor_x + 1) % self.width;
                    }
                    1 => {
                        self.cursor_x = (self.cursor_x + stack[0]) % self.width;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'D' => match stack.len() {
                    0 => {
                        self.cursor_x = self.cursor_x.saturating_sub(1);
                    }
                    1 => {
                        self.cursor_x = (self.cursor_x + stack[0]) % self.width;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'E' => match stack.len() {
                    1 => {
                        self.cursor_y = (self.cursor_y + stack[0]) % self.height;
                        self.cursor_x = 0;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'F' => match stack.len() {
                    1 => {
                        self.cursor_y = self.cursor_y.saturating_sub(stack[0]);
                        self.cursor_x = 0;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'G' => match stack.len() {
                    1 => {
                        self.cursor_x = stack[0] % self.width;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'H' | 'f' => match stack.len() {
                    0 => {
                        self.cursor_x = 0;
                        self.cursor_y = 0;
                    }
                    1 => {
                        self.cursor_y = stack[0] % self.height;
                        self.cursor_x = 0;
                    }
                    2 => {
                        self.cursor_y = stack[0] % self.width;
                        self.cursor_x = stack[1] % self.height;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'J' => match stack.len() {
                    0 => self.grid.fill(Symbol {
                        c: ' ',
                        foreground_color: String::new(),
                        background_color: String::new(),
                    }),
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'm' => match stack.len() {
                    0 => self.foreground_color = "Default ForeGround".into(),
                    1 => match stack[0] {
                        0 => {
                            // reset all modes
                        }
                        1 => {
                            // Set bold mode
                        }
                        2 => {
                            // set fim/faint mode
                        }
                        22 => {
                            // reset bold and faint mode
                        }
                        3 => {
                            // set italic mode
                        }
                        23 => {
                            // reset italic mode
                        }
                        4 => {
                            // set underline mode
                        }
                        24 => {
                            // reset underline mode
                        }
                        5 => {
                            // set blinking mode
                        }
                        25 => {
                            // reset blinking mode
                        }
                        7 => {
                            // set inverse / reverse mode
                        }
                        27 => {
                            // reset inverse mode
                        }
                        8 => {
                            // set hidden / invisible mode
                        }
                        28 => {
                            // unset hidden mode
                        }
                        9 => {
                            // set strikethrough mode
                        }
                        29 => {
                            // unset strikethrough mode
                        }
                        30..=37 => self.foreground_color = Self::parse_ansi_color(stack[0] - 30)?,
                        90..=97 => self.foreground_color = Self::parse_ansi_color(stack[0] - 90)?,
                        39 => {
                            // default foreground color
                        }
                        40..=47 => self.background_color = Self::parse_ansi_color(stack[0] - 40)?,
                        100..=107 => {
                            self.background_color = Self::parse_ansi_color(stack[0] - 100)?
                        }
                        49 => {
                            // default background color
                        }
                        arg => return Err(ParserError::InvalidArgument(c, arg)),
                    },
                    5 => match stack[0] {
                        38 => match stack[1] {
                            2 => {
                                self.foreground_color =
                                    format!("{},{},{}", stack[2], stack[3], stack[4])
                            }
                            mode => return Err(ParserError::InvalidMode(mode)),
                        },
                        48 => match stack[1] {
                            2 => {
                                self.foreground_color =
                                    format!("{},{},{}", stack[2], stack[3], stack[4])
                            }
                            mode => return Err(ParserError::InvalidMode(mode)),
                        },
                        mode => return Err(ParserError::InvalidMode(mode)),
                    },
                    args => return Err(ParserError::InvalidNumberOfArguments(c, args)),
                },
                '?' => match stack.len() {
                    0 => self.state = ParserState::PrivateMode(String::new()),
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                c => return Err(ParserError::Todo(c)),
            },
            _ => unreachable!(),
        };

        self.state = ParserState::Write;

        Ok(())
    }

    fn parse_ansi_color(ansi: usize) -> Result<String, ParserError> {
        Ok(match ansi {
            0 => "0,0,0".into(),        // Black
            1 => "255,0,0".into(),      // Red
            2 => "0,255,0".into(),      // Green
            3 => "255,255,0".into(),    // Yellow
            4 => "0,0,255".into(),      // Blue
            5 => "255,0,255".into(),    // Magenta
            6 => "0,255,255".into(),    // Cyan
            7 => "255,255,255".into(),  // White
            8 => "128,128,128".into(),  // Bright black (gray)
            9 => "255,0,0".into(),      // Bright red
            10 => "0,255,0".into(),     // Bright green
            11 => "255,255,0".into(),   // Bright yellow
            12 => "0,0,255".into(),     // Bright blue
            13 => "255,0,255".into(),   // Bright magenta
            14 => "0,255,255".into(),   // Bright cyan
            15 => "255,255,255".into(), // Bright white
            16..=231 => {
                // Colors in the 216-color cube
                let color = ansi - 16;
                let r = (color / 36) * 51;
                let g = ((color % 36) / 6) * 51;
                let b = (color % 6) * 51;
                format!("{},{},{}", r, g, b)
            }
            232..=255 => {
                // Grayscale colors (232-255)
                let gray = (ansi - 232) * 11;
                format!("{},{},{},", gray, gray, gray)
            }
            invalid => return Err(ParserError::InvalidArgument(' ', invalid)),
        })
    }

    pub fn rows(&self) -> impl Iterator<Item = impl Into<Cow<'_, str>>> {
        self.grid
            .chunks(self.width)
            .map(|row| row.iter().map(|symbol| symbol.c).collect::<String>())
    }

    pub fn view<Message: 'static>(&self) -> Element<Message> {
        for row in self.rows() {
            println!("{}", row.into())
        }
        column(self.rows().map(|row| text(row.into()).into())).into()
    }
}
