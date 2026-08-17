use super::super::types::OsuHitObject;

#[must_use]
pub fn parse_hit_object(line: &str) -> Option<OsuHitObject> {
    parse_hit_object_bytes(line.as_bytes())
}

#[must_use]
pub fn parse_hit_object_bytes(line: &[u8]) -> Option<OsuHitObject> {
    let mut iter = memchr::memchr_iter(b',', line);
    let mut start = 0;

    let mut next_field = || {
        if let Some(end) = iter.next() {
            let field = &line[start..end];
            start = end + 1;
            Some(field)
        } else if start <= line.len() {
            let field = &line[start..];
            start = line.len() + 1;
            Some(field)
        } else {
            None
        }
    };

    let x: i32 = atoi::atoi(next_field()?)?;
    let _y: i32 = atoi::atoi(next_field()?)?;
    let time: i32 = atoi::atoi(next_field()?)?;
    let object_type: u8 = atoi::atoi(next_field()?)?;
    let hit_sound: u8 = atoi::atoi(next_field()?)?;
    let extras_start = start;

    let end_time = parse_hold_end_time(object_type, line, start);
    let extras = extract_extras(line, extras_start);

    Some(OsuHitObject {
        x,
        y: 192,
        time,
        object_type,
        hit_sound,
        end_time,
        extras,
    })
}

fn parse_hold_end_time(object_type: u8, line: &[u8], start: usize) -> Option<i32> {
    if (object_type & 128) == 0 {
        return None;
    }
    let rest = line.get(start..)?;
    let end_pos = memchr::memchr(b',', rest).unwrap_or(rest.len());
    let field = &rest[..end_pos];
    let colon_pos = memchr::memchr(b':', field).unwrap_or(field.len());
    atoi::atoi(&field[..colon_pos])
}

fn extract_extras(line: &[u8], extras_start: usize) -> compact_str::CompactString {
    if extras_start < line.len() {
        // Safety: osu files are validated as UTF-8 at the parse() entry point
        unsafe { compact_str::CompactString::from_utf8_unchecked(&line[extras_start..]) }
    } else {
        compact_str::CompactString::new("")
    }
}
