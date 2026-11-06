#![doc = include_str!("README.md")]

use std::io::{Result, Write};

#[derive(Debug, Default, PartialEq, Eq)]
pub enum Mode {
	#[default]
	ReturnOnUndecoratedNewLine,
	ReturnOnShiftNewLine,
}

/// Options for reading input
#[derive(Debug, Default)]
pub struct Options<'a> {
	/// Initial printed line (with new line);
	pub prompt: &'a str,
	/// Prepended on each new line
	pub prefix: &'a str,
	/// What key combination to return on
	pub return_mode: Mode,
	/// An initial input to edit / build apon
	pub initial: &'a str,
}

pub fn read_to_string(options: Options<'_>) -> Result<String> {
	use std::fmt::Write;

	let rows = read_to_vec_string(options)?;
	let mut buf = String::new();
	for row in rows {
		writeln!(&mut buf, "{row}").unwrap();
	}
	Ok(buf)
}

struct Buf {
	rows: Vec<String>,
	row: usize,
	column: usize,
}

impl Buf {
	pub fn new(existing: &str) -> Buf {
		let rows: Vec<String> = if existing.is_empty() {
			vec![String::new()]
		} else {
			existing.lines().map(str::to_owned).collect()
		};
		let row = rows.len() - 1;
		let column = rows.last().unwrap().len();
		Self { rows, row, column }
	}

	// TODO what about `\r` etc..?
	pub fn push(&mut self, c: char) {
		if c == '\n' {
			self.new_line();
		} else {
			let row = &mut self.rows[self.row];
			if self.column > row.len() {
				row.push(c)
			} else {
				row.insert(self.column, c);
			}
			self.column += c.len_utf8();
		}
	}

	pub fn to_right_of_cursor(&self) -> &str {
		&self.rows[self.row][self.column..]
	}
	pub fn move_up(&mut self, up: usize, stdout: &mut impl Write) -> Result<()> {
		if self.row >= up {
			self.row -= up;
			write!(stdout, "\x1b[{up}A")?;
			if let Some(move_left) = self.column.checked_sub(self.rows[self.row].len()) {
				self.column -= move_left;
				write!(stdout, "\x1b[{move_left}D")?;
			}
			Write::flush(stdout)?;
		}
		Ok(())
	}

	pub fn move_down(&mut self, down: usize, stdout: &mut impl Write) -> Result<()> {
		if self.row + down < self.rows.len() {
			self.row += down;
			write!(stdout, "\x1b[{down}B")?;
			if let Some(move_left) = self.column.checked_sub(self.rows[self.row].len()) {
				self.column -= move_left;
				write!(stdout, "\x1b[{move_left}D")?;
			}
			Write::flush(stdout)?;
		}
		Ok(())
	}

	pub fn move_right(&mut self, right: usize, stdout: &mut impl Write) -> Result<()> {
		let row = &self.rows[self.row];
		if right + self.column <= row.len() {
			self.column += right;
			write!(stdout, "\x1b[{right}C")?;
			Write::flush(stdout)?;
		}
		Ok(())
	}

	pub fn move_left(&mut self, left: usize, stdout: &mut impl Write) -> Result<()> {
		if left <= self.column {
			self.column -= left;
			write!(stdout, "\x1b[{left}D")?;
			Write::flush(stdout)?;
		}
		Ok(())
	}

	pub fn pop(&mut self) -> Option<char> {
		if let Some(chr) = self.rows[self.row].pop() {
			self.column -= chr.len_utf8();
			Some(chr)
		} else if self.row >= 1 {
			let _ = self.rows.remove(self.row);
			self.row -= 1;
			self.column = self.rows[self.row].len();
			Some('\n')
		} else {
			None
		}
	}

	fn new_line(&mut self) {
		self.column = 0;
		self.row += 1;
		self.rows.insert(self.row, String::new());
	}

	pub fn into_rows(self) -> Vec<String> {
		self.rows
	}
}

