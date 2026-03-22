use core_lifeform_model::{LifeformState, LifeformTick, TickInput};

fn main() {
    let mut human = LifeformState::baseline_human();

    // Simulate a hard 12-hour period with low intake.
    let outcome = human
        .tick(TickInput {
            elapsed_hours: 12,
            environment_stress_multiplier: 1.6,
            food_intake_units: 0.6,
            water_intake_units: 0.4,
        })
        .expect("tick should succeed");

    println!("age_hours={}", human.age_hours);
    println!("capability_snapshot={:.2}", human.capability_snapshot());
    println!("incidents={:?}", outcome.incidents);

    // NOTE: livestock/crop couplings are planned for module integration workstreams.
}
