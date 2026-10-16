//! Message tables (`message_X.str` / `message_X.bin`, X = e/f/g/i/s) and their text
//! encoding with control codes. Spec: `docs/formats/text.md`.
//!
//! `.str`: `u32` offset table (first offset / 4 = string count), then NUL-terminated
//! strings. `.bin` holds the same strings as `{u16 entry size in 16-bit words, string,
//! NUL, pad to even}` records. Not compressed: the survey's "Huffman" guess came from
//! the first offset's low byte (0x20).
//!
//! Encoding: single bytes are Latin-1 (`0x0A` newline); `0x81..=0x9F` lead a two-byte
//! game glyph (button icons, symbols; Shift-JIS-like codes); `#`, `$` and `~` start
//! control codes, kept as [`Token`]s.

use crate::read::{self as bytes, Error, Result};

/// One decoded piece of a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    /// Plain text.
    Text(String),
    /// Line break (`0x0A`).
    Newline,
    /// `#C<x>`: set text color `x`.
    Color(char),
    /// `#R`: end `#C` color.
    ColorEnd,
    /// `#c<x>`: set the alternative (name) color `x`.
    AltColor(char),
    /// `#r`: end `#c` color.
    AltColorEnd,
    /// `#P`: wait for a button, then new page.
    PageBreak,
    /// `#W`: wait for a button.
    WaitButton,
    /// `#=<b>`: move the cursor to x = `b` pixels.
    SetX(u8),
    /// `#y<b>`: move the cursor to y = `b` (`[hypothesis]`).
    SetY(u8),
    /// Other `#<x>` codes: `#+` (centre the line, `[red-hypothesis]`), `#>` (alignment,
    /// `[red-hypothesis]`), `#1`..`#9` (meaning not yet confirmed).
    Code(char),
    /// `$<kind><n>` placeholder: `m` monster, `i` item, `d` number, `n` name, `t` team
    /// name, `h` friend area (`[red-hypothesis]`, from Red's `charmap.txt`); `v` is not
    /// identified.
    Var {
        /// Kind letter.
        kind: char,
        /// Slot digit, if present.
        index: Option<u8>,
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
            (b'#', Some(c)) => Some(match c {
                b'C' | b'c' if i + 2 < s.len() => {
                    let x = char::from(s[i + 2]);
                    (
                        if c == b'C' {
                            Token::Color(x)
                        } else {
                            Token::AltColor(x)
                        },
                        3,
                    )
                }
                b'R' => (Token::ColorEnd, 2),
                b'r' => (Token::AltColorEnd, 2),
                b'P' => (Token::PageBreak, 2),
                b'W' => (Token::WaitButton, 2),
                b'=' if i + 2 < s.len() => (Token::SetX(s[i + 2]), 3),
                b'y' if i + 2 < s.len() => (Token::SetY(s[i + 2]), 3),
                _ => (Token::Code(char::from(c)), 2),
            }),
            (b'$', Some(k)) if k.is_ascii_alphabetic() => {
                let d = s
                    .get(i + 2)
                    .filter(|d| d.is_ascii_digit())
                    .map(|d| d - b'0');
                Some((
                    Token::Var {
                        kind: char::from(k),
                        index: d,
                    },
                    2 + usize::from(d.is_some()),
                ))
            }
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
            (0x81..=0x9F, Some(lo)) => Some((Token::Glyph(u16::from(b) << 8 | u16::from(lo)), 2)),
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
}
