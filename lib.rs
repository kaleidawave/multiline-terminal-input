#![doc = include_str!("README.md")]

#[derive(Debug, Default, PartialEq, Eq)]
pub enum Mode {
	#[default]
	ReturnOnUndecoratedNewLine,
	ReturnOnShiftNewLine,
}

/// Prompt, mode and prefix for reading
#[derive(Debug, Default)]
pub struct Options<'a> {
	/// Initial printed line (with new line);
	pub prompt: &'a str,
	/// Prepended on each new line
	pub prefix: &'a str,
	/// What key combination to return on
	pub return_mode: Mode,
}

#[cfg(any(target_family = "windows", target_family = "unix"))]
pub fn read_to_string(options: Options<'_>) -> std::io::Result<String> {
	use std::fmt::Write;

	let rows = read_to_vec_string(options)?;
	let mut buf = String::new();
	for row in rows {
		writeln!(&mut buf, "{row}").unwrap();
	}
	Ok(buf)
}

#[cfg(not(any(target_family = "windows", target_family = "unix")))]
pub fn read_to_string(_options: Options<'_>) -> std::io::Result<String> {
	Err(std::io::ErrorKind::Unsupported.into())
}

/// Reads string from `stdin`. If shift is pressed while new line then keeps reading (or skips depending on `options.mode`).
#[cfg(target_family = "windows")]
pub fn read_to_vec_string(options: Options<'_>) -> std::io::Result<Vec<String>> {
	use std::io::{self, Write};
	use winconsole::input::{is_key_down, KeyCode};

	let stdin = io::stdin();
	// TODO we could have Vec<String> here, but then we would have to account of backspace etc
	let mut buf = String::new();

	let Options { prefix, return_mode, prompt } = options;

	if !prompt.is_empty() {
		let mut stdout = io::stdout();
		write!(&mut stdout, "{prompt}");
		Write::flush(&mut stdout)?;
	}

	if !prefix.is_empty() {
		let mut stdout = io::stdout();
		write!(&mut stdout, "{prefix}")?;
		Write::flush(&mut stdout)?;
	}

	while let Ok(count) = stdin.read_line(&mut buf) {
		if is_key_down(KeyCode::Shift) == matches!(return_mode, Mode::ReturnOnUndecoratedNewLine) {
			let mut stdout = io::stdout();
			write!(&mut stdout, "{prefix}");
			Write::flush(&mut stdout);
		} else {
			break;
		}
	}

	Ok(buf.split().map(str::to_owned).collect())
}

/// Reads string from `stdin`. If shift is pressed while new line then keeps reading (or skips depending on `options.mode`).
#[cfg(target_family = "unix")]
pub fn read_to_vec_string(options: Options<'_>) -> std::io::Result<Vec<String>> {
	use crossterm::terminal;
	use std::io::{self, Read, Write};

	let Options { prefix, return_mode, prompt } = options;

	struct Buf {
		rows: Vec<String>,
		row: usize,
		column: usize,
	}

	impl Default for Buf {
		fn default() -> Self {
			Self { rows: vec![String::new()], row: 0, column: 0 }
		}
	}

	impl Buf {
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

		pub fn move_left(&mut self, left: usize) {
			if left > self.column {
				self.column = 0;
			} else {
				self.column -= left;
			}
		}

		pub fn move_right(&mut self, right: usize) {
			let row = &self.rows[self.row];
			if right + self.column > row.len() {
				self.column = row.len();
			} else {
				self.column += right;
			}
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

	let mut buf: Buf = Buf::default();

	let mut log = std::fs::File::create("private/log.txt").unwrap();

	let mut stdout = io::stdout();
	let stdin = io::stdin();
	let mut bytes = stdin.bytes();

	if !prompt.is_empty() {
		writeln!(&mut stdout, "{prompt}")?;
		Write::flush(&mut stdout)?;
	}

	if !prefix.is_empty() {
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
								// Shift enter I think
								b'~' => {
									if arguments != [27, 2, 13] {
										writeln!(&mut log, "{arguments:?} != [27, 2, 13]").unwrap();
										log.flush().unwrap();
									}
									if return_mode == Mode::ReturnOnShiftNewLine {
										break 'main;
									}
									write!(&mut stdout, "\r\n{prefix}")?;
									Write::flush(&mut stdout)?;
									buf.push('\n');
								}
								// Cursor forward
								b'C' => {
									let arg = arguments.pop().unwrap_or(1);
									buf.move_right(arg);
									assert!(arguments.is_empty());
									// TODO if across new line
									write!(&mut stdout, "\x1b[{arg}C")?;
									Write::flush(&mut stdout)?;
								}
								// Cursor backwards
								b'D' => {
									let arg = arguments.pop().unwrap_or(1);
									buf.move_left(arg);
									assert!(arguments.is_empty());
									// TODO if across new line
									write!(&mut stdout, "\x1b[{arg}D")?;
									Write::flush(&mut stdout)?;
								}
								byte => {
									{
										writeln!(
											&mut log,
											"unhandled x1b [ {arguments:?} {byte}",
											byte = byte as char
										)
										.unwrap();
										log.flush().unwrap();
									}
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
							break;
						}
					}
					byte => {
						{
							writeln!(&mut log, "x1b then {byte}", byte = byte as char).unwrap();
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
					// TODO make sure not new line etc
					append_to_buf!(c);
				} else {
					writeln!(&mut log, "unknown char {acc}").unwrap();
					log.flush().unwrap();
				}
			}
			5 => {
				writeln!(&mut log, "5").unwrap();
				log.flush().unwrap();
			}
			// TODO what is this?
			// 194 => {
			// 	writeln!(&mut log, "194?").unwrap();
			// 	log.flush().unwrap();
			// }
			// otherwise ascii characters
			byte => {
				if let Some(c) = char::from_u32(u32::from(byte)) {
					append_to_buf!(c);
				} else {
					{
						writeln!(&mut log, "unknown char {byte}").unwrap();
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
