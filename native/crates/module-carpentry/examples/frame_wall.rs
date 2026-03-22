use module_carpentry::{
    execute_step, CarpentryTask, JoinType, ToolCondition, WorkerProfile,
};

fn main() {
    let report = execute_step(
        CarpentryTask {
            complexity: 65.0,
            target_tolerance_mm: 1.2,
            join: JoinType::MortiseTenon,
        },
        WorkerProfile {
            skill: 72.0,
            fatigue: 25.0,
        },
        ToolCondition {
            sharpness: 78.0,
            calibration: 80.0,
        },
    );

    println!("achieved_tolerance_mm={:.2}", report.achieved_tolerance_mm);
    println!("quality_score={:.2}", report.quality_score);
    println!("defect_risk={:.2}", report.defect_risk);
}
