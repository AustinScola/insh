/*!
The ANSI escape sequences which both a terminal and the programs running in it send, and the colors
and styles which they carry.

There are two layers to this. [`AnsiEscapeSequence`] is the structure of a sequence: which sort it
is, and its private marker, parameters, intermediates, final byte and string payload. Knowing where
a sequence ends is enough to pass one along without understanding it, which is what matters for
anything sitting between a terminal and a program.

[`ControlFunction`] is what a sequence means, which is worked out from the structure of it by
[`AnsiEscapeSequence::control_function`]. A sequence which is not one that is recognized reads as
`None` rather than being thrown away, so there is always the sequence itself to fall back on.

Both layers go back the other way as well, so anything here can be written out as the bytes for it.
A [`ControlFunction`] is written as short as it goes, leaving out the parameters which are the
default anyway.

Nothing here reads from or writes to a terminal. Reading input from one is what `term` is for, and
drawing on one is what `rend` is for; this is only the vocabulary which the two of them share.

# References

These are what this crate is written against. Anything added to it should be checked against them,
and the clause numbers below are the ones the doc comments here refer to.

- [ECMA-48, 5th edition (June 1991)][ecma-48] is the standard itself, and is the authority for
  everything here unless it says otherwise. Clause 5.3 is the structure of an escape sequence, 5.4
  is the structure of a control sequence, 5.6 is control strings, and 8.3 defines each control
  function along with the default value of each of its parameters.
- [An HTML transcription of it][ecma-48-html], which is easier to search than the PDF.
- [ECMA-48 5.4, annotated by the xterm maintainer][parameter-format], on which bytes may appear
  where in a control sequence and how a parameter which is left out is read.
- [XTerm Control Sequences][ctlseqs] is the de facto reference for the control functions which are
  not in ECMA-48, which is most of the ones a terminal actually uses: the modes which are private to
  a terminal, the operating system commands, and the sequences the keys send.
- [The ANSI escape code article on Wikipedia][wikipedia] is the readable summary, and is careful
  about saying which select graphic rendition parameters come from where.

Where a control function is not from ECMA-48 the doc comment for it says so.

[ecma-48]: https://ecma-international.org/wp-content/uploads/ECMA-48_5th_edition_june_1991.pdf
[ecma-48-html]: https://wezfurlong.org/ecma48/
[parameter-format]: https://invisible-island.net/xterm/ecma-48-parameter-format.html
[ctlseqs]: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
[wikipedia]: https://en.wikipedia.org/wiki/ANSI_escape_code
*/
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(clippy::needless_return)]

mod control_function;
mod graphic_rendition;
mod operating_system_command;
mod sequence;

pub use control_function::{
    ClearTabStops, ControlFunction, CursorTabulationControl, EraseInDisplay, EraseInLine,
    MediaCopy, Mode,
};
pub use graphic_rendition::{Color, GraphicRendition, Underline};
pub use operating_system_command::OperatingSystemCommand;
pub use sequence::{
    AnsiEscapeSequence, AnsiEscapeSequenceParseError, BracketedPaste, ControlSequence,
    DeviceControlString, Parameter, ParsedAnsiEscapeSequence, BELL, ESCAPE, STRING_TERMINATOR,
};
