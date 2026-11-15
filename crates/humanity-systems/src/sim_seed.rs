//! The simulation's seed (the headless sim runner's increment 2, 2026-11-15;
//! docs/design/headless-sim-runner.md section 2, "Safety"): a run that must repeat (the soak, the
//! step ladder, the tests that compare one span stepped two ways) puts a `u64` in the DataStore
//! under [`SLOT`] before the first tick, and each system that draws its own random stream
//! (WeatherSystem, DisasterSystem) starts that stream from it, salted per system so no two share
//! a sequence. The windowed game never sets it, so there the streams start from the operating
//! system's entropy exactly as before.
//!
//! Not yet seeded: the draws that go through the thread's shared generator (`rand::rng()`,
//! `rand::random`): an illness rolled on eating, a crafted item's grade, a harvest's count, the
//! creatures' wandering, combat and fire. None of them runs in an untouched homestead's day (no
//! meal, no craft, no harvest, no fight), which is what the soak steps; they are the remainder of
//! the design's "sim seeds" and come with the runs that exercise them.

use humanity_core::hot_reload::data_store::DataStore;

/// DataStore slot: the run's seed, a plain `u64`. Absent in the windowed game.
pub const SLOT: &str = "sim_seed";

/// The salt WeatherSystem mixes into the run's seed.
pub const WEATHER: u64 = 0x5745_4154_4845_5221;
/// The salt DisasterSystem mixes into the run's seed.
pub const DISASTERS: u64 = 0x4449_5341_5354_4552;

/// The seed one system's stream starts from this run: the run's seed mixed with the system's
/// salt (SplitMix64's finaliser, so neighbouring seeds give unrelated streams), or None when the
/// run set no seed (the windowed game: the system keeps its entropy-seeded stream).
pub fn for_system(data: &DataStore, salt: u64) -> Option<u64> {
    data.get::<u64>(SLOT).map(|seed| mix(*seed ^ salt))
}

/// SplitMix64's output function.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_without_a_seed_keeps_its_entropy_and_one_with_a_seed_repeats() {
        let mut data = DataStore::new();
        assert_eq!(for_system(&data, WEATHER), None, "the windowed game sets no seed");
        data.insert(SLOT, 7_u64);
        let a = for_system(&data, WEATHER);
        assert_eq!(a, for_system(&data, WEATHER), "the same run's seed gives the same stream");
        assert_ne!(a, for_system(&data, DISASTERS), "two systems share a stream");
        data.insert(SLOT, 8_u64);
        assert_ne!(a, for_system(&data, WEATHER), "neighbouring seeds give the same stream");
    }
}