/// Reads string from `stdin`.
/// If shift is pressed while new line then keeps reading (or skips depending on `options.mode`).
pub fn read_to_vec_string(options: Options<'_>) -> Result<Vec<String>> {
	use crossterm::terminal;
	use std::io::{self, Read, Write};

	let Options { prefix, return_mode, prompt, initial } = options;

	let mut buf: Buf = Buf::new(initial);

	// For debugging
	const LOG: bool = true;
	let mut log = LOG.then(|| std::fs::File::create("private/log.txt").unwrap());

	let mut stdout = io::stdout();
	let stdin = io::stdin();
	let mut bytes = std::io::BufReader::new(stdin).bytes();

	if !prompt.is_empty() {
		writeln!(&mut stdout, "{prompt}")?;
		Write::flush(&mut stdout)?;
	}

	if !initial.is_empty() {
		for (idx, line) in initial.lines().enumerate() {
			if idx > 0 {
				writeln!(&mut stdout)?;
			}
			write!(&mut stdout, "{prefix}{line}")?;
		}
		Write::flush(&mut stdout)?;
	} else if !prefix.is_empty() {
		write!(&mut stdout, "{prefix}")?;
		Write::flush(&mut stdout)?;
	}

	terminal::enable_raw_mode()?;

	macro_rules! append_to_buf {
		($c:expr) => {{
			let c = $c;
			buf.push(c);
			write!(&mut stdout, "{c}")?;
			let to_right_of_cursor = buf.to_right_of_cursor();
			if !to_right_of_cursor.is_empty() {
				let after = to_right_of_cursor.len();
				// clear rest of line. append content, reset cursor
				write!(&mut stdout, "{to_right_of_cursor}\x1b[{after}D")?;
			}
			Write::flush(&mut stdout)?;
		}};
	}

	'main: while let Some(byte) = bytes.next() {
		let byte = byte?;

		match byte {
			b'\n' => {
				// print return to get to the start
				if return_mode == Mode::ReturnOnShiftNewLine {
					write!(&mut stdout, "\r\n")?;
					Write::flush(&mut stdout)?;
					break;
				}

				write!(&mut stdout, "\r\n{prefix}")?;
				Write::flush(&mut stdout)?;
				buf.push('\n');
			}
			// TODO not sure
			b'\r' => {
				if return_mode == Mode::ReturnOnUndecoratedNewLine {
					write!(&mut stdout, "\r\n")?;
					Write::flush(&mut stdout)?;
					break;
				}

				write!(&mut stdout, "\r\n{prefix}")?;
				Write::flush(&mut stdout)?;
				buf.push('\n');
			}
			// delete key
			0x7f => {
				if let Some(chr) = buf.pop() {
					write!(&mut stdout, "\x08 \x08")?;
					if let '\n' = chr {
						for _ in 0..=prefix.len() {
							write!(&mut stdout, "\x08 \x08")?;
						}
					}
					let to_right_of_cursor = buf.to_right_of_cursor();
					if !to_right_of_cursor.is_empty() {
						let after = to_right_of_cursor.len();
						// clear rest of line. append content, reset cursor
						write!(&mut stdout, "\x1b[K{to_right_of_cursor}\x1b[{after}D")?;
					}
					Write::flush(&mut stdout)?;
				}
			}
			// ESC
			0x1b => {
				let byte = bytes.next().unwrap().unwrap_or_default();
				match byte {
					// OSC
					b'[' => {
						let mut arguments: Vec<usize> = Vec::new();
						let mut acc: usize = 0;
						while let Some(Ok(byte)) = bytes.next() {
							if let b'0'..=b'9' = byte {
								acc *= 10;
								acc += (byte - b'0') as usize;
								continue;
							}

							if byte == b';' {
								arguments.push(acc);
								continue;
							}

							match byte {
								// Shift enter
								b'~' => {
									if arguments != [27, 2, 13] {
										if let Some(ref mut log) = log {
											writeln!(log, "{arguments:?} != [27, 2, 13]").unwrap();
											log.flush().unwrap();
										}
									}
									if return_mode == Mode::ReturnOnShiftNewLine {
										break 'main;
									}
									write!(&mut stdout, "\r\n{prefix}")?;
									Write::flush(&mut stdout)?;
									buf.push('\n');
								}
								// Cursor up
								b'A' => {
									let arg = arguments.pop().unwrap_or(1);
									assert!(arguments.is_empty());
									buf.move_up(arg, &mut stdout)?;
								}
								// Cursor down
								b'B' => {
									let arg = arguments.pop().unwrap_or(1);
									assert!(arguments.is_empty());
									buf.move_down(arg, &mut stdout)?;
								}
								// Cursor forward
								b'C' => {
									let arg = arguments.pop().unwrap_or(1);
									assert!(arguments.is_empty());
									buf.move_right(arg, &mut stdout)?;
								}
								// Cursor backwards
								b'D' => {
									let arg = arguments.pop().unwrap_or(1);
									assert!(arguments.is_empty());
									buf.move_left(arg, &mut stdout)?;
								}
								byte => {
									if let Some(ref mut log) = log {
										writeln!(
											log,
											"unhandled x1b [ {arguments:?} {byte}",
											byte = byte as char
										)
										.unwrap();
										log.flush().unwrap();
									}
									if false {
										write!(&mut stdout, "\x1b[")?;
										for (idx, argument) in arguments.into_iter().enumerate() {
											if idx > 0 {
												write!(&mut stdout, ";")?;
											}
											write!(&mut stdout, "{argument}")?;
										}
										let _ = stdout.write(&[byte])?;
										Write::flush(&mut stdout)?;
									}
								}
							}
							break;
						}
					}
					byte => {
						if let Some(ref mut log) = log {
							writeln!(log, "x1b then {byte}", byte = byte as char).unwrap();
							log.flush().unwrap();
						}
						// No idea, echo it out
						let _ = stdout.write(&[byte])?;
					}
				}
			}
			// non ascii characters
			240 => {
				let mut acc: u32 = 0b1111_0000; // byte as u32;
				for byte in bytes.by_ref() {
					let Ok(byte) = byte else {
						break;
					};
					acc <<= 8;
					acc |= u32::from(byte);
					if byte & 0b1000_0000 != 0b1000_0000 {
						break;
					}
				}
				if let Some(c) = char::from_u32(acc) {
					write!(&mut stdout, "{c}")?;
					Write::flush(&mut stdout)?;
					append_to_buf!(c);
				} else if let Some(ref mut log) = log {
					writeln!(log, "unknown char {acc}").unwrap();
					log.flush().unwrap();
				}
			}
			5 => {
				if let Some(ref mut log) = log {
					writeln!(log, "5").unwrap();
					log.flush().unwrap();
				}
			}
			// otherwise ascii characters
			byte => {
				if let Some(c) = char::from_u32(u32::from(byte)) {
					append_to_buf!(c);
				} else {
					if let Some(ref mut log) = log {
						writeln!(log, "unknown char {byte}").unwrap();
						log.flush().unwrap();
					}
					break;
				}
			}
		}
	}

	terminal::disable_raw_mode()?;

	Ok(buf.into_rows())
}
