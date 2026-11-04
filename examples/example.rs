use multiline_terminal_input::{read_to_string, Mode, Options};

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let options = Options {
		prefix: "/// ",
		// return_mode: Mode::ReturnOnShiftNewLine,
		return_mode: Mode::ReturnOnUndecoratedNewLine,
		prompt: "enter comment",
	};
	dbg!(read_to_string(options)?);
	Ok(())
}
