//! Tests of the Clone Hero decoders. Every file here is built in memory (charts as the format notes lay them out,
//! `.sng` packages with the documented mask); no real file is used.
#![allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // sizes and bytes of small synthetic files
#![allow(clippy::format_push_string)]

use rox::audio_plan::SoundEvent;
use rox::codec::Decoder;
use rox::model::{Note, TimingPoint};

use super::{ChartContext, ChartDecoder, Difficulty, Instrument, ShiftSource, SngPackage, SongIni};

/// `[Song]` with `extra` lines, an empty-ish `[SyncTrack]` made of `sync` lines, then the `tracks` (name, lines).
fn chart_text(song: &[&str], sync: &[&str], tracks: &[(&str, &[&str])]) -> String {
    let mut out = String::from("[Song]\n{\n");
    for line in song {
        out.push_str(&format!("  {line}\n"));
    }
    out.push_str("}\n[SyncTrack]\n{\n");
    for line in sync {
        out.push_str(&format!("  {line}\n"));
    }
    out.push_str("}\n[Events]\n{\n  0 = E \"section Intro\"\n}\n");
    for (name, lines) in tracks {
        out.push_str(&format!("[{name}]\n{{\n"));
        for line in *lines {
            out.push_str(&format!("  {line}\n"));
        }
        out.push_str("}\n");
    }
    out
}

const BASIC_SONG: [&str; 2] = ["Name = \"Example Song\"", "Resolution = 192"];
const BASIC_SYNC: [&str; 2] = ["0 = TS 4", "0 = B 120000"];

fn one(lines: &[&str]) -> String {
    chart_text(&BASIC_SONG, &BASIC_SYNC, &[("ExpertSingle", lines)])
}

fn decode(text: &str) -> rox::model::RoxChart {
    ChartDecoder::decode(text.as_bytes()).expect("decodes")
}

fn times(chart: &rox::model::RoxChart) -> Vec<i64> {
    chart.notes.iter().map(|n| n.time_us).collect()
}

fn columns(chart: &rox::model::RoxChart) -> Vec<u8> {
    chart.notes.iter().map(|n| n.column).collect()
}

