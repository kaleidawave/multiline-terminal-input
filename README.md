# multiline-terminal-input

[![crates.io badge](https://img.shields.io/crates/v/multiline-terminal-input)](https://crates.io/crates/multiline-terminal-input)
[![docs.rs badge](https://img.shields.io/docsrs/multiline-terminal-input)](https://docs.rs/multiline-terminal-input/latest/multiline-terminal-input/)
[![compiler-club](https://f005.backblazeb2.com/file/6948446f806c172a098591088e0715b8258b380b/join-compiler-club.svg)](https://kaleidawave-public-pages.val.run/r/join-cc)

An input library that handles `shift+enter`. If `mode == Mode::ReturnOnUndecoratedNewLine` then adds newline (same behavior as nodejs's REPL) else with `mode == Mode::ReturnOnShiftNewLine` continues until `shift+enter` is pressed.

## Features

- `read_to_string` and `read_to_vec_string`
- configurability for prompt, prefix and whether shift-enter breaks (enter continues) or enter breaks (shift-enter continues)
- ability to set an existing input

## Implementation

<!-- For windows there is an API that exposes `is_key_down(KeyCode::Shift)`. Depending on that it either continues or breaks reading lines from `std::io::Stdin`. -->

<!--For macos there isn't an API. Instead it -->

Puts the terminal in raw mode and attempts to handle the entire input, appending various inputs to a buffer and breaking on a certain sequences. This does mean that this library attempts to parse all input sequences from a terminal such as ANSI and unicode. It is incomplete.

## Testing

[This crate lacks testing](https://github.com/kaleidawave/multiline-term-input/issues/3).

## References

- [VT Sequence Reference](https://ghostty.org/docs/vt/reference)
