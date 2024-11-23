use std::{
    borrow::Cow,
    str::Chars,
    sync::{Arc, Mutex},
};
use thiserror::Error;

use iced::widget::{column, row, shader::wgpu::naga::front::wgsl::ParseError, text};

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

#[derive(Debug, Clone)]
pub enum ParserState {
    Write,
    Escaped,
    Cursor(Vec<String>, String),
    PrivateMode(String),
    SwitchCharacterSet,
    OsMode(Vec<String>, String),
}

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("not yet implemented")]
    Todo(char),
    #[error("invalid number of arguments for command {0}: {1}")]
    InvalidNumberOfArguments(char, usize),
    #[error("invalid argument for command {0}: {1}")]
    InvalidArgument(char, String),
    #[error("invalid mode selection: {0}")]
    InvalidMode(usize),
    #[error("unexpected character")]
    UnexpectedCharacter(char),
    #[error("the cursor is out of bounds: {0}, {1}")]
    CursorOutOfBounds(usize, usize),
}

type ParseResult = Result<(), ParserError>;

#[derive(Debug)]
pub struct ParseTrace {
    error: ParserError,
    characters_parsed: usize,
    around_here: String,
    current_state: ParserState,
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
            let result = if self.cursor_x >= self.width || self.cursor_y >= self.height {
                Err(ParserError::CursorOutOfBounds(self.cursor_x, self.cursor_y))
            } else {
                match &mut self.state {
                    ParserState::Write => self.write(c),
                    ParserState::Escaped => self.escaped(c),
                    ParserState::Cursor(stack, argument) => match c {
                        '0'..='9' => {
                            argument.push(c);
                            Ok(())
                        }
                        ';' => {
                            stack.push(argument.split_off(0));
                            Ok(())
                        }
                        c => {
                            if !argument.is_empty() {
                                stack.push(argument.split_off(0));
                            }
                            self.cursor(c)
                        }
                    },
                    ParserState::OsMode(stack, argument) => match c {
                        ';' => {
                            stack.push(argument.split_off(0));
                            Ok(())
                        }
                        '\x07' => {
                            stack.push(argument.split_off(0));

                            // todo: do something with this sequence
                            self.state = ParserState::Write;
                            Ok(())
                        }
                        c => {
                            if !c.is_control() {
                                argument.push(c);
                                Ok(())
                            } else {
                                Err(ParserError::Todo(c))
                            }
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
                                self.state = ParserState::Write;
                                Ok(())
                            }
                        }
                        'l' => {
                            if num.is_empty() {
                                Err(ParserError::InvalidNumberOfArguments('l', 0))
                            } else {
                                // Todo: implement private modes
                                self.state = ParserState::Write;
                                Ok(())
                            }
                        }
                        c => Err(ParserError::UnexpectedCharacter(c)),
                    },
                    ParserState::SwitchCharacterSet => {
                        // Todo: actually load and switch character sets
                        self.state = ParserState::Write;
                        Ok(())
                    }
                }
            };
            if let Err(err) = result {
                let characters_parsed = *self.characters_parsed.lock().unwrap();
                return Err(ParseTrace {
                    error: err,
                    characters_parsed,
                    current_state: self.state.clone(),
                    around_here: input[characters_parsed - 10..characters_parsed + 20].to_string(),
                });
            }
        }

        Ok(())
    }

    fn write(&mut self, c: char) -> ParseResult {
        match c {
            '\x1b' => {
                self.state = ParserState::Escaped;
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
            '\x08' => {
                self.cursor_x = self.cursor_x.saturating_sub(1);
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
            '(' => self.state = ParserState::SwitchCharacterSet,
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
                        self.cursor_y = self.cursor_y.saturating_sub(
                            stack[0]
                                .parse::<usize>()
                                .map_err(|_| ParserError::InvalidArgument(c, stack[0].clone()))?,
                        );
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'B' => match stack.len() {
                    0 => {
                        self.cursor_y = (self.cursor_y + 1) % self.height;
                    }
                    1 => {
                        self.cursor_y = (self.cursor_y
                            + stack[0]
                                .parse::<usize>()
                                .map_err(|_| ParserError::InvalidArgument(c, stack[0].clone()))?)
                            % self.height;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'C' => match stack.len() {
                    0 => {
                        self.cursor_x = (self.cursor_x + 1) % self.width;
                    }
                    1 => {
                        self.cursor_x = (self.cursor_x
                            + stack[0]
                                .parse::<usize>()
                                .map_err(|_| ParserError::InvalidArgument(c, stack[0].clone()))?)
                            % self.width;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'D' => match stack.len() {
                    0 => {
                        self.cursor_x = self.cursor_x.saturating_sub(1);
                    }
                    1 => {
                        self.cursor_x = self.cursor_x.saturating_sub(
                            stack[0]
                                .parse::<usize>()
                                .map_err(|_| ParserError::InvalidArgument(c, stack[0].clone()))?,
                        );
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'E' => match stack.len() {
                    1 => {
                        self.cursor_y = (self.cursor_y
                            + stack[0]
                                .parse::<usize>()
                                .map_err(|_| ParserError::InvalidArgument(c, stack[0].clone()))?)
                            % self.height;
                        self.cursor_x = 0;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'F' => match stack.len() {
                    1 => {
                        self.cursor_y = self.cursor_y.saturating_sub(
                            stack[0]
                                .parse::<usize>()
                                .map_err(|_| ParserError::InvalidArgument(c, stack[0].clone()))?,
                        );
                        self.cursor_x = 0;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'G' => match stack.len() {
                    1 => {
                        self.cursor_x = (stack[0]
                            .parse::<usize>()
                            .map_err(|_| ParserError::InvalidArgument(c, stack[0].clone()))?
                            + 1)
                            % self.width;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'H' | 'f' => match stack.len() {
                    0 => {
                        self.cursor_x = 0;
                        self.cursor_y = 0;
                    }
                    1 => {
                        self.cursor_y = (stack[0]
                            .parse::<usize>()
                            .map_err(|_| ParserError::InvalidArgument(c, stack[0].clone()))?
                            + 1)
                            % self.height;
                        self.cursor_x = 0;
                    }
                    2 => {
                        self.cursor_y = (stack[0]
                            .parse::<usize>()
                            .map_err(|_| ParserError::InvalidArgument(c, stack[0].clone()))?
                            + 1)
                            % self.height;
                        self.cursor_x = (stack[1]
                            .parse::<usize>()
                            .map_err(|_| ParserError::InvalidArgument(c, stack[1].clone()))?
                            + 1)
                            % self.width;
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'J' => match stack.len() {
                    0 => {
                        self.grid[self.cursor_y * self.width + self.cursor_x..].fill(Symbol {
                            c: ' ',
                            foreground_color: String::new(),
                            background_color: String::new(),
                        });
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'K' => match stack.len() {
                    0 => {
                        self.grid[self.cursor_y * self.width + self.cursor_x
                            ..(self.cursor_y + 1) * self.width - 1]
                            .fill(Symbol {
                                c: ' ',
                                foreground_color: String::new(),
                                background_color: String::new(),
                            });
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'm' => {
                    if stack.is_empty() {
                        self.foreground_color = "Default color".into()
                    } else {
                        let stack = stack
                            .iter()
                            .map(|s| {
                                s.parse::<usize>()
                                    .map_err(|_| ParserError::InvalidArgument(c, s.clone()))
                            })
                            .collect::<Result<Vec<_>, ParserError>>()?;

                        let mut iter = stack.iter();

                        while let Some(style) = iter.next() {
                            match style {
                                38 | 48 => {
                                    match iter.next().ok_or(
                                        ParserError::InvalidNumberOfArguments(c, stack.len()),
                                    )? {
                                        2 => {
                                            let r = iter.next().ok_or(
                                                ParserError::InvalidNumberOfArguments(
                                                    c,
                                                    stack.len(),
                                                ),
                                            )?;
                                            let g = iter.next().ok_or(
                                                ParserError::InvalidNumberOfArguments(
                                                    c,
                                                    stack.len(),
                                                ),
                                            )?;
                                            let b = iter.next().ok_or(
                                                ParserError::InvalidNumberOfArguments(
                                                    c,
                                                    stack.len(),
                                                ),
                                            )?;
                                            match style {
                                                38 => {
                                                    self.foreground_color =
                                                        format!("{},{},{}", r, g, b)
                                                }
                                                48 => {
                                                    self.background_color =
                                                        format!("{},{},{}", r, g, b)
                                                }
                                                _ => unreachable!(),
                                            }
                                        }
                                        5 => {
                                            let color = iter.next().ok_or(
                                                ParserError::InvalidNumberOfArguments(
                                                    c,
                                                    stack.len(),
                                                ),
                                            )?;
                                            match style {
                                                38 => {
                                                    self.foreground_color =
                                                        Self::parse_ansi_color(*color)?
                                                }
                                                48 => {
                                                    self.background_color =
                                                        Self::parse_ansi_color(*color)?
                                                }
                                                _ => unreachable!(),
                                            }
                                        }
                                        arg => {
                                            return Err(ParserError::InvalidArgument(
                                                c,
                                                arg.to_string(),
                                            ))
                                        }
                                    }
                                }
                                style => self.set_style(c, *style)?,
                            }
                        }
                    }
                }
                '?' => match stack.len() {
                    0 => {
                        self.state = ParserState::PrivateMode(String::new());
                        return Ok(());
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                c => return Err(ParserError::Todo(c)),
            },
            _ => unreachable!(),
        };

        self.state = ParserState::Write;

        Ok(())
    }

    fn set_style(&mut self, c: char, style: usize) -> ParseResult {
        match style {
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
            30..=37 => self.foreground_color = Self::parse_ansi_color(style - 30)?,
            90..=97 => self.foreground_color = Self::parse_ansi_color(style - 90)?,
            39 => {
                // default foreground color
            }
            40..=47 => self.background_color = Self::parse_ansi_color(style - 40)?,
            100..=107 => self.background_color = Self::parse_ansi_color(style - 100)?,
            49 => {
                // default background color
            }
            arg => return Err(ParserError::InvalidArgument(c, arg.to_string())),
        }

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
            invalid => return Err(ParserError::InvalidArgument(' ', invalid.to_string())),
        })
    }

    pub fn rows(&self) -> Vec<&[Symbol]> {
        self.grid.chunks(self.width).collect()
    }

    pub fn view<Message: 'static>(&self) -> Element<Message> {
        for row in self.rows() {
            for symbol in row {
                print!("{}", symbol.c)
            }
            print!("\n");
        }

        column(self.rows().iter().map(|r| {
            row(r
                .iter()
                .map(|symbol| text(symbol.c.to_string()).width(12).center().into()))
            .into()
        }))
        .into()
    }
}