fn bpms(chart: &rox::model::RoxChart) -> Vec<(i64, f32, u8)> {
    chart
        .timing_points
        .iter()
        .filter_map(|tp| match tp {
            TimingPoint::Bpm {
                time_us,
                bpm,
                signature,
            } => Some((*time_us, *bpm, *signature)),
            TimingPoint::Sv { .. } => None,
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------------- notes

#[test]
fn frets_are_the_five_columns() {
    let chart = decode(&one(&[
        "0 = N 0 0",
        "192 = N 1 0",
        "384 = N 2 0",
        "576 = N 3 0",
        "768 = N 4 0",
    ]));
    assert_eq!(chart.key_count, 5);
    assert_eq!(columns(&chart), [0, 1, 2, 3, 4]);
    assert_eq!(times(&chart), [0, 500_000, 1_000_000, 1_500_000, 2_000_000]);
    assert!(chart.notes.iter().all(|n| !n.is_hold()));
    assert_eq!(chart.hitsounds.len(), 0);
    assert_eq!(chart.metadata.audio_file, "");
    assert_eq!(chart.metadata.title, "Example Song");
    assert_eq!(chart.metadata.difficulty_name, "Guitar Expert");
}

#[test]
fn chords_are_simultaneous_notes() {
    let chart = decode(&one(&["192 = N 0 0", "192 = N 2 0", "192 = N 4 0"]));
    assert_eq!(times(&chart), [500_000; 3]);
    assert_eq!(columns(&chart), [0, 2, 4]);
}

#[test]
fn forced_tap_and_open_are_counted_not_kept() {
    let text = one(&[
        "0 = N 0 0",
        "0 = N 5 0",
        "192 = N 1 0",
        "192 = N 6 0",
        "384 = N 7 0",
        "576 = N 2 0",
        "576 = N 5 0",
        "576 = N 6 0",
        "768 = N 8 0",
    ]);
    let (chart, report) = ChartDecoder::decode_with_report(text.as_bytes(), None, 0).unwrap();
    assert_eq!(columns(&chart), [0, 1, 2]);
    assert_eq!(times(&chart), [0, 500_000, 1_500_000]);
    assert_eq!(report.forced_flags, 2);
    assert_eq!(report.tap_flags, 2);
    assert_eq!(report.open_notes_dropped, 1);
    assert_eq!(report.unknown_note_types, 1);
}

#[test]
fn a_section_with_only_open_notes_is_not_a_variant() {
    let text = chart_text(
        &BASIC_SONG,
        &BASIC_SYNC,
        &[
            ("ExpertSingle", &["0 = N 7 0", "192 = N 7 0"]),
            ("HardSingle", &["0 = N 1 0"]),
        ],
    );
    let variants = ChartDecoder::variants(text.as_bytes()).unwrap();
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].section, "HardSingle");
}

#[test]
fn the_first_note_of_a_tick_and_fret_stays() {
    let text = one(&["0 = N 0 0", "0 = N 0 96", "192 = N 1 96", "192 = N 1 0"]);
    let (chart, report) = ChartDecoder::decode_with_report(text.as_bytes(), None, 0).unwrap();
    assert_eq!(chart.notes.len(), 2);
    assert!(!chart.notes[0].is_hold()); // length 0 was first
    assert!(chart.notes[1].is_hold()); // length 96 was first
    assert_eq!(report.duplicate_notes_dropped, 2);
}

// ----------------------------------------------------------------------------------------------------- sustains

#[test]
fn a_sustain_is_a_hold_whatever_its_length() {
    // `.chart` has no sustain cutoff by default: 1 tick is a hold.
    let chart = decode(&one(&["0 = N 0 96", "192 = N 1 1", "384 = N 2 0"]));
    assert_eq!(
        chart.notes,
        [
            Note::hold(0, 250_000, 0),
            Note::hold(500_000, 2604, 1), // 1 tick = 2604.17 µs, rounded
            Note::tap(1_000_000, 2)
        ]
    );
}

#[test]
fn a_sustain_stops_at_the_next_note_of_its_fret() {
    let text = one(&[
        "0 = N 0 1000",
        "0 = N 1 1000",
        "192 = N 0 0",
        "384 = N 0 96",
    ]);
    let (chart, report) = ChartDecoder::decode_with_report(text.as_bytes(), None, 0).unwrap();
    let hold = &chart.notes[0];
    assert_eq!(
        (hold.time_us, hold.duration_us(), hold.column),
        (0, 500_000, 0)
    );
    // The other fret has no next note: its sustain is whole (1000 ticks).
    let long = chart.notes.iter().find(|n| n.column == 1).unwrap();
    assert_eq!(long.duration_us(), 2_604_167);
    assert_eq!(report.sustains_clamped, 1);
    chart.validate().unwrap();
}

#[test]
fn the_ini_cutoff_turns_short_sustains_into_taps() {
    let text = one(&["0 = N 0 10", "192 = N 1 64", "384 = N 2 65"]);
    let ini = b"[song]\nsustain_cutoff_threshold = 65\n";
    let (chart, report) = ChartDecoder::decode_with_report(text.as_bytes(), Some(ini), 0).unwrap();
    assert_eq!(
        chart.notes.iter().map(Note::is_hold).collect::<Vec<_>>(),
        [false, false, true]
    );
    assert_eq!(report.sustains_cut_to_taps, 2);
}

// ------------------------------------------------------------------------------------------------------ timing

#[test]
fn tempo_changes_and_signatures() {
    let text = chart_text(
        &BASIC_SONG,
        &[
            "0 = TS 4",
            "0 = B 120000",
            "768 = TS 3",
            "768 = B 240000",
            "1536 = B 60000",
        ],
        &[(
            "ExpertSingle",
            &[
                "0 = N 0 0",
                "768 = N 1 0",
                "960 = N 2 0",
                "1536 = N 3 0",
                "1728 = N 4 0",
            ],
        )],
    );
    let chart = decode(&text);
    // 768 ticks at 120 BPM = 2 s; 768 more at 240 BPM = 1 s; then 60 BPM: 192 ticks = 1 s.
    assert_eq!(
        times(&chart),
        [0, 2_000_000, 2_250_000, 3_000_000, 4_000_000]
    );
    assert_eq!(
        bpms(&chart),
        [(0, 120.0, 4), (2_000_000, 240.0, 3), (3_000_000, 60.0, 3)]
    );
}

#[test]
fn resolution_and_thousandths_of_a_bpm() {
    let text = chart_text(
        &["Resolution = 480"],
        &["0 = B 150325"],
        &[("ExpertSingle", &["480 = N 0 0", "960 = N 1 0"])],
    );
    let chart = decode(&text);
    // 60 / 150.325 s per beat = 399135.2 µs; two beats 798270.4 µs.
    assert_eq!(times(&chart), [399_135, 798_270]);
    assert_eq!(bpms(&chart), [(0, 150.325, 4)]);
}

#[test]
fn the_tempo_is_120_until_the_first_b_and_the_sync_track_is_optional() {
    let text = chart_text(
        &BASIC_SONG,
        &["192 = B 60000"],
        &[("ExpertSingle", &["0 = N 0 0", "192 = N 1 0", "384 = N 2 0"])],
    );
    let chart = decode(&text);
    assert_eq!(times(&chart), [0, 500_000, 1_500_000]);
    assert_eq!(bpms(&chart), [(0, 120.0, 4), (500_000, 60.0, 4)]);

    let no_sync = "[Song]\n{\nResolution = 192\n}\n[ExpertSingle]\n{\n192 = N 0 0\n}\n";
    let (chart, report) = ChartDecoder::decode_with_report(no_sync.as_bytes(), None, 0).unwrap();
    assert_eq!(times(&chart), [500_000]);
    assert!(report.default_tempo_used);
}

#[test]
fn signatures_are_counted_in_quarter_notes() {
    let text = chart_text(
        &BASIC_SONG,
        &[
            "0 = TS 7 4",
            "192 = B 100000",
            "192 = TS 3 3",
            "384 = B 110000",
            "384 = TS 6",
        ],
        &[("ExpertSingle", &["0 = N 0 0"])],
    );
    let chart = decode(&text);
    // 7/16 = 1.75 quarters -> 2 ; 3/8 = 1.5 quarters -> 2 ; 6/4 = 6.
    assert_eq!(
        bpms(&chart).iter().map(|b| b.2).collect::<Vec<_>>(),
        [2, 2, 6]
    );
}

#[test]
fn bad_sync_events_are_counted_and_the_last_event_of_a_tick_wins() {
    let text = chart_text(
        &BASIC_SONG,
        &[
            "0 = B 0",
            "0 = B 100000",
            "0 = B 90000",
            "0 = TS 0",
            "0 = A 123",
            "nonsense",
            "96 = B -5",
        ],
        &[("ExpertSingle", &["192 = N 0 0"])],
    );
    let (chart, report) = ChartDecoder::decode_with_report(text.as_bytes(), None, 0).unwrap();
    assert_eq!(bpms(&chart), [(0, 90.0, 4)]);
    assert_eq!(report.ignored_sync_events, 3); // B 0, TS 0, B -5
    assert_eq!(report.replaced_sync_events, 1);
    assert_eq!(report.malformed_sync_lines, 1);
}

#[test]
fn events_are_sorted_by_tick() {
    let chart = decode(&one(&["384 = N 2 0", "0 = N 0 0", "192 = N 1 0"]));
    assert_eq!(columns(&chart), [0, 1, 2]);
    assert_eq!(times(&chart), [0, 500_000, 1_000_000]);
}

// --------------------------------------------------------------------------------------- offset and delay

#[test]
fn offset_moves_the_chart_later_in_the_audio() {
    // Offset = 0.56: the audio starts 0.56 s sooner than the chart, so the chart is 0.56 s into the audio.
    let text = chart_text(
        &["Resolution = 192", "Offset = 0.56"],
        &BASIC_SYNC,
        &[("ExpertSingle", &["0 = N 0 0", "192 = N 1 0"])],
    );
    let (chart, report) = ChartDecoder::decode_with_report(text.as_bytes(), None, 0).unwrap();
    assert_eq!(times(&chart), [560_000, 1_060_000]);
    assert_eq!(bpms(&chart), [(560_000, 120.0, 4)]);
    assert_eq!(report.shift_us, 560_000);
    assert_eq!(report.shift_source, ShiftSource::ChartOffset);
    assert_eq!(chart.metadata.audio_offset_us, 0);

    let negative = text.replace("0.56", "-0.25");
    let chart = decode(&negative);
    assert_eq!(times(&chart), [-250_000, 250_000]);
}

#[test]
fn ini_delay_has_the_same_sign_and_wins_over_offset() {
    let text = chart_text(
        &["Resolution = 192", "Offset = 0.56"],
        &BASIC_SYNC,
        &[("ExpertSingle", &["0 = N 0 0"])],
    );
    let (chart, report) =
        ChartDecoder::decode_with_report(text.as_bytes(), Some(b"[song]\ndelay = 1500\n"), 0)
            .unwrap();
    assert_eq!(times(&chart), [1_500_000]);
    assert_eq!(report.shift_source, ShiftSource::IniDelay);
    // A negative delay.
    let chart = ChartDecoder::decode_with_ini(text.as_bytes(), Some(b"delay=-200"), 0).unwrap();
    assert_eq!(times(&chart), [-200_000]);
    // `delay = 0` does not count: Offset applies.
    let chart = ChartDecoder::decode_with_ini(text.as_bytes(), Some(b"delay = 0"), 0).unwrap();
    assert_eq!(times(&chart), [560_000]);
}

#[test]
fn a_shift_beyond_range_is_not_applied() {
    let text = chart_text(
        &["Resolution = 192", "Offset = 99999999999"],
        &BASIC_SYNC,
        &[("ExpertSingle", &["0 = N 0 0"])],
    );
    let (chart, report) = ChartDecoder::decode_with_report(text.as_bytes(), None, 0).unwrap();
    assert_eq!(times(&chart), [0]);
    assert!(report.shift_out_of_range);
}

// ------------------------------------------------------------------------------------------------ variants

#[test]
fn variants_are_ordered_by_instrument_then_difficulty() {
    let notes: &[&str] = &["0 = N 0 0"];
    let text = chart_text(
        &BASIC_SONG,
        &BASIC_SYNC,
        &[
            ("EasyKeyboard", notes),
            ("ExpertDrums", notes),
            ("EasySingle", notes),
            ("HardDoubleBass", notes),
            ("ExpertGHLGuitar", notes),
            ("ExpertSingle", notes),
            ("ExpertSingleBass", notes),
            ("MediumSingle", notes),
            ("ExpertDoubleRhythm", notes),
            ("HardSingle", notes),
            ("ExpertDoubleGuitar", notes),
            ("ExpertDoubleBass", notes),
            ("MediumSingle", &["0 = N 1 0"]), // a second section of the same name is merged
            ("ExpertKeyboard", notes),
            ("ExpertDrums", notes),
        ],
    );
    let variants = ChartDecoder::variants(text.as_bytes()).unwrap();
    let got: Vec<(&str, Instrument, Difficulty)> = variants
        .iter()
        .map(|v| (v.section.as_str(), v.instrument, v.difficulty))
        .collect();
    let names: Vec<&str> = got.iter().map(|g| g.0).collect();
    assert_eq!(
        names,
        [
            "ExpertSingle",
            "HardSingle",
            "MediumSingle",
            "EasySingle",
            "ExpertDoubleGuitar",
            "ExpertDoubleBass",
            "HardDoubleBass",
            "ExpertDoubleRhythm",
            "ExpertKeyboard",
            "EasyKeyboard",
        ]
    );
    assert_eq!(
        variants.iter().map(|v| v.index).collect::<Vec<_>>(),
        (0..10).collect::<Vec<_>>()
    );
    assert!(variants.iter().all(|v| v.key_count == 5));
    assert_eq!(variants[2].notes, 2); // MediumSingle merged
    let chart = ChartDecoder::decode_variant(text.as_bytes(), 8).unwrap();
    assert_eq!(chart.metadata.difficulty_name, "Keys Expert");
    let chart = ChartDecoder::decode_variant(text.as_bytes(), 4).unwrap();
    assert_eq!(chart.metadata.difficulty_name, "Guitar Co-op Expert");
    assert!(ChartDecoder::decode_variant(text.as_bytes(), 10).is_err());

    let (_, report) = ChartDecoder::decode_with_report(text.as_bytes(), None, 0).unwrap();
    assert_eq!(
        report.unsupported_tracks,
        ["ExpertDrums", "ExpertGHLGuitar", "ExpertSingleBass"]
    );
}

#[test]
fn a_file_with_no_playable_track_is_an_error() {
    let text = chart_text(&BASIC_SONG, &BASIC_SYNC, &[("ExpertDrums", &["0 = N 0 0"])]);
    assert!(ChartDecoder::decode(text.as_bytes()).is_err());
    assert_eq!(ChartDecoder::variants(text.as_bytes()).unwrap().len(), 0);
}

// ---------------------------------------------------------------------------------------- text and metadata

#[test]
fn encodings_quotes_line_ends_and_sections() {
    // BOM, CRLF, Latin-1 (0xE9), quotes inside a value, sections in another order, an unclosed last body.
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(
        b"[SyncTrack]\r\n{\r\n0 = B 120000\r\n}\r\n[ExpertSingle]\r\n{\r\n192 = N 0 0\r\n}\r\n[Song]\r\n{\r\nName = \"Caf\xE9 \"live\"\"\r\nResolution = 192\r\n}\r\n[HardSingle]\r\n{\r\n0 = N 1 0\r\n",
    );
    let chart = ChartDecoder::decode(&bytes).unwrap();
    assert_eq!(chart.metadata.title, "Caf\u{e9} \"live\"");
    assert_eq!(times(&chart), [500_000]);
    assert_eq!(ChartDecoder::variants(&bytes).unwrap().len(), 2);

    let utf8 = "[Song]\n{\n  Name = \"Caf\u{e9}\"\n}\n[ExpertSingle]\n{\n 0 = N 0 0\n}\n";
    assert_eq!(decode(utf8).metadata.title, "Caf\u{e9}");
    // No Resolution: 192.
    let (chart, report) = ChartDecoder::decode_with_report(utf8.as_bytes(), None, 0).unwrap();
    assert_eq!(chart.notes.len(), 1);
    assert!(report.resolution_defaulted);
    assert_eq!(report.resolution, 192);
}

#[test]
fn the_ini_takes_precedence_over_song() {
    let text = chart_text(
        &[
            "Name = \"Chart Name\"",
            "Artist = \"Chart Artist\"",
            "Charter = \"Chart Charter\"",
            "Genre = \"rock\"",
            "Resolution = 192",
            "PreviewStart = 45.28",
            "PreviewEnd = 75.28",
        ],
        &BASIC_SYNC,
        &[
            ("ExpertSingle", &["0 = N 0 0"]),
            ("HardDoubleBass", &["0 = N 0 0"]),
        ],
    );
    let plain = decode(&text);
    assert_eq!(plain.metadata.title, "Chart Name");
    assert_eq!(plain.metadata.artist, "Chart Artist");
    assert_eq!(plain.metadata.creator, "Chart Charter");
    assert_eq!(plain.metadata.genre.as_deref(), Some("rock"));
    assert_eq!(plain.metadata.preview_time_us, 45_280_000);
    assert_eq!(plain.metadata.preview_duration_us, 30_000_000);
    assert_eq!(plain.metadata.difficulty_value, None);

    let ini = b"[song]\nname = Ini Name\nartist = <color=#00FF00>Ini</color> Artist\ncharter = Ini Charter\nfrets = Ignored\ndiff_guitar = 4\ndiff_bass = -1\npreview_start_time = 1000\n";
    let chart = ChartDecoder::decode_with_ini(text.as_bytes(), Some(ini), 0).unwrap();
    assert_eq!(chart.metadata.title, "Ini Name");
    assert_eq!(chart.metadata.artist, "Ini Artist");
    assert_eq!(chart.metadata.creator, "Ini Charter");
    assert_eq!(chart.metadata.difficulty_value, Some(4.0));
    assert_eq!(chart.metadata.genre.as_deref(), Some("rock"));
    assert_eq!(chart.metadata.preview_time_us, 1_000_000);
    // `diff_bass = -1`: unset.
    let bass = ChartDecoder::decode_with_ini(text.as_bytes(), Some(ini), 1).unwrap();
    assert_eq!(bass.metadata.difficulty_value, None);
}

#[test]
fn song_ini_parsing() {
    let bytes = b"\xEF\xBB\xBF; a comment\r\n[Song]\r\n# another\r\nName=Caf\xE9\r\nARTIST   =   The Band  \r\nCharter = Me\r\nframes = x\r\nFrets = Not Me\r\ndelay = -250\r\nsong_length = 180000\r\npreview_start_time = 45000\r\npreview_end_time = 75000\r\nsustain_cutoff_threshold = 80\r\nbackground = bg.png\r\nyear = , 2002\r\ndiff_guitar = 6\r\nDIFF_BAND = -1\r\ndiff_keys = abc\r\n[other]\r\nname = nope\r\n";
    let ini = SongIni::parse(bytes).unwrap();
    assert_eq!(ini.name.as_deref(), Some("Caf\u{e9}")); // Latin-1
    assert_eq!(ini.artist.as_deref(), Some("The Band"));
    assert_eq!(ini.charter.as_deref(), Some("Me"));
    assert_eq!(ini.delay_ms, Some(-250));
    assert_eq!(ini.song_length_ms, Some(180_000));
    assert_eq!(ini.preview_start_time_ms, Some(45_000));
    assert_eq!(ini.preview_end_time_ms, Some(75_000));
    assert_eq!(ini.sustain_cutoff_threshold, Some(80));
    assert_eq!(ini.background.as_deref(), Some("bg.png"));
    assert_eq!(ini.year.as_deref(), Some(", 2002"));
    assert_eq!(ini.difficulty("diff_guitar"), Some(6));
    assert_eq!(ini.difficulty("diff_band"), None); // -1
    assert_eq!(ini.difficulty("diff_keys"), None); // not a number
    assert_eq!(ini.album, None);

    let utf8 = SongIni::parse("name = Caf\u{e9} \u{1F3B8}\n".as_bytes()).unwrap();
    assert_eq!(utf8.name.as_deref(), Some("Caf\u{e9} \u{1F3B8}"));
    // A `<` that is not a formatting tag stays.
    let lt = SongIni::parse(b"name = I <3 You <b>so</b> much <").unwrap();
    assert_eq!(lt.name.as_deref(), Some("I <3 You so much <"));
    assert_eq!(SongIni::parse(b"").unwrap(), SongIni::default());
    assert_eq!(
        SongIni::parse(&[0xFF, 0x00, 0x80]).unwrap(),
        SongIni::default()
    );
}

// ------------------------------------------------------------------------------------------------ audio plan

fn names(plan: &rox::audio_plan::AudioPlan) -> Vec<&str> {
    plan.events.iter().map(|e| e.file.as_str()).collect()
}

#[test]
fn the_plan_without_a_listing_names_every_stem_as_optional() {
    let text = chart_text(
        &[
            "Resolution = 192",
            "MusicStream = \"Example Song.ogg\"",
            "GuitarStream = \"guitar.ogg\"",
            "BassStream = \"stems/Bass Stem.mp3\"",
            "CrowdStream = None",
            "VocalStream = \"../outside.ogg\"",
            "DrumStream = \"/abs/drums.ogg\"",
        ],
        &BASIC_SYNC,
        &[("ExpertSingle", &["0 = N 0 0"])],
    );
    let (_, report, plan) =
        ChartDecoder::decode_with_audio_plan(text.as_bytes(), &ChartContext::default(), 0).unwrap();
    assert_eq!(
        names(&plan),
        [
            "song.ogg",
            "guitar.ogg",
            "rhythm.ogg",
            "bass.ogg",
            "keys.ogg",
            "drums.ogg",
            "drums_1.ogg",
            "drums_2.ogg",
            "drums_3.ogg",
            "drums_4.ogg",
            "vocals.ogg",
            "vocals_1.ogg",
            "vocals_2.ogg",
            "crowd.ogg",
            "Example Song.ogg",
            "stems/Bass Stem.mp3",
        ]
    );
    assert!(
        plan.events
            .iter()
            .all(|e| e.optional && e.time_us == 0 && e.index.is_none())
    );
    assert_eq!(plan.events[0], SoundEvent::optional_whole(0, "song.ogg"));
    assert_eq!(report.unsafe_names, 2);
}

#[test]
fn the_plan_with_a_listing_holds_the_files_that_exist() {
    let text = chart_text(
        &[
            "Resolution = 192",
            "MusicStream = \"Old Name.ogg\"",
            "GuitarStream = \"Old Guitar.ogg\"",
            "VocalStream = \"voice.ogg\"",
            "RhythmStream = \"rhy.ogg\"",
        ],
        &BASIC_SYNC,
        &[("ExpertSingle", &["0 = N 0 0"])],
    );
    let files: Vec<String> = [
        "notes.chart",
        "song.ini",
        "SONG.OGG", // a reserved name wins over the MusicStream tag; any case
        "song.mp3",
        "Old Guitar.OPUS", // the tag names another extension of an existing file
        "drums.ogg",       // ignored: numbered drums exist
        "drums_1.opus",
        "drums_2.ogg",
        "vocals.ogg", // ignored: numbered vocals exist
        "vocals_1.wav",
        "voice.ogg", // ignored: reserved vocals exist
        "preview.ogg",
        "vocals_explicit.ogg",
        "crowd.mp3",
        "album.png",
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    let ctx = ChartContext {
        ini: None,
        files: Some(&files),
    };
    let (_, _, plan) = ChartDecoder::decode_with_audio_plan(text.as_bytes(), &ctx, 0).unwrap();
    assert_eq!(
        names(&plan),
        [
            "SONG.OGG",
            "Old Guitar.OPUS",
            "crowd.mp3",
            "drums_1.opus",
            "drums_2.ogg",
            "vocals_1.wav",
        ]
    );
    assert!(
        plan.events
            .iter()
            .all(|e| !e.optional && e.time_us == 0 && e.index.is_none())
    );

    // The tags are the fallback when no reserved file exists.
    let files = vec![
        "Old Name.ogg".to_owned(),
        "voice.ogg".to_owned(),
        "rhy.OGG".to_owned(),
    ];
    let ctx = ChartContext {
        ini: None,
        files: Some(&files),
    };
    let (_, _, plan) = ChartDecoder::decode_with_audio_plan(text.as_bytes(), &ctx, 0).unwrap();
    assert_eq!(names(&plan), ["Old Name.ogg", "rhy.OGG", "voice.ogg"]);
    // An empty listing: nothing to play.
    let none: Vec<String> = Vec::new();
    let ctx = ChartContext {
        ini: None,
        files: Some(&none),
    };
    let (_, _, plan) = ChartDecoder::decode_with_audio_plan(text.as_bytes(), &ctx, 0).unwrap();
    assert!(plan.is_empty());
}

#[test]
fn the_plan_does_not_depend_on_the_track_and_the_shift_is_in_the_chart() {
    let notes: &[&str] = &["0 = N 0 0"];
    let text = chart_text(
        &["Resolution = 192", "Offset = 2"],
        &BASIC_SYNC,
        &[("ExpertSingle", notes), ("EasyDoubleBass", notes)],
    );
    let ctx = ChartContext::default();
    let (a_chart, _, a) = ChartDecoder::decode_with_audio_plan(text.as_bytes(), &ctx, 0).unwrap();
    let (b_chart, _, b) = ChartDecoder::decode_with_audio_plan(text.as_bytes(), &ctx, 1).unwrap();
    assert_eq!(a, b);
    assert!(a.events.iter().all(|e| e.time_us == 0));
    assert_eq!(times(&a_chart), [2_000_000]);
    assert_eq!(times(&b_chart), [2_000_000]);
}

// ------------------------------------------------------------------------------------------------ hostile

#[test]
fn hostile_input_never_panics() {
    let long_digits = "9".repeat(5000);
    let cases: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"[".to_vec(),
        b"[Song]".to_vec(),
        b"[Song]\n{".to_vec(),
        b"}}}{{{[[[]]]".to_vec(),
        vec![0u8; 4096],
        (0..=255u8).cycle().take(10_000).collect(),
        b"[Song]\n{\nResolution = 0\n}\n".to_vec(),
        b"[Song]\n{\nResolution = -4\n}\n".to_vec(),
        b"[Song]\n{\nResolution = 99999999999999999999\n}\n".to_vec(),
        format!("[Song]\n{{\nResolution = {long_digits}\n}}\n").into_bytes(),
        format!(
            "[Song]\n{{\nResolution = 192\nOffset = {long_digits}\n}}\n[SyncTrack]\n{{\n0 = B {long_digits}\n{long_digits} = B 120000\n}}\n[ExpertSingle]\n{{\n{long_digits} = N 0 {long_digits}\n0 = N {long_digits} 0\n0 = N 0\n0 = N\n= N 0 0\n0 =\nN 0 0\n0 = N 0 {long_digits}\n}}\n"
        )
        .into_bytes(),
        b"[Song]\n{\nResolution = 1\n}\n[SyncTrack]\n{\n0 = B 1\n}\n[ExpertSingle]\n{\n0 = N 0 0\n18446744073709551615 = N 1 18446744073709551615\n99999999 = N 2 0\n}\n".to_vec(),
        b"[Song]\n{\nResolution = 4294967295\n}\n[SyncTrack]\n{\n0 = B 4294967295\n0 = TS 4294967295 31\n1 = TS 1 32\n}\n[ExpertSingle]\n{\n0 = N 0 0\n1 = N 1 4294967295\n}\n".to_vec(),
    ];
    for bytes in &cases {
        let _ = ChartDecoder::decode(bytes);
        let _ = ChartDecoder::variants(bytes);
        let _ = ChartDecoder::decode_with_audio_plan(bytes, &ChartContext::default(), 0);
        let _ = SongIni::parse(bytes);
    }
    // An absurd tempo sends notes out of range: they are dropped and counted, the rest decodes.
    let text = b"[Song]\n{\nResolution = 1\n}\n[SyncTrack]\n{\n0 = B 1\n}\n[ExpertSingle]\n{\n0 = N 0 0\n1000000 = N 1 0\n}\n";
    let (chart, report) = ChartDecoder::decode_with_report(text, None, 0).unwrap();
    assert_eq!(chart.notes.len(), 1);
    assert_eq!(report.events_out_of_range, 1);
    // A resolution that is not a number is an error, not a default.
    assert!(
        ChartDecoder::decode(b"[Song]\n{\nResolution = abc\n}\n[ExpertSingle]\n{\n0 = N 0 0\n}\n")
            .is_err()
    );
    // Input size is capped.
    assert!(SongIni::parse(&vec![b'a'; 65 * 1024 * 1024]).is_err());
}

#[test]
fn a_flood_of_sections_is_bounded() {
    let mut text = String::from("[Song]\n{\n}\n[ExpertSingle]\n{\n0 = N 0 0\n}\n");
    for i in 0..200_000 {
        text.push_str(&format!("[Expert{i}]\n{{\n0 = N 0 0\n}}\n"));
    }
    let (chart, report) = ChartDecoder::decode_with_report(text.as_bytes(), None, 0).unwrap();
    assert_eq!(chart.notes.len(), 1);
    assert!(report.unsupported_tracks.len() <= 4096);
}

#[test]
fn a_sustain_that_ends_out_of_range_is_a_tap() {
    let text = b"[Song]\n{\nResolution = 1\n}\n[SyncTrack]\n{\n0 = B 1\n}\n[ExpertSingle]\n{\n0 = N 0 100000000\n}\n";
    let (chart, report) = ChartDecoder::decode_with_report(text, None, 0).unwrap();
    assert!(!chart.notes[0].is_hold());
    assert_eq!(report.sustains_collapsed_to_taps, 1);
}

#[test]
fn decoded_charts_always_validate() {
    // Dense chords, sustains over sustains, tempo changes: `Decoder::decode` validates.
    let mut lines = Vec::new();
    for i in 0..200u32 {
        lines.push(format!("{} = N {} {}", i * 7, i % 5, 50 + i * 3));
        lines.push(format!("{} = N {} 0", i * 7, (i + 2) % 5));
    }
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let text = chart_text(
        &BASIC_SONG,
        &[
            "0 = B 120000",
            "300 = B 133333",
            "777 = B 99999",
            "1000 = B 250000",
        ],
        &[("ExpertSingle", &refs)],
    );
    let chart = decode(&text);
    chart.validate().unwrap();
    assert!(chart.notes.len() > 300);
}

// ------------------------------------------------------------------------------------------------- files

#[test]
fn auto_decode_reads_the_song_ini_of_the_folder() {
    use crate::formats::{auto_decode, auto_decode_variant};
    let dir = std::env::temp_dir().join(format!("rox-clone-hero-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let text = chart_text(
        &BASIC_SONG,
        &BASIC_SYNC,
        &[
            ("ExpertSingle", &["0 = N 0 0"]),
            ("EasySingle", &["192 = N 1 0"]),
        ],
    );
    std::fs::write(dir.join("notes.chart"), &text).unwrap();
    std::fs::write(
        dir.join("Song.INI"),
        "[song]\nname = Ini Title\ndelay = 100\n",
    )
    .unwrap();
    let first = auto_decode(dir.join("notes.chart")).unwrap();
    assert_eq!(first.metadata.title, "Ini Title");
    assert_eq!(times(&first), [100_000]);
    let second = auto_decode_variant(dir.join("notes.chart"), 1).unwrap();
    assert_eq!(second.metadata.difficulty_name, "Guitar Easy");
    assert_eq!(times(&second), [600_000]);
    assert!(auto_decode_variant(dir.join("notes.chart"), 2).is_err());
    // Without a song.ini beside it.
    std::fs::remove_file(dir.join("Song.INI")).unwrap();
    let plain = auto_decode(dir.join("notes.chart")).unwrap();
    assert_eq!(plain.metadata.title, "Example Song");
    assert_eq!(times(&plain), [0]);
    std::fs::remove_dir_all(&dir).unwrap();
}

// ------------------------------------------------------------------------------------------------------ sng

pub(super) struct SngBuilder {
    mask: [u8; 16],
    metadata: Vec<(String, String)>,
    files: Vec<(String, Vec<u8>)>,
}

impl SngBuilder {
    pub(super) fn new() -> Self {
        Self {
            mask: [
                0x13, 0x37, 0xA5, 0x5A, 0x00, 0xFF, 0x80, 0x01, 0x42, 0x24, 0x99, 0x66, 0xC3, 0x3C,
                0x0F, 0xF0,
            ],
            metadata: Vec::new(),
            files: Vec::new(),
        }
    }

    pub(super) fn meta(mut self, key: &str, value: &str) -> Self {
        self.metadata.push((key.into(), value.into()));
        self
    }

    pub(super) fn file(mut self, name: &str, bytes: &[u8]) -> Self {
        self.files.push((name.into(), bytes.to_vec()));
        self
    }

    pub(super) fn build(&self) -> Vec<u8> {
        let mut out = b"SNGPKG".to_vec();
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&self.mask);
        let mut meta = (self.metadata.len() as u64).to_le_bytes().to_vec();
        for (k, v) in &self.metadata {
            meta.extend_from_slice(&(k.len() as i32).to_le_bytes());
            meta.extend_from_slice(k.as_bytes());
            meta.extend_from_slice(&(v.len() as i32).to_le_bytes());
            meta.extend_from_slice(v.as_bytes());
        }
        out.extend_from_slice(&(meta.len() as u64).to_le_bytes());
        out.extend_from_slice(&meta);
        // File index: its size is known before the offsets, which are absolute.
        let index_len: usize = 8 + self
            .files
            .iter()
            .map(|(n, _)| 1 + n.len() + 16)
            .sum::<usize>();
        let data_start = out.len() + 8 + index_len + 8;
        let mut index = (self.files.len() as u64).to_le_bytes().to_vec();
        let mut data = Vec::new();
        for (name, bytes) in &self.files {
            index.push(name.len() as u8);
            index.extend_from_slice(name.as_bytes());
            index.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            index.extend_from_slice(&((data_start + data.len()) as u64).to_le_bytes());
            for (i, b) in bytes.iter().enumerate() {
                data.push(b ^ self.mask[i % 16] ^ (i & 0xFF) as u8);
            }
        }
        out.extend_from_slice(&(index.len() as u64).to_le_bytes());
        out.extend_from_slice(&index);
        out.extend_from_slice(&(data.len() as u64).to_le_bytes());
        out.extend_from_slice(&data);
        out
    }
}

fn sample_sng() -> (Vec<u8>, String) {
    let chart = chart_text(
        &[
            "Name = \"Chart Title\"",
            "Resolution = 192",
            "GuitarStream = \"guitar.ogg\"",
            "MusicStream = \"Backing.ogg\"",
        ],
        &BASIC_SYNC,
        &[
            ("ExpertSingle", &["0 = N 0 0", "192 = N 1 96"]),
            ("HardSingle", &["192 = N 2 0"]),
        ],
    );
    // More than 256 bytes of audio so that the `i & 0xFF` part of the mask wraps.
    let audio: Vec<u8> = (0..700u32).map(|i| (i * 31 % 251) as u8).collect();
    let bytes = SngBuilder::new()
        .meta("name", "Sng Title")
        .meta("charter", "Sng Charter")
        .meta("delay", "250")
        .meta("diff_guitar", "5")
        .file("notes.chart", chart.as_bytes())
        .file("song.ogg", &audio)
        .file("Backing.ogg", b"backing")
        .file("guitar.opus", b"gtr")
        .file("drums.ogg", b"d")
        .file("drums_1.ogg", b"d1")
        .file("album.png", b"png")
        .build();
    (bytes, chart)
}

#[test]
fn sng_round_trip() {
    let (bytes, chart_text) = sample_sng();
    let pkg = SngPackage::open(&bytes).unwrap();
    assert_eq!(pkg.entries.len(), 7);
    assert_eq!(pkg.entries[0].name, "notes.chart");
    assert_eq!(pkg.read(0).unwrap(), chart_text.as_bytes());
    let audio: Vec<u8> = (0..700u32).map(|i| (i * 31 % 251) as u8).collect();
    assert_eq!(pkg.read(pkg.find("SONG.OGG").unwrap()).unwrap(), audio);
    assert_eq!(pkg.metadata.len(), 4);
    assert_eq!(pkg.ini().name.as_deref(), Some("Sng Title"));
    assert!(pkg.read(99).is_err());

    let variants = pkg.variants().unwrap();
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[1].section, "HardSingle");
    let chart = pkg.decode_variant(0).unwrap();
    assert_eq!(chart.metadata.title, "Sng Title"); // metadata of the package = song.ini
    assert_eq!(chart.metadata.creator, "Sng Charter");
    assert_eq!(chart.metadata.difficulty_value, Some(5.0));
    assert_eq!(times(&chart), [250_000, 750_000]); // delay 250
    assert!(chart.notes[1].is_hold());

    let (_, _, plan) = pkg.decode_with_audio_plan("packs/Song.sng", 0).unwrap();
    // `Backing.ogg` is MusicStream but `song.ogg` exists: reserved first. `drums.ogg` is dropped (drums_1).
    let got: Vec<(&str, Option<u32>)> = plan
        .events
        .iter()
        .map(|e| (e.file.as_str(), e.index))
        .collect();
    assert_eq!(
        got,
        [
            ("packs/Song.sng", Some(1)), // song.ogg
            ("packs/Song.sng", Some(3)), // guitar.opus
            ("packs/Song.sng", Some(5)), // drums_1.ogg
        ]
    );
    assert!(plan.events.iter().all(|e| !e.optional && e.time_us == 0));
    assert!(pkg.decode_with_audio_plan("../x.sng", 0).is_err());
    assert!(pkg.decode_with_audio_plan("/abs/x.sng", 0).is_err());
    assert!(pkg.decode_with_audio_plan("x.sng", 2).is_err());
}

