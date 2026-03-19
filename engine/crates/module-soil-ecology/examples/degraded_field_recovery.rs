use module_soil_ecology::{apply_amendment, simulate_season, SeasonInput, SoilCell, SoilTexture};

fn main() {
    let mut cell = SoilCell {
        texture: SoilTexture::Clay,
        moisture: 30.0,
        nutrient_index: 22.0,
        compaction: 70.0,
        biology: 18.0,
    };

    apply_amendment(&mut cell, 30.0);

    let trend_1 = simulate_season(
        &mut cell,
        SeasonInput {
            rainfall: 60.0,
            heat: 50.0,
            tillage_intensity: 30.0,
            amendment_boost: 20.0,
        },
    )
    .expect("season 1");

    let trend_2 = simulate_season(
        &mut cell,
        SeasonInput {
            rainfall: 55.0,
            heat: 45.0,
            tillage_intensity: 15.0,
            amendment_boost: 25.0,
        },
    )
    .expect("season 2");

    println!("nutrient_index={:.2}", cell.nutrient_index);
    println!("biology={:.2}", cell.biology);
    println!("compaction={:.2}", cell.compaction);
    println!("erosion_risk_s1={:.2}", trend_1.erosion_risk);
    println!("erosion_risk_s2={:.2}", trend_2.erosion_risk);
}
