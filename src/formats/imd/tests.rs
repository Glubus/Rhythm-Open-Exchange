//! Tests of the `.imd` decoder. Every file is built here byte by byte from the format notes (no real chart is in the repository);
//! the times are computed by hand.

use rox::codec::Decoder;
use rox::model::{NoteType, RoxChart, TimingPoint};

use super::decoder::{ImdDecoder, ImdDifficulty, ImdReport, parse_name};
use super::sniff::{is_imd_chart_file, looks_like_imd};

/// A file under construction: `(time_ms, bpm)` entries and `(type, time_ms, track, action)` records.
struct File {
    total_ms: u32,
    bpms: Vec<(u32, f64)>,
    notes: Vec<(u16, u32, u8, i32)>,
    separator: u16,
}

impl File {
    fn new(notes: &[(u16, u32, u8, i32)]) -> Self {
        Self {
            total_ms: 90_000,
            bpms: vec![(0, 120.0)],
            notes: notes.to_vec(),
            separator: 0x0303,
        }
    }

    fn bpms(mut self, bpms: &[(u32, f64)]) -> Self {
        self.bpms = bpms.to_vec();
        self
    }

    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(self.total_ms.to_le_bytes());
        out.extend(u32::try_from(self.bpms.len()).unwrap().to_le_bytes());
        for (time, bpm) in &self.bpms {
            out.extend(time.to_le_bytes());
            out.extend(bpm.to_le_bytes());
        }
        out.extend(self.separator.to_le_bytes());
        out.extend(u32::try_from(self.notes.len()).unwrap().to_le_bytes());
        for (kind, time, track, action) in &self.notes {
            out.extend(kind.to_le_bytes());
            out.extend(time.to_le_bytes());
            out.push(*track);
            out.extend(action.to_le_bytes());
        }
        out
    }
}

fn decode(notes: &[(u16, u32, u8, i32)], name: &str) -> (RoxChart, ImdReport) {
    ImdDecoder::decode_with_report(&File::new(notes).bytes(), name).unwrap()
}

/// `(time_us, column, duration_us)` of every note (0 for a tap).
fn summary(chart: &RoxChart) -> Vec<(i64, u8, i64)> {
    chart
        .notes
        .iter()
        .map(|note| {
            (
                note.time_us,
                note.column,
                match note.note_type {
                    NoteType::Hold { duration_us } => duration_us,
                    _ => 0,
                },
            )
        })
        .collect()
}

