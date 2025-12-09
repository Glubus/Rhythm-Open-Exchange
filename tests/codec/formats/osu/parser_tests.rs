//! Tests for osu parser.

use rhythm_open_exchange::codec::formats::osu::{OsuHitObject, parse};

#[test]
fn test_parse_timing_point_bpm() {
    let data = b"osu file format v14\n\n[General]\nMode: 3\n\n[TimingPoints]\n404,322.58064516129,4,1,1,50,1,0\n";
    let beatmap = parse(data).unwrap();

    assert_eq!(beatmap.timing_points.len(), 1);
    let tp = &beatmap.timing_points[0];
    assert_eq!(tp.time, 404.0);
    assert!(tp.uninherited);
    assert!((tp.bpm().unwrap() - 186.0).abs() < 1.0);
}

#[test]
fn test_parse_timing_point_sv() {
    let data = b"osu file format v14\n\n[General]\nMode: 3\n\n[TimingPoints]\n21855,-133.333333333333,4,1,1,50,0,0\n";
    let beatmap = parse(data).unwrap();

    let tp = &beatmap.timing_points[0];
    assert!(!tp.uninherited);
    assert!((tp.scroll_velocity() - 0.75).abs() < 0.01);
}

#[test]
fn test_parse_timing_point_sv_normal() {
    let data =
        b"osu file format v14\n\n[General]\nMode: 3\n\n[TimingPoints]\n32500,-100,4,1,1,50,0,0\n";
    let beatmap = parse(data).unwrap();

    let tp = &beatmap.timing_points[0];
    assert!((tp.scroll_velocity() - 1.0).abs() < 0.01);
}

#[test]
fn test_parse_hit_object_tap() {
    let data = b"osu file format v14\n\n[General]\nMode: 3\n\n[Difficulty]\nCircleSize:7\n\n[HitObjects]\n402,192,1694,5,0,0:0:0:0:\n";
    let beatmap = parse(data).unwrap();

    assert_eq!(beatmap.hit_objects.len(), 1);
    let ho = &beatmap.hit_objects[0];
    assert_eq!(ho.x, 402);
    assert_eq!(ho.time, 1694);
}

#[test]
fn test_column_calculation() {
    let ho = OsuHitObject {
        x: 36,
        y: 192,
        time: 0,
        object_type: 1,
        hit_sound: 0,
        end_time: None,
        extras: String::new(),
    };
    assert_eq!(ho.column(7), 0);

    let ho2 = OsuHitObject {
        x: 475,
        ..ho.clone()
    };
    assert_eq!(ho2.column(7), 6);

    let ho3 = OsuHitObject {
        x: 256,
        ..ho.clone()
    };
    assert_eq!(ho3.column(7), 3); // center
}

#[test]
fn test_parse_full_sample() {
    let data = include_bytes!("../../../../assets/osu/mania_7k.osu");
    let beatmap = parse(data).unwrap();

    assert_eq!(beatmap.general.mode, 3); // mania
    assert_eq!(beatmap.difficulty.circle_size, 7.0); // 7K
    assert!(!beatmap.timing_points.is_empty());
    assert!(!beatmap.hit_objects.is_empty());
    assert_eq!(beatmap.metadata.version, "7K Awakened");
}
