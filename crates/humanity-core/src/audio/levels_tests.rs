// Child of audio/levels.rs (#[path]): the targets, the gain, and the shipped sounds levelled.
// Every test was seen failing on purpose once; the break and what it said are at the test.

use super::*;

fn repo() -> &'static Path {
    Path::new(env!("HUMANITY_REPO_ROOT"))
}

fn shipped_targets() -> LoudnessTargets {
    LoudnessTargets::load(&repo().join("data")).expect("data/audio/loudness.toml reads")
}

/// The shipped targets name every kind the design lists (speech, sound effects, music, video,
/// interface, notifications) and every bus `data/sounds.toml` uses has a kind, so no sound falls to
/// the default by accident. RED first (2026-11-06): with the `voice` line taken out of [buses],
/// "bus \"voice\" of data/sounds.toml has no kind in loudness.toml".
#[test]
fn the_targets_name_every_kind_and_every_bus_has_one() {
    let t = shipped_targets();
    for kind in ["speech", "sfx", "music", "video", "interface", "notification"] {
        assert!(t.categories.contains_key(kind), "loudness.toml has no {kind:?} kind");
    }
    let sounds: toml::Table = std::fs::read_to_string(repo().join("data/sounds.toml")).unwrap().parse().unwrap();
    for (group, table) in &sounds {
        for (id, entry) in table.as_table().into_iter().flatten() {
            let bus = entry.get("bus").and_then(|b| b.as_str()).unwrap_or("sfx");
            assert!(t.buses.contains_key(bus), "bus {bus:?} of data/sounds.toml has no kind in loudness.toml ({group}.{id})");
            if let Some(kind) = entry.get("category").and_then(|c| c.as_str()) {
                assert!(t.categories.contains_key(kind), "{group}.{id} names the kind {kind:?}, which loudness.toml does not have");
            }
        }
    }
    assert!(LoudnessTargets::parse("ceiling_dbtp = -1.0\nmax_boost_db = 12.0\ndefault_category = \"nope\"\n[categories.sfx]\ntarget_lufs = -20.0\n").is_err(), "an unknown default kind is refused");
}

/// The gain brings a sound to its target: down as far as it takes, up at most `max_boost_db`, and
/// never so far that its true peak AS PLAYED (its authored volume counted) passes the ceiling (a
/// sound already over it is not raised); silence is left alone. RED first (2026-11-06): with the
/// ceiling check removed, "a quiet sound peaking at -3 dBTP is raised only to the -1 ceiling: 6.0
/// dB"; and with the volume left out of the ceiling, "at volume 0.25 the same sound has 12 dB more
/// room: 2.0 dB".
#[test]
fn the_gain_levels_down_and_raises_within_both_limits() {
    let db = |g: f64| 20.0 * g.log10();
    let s = |lufs: f64, tp: f64| SoundLoudness { lufs, measure: Measure::MaxMomentary, true_peak_dbtp: tp, seconds: 1.0 };
    assert!((db(levelling_gain(&s(-10.0, -1.0), -20.0, -1.0, 12.0, 1.0)) + 10.0).abs() < 1e-9, "a loud sound comes down 10 dB");
    assert!((db(levelling_gain(&s(-26.0, -20.0), -20.0, -1.0, 12.0, 1.0)) - 6.0).abs() < 1e-9, "a quiet sound goes up 6 dB");
    let g = db(levelling_gain(&s(-26.0, -3.0), -20.0, -1.0, 12.0, 1.0));
    assert!((g - 2.0).abs() < 1e-9, "a quiet sound peaking at -3 dBTP is raised only to the -1 ceiling: {g:.1} dB");
    let g = db(levelling_gain(&s(-26.0, -3.0), -20.0, -1.0, 12.0, 0.25));
    assert!((g - 6.0).abs() < 1e-9, "at volume 0.25 the same sound has 12 dB more room: {g:.1} dB");
    assert!((db(levelling_gain(&s(-50.0, -40.0), -20.0, -1.0, 12.0, 1.0)) - 12.0).abs() < 1e-9, "at most 12 dB up");
    assert_eq!(levelling_gain(&s(-26.0, 0.5), -20.0, -1.0, 12.0, 1.0), 1.0, "already over the ceiling: not raised");
    assert_eq!(levelling_gain(&s(f64::NEG_INFINITY, f64::NEG_INFINITY), -20.0, -1.0, 12.0, 1.0), 1.0, "silence is left alone");
}

