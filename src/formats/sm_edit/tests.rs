//! Tests of the `.edit` reader. Every file is built in memory from the layouts of the format notes (the writer's `.ssc` layout, the
//! old `.sm` layout); no real file is used (the repository has no `.edit`).

use rox::model::NoteType;

use super::{EditDecoder, MAX_EDIT_SIZE};
use crate::formats::sm::song_header::SongHeader;
use crate::formats::sm::{decoder::from_chart, parser};

/// 120 BPM, a one-second stop on beat 2, the first beat at 0.5 s.
const SONG_SM: &str = "#TITLE:Song Title;\n#ARTIST:The Artist;\n#CREDIT:Song Author;\n#MUSIC:song.ogg;\n#OFFSET:-0.5;\n#BPMS:0=120;\n#STOPS:2=1;\n";

const MEASURES: &str = "1000\n0100\n0010\n0001\n,\n1000\n0000\n0000\n0000\n";

fn sm_edit(description: &str, difficulty: &str, steps_type: &str, measures: &str) -> String {
    format!(
        "#SONG:Pack/Song Folder;\n#NOTES:\n     {steps_type}:\n     {description}:\n     {difficulty}:\n     7:\n     0,0,0,0,0:\n{measures};\n"
    )
}

fn ssc_edit(extra: &str, measures: &str) -> String {
    format!(
        "#SONG:Pack/Song Folder;\n//---------------dance-single - my edit----------------\n#NOTEDATA:;\n#CHARTNAME:;\n#STEPSTYPE:dance-single;\n#DESCRIPTION:ssc edit;\n#DIFFICULTY:Hard;\n#METER:9;\n#RADARVALUES:0,0,0,0,0;\n#CREDIT:Edit Author;\n{extra}#NOTES:\n{measures};\n"
    )
}

fn times(chart: &rox::model::RoxChart) -> Vec<(i64, u8)> {
    chart.notes.iter().map(|n| (n.time_us, n.column)).collect()
}

#[test]
fn sm_style_edit_takes_the_songs_timing_and_metadata() {
    let edit = sm_edit("my edit", "Challenge", "dance-single", MEASURES);
    let chart = EditDecoder::decode_for_song(edit.as_bytes(), SONG_SM.as_bytes(), "sm").unwrap();
    assert_eq!(chart.key_count, 4);
    assert_eq!(chart.metadata.title, "Song Title");
    assert_eq!(chart.metadata.artist, "The Artist");
    assert_eq!(chart.metadata.creator, "Song Author");
    assert_eq!(chart.metadata.audio_file, "song.ogg");
    // The difficulty written in the file is replaced: an Edit is named by its description.
    assert_eq!(chart.metadata.difficulty_name, "my edit");
    assert_eq!(chart.metadata.difficulty_value, Some(7.0));
    assert_eq!(chart.metadata.audio_offset_us, 0);
    // Beat n at 0.5 s + 0.5 s per beat; the note on the stop's beat is hit when the stop begins, the next ones are one second later.
    assert_eq!(
        times(&chart),
        [
            (500_000, 0),
            (1_000_000, 1),
            (1_500_000, 2),
            (3_000_000, 3),
            (3_500_000, 0)
        ]
    );
    assert_eq!(chart.timing_points.len(), 1);
    assert_eq!(chart.timing_points[0].bpm_value(), Some(120.0));
}

#[test]
fn an_edit_is_the_same_chart_as_inside_the_song() {
    let edit = sm_edit("my edit", "Edit", "dance-single", MEASURES);
    let inline = format!(
        "{SONG_SM}#NOTES:\n     dance-single:\n     my edit:\n     Edit:\n     7:\n     0,0,0,0,0:\n{MEASURES};\n"
    );
    let song = parser::parse(inline.as_bytes()).unwrap();
    let expected = from_chart(&song, &song.charts[0]);
    let decoded = EditDecoder::decode_for_song(edit.as_bytes(), SONG_SM.as_bytes(), "sm").unwrap();
    assert_eq!(decoded.notes, expected.notes);
    assert_eq!(decoded.timing_points, expected.timing_points);
    assert_eq!(decoded.metadata.title, expected.metadata.title);
    assert_eq!(decoded.metadata.audio_file, expected.metadata.audio_file);
}

