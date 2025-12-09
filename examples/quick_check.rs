use rhythm_open_exchange::{Decoder, RoxCodec};
fn main() {
    let chart = RoxCodec::decode_from_path("large_chart.rox").unwrap();
    println!("Notes: {}", chart.notes.len());
    println!("First note time: {}", chart.notes[0].time_us);
    println!("Note 1000 time: {}", chart.notes[1000].time_us);
    println!("Note 15000 time: {}", chart.notes[15000].time_us);
    println!("Last note time: {}", chart.notes.last().unwrap().time_us);
    println!("Timing points: {}", chart.timing_points.len());
    // Check expected values
    assert_eq!(chart.notes.len(), 32000);
    assert_eq!(chart.timing_points.len(), 100);
    println!("ALL CHECKS PASSED!");
}