/// A sound is known by the same key however its path is spelled.
#[test]
fn a_sound_is_known_by_one_key() {
    for p in ["assets/audio/sfx/crash.ogg", "audio/sfx/crash.ogg", "assets\\audio\\sfx\\crash.ogg", "./assets/audio/sfx/crash.ogg", "C:/game/assets/audio/sfx/crash.ogg"] {
        assert_eq!(sound_key(p), "audio/sfx/crash.ogg", "{p}");
    }
}

/// The measured data round-trips through its text.
#[test]
fn the_measured_data_reads_back_what_it_wrote() {
    let mut m = MeasuredSounds::default();
    m.sounds.insert("audio/a.ogg".into(), SoundLoudness { lufs: -21.25, measure: Measure::MaxMomentary, true_peak_dbtp: -3.5, seconds: 0.42 });
    m.sounds.insert("audio/b.ogg".into(), SoundLoudness { lufs: f64::NEG_INFINITY, measure: Measure::Integrated, true_peak_dbtp: f64::NEG_INFINITY, seconds: 5.0 });
    assert_eq!(MeasuredSounds::parse(&m.to_toml()).unwrap(), m);
}

#[cfg(feature = "native")]
mod shipped {
    use super::*;

    /// Decode a shipped sound and measure it as it plays at `gain` (its samples times the gain).
    fn played_at(key: &str, gain: f64) -> f64 {
        let data = kira::sound::static_sound::StaticSoundData::from_file(repo().join("assets").join(key)).unwrap();
        let mut samples = Vec::with_capacity(data.frames.len() * 2);
        for f in data.frames.iter() {
            samples.push((f.left as f64 * gain) as f32);
            samples.push((f.right as f64 * gain) as f32);
        }
        crate::audio::loudness::measure_sound(data.sample_rate, 2, &samples).lufs
    }

    /// THE DATA IS FRESH: `data/audio/measured_loudness.toml` holds exactly what the shipped sound
    /// files measure now (to its hundredths), so a sound file replaced without `just loudness` fails
    /// here instead of playing at the wrong level. With `HUMANITY_WRITE_LOUDNESS=1` it writes the file
    /// instead (that is what `just loudness` runs). RED first (2026-11-06): with the crash's
    /// loudness edited from -20.79 to -14.00 in the data, "audio/sfx/crash.ogg: the data says -14.00
    /// LUFS, the file measures -20.79 (run `just loudness`)".
    #[test]
    fn the_measured_loudness_data_matches_the_shipped_sounds() {
        let now = crate::audio::measure_shipped(repo()).expect("every shipped sound decodes");
        assert!(now.sounds.len() >= 25, "the shipped sounds are measured: {}", now.sounds.len());
        let file = repo().join("data").join(MEASURED_FILE);
        if std::env::var_os("HUMANITY_WRITE_LOUDNESS").is_some_and(|v| v == "1") {
            crate::atomic_write::write_atomic(&file, now.to_toml().as_bytes()).expect("write the measured loudness");
            println!("wrote {} sounds to {}", now.sounds.len(), file.display());
            return;
        }
        let kept = MeasuredSounds::parse(&std::fs::read_to_string(&file).expect("data/audio/measured_loudness.toml (run `just loudness`)")).unwrap();
        let close = |a: f64, b: f64| (a.is_infinite() && a == b) || (a - b).abs() < 0.006;
        for (key, s) in &now.sounds {
            let k = kept.sounds.get(key).unwrap_or_else(|| panic!("{key}: not in the data (run `just loudness`)"));
            assert!(close(k.lufs, s.lufs), "{key}: the data says {:.2} LUFS, the file measures {:.2} (run `just loudness`)", k.lufs, s.lufs);
            assert!(close(k.true_peak_dbtp, s.true_peak_dbtp), "{key}: the data says {:.2} dBTP, the file measures {:.2} (run `just loudness`)", k.true_peak_dbtp, s.true_peak_dbtp);
            assert_eq!(k.measure, s.measure, "{key}: measured another way (run `just loudness`)");
        }
        for key in kept.sounds.keys() {
            assert!(now.sounds.contains_key(key), "{key}: in the data but no longer shipped (run `just loudness`)");
        }
    }

