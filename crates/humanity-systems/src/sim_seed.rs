//! The simulation's seed (the headless sim runner's increment 2, 2026-11-15;
//! docs/design/headless-sim-runner.md section 2, "Safety"): a run that must repeat (the soak, the
//! step ladder, the tests that compare one span stepped two ways) puts a `u64` in the DataStore
//! under [`SLOT`] before the first tick, and each system that draws its own random stream
//! (WeatherSystem, DisasterSystem) starts that stream from it, salted per system so no two share
//! a sequence. The windowed game never sets it, so there the streams start from the operating
//! system's entropy exactly as before.
//!
//! Every other draw a system makes goes through a [`Stream`] of its own (BUG-333, 2026-11-20):
//! an illness rolled on eating and a wound going bad (FoodSystem), a harvest's count (the farm's
//! picking), a crafted item's grade, the creatures' wandering, combat's loot and fire's spread.
//! They used to draw from the thread's shared generator (`rand::rng()`, `rand::random`), which no
//! seed reaches, so the routine soak (`--sim soak --routine daily`), whose person eats the home
//! storage's raw rice, fell ill in some runs and not others: three runs of one exe ended day 6 at
//! hydration 85.3, 94.8 and 93.1, with 34, 32 and 34 notices. tests/sim_random_lint.rs keeps the
//! thread's generator out of the systems.

use humanity_core::hot_reload::data_store::DataStore;
use rand::rngs::StdRng;
use rand::SeedableRng;

/// DataStore slot: the run's seed, a plain `u64`. Absent in the windowed game.
pub const SLOT: &str = "sim_seed";

/// The salt WeatherSystem mixes into the run's seed.
pub const WEATHER: u64 = 0x5745_4154_4845_5221;
/// The salt DisasterSystem mixes into the run's seed.
pub const DISASTERS: u64 = 0x4449_5341_5354_4552;
/// FoodSystem's: the illness a raw or spoiled meal can bring, a wound going bad.
pub const FOOD: u64 = 0x464F_4F44_5359_5354;
/// The farm's picking: how much a harvest gives, and its seeds.
pub const HARVEST: u64 = 0x4841_5256_4553_5421;
/// CraftingSystem's: the random spread in a hand-made item's grade.
pub const CRAFTING: u64 = 0x4352_4146_5449_4E47;
/// AISystem's: where a creature wanders next, and for how long.
pub const CREATURES: u64 = 0x4352_4541_5455_5245;
/// CombatSystem's: the loot a death drops.
pub const COMBAT: u64 = 0x434F_4D42_4154_2121;
/// FireSystem's: whether a fire spreads to its neighbour.
pub const FIRE: u64 = 0x4649_5245_5350_5244;

/// One system's own random stream (BUG-333, 2026-11-20). It starts from the run's seed mixed with
/// the system's salt when the run set a seed (`for_system`), and from the operating system's
/// entropy when it set none (the windowed game). Which of the two is decided at each `rng`, so a
/// seed put in the DataStore after the system was made (the soak puts it there after the world
/// entry) still starts the stream from it, and a stream once started is never started again while
/// the seed stays the same.
#[derive(Debug, Clone)]
pub struct Stream {
    salt: u64,
    /// The stream, and the run seed it started from (None: the entropy). None before the first draw.
    rng: Option<(Option<u64>, StdRng)>,
}

impl Stream {
    /// A stream salted with `salt` (one of the consts above: no two systems share one).
    pub const fn new(salt: u64) -> Self {
        Stream { salt, rng: None }
    }

    /// The generator to draw from now, started from this run's seed (or the entropy) on the first
    /// draw and again only when the run's seed changed since.
    pub fn rng(&mut self, data: &DataStore) -> &mut StdRng {
        let seed = for_system(data, self.salt);
        if self.rng.as_ref().is_none_or(|(from, _)| *from != seed) {
            self.rng = Some((seed, seed.map_or_else(StdRng::from_os_rng, StdRng::seed_from_u64)));
        }
        &mut self.rng.as_mut().expect("the stream was started above").1
    }

    /// Decide where the stream starts for this tick, for a system whose draws happen deep in
    /// helpers that are not handed the DataStore (call it at the top of the tick, then `primed`).
    pub fn prime(&mut self, data: &DataStore) {
        let _ = self.rng(data);
    }

    /// The generator as `prime` left it; never primed, it starts from the entropy (a draw outside
    /// any tick, which no run's seed reaches).
    pub fn primed(&mut self) -> &mut StdRng {
        &mut self.rng.get_or_insert_with(|| (None, StdRng::from_os_rng())).1
    }
}

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

    /// A system's `Stream` (BUG-333): one seed draws the same numbers in two streams, two systems'
    /// salts draw different ones, a seed set after a draw from the entropy restarts the stream from
    /// it (the soak sets its seed after the world entry), and with no seed two streams differ.
    #[test]
    fn a_stream_repeats_with_its_seed_and_keeps_the_entropy_without_one() {
        use rand::Rng;
        let draws = |s: &mut Stream, data: &DataStore| (0..8).map(|_| s.rng(data).random::<u64>()).collect::<Vec<_>>();
        let mut data = DataStore::new();
        let (mut free_a, mut free_b) = (Stream::new(FOOD), Stream::new(FOOD));
        assert_ne!(draws(&mut free_a, &data), draws(&mut free_b, &data), "two streams with no seed drew alike: no entropy");
        data.insert(SLOT, 7_u64);
        let seeded = draws(&mut Stream::new(FOOD), &data);
        assert_eq!(seeded, draws(&mut free_a, &data), "a seed set after the first draw did not restart the stream from it");
        assert_eq!(draws(&mut Stream::new(FOOD), &data)[..4], draws(&mut Stream::new(FOOD), &data)[..4], "one seed drew two ways");
        assert_ne!(seeded, draws(&mut Stream::new(HARVEST), &data), "two systems' salts drew alike");
        let mut primed = Stream::new(FOOD);
        primed.prime(&data);
        assert_eq!(seeded[0], primed.primed().random::<u64>(), "a primed stream does not draw from the seed");
    }
}
