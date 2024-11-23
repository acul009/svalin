use std::sync::{Arc, Mutex};
use thiserror::Error;

use iced::{
    widget::{column, row, text},
    Color,
};

use crate::Element;

pub struct AnsiGrid {
    cursor_x: usize,
    cursor_y: usize,
    width: usize,
    height: usize,
    grid: Vec<Symbol>,
    cursor_format: Format,
    state: ParserState,
    characters_parsed: Arc<Mutex<usize>>,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    c: char,
    format: Format,
}

#[derive(Debug, Clone)]
pub struct Format {
    foreground: Option<Color>,
    background: Option<Color>,
    bold: bool,
    faint: bool,
    italic: bool,
    underline: bool,
    blinking: bool,
    inverse: bool,
    hidden: bool,
    strikethrough: bool,
}

impl Default for Format {
    fn default() -> Self {
        Self {
            foreground: None,
            background: None,
            bold: false,
            faint: false,
            italic: false,
            underline: false,
            blinking: false,
            inverse: false,
            hidden: false,
            strikethrough: false,
        }
    }
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
            cursor_format: Default::default(),
            grid: vec![
                Symbol {
                    c: ' ',
                    format: Default::default()
                };
                width * height
            ],
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
                        format: self.cursor_format.clone(),
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
                            format: self.cursor_format.clone(),
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
                                format: self.cursor_format.clone(),
                            });
                    }
                    _ => return Err(ParserError::InvalidNumberOfArguments(c, stack.len())),
                },
                'm' => {
                    if stack.is_empty() {
                        self.cursor_format = Default::default();
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
                                                    self.cursor_format.foreground =
                                                        Some(Color::from_rgb8(
                                                            *r as u8, *g as u8, *b as u8,
                                                        ));
                                                }
                                                48 => {
                                                    self.cursor_format.background =
                                                        Some(Color::from_rgb8(
                                                            *r as u8, *g as u8, *b as u8,
                                                        ));
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
                                                    self.cursor_format.foreground =
                                                        Some(Self::parse_ansi_color(*color)?)
                                                }
                                                48 => {
                                                    self.cursor_format.background =
                                                        Some(Self::parse_ansi_color(*color)?)
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
                let new_format = Format {
                    foreground: self.cursor_format.foreground,
                    background: self.cursor_format.background,
                    ..Default::default()
                };

                self.cursor_format = new_format;
            }
            1 => {
                // Set bold mode
                self.cursor_format.bold = true;
            }
            2 => {
                // set fim/faint mode
                self.cursor_format.faint = true;
            }
            22 => {
                // reset bold and faint mode
                self.cursor_format.bold = false;
                self.cursor_format.faint = false;
            }
            3 => {
                // set italic mode
                self.cursor_format.italic = true;
            }
            23 => {
                // reset italic mode
                self.cursor_format.italic = false;
            }
            4 => {
                // set underline mode
                self.cursor_format.underline = true;
            }
            24 => {
                // reset underline mode
                self.cursor_format.underline = false;
            }
            5 => {
                // set blinking mode
                self.cursor_format.blinking = true;
            }
            25 => {
                // reset blinking mode
                self.cursor_format.blinking = false;
            }
            7 => {
                // set inverse / reverse mode
                self.cursor_format.inverse = true;
            }
            27 => {
                // reset inverse mode
                self.cursor_format.inverse = false;
            }
            8 => {
                // set hidden / invisible mode
                self.cursor_format.hidden = true;
            }
            28 => {
                // unset hidden mode
                self.cursor_format.hidden = false;
            }
            9 => {
                // set strikethrough mode
                self.cursor_format.strikethrough = true;
            }
            29 => {
                // unset strikethrough mode
                self.cursor_format.strikethrough = false;
            }
            30..=37 => self.cursor_format.foreground = Some(Self::parse_ansi_color(style - 30)?),
            90..=97 => self.cursor_format.foreground = Some(Self::parse_ansi_color(style - 90)?),
            39 => {
                // default foreground color
            }
            40..=47 => self.cursor_format.background = Some(Self::parse_ansi_color(style - 40)?),
            100..=107 => self.cursor_format.background = Some(Self::parse_ansi_color(style - 100)?),
            49 => {
                // default background color
            }
            arg => return Err(ParserError::InvalidArgument(c, arg.to_string())),
        }

        Ok(())
    }

    fn parse_ansi_color(ansi: usize) -> Result<Color, ParserError> {
        Ok(match ansi {
            0..=15 => {
                let r = ansi & 0b1;
                let g = (ansi >> 1) & 0b1;
                let b = (ansi >> 2) & 0b1;

                Color::from_rgb(r as f32, g as f32, b as f32)
            }
            16..=231 => {
                // Colors in the 216-color cube
                let color = ansi - 16;
                let r = (color / 36) * 51;
                let g = ((color % 36) / 6) * 51;
                let b = (color % 6) * 51;

                Color::from_rgb(r as f32, g as f32, b as f32)
            }
            232..=255 => {
                // Grayscale colors (232-255)
                let gray = ((ansi - 232) * 11) as f32;
                Color::from_rgb(gray, gray, gray)
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
            row(r.iter().map(|symbol| {
                text(symbol.c.to_string())
                    .color_maybe(symbol.format.foreground)
                    .width(12)
                    .center()
                    .into()
            }))
            .into()
        }))
        .into()
    }
}