    /// A QUIET AND A LOUD SHIPPED SOUND PLAY AT ONE LOUDNESS WITH "EVEN OUT LOUDNESS" ON, AND AT
    /// THEIR OWN WITH IT OFF. Of each kind's shipped sounds that share an authored volume, the
    /// quietest and the loudest file (picked from the data, so the test follows the sounds) are
    /// decoded, scaled by exactly what `AudioManager` multiplies them by (`level_for` times the
    /// authored volume) and measured again: on, within 0.2 LU of each other and of the kind's
    /// target plus the volume; off, as far apart as their files. RED first: with `level_for` always
    /// 1.0, "on: audio/ui/inventory_drop.ogg and audio/ui/menu_open.ogg play 5.37 LU apart".
    #[test]
    fn a_quiet_and_a_loud_shipped_sound_play_at_one_loudness_with_even_out_on() {
        let catalog = crate::audio::sounds::SoundCatalog::load(&repo().join("data"));
        let levels = LoudnessLevels::load(&repo().join("data"), catalog.loudness_sources()).expect("the shipped levels load");
        // (kind, authored volume) -> [(key, measured)] for the shipped files.
        let mut groups: BTreeMap<(String, i64), Vec<(String, SoundLoudness)>> = BTreeMap::new();
        for e in catalog.entries().values() {
            let key = sound_key(&e.path);
            if let Some(m) = levels.measured(&key) {
                groups.entry((levels.kind_of(&key).to_string(), (e.volume * 1000.0).round() as i64)).or_default().push((key, *m));
            }
        }
        let mut checked = 0;
        for ((kind, vol_milli), mut files) in groups {
            files.sort_by(|a, b| a.1.lufs.total_cmp(&b.1.lufs));
            let (Some(quiet), Some(loud)) = (files.first().cloned(), files.last().cloned()) else { continue };
            let spread = loud.1.lufs - quiet.1.lufs;
            if spread < 2.0 {
                continue;
            }
            let vol = vol_milli as f64 / 1000.0;
            let target = levels.target_for(&quiet.0);
            // A raise held at the ceiling or the boost limit is not levelled all the way; such a pair
            // would test the limit, which the gain test does.
            let held = |s: &SoundLoudness| target - s.lufs > levels.targets().max_boost_db || target - s.lufs > levels.targets().ceiling_dbtp - (s.true_peak_dbtp + 20.0 * vol.log10());
            if held(&quiet.1) || held(&loud.1) {
                continue;
            }
            let on = |k: &str| level_for(Some(&levels), true, k, vol, true, || None) * vol;
            let off = |k: &str| level_for(Some(&levels), false, k, vol, true, || None) * vol;
            let (q_on, l_on) = (played_at(&quiet.0, on(&quiet.0)), played_at(&loud.0, on(&loud.0)));
            assert!((q_on - l_on).abs() < 0.2, "on: {} and {} play {:.2} LU apart", quiet.0, loud.0, (q_on - l_on).abs());
            let want = target + 20.0 * vol.log10();
            assert!((q_on - want).abs() < 0.2, "on: {} plays at {q_on:.2}, its kind {kind} at volume {vol} is {want:.2}", quiet.0);
            let (q_off, l_off) = (played_at(&quiet.0, off(&quiet.0)), played_at(&loud.0, off(&loud.0)));
            assert!((l_off - q_off - spread).abs() < 0.2, "off: {} and {} play {:.2} LU apart, their files {spread:.2}", quiet.0, loud.0, l_off - q_off);
            println!("{kind} at volume {vol}: {} ({:.2}) and {} ({:.2}): on {q_on:.2} / {l_on:.2}, off {q_off:.2} / {l_off:.2}", quiet.0, quiet.1.lufs, loud.0, loud.1.lufs);
            checked += 1;
        }
        assert!(checked >= 1, "at least one kind has a quiet and a loud shipped sound to compare");
    }

    /// With reuse off the data is not trusted: the sound is measured on the spot (so a stale table
    /// is ruled out with the Dev page's one switch); with it on, the table answers and nothing is
    /// measured. RED first: with `trust_data` ignored, "reuse off: measured on the spot" failed
    /// (the wrong table entry answered).
    #[test]
    fn with_reuse_off_a_sound_is_measured_not_looked_up() {
        let catalog = crate::audio::sounds::SoundCatalog::load(&repo().join("data"));
        let mut levels = LoudnessLevels::load(&repo().join("data"), catalog.loudness_sources()).unwrap();
        let key = levels.measured.sounds.keys().next().cloned().expect("a measured sound");
        let real = *levels.measured(&key).unwrap();
        // The table made wrong by 10 LU: what a stale entry would look like.
        levels.measured.sounds.get_mut(&key).unwrap().lufs = real.lufs - 10.0;
        let mut measured = false;
        let from_table = level_for(Some(&levels), true, &key, 1.0, true, || {
            measured = true;
            Some(real)
        });
        assert!(!measured, "reuse on: the table answers");
        let mut measured = false;
        let on_the_spot = level_for(Some(&levels), true, &key, 1.0, false, || {
            measured = true;
            Some(real)
        });
        assert!(measured, "reuse off: measured on the spot");
        assert!((on_the_spot - levels.gain_for(&key, &real, 1.0)).abs() < 1e-12 && on_the_spot != from_table, "and the measurement, not the table, sets the level");
    }
}