#[test]
fn sng_plan_falls_back_on_the_stream_tags() {
    let chart = chart_text(
        &["Resolution = 192", "MusicStream = \"Backing.ogg\""],
        &BASIC_SYNC,
        &[("ExpertSingle", &["0 = N 0 0"])],
    );
    let bytes = SngBuilder::new()
        .file("notes.chart", chart.as_bytes())
        .file("backing.OPUS", b"x")
        .build();
    let pkg = SngPackage::open(&bytes).unwrap();
    let (_, _, plan) = pkg.decode_with_audio_plan("a.sng", 0).unwrap();
    assert_eq!(plan.events.len(), 1);
    assert_eq!(plan.events[0], SoundEvent::in_container(0, "a.sng", 1));
}

#[test]
fn sng_without_a_chart_or_with_a_mid() {
    let bytes = SngBuilder::new().file("song.ogg", b"x").build();
    assert!(SngPackage::open(&bytes).unwrap().variants().is_err());
    let bytes = SngBuilder::new().file("notes.mid", b"MThd").build();
    // A notes.mid is read now (see mid_tests.rs); this one is not a MIDI file.
    let err = SngPackage::open(&bytes).unwrap().variants().unwrap_err();
    assert!(err.to_string().contains("mid:"), "{err}");
    let bytes = SngBuilder::new().build();
    assert_eq!(SngPackage::open(&bytes).unwrap().entries.len(), 0);
}

