use module_crop_systems::{
    apply_intervention, harvest_report, tick_growth, CropInstance, EnvironmentInput, GrowthStage,
    Intervention,
};

fn run_plot(name: &str, mut crop: CropInstance, env: EnvironmentInput) {
    for day in 0..45 {
        tick_growth(&mut crop, env).expect("tick");
        if day % 10 == 0 {
            apply_intervention(
                &mut crop,
                Intervention {
                    irrigation_boost: 20.0,
                    nutrient_boost: 15.0,
                    pest_control: 10.0,
                },
            )
            .expect("intervention");
        }
        if crop.stage == GrowthStage::Harvestable {
            break;
        }
    }

    println!("[{name}] stage={:?} vitality={:.1} stress={:.1}", crop.stage, crop.vitality, crop.stress);
    if let Ok(report) = harvest_report(&crop) {
        println!("[{name}] yield={:.1} quality={:.1}", report.yield_score, report.quality_score);
    }
}

fn main() {
    let base = CropInstance {
        stage: GrowthStage::Seed,
        vitality: 60.0,
        stress: 15.0,
        growth_progress: 0.0,
    };

    run_plot(
        "Plot-A",
        base.clone(),
        EnvironmentInput {
            moisture: 80.0,
            nutrient_index: 70.0,
            temperature_suitability: 78.0,
            pollination_support: 70.0,
        },
    );

    run_plot(
        "Plot-B",
        base.clone(),
        EnvironmentInput {
            moisture: 55.0,
            nutrient_index: 65.0,
            temperature_suitability: 60.0,
            pollination_support: 40.0,
        },
    );

    run_plot(
        "Plot-C",
        base,
        EnvironmentInput {
            moisture: 30.0,
            nutrient_index: 35.0,
            temperature_suitability: 45.0,
            pollination_support: 25.0,
        },
    );
}