#[test]
fn ssc_style_edit_reads_its_chart_tags() {
    let edit = ssc_edit("", MEASURES);
    let chart = EditDecoder::decode_for_song(edit.as_bytes(), SONG_SM.as_bytes(), "sm").unwrap();
    assert_eq!(chart.metadata.difficulty_name, "ssc edit");
    assert_eq!(chart.metadata.difficulty_value, Some(9.0));
    // The chart's own #CREDIT wins over the song's.
    assert_eq!(chart.metadata.creator, "Edit Author");
    assert_eq!(chart.metadata.title, "Song Title");
    assert_eq!(chart.notes[3].time_us, 3_000_000);
}

#[test]
fn ssc_style_edit_with_its_own_timing_replaces_the_songs() {
    // 60 BPM: a second per beat, no stop (its own, empty #STOPS: the chart's timing is split), no offset (its own #OFFSET).
    let edit = ssc_edit("#OFFSET:0.000;\n#BPMS:0.000=60.000;\n#STOPS:;\n", MEASURES);
    let chart = EditDecoder::decode_for_song(edit.as_bytes(), SONG_SM.as_bytes(), "sm").unwrap();
    assert_eq!(
        chart.notes.iter().map(|n| n.time_us).collect::<Vec<_>>(),
        [0, 1_000_000, 2_000_000, 3_000_000, 4_000_000]
    );
    assert_eq!(chart.timing_points[0].bpm_value(), Some(60.0));
    // Without its own BPMS, only a #OFFSET of the chart is its own: the song's BPMS and STOPS stay.
    let edit = ssc_edit("#OFFSET:0.000;\n", MEASURES);
    let chart = EditDecoder::decode_for_song(edit.as_bytes(), SONG_SM.as_bytes(), "sm").unwrap();
    assert_eq!(chart.notes[3].time_us, 2_500_000);
}

#[test]
fn an_ssc_song_gives_its_header_not_its_charts() {
    let song = "#VERSION:0.83;\n#TITLE:Ssc Song;\n#ARTIST:A;\n#MUSIC:s.ogg;\n#OFFSET:0;\n#BPMS:0=120;\n#NOTEDATA:;\n#STEPSTYPE:dance-single;\n#DESCRIPTION:;\n#DIFFICULTY:Hard;\n#METER:1;\n#BPMS:0=240;\n#NOTES:\n1000\n0000\n0000\n0000\n;\n";
    let edit = sm_edit("e", "Edit", "dance-single", MEASURES);
    let chart = EditDecoder::decode_for_song(edit.as_bytes(), song.as_bytes(), "SSC").unwrap();
    assert_eq!(chart.metadata.title, "Ssc Song");
    assert_eq!(chart.notes[1].time_us, 500_000);
}

#[test]
fn other_supported_step_types_keep_their_columns() {
    let measures = "10000001\n01000010\n00100100\n00011000\n";
    let edit = sm_edit("d", "Edit", "dance-double", measures);
    let chart = EditDecoder::decode_for_song(edit.as_bytes(), SONG_SM.as_bytes(), "sm").unwrap();
    assert_eq!(chart.key_count, 8);
    assert_eq!(chart.notes.len(), 8);
    let pump = sm_edit("p", "Edit", "pump-single", "10000\n00001\n00000\n00000\n");
    assert_eq!(
        EditDecoder::decode_for_song(pump.as_bytes(), SONG_SM.as_bytes(), "sm")
            .unwrap()
            .key_count,
        5
    );
}

#[test]
fn holds_rolls_and_mines_are_read_as_in_a_song() {
    let measures = "2000\n3000\n0400\n0300\n,\n0000\n0000\n0000\n000M\n";
    let edit = sm_edit("h", "Edit", "dance-single", measures);
    let chart = EditDecoder::decode_for_song(edit.as_bytes(), SONG_SM.as_bytes(), "sm").unwrap();
    let kinds: Vec<NoteType> = chart.notes.iter().map(|n| n.note_type).collect();
    assert!(matches!(
        kinds[0],
        NoteType::Hold {
            duration_us: 500_000
        }
    ));
    assert!(matches!(kinds[1], NoteType::Burst { .. }));
    assert!(matches!(kinds[2], NoteType::Mine));
}

