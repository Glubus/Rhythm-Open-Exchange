#![warn(clippy::pedantic)]
//! Tokenizer of the `#TAG:value;` syntax shared by `.sm` and `.ssc` files (StepMania's "MSD"
//! format): `//` starts a comment up to the end of the line, `\` escapes the next character, a
//! value ends at `;` and its parameters are separated by `:`. A `#` that opens a line while a
//! value is still open starts the next tag (a forgotten `;` does not swallow the rest of the file).
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

/// One `#NAME:param:param;` tag, with the name upper-cased and every parameter trimmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    pub params: Vec<String>,
}

impl Tag {
    /// The whole value, parameters rejoined with `:` (a raw `:` in a title is not a separator).
    #[must_use]
    pub fn text(&self) -> String {
        self.params.join(":")
    }

    /// The first parameter.
    #[must_use]
    pub fn first(&self) -> &str {
        self.params.first().map_or("", String::as_str)
    }
}

/// Splits `content` into its tags, in file order.
#[must_use]
pub fn tokenize(content: &str) -> Vec<Tag> {
    let mut tags = Vec::new();
    let mut chars = content.chars().peekable();
    // Whether only whitespace has been seen since the last line break.
    let mut line_start = true;
    let mut current: Option<(String, Vec<String>, String)> = None; // (name, params, buffer)
    let mut in_name = false;
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') {
            while chars.next_if(|&next| next != '\n').is_some() {}
            continue;
        }
        if c == '#' && (current.is_none() || line_start) {
            finish(&mut tags, current.take(), in_name);
            current = Some((String::new(), Vec::new(), String::new()));
            in_name = true;
            line_start = false;
            continue;
        }
        let Some((name, params, buffer)) = current.as_mut() else {
            line_start = c == '\n' || (line_start && c.is_whitespace());
            continue;
        };
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    buffer.push(next);
                }
            }
            ':' if in_name => {
                *name = core::mem::take(buffer);
                in_name = false;
            }
            ':' => params.push(core::mem::take(buffer)),
            ';' => {
                if in_name {
                    *name = core::mem::take(buffer);
                } else {
                    params.push(core::mem::take(buffer));
                }
                finish(&mut tags, current.take(), false);
                in_name = false;
            }
            _ => buffer.push(c),
        }
        line_start = c == '\n' || (line_start && c.is_whitespace());
    }
    finish(&mut tags, current.take(), in_name);
    tags
}

/// Closes a tag; one cut off by the end of the file or by the next tag keeps what it has read.
fn finish(tags: &mut Vec<Tag>, current: Option<(String, Vec<String>, String)>, in_name: bool) {
    let Some((mut name, mut params, buffer)) = current else {
        return;
    };
    if in_name {
        name = buffer;
    } else if !buffer.is_empty() {
        params.push(buffer);
    }
    let name = name.trim().to_ascii_uppercase();
    if name.is_empty() {
        return;
    }
    let params = params.into_iter().map(|p| p.trim().into()).collect();
    tags.push(Tag { name, params });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_tags_params_and_trims() {
        let tags = tokenize("#title : Hi ;\n#NOTES:\n dance-single :\n desc:\n 0000\n;\n");
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "TITLE");
        assert_eq!(tags[0].first(), "Hi");
        assert_eq!(tags[1].params, ["dance-single", "desc", "0000"]);
    }

    #[test]
    fn comments_escapes_and_missing_semicolons() {
        let tags = tokenize("#A:x // gone\ny;\n#B:a\\;b\\:c;\n#C:no end\n#D:ok;");
        assert_eq!(tags[0].first(), "x \ny");
        assert_eq!(tags[1].first(), "a;b:c");
        assert_eq!(tags[2].name, "C");
        assert_eq!(tags[2].first(), "no end");
        assert_eq!(tags[3].name, "D");
    }
}