fn bpm_points(chart: &RoxChart) -> Vec<(i64, f32)> {
    chart
        .timing_points
        .iter()
        .filter_map(|point| match point {
            TimingPoint::Bpm { time_us, bpm, .. } => Some((*time_us, *bpm)),
            TimingPoint::Sv { .. } => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------- names

#[test]
fn a_file_name_gives_the_song_the_lanes_and_the_difficulty() {
    let name = parse_name("keluodiya_4k_hd.imd");
    assert_eq!(
        (name.song, name.key_count, name.difficulty),
        ("keluodiya", Some(4), Some(ImdDifficulty::Hard))
    );
    let name = parse_name("Some Song_5K_Ez.IMD");
    assert_eq!(
        (name.song, name.key_count, name.difficulty),
        ("Some Song", Some(5), Some(ImdDifficulty::Easy))
    );
    let name = parse_name("songs/x\\a_b_6k_nm.imd");
    assert_eq!(
        (name.song, name.key_count, name.difficulty),
        ("a_b", Some(6), Some(ImdDifficulty::Normal))
    );
    // No suffix, a lane count the game does not have, an unknown difficulty: the whole stem, nothing known.
    for plain in [
        "song.imd",
        "song_7k_hd.imd",
        "song_3k_hd.imd",
        "song_4k_xx.imd",
        "song_4k.imd",
        "song_4khd.imd",
        "4k_hd.imd",
        "",
        "\u{e9}\u{e9}\u{e9}.imd",
    ] {
        let name = parse_name(plain);
        assert_eq!((name.key_count, name.difficulty), (None, None), "{plain}");
        assert_eq!(name.song, plain.strip_suffix(".imd").unwrap_or(plain));
    }
    // The suffix alone is a name with no song: the stem is then kept whole, not an empty title.
    let name = parse_name("_4k_hd.imd");
    assert_eq!((name.song, name.key_count), ("_4k_hd", Some(4)));
}

#[test]
fn the_name_wins_over_the_notes_for_the_lane_count() {
    // Only lanes 0..=2 are used: 4 columns without a name, 6 with `_6k_`, 5 with `_5k_`.
    let notes = [(0, 1000, 0, 0), (0, 2000, 2, 0)];
    let (chart, report) = decode(&notes, "song.imd");
    assert_eq!(
        (
            chart.key_count,
            report.key_count,
            report.key_count_from_name
        ),
        (4, 4, false)
    );
    for (name, keys) in [("a_4k_ez.imd", 4), ("a_5k_nm.imd", 5), ("a_6k_hd.imd", 6)] {
        let (chart, report) = decode(&notes, name);
        assert_eq!(
            (chart.key_count, report.key_count_from_name),
            (keys, true),
            "{name}"
        );
        assert_eq!(chart.metadata.title, "a");
    }
}

#[test]
fn the_lane_count_without_a_name_is_read_from_the_notes() {
    for (highest, keys) in [(0u8, 4u8), (3, 4), (4, 5), (5, 6)] {
        let (chart, _) = decode(&[(0, 1000, highest, 0)], "");
        assert_eq!(chart.key_count, keys, "highest track {highest}");
        assert_eq!(chart.notes[0].column, highest);
    }
    // A track the game does not have (6 and above) is dropped and never raises the count.
    let (chart, report) = decode(&[(0, 1000, 1, 0), (0, 1500, 6, 0), (0, 2000, 200, 0)], "");
    assert_eq!(
        (
            chart.key_count,
            chart.notes.len(),
            report.notes_outside_lanes
        ),
        (4, 1, 2)
    );
}

#[test]
fn a_note_on_a_lane_the_name_does_not_have_is_dropped_and_counted() {
    let (chart, report) = decode(
        &[(0, 1000, 3, 0), (0, 2000, 4, 0), (0, 3000, 5, 0)],
        "x_4k_hd.imd",
    );
    assert_eq!(summary(&chart), vec![(1_000_000, 3, 0)]);
    assert_eq!(report.notes_outside_lanes, 2);
    let (chart, report) = decode(
        &[(0, 1000, 3, 0), (0, 2000, 4, 0), (0, 3000, 5, 0)],
        "x_5k_hd.imd",
    );
    assert_eq!((chart.notes.len(), report.notes_outside_lanes), (2, 1));
}

#[test]
fn the_title_the_difficulty_and_the_audio_come_from_the_name() {
    let (chart, _) = decode(&[(0, 1000, 0, 0)], "keluodiya_4k_hd.imd");
    assert_eq!(chart.metadata.title, "keluodiya");
    assert_eq!(chart.metadata.difficulty_name, "Hard");
    assert_eq!(chart.metadata.difficulty_value, None);
    assert!(chart.metadata.audio_file.is_empty());
    assert!(chart.metadata.artist.is_empty() && chart.metadata.creator.is_empty());
    let (chart, _) = decode(&[(0, 1000, 0, 0)], "plain.imd");
    assert_eq!(chart.metadata.title, "plain");
    assert_eq!(chart.metadata.difficulty_name, "");
    for (name, difficulty) in [("a_4k_ez.imd", "Easy"), ("a_4k_nm.imd", "Normal")] {
        assert_eq!(
            decode(&[(0, 1000, 0, 0)], name).0.metadata.difficulty_name,
            difficulty
        );
    }
    let chart =
        ImdDecoder::decode_with_title(&File::new(&[(0, 1000, 0, 0)]).bytes(), "Given").unwrap();
    assert_eq!(
        (chart.metadata.title.as_str(), chart.key_count),
        ("Given", 4)
    );
    let chart = ImdDecoder::decode(&File::new(&[(0, 1000, 0, 0)]).bytes()).unwrap();
    assert_eq!(chart.metadata.title, "");
}

// ---------------------------------------------------------------------------------------------------- notes

#[test]
fn taps_are_taps_and_times_are_milliseconds_times_a_thousand() {
    let (chart, report) = decode(
        &[(0, 3750, 2, 0), (0, 3906, 3, 0), (0, 3750, 0, 0)],
        "a_4k_hd.imd",
    );
    // Sorted by time, then by lane.
    assert_eq!(
        summary(&chart),
        vec![(3_750_000, 0, 0), (3_750_000, 2, 0), (3_906_000, 3, 0)]
    );
    assert_eq!(
        report,
        ImdReport {
            bpm_points: 1,
            bpm_entries: 1,
            declared_duration_ms: 90_000,
            key_count: 4,
            key_count_from_name: true,
            ..ImdReport::default()
        }
    );
}

#[test]
fn a_hold_lasts_its_action_in_milliseconds() {
    let (chart, report) = decode(&[(0x02, 4062, 1, 625), (0, 5000, 1, 0)], "a_4k_hd.imd");
    assert_eq!(
        summary(&chart),
        vec![(4_062_000, 1, 625_000), (5_000_000, 1, 0)]
    );
    assert_eq!((report.chain_holds, report.empty_holds_as_taps), (0, 0));
}

#[test]
fn a_hold_of_no_length_is_a_tap() {
    let (chart, report) = decode(
        &[(0x02, 1000, 0, 0), (0x02, 2000, 1, -5), (0x62, 3000, 2, 0)],
        "a_4k_hd.imd",
    );
    assert_eq!(
        summary(&chart),
        vec![(1_000_000, 0, 0), (2_000_000, 1, 0), (3_000_000, 2, 0)]
    );
    assert_eq!((report.empty_holds_as_taps, report.chain_holds), (3, 1));
}

#[test]
fn a_single_slide_is_a_tap_on_its_own_lane() {
    let (chart, report) = decode(&[(0x01, 1000, 1, 1), (0x01, 2000, 2, -1)], "a_4k_hd.imd");
    assert_eq!(summary(&chart), vec![(1_000_000, 1, 0), (2_000_000, 2, 0)]);
    assert_eq!(
        (report.single_slides_as_taps, report.chain_joints_dropped),
        (2, 0)
    );
}

/// A chain of three segments joined by slides, ending with a slide: hold on lane 3, slide to 2, hold, slide to 1, hold, end slide to 0.
#[test]
fn a_chain_is_one_hold_per_segment_and_its_slides_are_dropped() {
    let notes = [
        (0x62, 1000, 3, 100),
        (0x21, 1100, 3, -1),
        (0x22, 1100, 2, 100),
        (0x21, 1200, 2, -1),
        (0xA2, 1200, 1, 100),
        (0xA1, 1300, 1, -1),
    ];
    let (chart, report) = decode(&notes, "a_4k_hd.imd");
    assert_eq!(
        summary(&chart),
        vec![
            (1_000_000, 3, 100_000),
            (1_100_000, 2, 100_000),
            (1_200_000, 1, 100_000)
        ]
    );
    assert_eq!(
        (
            report.chain_holds,
            report.chain_joints_dropped,
            report.lone_chain_slides_as_taps
        ),
        (3, 3, 0)
    );
}

#[test]
fn a_chain_that_starts_with_a_slide_has_its_press_on_the_segment() {
    // Slide start on lane 3 toward lane 2 at 500 ms, the segment on lane 2 starts there: ONE note, the hold.
    let (chart, report) = decode(&[(0x61, 500, 3, -1), (0xA2, 500, 2, 200)], "a_4k_hd.imd");
    assert_eq!(summary(&chart), vec![(500_000, 2, 200_000)]);
    assert_eq!(report.chain_joints_dropped, 1);
}

#[test]
fn a_slide_ends_a_hold_within_two_milliseconds() {
    // The hold ends at 1100; the end slide comes at 1099, 1102, 1103 (off by one, two, three) and 1100.
    for (slide_time, joint) in [
        (1100, true),
        (1099, true),
        (1101, true),
        (1102, true),
        (1098, true),
        (1103, false),
        (1104, false),
    ] {
        let notes = [(0x62, 1000, 2, 100), (0xA1, slide_time, 2, 1)];
        let (chart, report) = decode(&notes, "a_4k_hd.imd");
        assert_eq!(
            (
                report.chain_joints_dropped,
                report.lone_chain_slides_as_taps,
                chart.notes.len()
            ),
            if joint { (1, 0, 1) } else { (0, 1, 2) },
            "slide at {slide_time}"
        );
    }
}

#[test]
fn a_slide_with_no_segment_beside_it_stays_a_tap() {
    // A joint on lane 1 with a segment on another lane or another time: not part of any chain this reader can see.
    let notes = [
        (0x21, 1000, 1, 1),
        (0x22, 1000, 3, 100),
        (0x61, 5000, 0, 1),
        (0xA1, 9000, 2, -1),
    ];
    let (chart, report) = decode(&notes, "a_4k_hd.imd");
    assert_eq!(
        summary(&chart),
        vec![
            (1_000_000, 3, 100_000),
            (1_000_000, 1, 0),
            (5_000_000, 0, 0),
            (9_000_000, 2, 0)
        ]
    );
    assert_eq!(
        (
            report.lone_chain_slides_as_taps,
            report.chain_joints_dropped
        ),
        (3, 0)
    );
}

#[test]
fn a_slide_to_a_lane_that_does_not_exist_is_not_a_negative_lookup() {
    // Delta -1 from lane 0 and a huge delta: no segment there, so the slide stays a tap; nothing overflows.
    let notes = [
        (0x61, 1000, 0, -1),
        (0x21, 2000, 0, i32::MAX),
        (0xA1, 3000, 0, i32::MIN),
    ];
    let (chart, report) = decode(&notes, "a_4k_hd.imd");
    assert_eq!(
        (chart.notes.len(), report.lone_chain_slides_as_taps),
        (3, 3)
    );
}

#[test]
fn two_chains_at_once_do_not_get_mixed_up() {
    // Lane 0 -> 1 and lane 3 -> 2 joint at the same time: each slide matches its own segments.
    let notes = [
        (0x62, 1000, 0, 100),
        (0x62, 1000, 3, 100),
        (0x21, 1100, 0, 1),
        (0x21, 1100, 3, -1),
        (0xA2, 1100, 1, 100),
        (0xA2, 1100, 2, 100),
    ];
    let (chart, report) = decode(&notes, "a_4k_hd.imd");
    assert_eq!(chart.notes.len(), 4);
    assert_eq!(report.chain_joints_dropped, 2);
}

#[test]
fn an_unknown_type_is_dropped_and_counted_and_a_file_of_nothing_else_is_refused() {
    let (chart, report) = decode(
        &[(0x3412, 500, 0, 0), (0x03, 600, 1, 0), (0, 1000, 1, 0)],
        "a_4k_hd.imd",
    );
    assert_eq!((chart.notes.len(), report.unknown_types), (1, 2));
    let error =
        ImdDecoder::decode_with_report(&File::new(&[(0x3412, 500, 0, 0)]).bytes(), "").unwrap_err();
    assert!(error.to_string().contains("no playable note"), "{error}");
    let error = ImdDecoder::decode(&File::new(&[]).bytes()).unwrap_err();
    assert!(error.to_string().contains("no playable note"), "{error}");
}

#[test]
fn notes_are_sorted_whatever_the_order_of_the_file() {
    // Real files keep the records in the order of the editor's lanes, not of time.
    let (chart, _) = decode(
        &[
            (0, 3000, 1, 0),
            (0x02, 1000, 2, 500),
            (0, 2000, 0, 0),
            (0, 1000, 0, 0),
        ],
        "a_4k_hd.imd",
    );
    assert_eq!(
        summary(&chart),
        vec![
            (1_000_000, 2, 500_000),
            (1_000_000, 0, 0),
            (2_000_000, 0, 0),
            (3_000_000, 1, 0)
        ]
    );
}

#[test]
fn a_note_inside_a_hold_of_its_lane_is_dropped_and_one_at_its_end_is_kept() {
    let notes = [
        (0x02, 1000, 1, 500),
        (0, 1200, 1, 0),
        (0, 1200, 2, 0),
        (0, 1500, 1, 0),
        (0, 1501, 1, 0),
    ];
    let (chart, report) = decode(&notes, "a_4k_hd.imd");
    assert_eq!(
        summary(&chart),
        vec![
            (1_000_000, 1, 500_000),
            (1_200_000, 2, 0),
            (1_500_000, 1, 0),
            (1_501_000, 1, 0)
        ]
    );
    assert_eq!(report.overlapping_notes_dropped, 1);
}

#[test]
fn a_hold_and_a_tap_at_the_same_time_keep_the_hold_and_two_taps_keep_one() {
    let notes = [
        (0, 1000, 0, 0),
        (0x02, 1000, 0, 300),
        (0, 5000, 2, 0),
        (0, 5000, 2, 0),
        (0x01, 5000, 2, 1),
    ];
    let (chart, report) = decode(&notes, "a_4k_hd.imd");
    assert_eq!(
        summary(&chart),
        vec![(1_000_000, 0, 300_000), (5_000_000, 2, 0)]
    );
    assert_eq!(
        (report.overlapping_notes_dropped, report.duplicate_taps),
        (1, 2)
    );
}

#[test]
fn the_largest_values_do_not_overflow() {
    let notes = [(0, u32::MAX, 0, 0), (0x02, u32::MAX - 10, 1, i32::MAX)];
    let (chart, _) = decode(&notes, "a_4k_hd.imd");
    assert_eq!(chart.notes.len(), 2);
    assert_eq!(chart.notes[0].time_us, (i64::from(u32::MAX) - 10) * 1000);
    assert_eq!(chart.notes[1].time_us, i64::from(u32::MAX) * 1000);
}

// ---------------------------------------------------------------------------------------------------- tempo

#[test]
fn a_table_of_one_entry_per_beat_is_one_point() {
    // 312.5 ms per beat at 192 BPM, as the official files write it (rounded to whole ms).
    let entries: Vec<(u32, f64)> = (0..417u32).map(|beat| (beat * 625 / 2, 192.0)).collect();
    let bytes = File::new(&[(0, 3750, 2, 0)]).bpms(&entries).bytes();
    let (chart, report) = ImdDecoder::decode_with_report(&bytes, "a_4k_hd.imd").unwrap();
    assert_eq!(bpm_points(&chart), vec![(0, 192.0)]);
    assert_eq!((report.bpm_entries, report.bpm_points), (417, 1));
    assert!(matches!(
        chart.timing_points[0],
        TimingPoint::Bpm { signature: 4, .. }
    ));
}

#[test]
fn a_change_of_tempo_is_a_point_where_the_bpm_changes() {
    let bpms = [
        (0, 120.0),
        (500, 120.0),
        (1000, 150.0),
        (1500, 150.0),
        (2000, 120.0),
    ];
    let bytes = File::new(&[(0, 100, 0, 0)]).bpms(&bpms).bytes();
    let (chart, report) = ImdDecoder::decode_with_report(&bytes, "").unwrap();
    assert_eq!(
        bpm_points(&chart),
        vec![(0, 120.0), (1_000_000, 150.0), (2_000_000, 120.0)]
    );
    assert_eq!(
        (
            report.bpm_entries,
            report.bpm_points,
            report.first_bpm_moved
        ),
        (5, 3, false)
    );
}

#[test]
fn the_tempo_table_is_sorted_and_the_last_of_two_at_one_time_wins() {
    let bpms = [(2000, 90.0), (0, 100.0), (2000, 80.0), (1000, 100.0)];
    let bytes = File::new(&[(0, 100, 0, 0)]).bpms(&bpms).bytes();
    let chart = ImdDecoder::decode(&bytes).unwrap();
    assert_eq!(bpm_points(&chart), vec![(0, 100.0), (2_000_000, 80.0)]);
}

#[test]
fn a_bpm_that_is_not_a_usable_number_is_ignored_and_counted() {
    let bpms = [
        (0, 0.0),
        (100, f64::NAN),
        (200, -3.0),
        (300, f64::INFINITY),
        (400, 1e9),
        (500, 140.0),
    ];
    let bytes = File::new(&[(0, 1000, 0, 0)]).bpms(&bpms).bytes();
    let (chart, report) = ImdDecoder::decode_with_report(&bytes, "").unwrap();
    // The first usable entry is the only one: its point is at its own time.
    assert_eq!(bpm_points(&chart), vec![(500_000, 140.0)]);
    assert_eq!(report.invalid_bpm_entries, 5);
    let none = File::new(&[(0, 1000, 0, 0)])
        .bpms(&[(0, 0.0), (10, f64::NAN)])
        .bytes();
    let error = ImdDecoder::decode(&none).unwrap_err();
    assert!(error.to_string().contains("no usable BPM"), "{error}");
}

#[test]
fn the_first_bpm_is_never_after_the_first_note() {
    let bytes = File::new(&[(0, 1000, 0, 0), (0, 9000, 1, 0)])
        .bpms(&[(4000, 100.0), (8000, 200.0)])
        .bytes();
    let (chart, report) = ImdDecoder::decode_with_report(&bytes, "").unwrap();
    assert_eq!(
        bpm_points(&chart),
        vec![(1_000_000, 100.0), (8_000_000, 200.0)]
    );
    assert!(report.first_bpm_moved);
    // A first entry before the first note stays where it is: 35 ms, as a real file starts its table.
    let bytes = File::new(&[(0, 2035, 0, 0)]).bpms(&[(35, 100.0)]).bytes();
    let (chart, report) = ImdDecoder::decode_with_report(&bytes, "").unwrap();
    assert_eq!(
        (bpm_points(&chart), report.first_bpm_moved),
        (vec![(35_000, 100.0)], false)
    );
}

// ---------------------------------------------------------------------------------------------------- hostile input

#[test]
fn a_file_that_is_not_exactly_an_imd_is_an_error() {
    let good = File::new(&[(0, 1000, 0, 0), (0, 2000, 1, 0)]).bytes();
    assert!(ImdDecoder::decode(&good).is_ok());
    // A byte too many, a byte too few, every shorter prefix.
    let mut longer = good.clone();
    longer.push(0);
    assert!(ImdDecoder::decode(&longer).is_err());
    for cut in 0..good.len() {
        assert!(
            ImdDecoder::decode(&good[..cut]).is_err(),
            "prefix of {cut} bytes"
        );
    }
    // A separator that is not 0x0303 (RM2Malody calls it a flag; RMEssentials refuses it).
    let mut file = File::new(&[(0, 1000, 0, 0)]);
    file.separator = 0x0000;
    let error = ImdDecoder::decode(&file.bytes()).unwrap_err();
    assert!(error.to_string().contains("separator"), "{error}");
    // No BPM entry.
    let none = File::new(&[(0, 1000, 0, 0)]).bpms(&[]).bytes();
    let error = ImdDecoder::decode(&none).unwrap_err();
    assert!(error.to_string().contains("no BPM entry"), "{error}");
}

#[test]
fn huge_counts_are_refused_before_anything_is_allocated() {
    let good = File::new(&[(0, 1000, 0, 0)]).bytes();
    // A BPM count of 4 billion, a note count of 4 billion, and both at once.
    let mut many_bpm = good.clone();
    many_bpm[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut many_notes = good.clone();
    let notes_at = 8 + 12 + 2;
    many_notes[notes_at..notes_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut both = many_bpm.clone();
    both[notes_at..notes_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    for data in [many_bpm, many_notes, both] {
        assert!(ImdDecoder::decode(&data).is_err());
        assert!(ImdDecoder::decode_with_report(&data, "a_4k_hd.imd").is_err());
        assert!(!looks_like_imd(&data, data.len() as u64));
    }
}

#[test]
fn bad_offsets_and_random_bytes_never_panic() {
    // Deterministic pseudo-random files of every small length, and valid files with one byte changed.
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for len in 0..300usize {
        let data: Vec<u8> = (0..len).map(|_| next().to_le_bytes()[0]).collect();
        let _ = ImdDecoder::decode(&data);
        let _ = ImdDecoder::decode_with_report(&data, "a_5k_hd.imd");
        let _ = looks_like_imd(&data, len as u64);
    }
    let good = File::new(&[
        (0x02, 1000, 0, 300),
        (0x61, 2000, 1, 1),
        (0xA2, 2000, 2, 100),
        (0xA1, 2100, 2, 1),
    ])
    .bytes();
    for at in 0..good.len() {
        for value in [0x00, 0x03, 0x7F, 0x80, 0xFF] {
            let mut data = good.clone();
            data[at] = value;
            let _ = ImdDecoder::decode_with_report(&data, "a_4k_hd.imd");
        }
    }
}

#[test]
fn a_file_above_the_size_limit_is_refused() {
    let mut data = File::new(&[(0, 1000, 0, 0)]).bytes();
    data.resize(super::decoder::MAX_FILE_SIZE + 1, 0);
    let error = ImdDecoder::decode(&data).unwrap_err();
    assert!(error.to_string().contains("too large"), "{error}");
}

// ---------------------------------------------------------------------------------------------------- telling an imd

#[test]
fn the_first_bytes_tell_an_imd_from_another_file() {
    let file = File::new(&[(0, 1000, 0, 0), (0x02, 2000, 1, 500)]).bytes();
    let length = file.len() as u64;
    assert!(looks_like_imd(&file, length));
    // The head alone is enough for a table that fits in it, and the size must match the note count.
    assert!(!looks_like_imd(&file, length + 1));
    assert!(!looks_like_imd(&file, length - 11));
    assert!(!looks_like_imd(&file[..4], length));
    assert!(!looks_like_imd(&[], 0));
    // A bad separator, no entry, a table that does not fit in the file.
    let mut bad = file.clone();
    bad[8 + 12] = 0x04;
    assert!(!looks_like_imd(&bad, length));
    let none = File::new(&[(0, 1000, 0, 0)]).bpms(&[]).bytes();
    assert!(!looks_like_imd(&none, none.len() as u64));
    let mut wide = file.clone();
    wide[4..8].copy_from_slice(&100u32.to_le_bytes());
    assert!(!looks_like_imd(&wide, length));
    // A table longer than the head: the head can only say the table fits (the decoder checks the rest).
    assert!(looks_like_imd(&file[..20], length));
    // Other files that carry the extension.
    for other in [
        &b"[Desktop Entry]\nName=x\n"[..],
        b"MThd\0\0\0\x06\0\x01\0\x01\x01\xe0",
        &[0xFF; 64],
    ] {
        assert!(!looks_like_imd(other, other.len() as u64));
    }
}

#[test]
fn a_big_table_whose_note_count_is_past_the_head_is_accepted_as_far_as_it_goes() {
    // 6 000 entries: 72 008 bytes of table, more than the 64 KiB the scan reads.
    let entries: Vec<(u32, f64)> = (0..6000u32).map(|beat| (beat * 312, 192.0)).collect();
    let file = File::new(&[(0, 1_900_000, 0, 0)]).bpms(&entries).bytes();
    let head = &file[..crate::formats::SNIFF_BYTES];
    assert!(looks_like_imd(head, file.len() as u64));
    assert!(!looks_like_imd(head, 70_000));
    assert_eq!(ImdDecoder::decode(&file).unwrap().notes.len(), 1);
}

// ---------------------------------------------------------------------------------------------------- dispatch

#[test]
fn auto_decode_routes_an_imd_by_its_path() {
    use crate::formats::auto::detect::{DetectedFormat, detect};
    use crate::formats::auto::{
        auto_decode, auto_decode_all, auto_decode_bytes, auto_decode_variant,
    };
    let dir = std::env::temp_dir().join(format!("rox_imd_auto_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let chart_path = dir.join("song_5k_nm.imd");
    let plain_path = dir.join("plain.imd");
    let other_path = dir.join("other.imd");
    let data = File::new(&[(0, 1000, 4, 0), (0x02, 2000, 0, 300)]).bytes();
    std::fs::write(&chart_path, &data).unwrap();
    std::fs::write(&plain_path, &data).unwrap();
    std::fs::write(&other_path, b"not an imd at all").unwrap();

    let chart = auto_decode(&chart_path).unwrap();
    let plain = auto_decode(&plain_path).unwrap();
    let zero = auto_decode_variant(&chart_path, 0).unwrap();
    let one = auto_decode_variant(&chart_path, 1);
    let all = auto_decode_all(&chart_path).unwrap();
    let hinted = auto_decode_bytes("x/song_6k_hd.imd", &data).unwrap();
    let unhinted = auto_decode_bytes("song.imd", &data).unwrap();
    let other = auto_decode(&other_path);
    let (sniffed, not_sniffed, missing) = (
        is_imd_chart_file(&chart_path),
        is_imd_chart_file(&other_path),
        is_imd_chart_file(&dir.join("gone.imd")),
    );
    let written = crate::formats::auto::auto_encode(&chart, dir.join("out.imd"));
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(
        detect(std::path::Path::new("a.imd")).unwrap(),
        DetectedFormat::Imd
    );
    assert_eq!(
        (
            chart.key_count,
            chart.metadata.title.as_str(),
            chart.metadata.difficulty_name.as_str()
        ),
        (5, "song", "Normal")
    );
    assert_eq!(
        (plain.key_count, plain.metadata.title.as_str()),
        (5, "plain")
    );
    assert_eq!(zero.notes, chart.notes);
    assert!(one.is_err());
    assert!(all.is_none(), "one chart per file");
    assert_eq!(
        (hinted.key_count, hinted.metadata.difficulty_name.as_str()),
        (6, "Hard")
    );
    assert_eq!(unhinted.key_count, 5);
    assert!(other.is_err());
    assert!(sniffed && !not_sniffed && !missing);
    assert!(written.is_err(), "decode only");
}

#[test]
fn an_imd_name_in_the_wrong_extension_is_not_one() {
    assert!(!is_imd_chart_file(std::path::Path::new("song_4k_hd.mp3")));
    assert!(!is_imd_chart_file(std::path::Path::new("")));
}