#[test]
fn sng_hostile_input() {
    let (bytes, _) = sample_sng();
    // Every truncation is an error, never a panic.
    for len in 0..bytes.len() {
        let cut = &bytes[..len];
        if let Ok(pkg) = SngPackage::open(cut) {
            // A cut that still parses cannot have lost file data.
            for i in 0..pkg.entries.len() {
                let _ = pkg.read(i);
            }
            panic!(
                "a truncated package ({len} of {} bytes) opened",
                bytes.len()
            );
        }
    }
    // Bad signature, bad version.
    let mut bad = bytes.clone();
    bad[0] = b'X';
    assert!(SngPackage::open(&bad).is_err());
    let mut bad = bytes.clone();
    bad[6] = 2;
    assert!(SngPackage::open(&bad).is_err());
    // A metadata count larger than the section, a file count larger than the section.
    let mut bad = bytes.clone();
    bad[26 + 8..26 + 16].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(SngPackage::open(&bad).is_err());
    // Every single flipped byte of the structure parses or errors, never panics.
    for at in 0..300.min(bytes.len()) {
        let mut flipped = bytes.clone();
        flipped[at] ^= 0xFF;
        if let Ok(pkg) = SngPackage::open(&flipped) {
            for i in 0..pkg.entries.len() {
                let _ = pkg.read(i);
            }
            let _ = pkg.variants();
        }
    }
    // Names that climb out, are absolute, hold `\` or `:`.
    for name in [
        "../x.ogg", "/x.ogg", "a\\b.ogg", "C:x.ogg", "a//b", "", "a..b",
    ] {
        let bytes = SngBuilder::new().file(name, b"x").build();
        assert!(SngPackage::open(&bytes).is_err(), "{name:?} was accepted");
    }
    // An offset outside the file data.
    let mut bad = SngBuilder::new().file("a.ogg", b"abc").build();
    let at = bad.len() - 3 - 8; // the offset of the only entry sits before the file data length
    bad[at - 8..at].copy_from_slice(&0u64.to_le_bytes());
    assert!(SngPackage::open(&bad).is_err());
    // Random garbage after a valid header.
    let mut junk = b"SNGPKG\x01\0\0\0".to_vec();
    junk.extend((0..2000u32).map(|i| (i * 7 + 3) as u8));
    let _ = SngPackage::open(&junk);
}

