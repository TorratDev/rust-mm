//! Message tables (`message_X.str` / `message_X.bin`, X = e/f/g/i/s) and their text
//! encoding with control codes. Spec: `docs/formats/text.md`.
//!
//! `.str`: `u32` offset table (first offset / 4 = string count), then NUL-terminated
//! strings. `.bin` holds the same strings as `{u16 entry size in 16-bit words, string,
//! NUL, pad to even}` records. Not compressed: the survey's "Huffman" guess came from
//! the first offset's low byte (0x20). The game never opens the `.bin` files.
//!
//! Encoding: single bytes are Latin-1 (`0x0A` newline). `0x81`..`0x84` and `0x87` lead a
//! two-byte game glyph (button icons, symbols). `~XX` is a hex-byte escape. `#` and `$`
//! start control codes and variables, kept as [`Token`]s.

use crate::read::{self as bytes, Error, Result};

/// One decoded piece of a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    /// Plain text.
    Text(String),
    /// Line break (`0x0A`).
    Newline,
    /// `#C<x>`: push text color `x`.
    Color(char),
    /// `#R`: pop the text color.
    ColorEnd,
    /// `#c<x>`: push the alternative (name) color `x`.
    AltColor(char),
    /// `#r`: pop the alternative color.
    AltColorEnd,
    /// `#P`: new page, then wait for a button.
    PageBreak,
    /// `#p`: as `#P`, with a flag set (flag meaning unknown).
    PageBreakFlagged,
    /// `#W`: wait for a button.
    WaitButton,
    /// `#=<b>`: set x to `b`.
    SetX(u8),
    /// `#y<b>`: set y to `b`.
    SetY(u8),
    /// `#.<b>`: add `b` to x.
    AddX(u8),
    /// `#n`: next line, x back to the saved origin, y + 11.
    LineFeed,
    /// `#:`: save x.
    SaveX,
    /// `#;<b>`: set x to the saved x plus `b`.
    SavedX(u8),
    /// `#+`: centre the line.
    Centre,
    /// `#>NNN.`: set x to the decimal `NNN`.
    SetXAbs(u16),
    /// `#_<b>`: push the shade from table entry `b`.
    Shade(u8),
    /// `#S<b><c>`: write `c & 0x7F` into table entry `b & 0x7F`.
    SetTable {
        /// Table index.
        index: u8,
        /// Value written.
        value: u8,
    },
    /// `#~<b>`: wait `b` frames.
    WaitFrames(u8),
    /// `#[ ... ]`: the bytes between the brackets, passed to a game callback.
    Tag(Vec<u8>),
    /// Other `#<x>` codes (`#1`..`#9`, ...): meaning not identified.
    Code(char),
    /// `$<kind><n>` placeholder: `m` monster, `i` item, `d` number, `n` name (with a
    /// slot digit); `mm` runtime monster id and `t` team name, `h` friend area (no
    /// digit). The table contents are not identified.
    Var {
        /// Kind letter.
        kind: char,
        /// Slot digit, if present.
        index: Option<u8>,
    },
    /// `$v<N><W>` / `$V<N><W>`: number from slot `N`, `W` wide; `$V` pads with zeros.
    Number {
        /// `$V` rather than `$v`.
        zero_pad: bool,
        /// Slot digit.
        index: u8,
        /// Width digit.
        width: u8,
    },
    /// Two-byte game glyph.
    Glyph(u16),
}

/// Splits a `.str` file into its strings (without NULs).
pub fn parse_str(data: &[u8]) -> Result<Vec<&[u8]>> {
    let first = bytes::offset_at(data, 0, "string table")?;
    if first == 0 || first % 4 != 0 || first > data.len() {
        return Err(Error::invalid(
            "string table",
            format!("first offset {first:#x}"),
        ));
    }
    let n = first / 4;
    (0..n)
        .map(|i| {
            let start = bytes::offset_at(data, i * 4, "string table")?;
            let end = if i + 1 < n {
                bytes::offset_at(data, i * 4 + 4, "string table")?
            } else {
                data.len()
            };
            if start < first || end < start || end > data.len() {
                return Err(Error::invalid(
                    "string table",
                    format!("string {i}: {start:#x}..{end:#x}"),
                ));
            }
            let s = &data[start..end];
            Ok(s.iter().position(|&b| b == 0).map_or(s, |z| &s[..z]))
        })
        .collect()
}

