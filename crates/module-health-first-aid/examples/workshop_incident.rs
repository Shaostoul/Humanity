use module_health_first_aid::{
    apply_first_aid, recovery_projection, triage_level, HealthIncident, ResponseProfile,
};

fn main() {
    let incident = HealthIncident {
        severity: 72.0,
        elapsed_minutes: 18.0,
    };

    let triage = triage_level(incident);
    let projection = apply_first_aid(
        incident,
        ResponseProfile {
            responder_skill: 68.0,
            resource_readiness: 70.0,
        },
    );

    println!("triage={triage:?}");
    println!("stabilization_score={:.1}", projection.stabilization_score);
    println!("complication_risk={:.2}", projection.complication_risk);
    println!("status={}", recovery_projection(projection));
}
