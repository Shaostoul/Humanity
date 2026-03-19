use module_water_systems::{
    risk_report, route_water, treat_water, DemandProfile, Potability, TreatmentStep, WaterNode,
    WaterQuality, WaterSourceKind,
};

fn main() {
    let mut well = WaterNode {
        source_kind: WaterSourceKind::Well,
        liters: 80.0,
        quality: WaterQuality {
            contamination_index: 10.0,
            potability: Potability::Potable,
        },
    };

    let mut tank = WaterNode {
        source_kind: WaterSourceKind::Stored,
        liters: 25.0,
        quality: WaterQuality {
            contamination_index: 35.0,
            potability: Potability::NonPotable,
        },
    };

    route_water(&mut well, &mut tank, 20.0).expect("routing");
    treat_water(&mut tank, TreatmentStep { efficacy: 0.5 }).expect("treatment");

    let report = risk_report(
        tank.liters,
        &tank.quality,
        DemandProfile {
            liters_humans: 18.0,
            liters_livestock: 22.0,
            liters_irrigation: 30.0,
        },
    );

    println!("tank_liters={:.1}", tank.liters);
    println!("contamination_index={:.1}", tank.quality.contamination_index);
    println!("potability={:?}", tank.quality.potability);
    println!("shortage_risk={:.2}", report.shortage_risk);
    println!("contamination_risk={:.2}", report.contamination_risk);
}
