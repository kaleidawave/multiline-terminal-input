#![doc = include_str!("README.md")]
#![warn(clippy::pedantic)]

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
	pub fn push(&mut self, c: char, prefix: &str, stdout: &mut impl Write) -> Result<()> {
		if c == '\n' {
			self.new_line(prefix, stdout)?;
		} else {
			let row = &mut self.rows[self.row];
			if self.column > row.len() {
				row.push(c);
			} else {
				row.insert(self.column, c);
			}
			self.column += c.len_utf8();
			write!(stdout, "{c}")?;
			let to_right_of_cursor = self.to_right_of_cursor();
			if !to_right_of_cursor.is_empty() {
				let after = to_right_of_cursor.len();
				// clear rest of line. append content, reset cursor
				write!(stdout, "{to_right_of_cursor}\x1b[{after}D")?;
			}
			Write::flush(stdout)?;
		}
		Ok(())
	}

	fn new_line(&mut self, prefix: &str, stdout: &mut impl Write) -> Result<()> {
		self.column = 0;
		self.row += 1;
		let at_end = self.row == self.rows.len();
		if at_end {
			self.rows.push(String::new());
			// \r needed to return to start
			write!(stdout, "\r\n{prefix}")?;
		} else {
			self.rows.insert(self.row, String::new());
			write!(stdout, "\r\n{prefix}")?;
			write!(stdout, "\x1b[s")?;
			Write::flush(stdout)?;

			// clear screen
			write!(stdout, "\x1b[J")?;
			for row in &self.rows[self.row..] {
				writeln!(stdout, "{prefix}{row}")?;
			}
			write!(stdout, "\x1b[u")?;
		}
		Write::flush(stdout)?;
		Ok(())
	}

	pub fn pop(&mut self, prefix: &str, stdout: &mut impl Write) -> Result<()> {
		if let Some(chr) = self.rows[self.row].pop() {
			self.column -= chr.len_utf8();
			// TODO why twice
			write!(stdout, "\x08 \x08")?;

			let to_right_of_cursor = self.to_right_of_cursor();
			if !to_right_of_cursor.is_empty() {
				let after = to_right_of_cursor.len();
				// clear rest of line. append content, reset cursor
				write!(stdout, "\x1b[K{to_right_of_cursor}\x1b[{after}D")?;
			}
			Write::flush(stdout)?;
		} else if self.row >= 1 {
			for _ in 0..prefix.len() {
				write!(stdout, "\x08 \x08")?;
			}
			// Move up (and to the right)
			{
				write!(stdout, "\x1b[A")?;
				let row_len = self.rows[self.row - 1].len();
				self.column = row_len;
				let move_right = prefix.len() + row_len;
				write!(stdout, "\x1b[{move_right}C")?;
				Write::flush(stdout)?;
			}
			let at_end = self.row == self.rows.len();
			if at_end {
				let _ = self.rows.pop();
			} else {
				let _ = self.rows.remove(self.row);
				write!(stdout, "\x1b[s")?;
				Write::flush(stdout)?;

				// clear screen
				write!(stdout, "\x1b[J")?;
				for row in &self.rows[self.row..] {
					writeln!(stdout, "{prefix}{row}")?;
				}
				write!(stdout, "\x1b[u")?;
			}
			self.row -= 1;
		}
		Ok(())
	}

	pub fn pop_slice(&mut self, prefix: &str, stdout: &mut impl Write) -> Result<()> {
		/// TODO wip
		fn is_tab_stop(chr: char) -> bool {
			!chr.is_ascii_alphanumeric()
		}

		if self.column == 0 {
			self.pop(prefix, stdout)
		} else {
			let left_of_cursor = &self.rows[self.row][..self.column];
			let first_tab_stop = left_of_cursor.rfind(is_tab_stop).unwrap_or_default();
			let tab_stop_size =
				left_of_cursor[first_tab_stop..].chars().next().map_or_default(char::len_utf8);

			let mut delete_until = first_tab_stop + tab_stop_size;
			if delete_until == self.column {
				delete_until = delete_until.saturating_sub(1);
			}

			let delete_range = delete_until..self.column;
			self.rows[self.row].drain(delete_range.clone());
			for _ in delete_range {
				// TODO why twice
				write!(stdout, "\x08 \x08")?;
			}
			self.column = delete_until;

			let to_right_of_cursor = self.to_right_of_cursor();
			if !to_right_of_cursor.is_empty() {
				let after = to_right_of_cursor.len();
				// clear rest of line. append content, reset cursor
				write!(stdout, "\x1b[K{to_right_of_cursor}\x1b[{after}D")?;
			}
			Write::flush(stdout)?;
			Ok(())
		}
	}

	fn to_right_of_cursor(&self) -> &str {
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

	// resets cursor position to end
	pub fn finish(self, prefix: &str, stdout: &mut impl Write) -> Result<Vec<String>> {
		// If not on last row
		if self.row != self.rows.len() - 1 {
			let move_left = self.column + prefix.len();
			// move left, clear screen, write out the rest
			write!(stdout, "\x1b[{move_left}D\x1b[J")?;
			for row in &self.rows[self.row..][1..] {
				write!(stdout, "{prefix}{row}\r\n")?;
			}
		}
		Ok(self.rows)
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
	// const LOG: bool = true;
	// let mut log = LOG.then(|| std::fs::File::create("private/log.txt").unwrap());

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

				buf.push('\n', prefix, &mut stdout)?;
			}
			// TODO not sure
			b'\r' => {
				if return_mode == Mode::ReturnOnUndecoratedNewLine {
					write!(&mut stdout, "\r\n")?;
					Write::flush(&mut stdout)?;
					break;
				}

				buf.push('\n', prefix, &mut stdout)?;
			}
			// delete key
			0x7f => {
				buf.pop(prefix, &mut stdout)?;
			}
			// ESC
			0x1b => {
				let Some(byte) = bytes.next() else {
					return Err(std::io::ErrorKind::InvalidData.into());
				};
				let byte = byte?;
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
									// if arguments != [27, 2, 13] {
									// 	if let Some(ref mut log) = log {
									// 		writeln!(log, "{arguments:?} != [27, 2, 13]").unwrap();
									// 		log.flush().unwrap();
									// 	}
									// }
									if return_mode == Mode::ReturnOnShiftNewLine {
										break 'main;
									}
									buf.push('\n', prefix, &mut stdout)?;
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
								_byte => {
									// if let Some(ref mut log) = log {
									// 	writeln!(
									// 		log,
									// 		"unhandled x1b [ {arguments:?} {byte}",
									// 		byte = byte as char
									// 	)
									// 	.unwrap();
									// 	log.flush().unwrap();
									// }

									// Do not print unknown sequencies
									// if false {
									// 	write!(&mut stdout, "\x1b[")?;
									// 	for (idx, argument) in arguments.into_iter().enumerate() {
									// 		if idx > 0 {
									// 			write!(&mut stdout, ";")?;
									// 		}
									// 		write!(&mut stdout, "{argument}")?;
									// 	}
									// 	let _ = stdout.write(&[byte])?;
									// 	Write::flush(&mut stdout)?;
									// }
								}
							}
							break;
						}
					}
					// opt + delete
					0x7f => {
						buf.pop_slice(prefix, &mut stdout)?;
					}
					_byte => {
						// if let Some(ref mut log) = log {
						// 	writeln!(log, "x1b then {byte}", byte = byte as char).unwrap();
						// 	log.flush().unwrap();
						// }
						// No idea, echo it out
						// if false {
						// 	let _ = stdout.write(&[byte])?;
						// }
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
					buf.push(c, prefix, &mut stdout)?;
				}
				// else if let Some(ref mut log) = log {
				// 	writeln!(log, "unknown char {acc}").unwrap();
				// 	log.flush().unwrap();
				// }
			}
			// not sure what these do?
			1 | 5 => {
				// if let Some(ref mut log) = log {
				// 	writeln!(log, "byte: {byte}").unwrap();
				// 	log.flush().unwrap();
				// }
			}
			// ctrl+c (I think)
			3 => {
				break;
				// return Err();
			}
			// otherwise ascii characters
			byte => {
				if let Some(c) = char::from_u32(u32::from(byte)) {
					buf.push(c, prefix, &mut stdout)?;
				} else {
					// if let Some(ref mut log) = log {
					// 	writeln!(log, "unknown char {byte}").unwrap();
					// 	log.flush().unwrap();
					// }
					break;
				}
			}
		}
	}

	let out = buf.finish(prefix, &mut stdout)?;

	terminal::disable_raw_mode()?;

	Ok(out)
}