/// Splits a `.bin` file into its strings.
pub fn parse_bin(data: &[u8]) -> Result<Vec<&[u8]>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 2 <= data.len() {
        let words = usize::from(bytes::u16_at(data, at, "message record")?);
        let s = bytes::cstr_at(data, at + 2, "message record")?;
        let size = (2 + s.len() + 1 + 1) & !1;
        if words * 2 != size {
            return Err(Error::invalid(
                "message record",
                format!("at {at:#x}: size {words} words, string needs {size} bytes"),
            ));
        }
        out.push(s);
        at += size;
    }
    Ok(out)
}

/// Decodes one message into tokens. Malformed codes at the end of a string decode as
/// plain text rather than failing: strings are data, the renderer can show them.
pub fn decode(s: &[u8]) -> Vec<Token> {
    let mut out = Vec::new();
    let mut text = String::new();
    let flush = |text: &mut String, out: &mut Vec<Token>| {
        if !text.is_empty() {
            out.push(Token::Text(std::mem::take(text)));
        }
    };
    let mut i = 0;
    while i < s.len() {
        let b = s[i];
        let next = s.get(i + 1).copied();
        let tok = match (b, next) {
            (b'\n', _) => Some((Token::Newline, 1)),
            (b'#', Some(_)) => Some(hash_code(s, i)),
            (b'$', Some(b'$')) => {
                text.push('$');
                i += 2;
                continue;
            }
            (b'$', Some(b'-')) => {
                i += 2;
                continue;
            }
            (b'$', Some(k)) if k.is_ascii_alphabetic() => match dollar(s, i) {
                Some(t) => Some(t),
                None => {
                    // Unknown letter: the game prints it without the `$`.
                    i += 1;
                    continue;
                }
            },
            (b'~', _) if i + 2 < s.len() => {
                let hex = std::str::from_utf8(&s[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(v) => {
                        text.push(char::from(v));
                        i += 3;
                        continue;
                    }
                    None => None,
                }
            }
            (0x81..=0x84 | 0x87, Some(lo)) => {
                Some((Token::Glyph(u16::from(b) << 8 | u16::from(lo)), 2))
            }
            _ => None,
        };
        match tok {
            Some((t, len)) => {
                flush(&mut text, &mut out);
                out.push(t);
                i += len;
            }
            None => {
                text.push(char::from(b));
                i += 1;
            }
        }
    }
    flush(&mut text, &mut out);
    out
}

/// The `#` code at `s[i..]` (`s[i] == b'#'`, and `s[i + 1]` exists): its token and length.
/// A code whose arguments are missing decodes as [`Token::Code`].
fn hash_code(s: &[u8], i: usize) -> (Token, usize) {
    let c = s[i + 1];
    let arg = s.get(i + 2).copied();
    match (c, arg) {
        (b'C', Some(x)) => (Token::Color(char::from(x)), 3),
        (b'c', Some(x)) => (Token::AltColor(char::from(x)), 3),
        (b'R', _) => (Token::ColorEnd, 2),
        (b'r', _) => (Token::AltColorEnd, 2),
        (b'P', _) => (Token::PageBreak, 2),
        (b'p', _) => (Token::PageBreakFlagged, 2),
        (b'W', _) => (Token::WaitButton, 2),
        (b'n', _) => (Token::LineFeed, 2),
        (b':', _) => (Token::SaveX, 2),
        (b'+', _) => (Token::Centre, 2),
        (b'=', Some(x)) => (Token::SetX(x), 3 + skip_dot(s, i + 3)),
        (b'y', Some(x)) => (Token::SetY(x), 3 + skip_dot(s, i + 3)),
        (b'.', Some(x)) => (Token::AddX(x), 3),
        (b';', Some(x)) => (Token::SavedX(x), 3),
        (b'_', Some(x)) => (Token::Shade(x), 3 + skip_dot(s, i + 3)),
        (b'~', Some(x)) => (Token::WaitFrames(x), 3),
        (b'S', Some(x)) if i + 3 < s.len() => (
            Token::SetTable {
                index: x & 0x7F,
                value: s[i + 3] & 0x7F,
            },
            4,
        ),
        (b'>', _) => set_x_abs(s, i).unwrap_or((Token::Code('>'), 2)),
        (b'[', _) => match s[i + 2..].iter().position(|&b| b == b']') {
            Some(end) => (Token::Tag(s[i + 2..i + 2 + end].to_vec()), end + 3),
            None => (Token::Code('['), 2),
        },
        _ => (Token::Code(char::from(c)), 2),
    }
}

/// Length of the optional `.` at `s[j]` that `#=`, `#y` and `#_` skip after their byte
/// (`FUN_02014098`). `#.`, `#;` and `#~` do not skip it.
fn skip_dot(s: &[u8], j: usize) -> usize {
    usize::from(s.get(j) == Some(&b'.'))
}

/// `#>NNN.` at `s[i..]`: decimal digits, then the `.` that ends them. `None` without both.
fn set_x_abs(s: &[u8], i: usize) -> Option<(Token, usize)> {
    let digits = s[i + 2..].iter().take_while(|d| d.is_ascii_digit()).count();
    let end = i + 2 + digits;
    if digits == 0 || s.get(end) != Some(&b'.') {
        return None;
    }
    let x = std::str::from_utf8(&s[i + 2..end]).ok()?.parse().ok()?;
    Some((Token::SetXAbs(x), digits + 3))
}

/// The `$` placeholder at `s[i..]` (`s[i] == b'$'`, `s[i + 1]` a letter). `None` for a
/// letter the game does not handle.
fn dollar(s: &[u8], i: usize) -> Option<(Token, usize)> {
    let dig = |j: usize| s.get(j).filter(|d| d.is_ascii_digit()).map(|d| d - b'0');
    let k = s[i + 1];
    Some(match k {
        b'v' | b'V' => (
            Token::Number {
                zero_pad: k == b'V',
                index: dig(i + 2)?,
                width: dig(i + 3)?,
            },
            4,
        ),
        b'm' if s.get(i + 2) == Some(&b'm') => (
            Token::Var {
                kind: 'm',
                index: None,
            },
            3,
        ),
        b'd' | b'm' | b'i' | b'n' => (
            Token::Var {
                kind: char::from(k),
                index: Some(dig(i + 2)?),
            },
            3,
        ),
        b't' | b'h' => (
            Token::Var {
                kind: char::from(k),
                index: None,
            },
            2,
        ),
        _ => return None,
    })
}

/// Plain readable text: control codes dropped, placeholders as `{m0}`, glyphs as
/// `{gXXXX}`.
pub fn to_plain(tokens: &[Token]) -> String {
    let mut s = String::new();
    for t in tokens {
        match t {
            Token::Text(x) => s.push_str(x),
            Token::Newline => s.push('\n'),
            Token::Var { kind, index } => {
                s.push('{');
                s.push(*kind);
                if let Some(d) = index {
                    s.push(char::from(b'0' + d));
                }
                s.push('}');
            }
            Token::Number {
                zero_pad,
                index,
                width,
            } => {
                let v = if *zero_pad { 'V' } else { 'v' };
                s.push_str(&format!("{{{v}{index}{width}}}"));
            }
            Token::Glyph(g) => s.push_str(&format!("{{g{g:04X}}}")),
            _ => {}
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_tables() {
        let mut d = Vec::new();
        d.extend(8u32.to_le_bytes());
        d.extend(11u32.to_le_bytes());
        d.extend(b"ab\0cd\0");
        assert_eq!(parse_str(&d).unwrap(), [&b"ab"[..], b"cd"]);
        assert!(parse_str(&[3, 0, 0, 0]).is_err());
        let bin = [4u8, 0, b'N', b'o', b'n', b'e', 0, 0, 2, 0, b'a', 0];
        assert_eq!(parse_bin(&bin).unwrap(), [&b"None"[..], b"a"]);
        assert!(parse_bin(&[9, 0, b'a', 0]).is_err());
    }

    #[test]
    fn decodes_control_codes() {
        let t = decode(b"Hi #CG$m0#R~27s\n#=\xb8x\x81\x60\xe9");
        assert_eq!(
            t,
            [
                Token::Text("Hi ".into()),
                Token::Color('G'),
                Token::Var {
                    kind: 'm',
                    index: Some(0)
                },
                Token::ColorEnd,
                Token::Text("'s".into()),
                Token::Newline,
                Token::SetX(0xB8),
                Token::Text("x".into()),
                Token::Glyph(0x8160),
                Token::Text("é".into()),
            ]
        );
        assert_eq!(to_plain(&t), "Hi {m0}'s\nx{g8160}é");
        assert_eq!(decode(b"#"), [Token::Text("#".into())]);
    }

    #[test]
    fn glyph_leads_are_narrow() {
        assert_eq!(decode(b"\x85\x40"), [Token::Text("\u{85}@".into())]);
        assert_eq!(decode(b"\x87\x40"), [Token::Glyph(0x8740)]);
    }

    #[test]
    fn hash_codes_consume_their_arguments() {
        let t = decode(b"#=\x05#yx#.\x01#:#;\x02#+#n#p#P#W#~\x09#_\x03#S\x12\x34#R#r#c\x41end");
        assert_eq!(
            t,
            [
                Token::SetX(5),
                Token::SetY(b'x'),
                Token::AddX(1),
                Token::SaveX,
                Token::SavedX(2),
                Token::Centre,
                Token::LineFeed,
                Token::PageBreakFlagged,
                Token::PageBreak,
                Token::WaitButton,
                Token::WaitFrames(9),
                Token::Shade(3),
                Token::SetTable {
                    index: 0x12,
                    value: 0x34
                },
                Token::ColorEnd,
                Token::AltColorEnd,
                Token::AltColor('A'),
                Token::Text("end".into()),
            ]
        );
    }

    #[test]
    fn optional_dot_after_set_codes() {
        assert_eq!(decode(b"#=@.x"), [Token::SetX(64), Token::Text("x".into())]);
        assert_eq!(decode(b"#y@.x"), [Token::SetY(64), Token::Text("x".into())]);
        assert_eq!(
            decode(b"#_@.x"),
            [Token::Shade(64), Token::Text("x".into())]
        );
        // The dot is not skipped after these.
        assert_eq!(
            decode(b"#.\x01.x"),
            [Token::AddX(1), Token::Text(".x".into())]
        );
        assert_eq!(
            decode(b"#;\x02.x"),
            [Token::SavedX(2), Token::Text(".x".into())]
        );
        assert_eq!(
            decode(b"#~\x09.x"),
            [Token::WaitFrames(9), Token::Text(".x".into())]
        );
    }

    #[test]
    fn set_x_absolute() {
        assert_eq!(
            decode(b"#>123.x"),
            [Token::SetXAbs(123), Token::Text("x".into())]
        );
        assert_eq!(
            decode(b"#>.x"),
            [Token::Code('>'), Token::Text(".x".into())]
        );
    }

    #[test]
    fn tags_and_unknown_codes() {
        assert_eq!(
            decode(b"#[ab]c"),
            [Token::Tag(b"ab".to_vec()), Token::Text("c".into())]
        );
        assert_eq!(
            decode(b"#[ab"),
            [Token::Code('['), Token::Text("ab".into())]
        );
        assert_eq!(decode(b"#5"), [Token::Code('5')]);
    }

    #[test]
    fn variables() {
        assert_eq!(
            decode(b"$v12$V34"),
            [
                Token::Number {
                    zero_pad: false,
                    index: 1,
                    width: 2
                },
                Token::Number {
                    zero_pad: true,
                    index: 3,
                    width: 4
                },
            ]
        );
        assert_eq!(
            decode(b"$mm$m0$t$$$-$e1"),
            [
                Token::Var {
                    kind: 'm',
                    index: None
                },
                Token::Var {
                    kind: 'm',
                    index: Some(0)
                },
                Token::Var {
                    kind: 't',
                    index: None
                },
                Token::Text("$e1".into()),
            ]
        );
    }
}