#[test]
fn only_the_first_chart_of_the_file_is_used() {
    let two = format!(
        "#SONG:Song Folder;\n#NOTES:\n dance-single:\n first:\n Edit:\n 3:\n 0,0,0,0,0:\n{MEASURES};\n#NOTES:\n dance-double:\n second:\n Edit:\n 3:\n 0,0,0,0,0:\n10000000\n00000000\n00000000\n00000000\n;\n"
    );
    let chart = EditDecoder::decode_for_song(two.as_bytes(), SONG_SM.as_bytes(), "sm").unwrap();
    assert_eq!(chart.metadata.difficulty_name, "first");
    assert_eq!(chart.key_count, 4);
}

#[test]
fn a_short_sm_notes_tag_is_skipped_like_the_loader_does() {
    let edit = format!(
        "#SONG:Song Folder;\n#NOTES:dance-single:x:Edit:3;\n#NOTES:\n dance-single:\n real:\n Edit:\n 3:\n 0,0,0,0,0:\n{MEASURES};\n"
    );
    let chart = EditDecoder::decode_for_song(edit.as_bytes(), SONG_SM.as_bytes(), "sm").unwrap();
    assert_eq!(chart.metadata.difficulty_name, "real");
}

#[test]
fn an_empty_description_names_the_chart_edit() {
    let edit = sm_edit("", "Hard", "dance-single", MEASURES);
    let header = SongHeader::parse(SONG_SM.as_bytes(), "sm").unwrap();
    let file = EditDecoder::parse_for_song(edit.as_bytes(), &header).unwrap();
    assert_eq!(file.charts[0].difficulty, "Edit");
    assert_eq!(
        EditDecoder::decode_for_header(edit.as_bytes(), &header)
            .unwrap()
            .metadata
            .difficulty_name,
        "Edit"
    );
}

#[test]
fn unplayable_charts_fail_with_their_reason() {
    let header = SongHeader::parse(SONG_SM.as_bytes(), "sm").unwrap();
    let edit = sm_edit("x", "Edit", "dance-threepanel", "100\n000\n000\n000\n");
    let file = EditDecoder::parse_for_song(edit.as_bytes(), &header).unwrap();
    assert!(file.charts.is_empty());
    assert!(
        file.skipped[0].contains("unsupported step type `dance-threepanel`"),
        "{:?}",
        file.skipped
    );
    let error = EditDecoder::decode_for_header(edit.as_bytes(), &header).unwrap_err();
    assert!(error.to_string().contains("dance-threepanel"), "{error}");
    // A row of the wrong width, and a song without BPMS.
    let edit = sm_edit("x", "Edit", "dance-single", "10\n00\n00\n00\n");
    assert!(EditDecoder::decode_for_header(edit.as_bytes(), &header).is_err());
    let no_bpm = SongHeader::parse(b"#TITLE:T;\n", "sm").unwrap();
    let edit = sm_edit("x", "Edit", "dance-single", MEASURES);
    assert!(EditDecoder::decode_for_header(edit.as_bytes(), &no_bpm).is_err());
    // No chart at all, an unknown song extension.
    assert!(EditDecoder::decode_for_header(b"#SONG:A/B;\n", &header).is_err());
    assert!(EditDecoder::decode_for_song(edit.as_bytes(), SONG_SM.as_bytes(), "dwi").is_err());
}

#[test]
fn song_path_is_normalised_and_safe() {
    let path = |value: &str| EditDecoder::song_path(format!("#SONG:{value};\n#NOTES:;").as_bytes());
    assert_eq!(path("Pack/Song Folder").unwrap(), "Pack/Song Folder");
    // A backslash is an escape in the tokenizer (the game reads with unescape on): a path separator is written `\\\\`.
    assert_eq!(path("Pack\\\\Song Folder").unwrap(), "Pack/Song Folder");
    assert_eq!(path("Pack\\Song").unwrap(), "PackSong");
    // `//` starts a comment in the tokenizer, as in the game: only a trailing separator is dropped.
    assert_eq!(path("Pack/Song Folder/").unwrap(), "Pack/Song Folder");
    assert_eq!(path("Song Folder").unwrap(), "Song Folder");
    assert_eq!(path("  Song ").unwrap(), "Song");
    for bad in [
        "",
        "/",
        "/etc/passwd",
        "\\\\host\\share",
        "C\\x/..",
        "../x",
        "a/..",
        "./a",
        "a/b/c",
        "a/.",
        "..",
    ] {
        assert!(path(bad).is_err(), "{bad:?} must be refused");
    }
    assert!(path("a\u{7}b").is_err());
    // The first #SONG counts; none at all is an error; the tag name is case-insensitive.
    assert_eq!(
        EditDecoder::song_path(b"#song:One;\n#SONG:Two;\n").unwrap(),
        "One"
    );
    assert!(EditDecoder::song_path(b"#NOTES:;").is_err());
    assert!(EditDecoder::song_path(&vec![b'#'; MAX_EDIT_SIZE + 1]).is_err());
}

