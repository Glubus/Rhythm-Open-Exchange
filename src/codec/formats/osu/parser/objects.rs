use super::super::types::OsuHitObject;

#[must_use]
pub fn parse_hit_object(line: &str) -> Option<OsuHitObject> {
    parse_hit_object_bytes(line.as_bytes())
}

pub fn parse_hit_object_bytes(line: &[u8]) -> Option<OsuHitObject> {
    let mut iter = memchr::memchr_iter(b',', line);

    // Helper to get next field and update start position
    let mut start = 0;
    let mut next_field = || {
        if let Some(end) = iter.next() {
            let field = &line[start..end];
            start = end + 1;
            Some(field)
        } else if start <= line.len() {
            let field = &line[start..];
            start = line.len() + 1; // Ensure we don't return empty string infinitely
            Some(field)
        } else {
            None
        }
    };

    let f_x = next_field()?;
    let f_y = next_field()?;
    let f_time = next_field()?;
    let f_type = next_field()?;
    let f_sound = next_field()?;

    let x: i32 = atoi::atoi(f_x)?;
    let y: i32 = atoi::atoi(f_y)?;
    let time: i32 = atoi::atoi(f_time)?;
    let object_type: u8 = atoi::atoi(f_type)?;
    let hit_sound: u8 = atoi::atoi(f_sound)?;
    // Save the start of extras for later
    let extras_start = start;

    // Check for hold note (type & 128)
    let end_time = if (object_type & 128) != 0 {
        // Hold note format: x,y,time,type,hitSound,endTime:extras
        // We need the next field (parts[5]) but without consuming it from 'start' used for 'extras'
        // Find end of the next field
        let rest = if start < line.len() {
            &line[start..]
        } else {
            &[]
        };
        let end_pos = memchr::memchr(b',', rest).unwrap_or(rest.len());
        let f_extras = &rest[..end_pos];

        // The endTime is the first part of the extras field before the colon
        let mut extra_iter = memchr::memchr_iter(b':', f_extras);
        if let Some(colon_pos) = extra_iter.next() {
            atoi::atoi(&f_extras[..colon_pos])
        } else {
            atoi::atoi(f_extras)
        }
    } else {
        None
    };

    // Extras are complex to parse fully with zero-copy without changing the struct to hold Cow or refs
    // For now we can just convert the remainder to string if needed.
    // The struct expects String.
    // We use extras_start which points to everything after the 5th comma.
    let extras = if extras_start < line.len() {
        // SAFETY: We assume valid UTF-8 as checked at file entry
        unsafe { compact_str::CompactString::from_utf8_unchecked(&line[extras_start..]) }
    } else {
        compact_str::CompactString::new("")
    };

    Some(OsuHitObject {
        x,
        y,
        time,
        object_type,
        hit_sound,
        end_time,
        extras,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hit_object_tap() {
        let line = "402,192,1694,5,0,0:0:0:0:";
        let ho = parse_hit_object(line).unwrap();

        assert_eq!(ho.x, 402);
        assert_eq!(ho.y, 192);
        assert_eq!(ho.time, 1694);
        assert_eq!(ho.object_type, 5);
        assert_eq!(ho.hit_sound, 0);
    }
}