#[test]
fn auto_decode_routes_a_sng_by_its_path_and_lists_every_variant() {
    use crate::formats::{auto_decode, auto_decode_all, auto_decode_variant};
    let dir = std::env::temp_dir().join(format!("rox-clone-hero-sng-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (bytes, _) = sample_sng();
    std::fs::write(dir.join("song.sng"), &bytes).unwrap();
    let first = auto_decode(dir.join("song.sng")).unwrap();
    assert_eq!(first.metadata.title, "Sng Title");
    assert_eq!(first.key_count, 5);
    let second = auto_decode_variant(dir.join("song.sng"), 1).unwrap();
    assert_eq!(second.metadata.difficulty_name, "Guitar Hard");
    assert!(auto_decode_variant(dir.join("song.sng"), 2).is_err());
    let all = auto_decode_all(dir.join("song.sng")).unwrap().unwrap();
    assert_eq!(
        all.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
        ["ExpertSingle", "HardSingle"]
    );
    assert!(all.iter().all(|v| v.chart.is_ok()));
    // Not a multi-chart file.
    std::fs::write(dir.join("plain.json"), b"{}").unwrap();
    assert!(auto_decode_all(dir.join("plain.json")).unwrap().is_none());
    assert!(auto_decode_all(dir.join("song.osu")).unwrap().is_none());
    // A broken package is an error, not a panic.
    std::fs::write(dir.join("bad.sng"), b"SNGPKG nope").unwrap();
    assert!(auto_decode(dir.join("bad.sng")).is_err());
    assert!(auto_decode_all(dir.join("bad.sng")).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}
