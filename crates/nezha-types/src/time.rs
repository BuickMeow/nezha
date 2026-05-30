const BLACK_KEYS: [bool; 12] = [false, true, false, true, false, false, true, false, true, false, true, false];

pub fn is_black_key(key: u8) -> bool {
    BLACK_KEYS[(key % 12) as usize]
}
