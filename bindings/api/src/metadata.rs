impl_string_getter!(rox_chart_title, title);
impl_string_getter!(rox_chart_artist, artist);
impl_string_getter!(rox_chart_creator, creator);
impl_string_getter!(rox_chart_difficulty, difficulty_name);

impl_string_setter!(rox_chart_set_title, title);
impl_string_setter!(rox_chart_set_artist, artist);
impl_string_setter!(rox_chart_set_creator, creator);
impl_string_setter!(rox_chart_set_difficulty, difficulty_name);

impl_primitive_getter!(rox_chart_key_count, method key_count, i32, i32::from);
impl_primitive_setter!(rox_chart_set_key_count, meta key_count, u8);

impl_primitive_getter!(rox_chart_note_count, method note_count, usize, |x: usize| x);
impl_primitive_getter!(rox_chart_duration_us, method duration_us, i64, |x: i64| x);

impl_primitive_getter!(rox_chart_is_coop, meta is_coop, i32, |b| if b { 1 } else { 0 }, 0);
impl_primitive_setter!(rox_chart_set_coop, meta is_coop, i32, |i| i != 0);
