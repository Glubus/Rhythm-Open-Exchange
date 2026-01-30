use super::super::types::OsuHitObject;

pub fn parse_hit_object(line: &str) -> Option<OsuHitObject> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 5 {
        return None;
    }

    let x: i32 = parts[0].parse().ok()?;
    let y: i32 = parts[1].parse().ok()?;
    let time: i32 = parts[2].parse().ok()?;
    let object_type: u8 = parts[3].parse().ok()?;
    let hit_sound: u8 = parts[4].parse().ok()?;

    // Check for hold note (type & 128)
    let end_time = if (object_type & 128) != 0 && parts.len() > 5 {
        // Hold note format: x,y,time,type,hitSound,endTime:extras
        let extras = parts[5];
        extras.split(':').next().and_then(|s| s.parse().ok())
    } else {
        None
    };

    let extras = if parts.len() > 5 {
        parts[5..].join(",")
    } else {
        String::new()
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