#[test]
fn oversized_edits_are_refused_like_the_game_does() {
    let header = SongHeader::parse(SONG_SM.as_bytes(), "sm").unwrap();
    let mut edit = sm_edit("x", "Edit", "dance-single", MEASURES);
    while edit.len() <= MAX_EDIT_SIZE {
        edit.push_str("//padding padding padding padding\n");
    }
    let error = EditDecoder::decode_for_header(edit.as_bytes(), &header).unwrap_err();
    assert!(error.to_string().contains("unreasonably large"), "{error}");
}

#[test]
fn decode_for_song_file_finds_the_audio_of_the_folder() {
    let dir = std::env::temp_dir().join(format!("rox_edit_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let song_no_music = SONG_SM.replace("#MUSIC:song.ogg;", "#MUSIC:;");
    std::fs::write(dir.join("song.sm"), &song_no_music).unwrap();
    std::fs::write(dir.join("Track.MP3"), b"x").unwrap();
    let edit = sm_edit("e", "Edit", "dance-single", MEASURES);
    let empty = EditDecoder::decode_for_song_file(edit.as_bytes(), &dir.join("song.sm")).unwrap();
    // A #MUSIC naming a file that is there is kept; a missing one falls back like the song's own parse does.
    std::fs::write(dir.join("song2.sm"), SONG_SM).unwrap();
    let missing =
        EditDecoder::decode_for_song_file(edit.as_bytes(), &dir.join("song2.sm")).unwrap();
    std::fs::write(dir.join("song.ogg"), b"x").unwrap();
    let present =
        EditDecoder::decode_for_song_file(edit.as_bytes(), &dir.join("song2.sm")).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(empty.metadata.audio_file, "Track.MP3");
    assert_eq!(missing.metadata.audio_file, "Track.MP3");
    assert_eq!(present.metadata.audio_file, "song.ogg");
}

#[test]
fn garbage_and_truncated_edits_never_panic() {
    let header = SongHeader::parse(SONG_SM.as_bytes(), "sm").unwrap();
    let edit = ssc_edit("#BPMS:0=60;\n", MEASURES);
    for len in 0..=edit.len() {
        let prefix = &edit.as_bytes()[..len];
        let _ = EditDecoder::song_path(prefix);
        let _ = EditDecoder::decode_for_header(prefix, &header);
    }
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..200 {
        let bytes: Vec<u8> = (0..200)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state & 0xFF) as u8
            })
            .collect();
        let _ = EditDecoder::song_path(&bytes);
        let _ = EditDecoder::decode_for_header(&bytes, &header);
    }
}

#[test]
fn auto_decode_refuses_an_edit_alone_with_a_typed_error() {
    use crate::formats::auto::{
        auto_decode, auto_decode_all, auto_decode_bytes, auto_decode_variant, auto_encode,
    };
    use rox::error::RoxError;
    let dir = std::env::temp_dir().join(format!("rox_edit_auto_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("My Edit.edit");
    let edit = sm_edit("e", "Edit", "dance-single", MEASURES);
    std::fs::write(&path, &edit).unwrap();
    let results = [
        auto_decode(&path),
        auto_decode_variant(&path, 0),
        auto_decode_bytes(&path, edit.as_bytes()),
    ];
    let all = auto_decode_all(&path);
    let song = EditDecoder::decode_for_song(edit.as_bytes(), SONG_SM.as_bytes(), "sm").unwrap();
    let encoded = auto_encode(&song, &path);
    std::fs::remove_dir_all(&dir).unwrap();
    for result in results {
        assert!(
            matches!(&result, Err(RoxError::InvalidFormat(m)) if m == "edit needs its song"),
            "{result:?}"
        );
    }
    assert!(all.unwrap().is_none());
    assert!(encoded.is_err());
}
