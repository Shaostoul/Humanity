use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriageLevel {
    Immediate,
    Urgent,
    Delayed,
    Minor,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HealthIncident {
    /// 0..100
    pub severity: f32,
    /// minutes since incident
    pub elapsed_minutes: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ResponseProfile {
    /// 0..100
    pub responder_skill: f32,
    /// 0..100
    pub resource_readiness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RecoveryProjection {
    pub stabilization_score: f32,
    pub complication_risk: f32,
}

pub fn triage_level(incident: HealthIncident) -> TriageLevel {
    match incident.severity {
        s if s >= 85.0 => TriageLevel::Immediate,
        s if s >= 65.0 => TriageLevel::Urgent,
        s if s >= 35.0 => TriageLevel::Delayed,
        _ => TriageLevel::Minor,
    }
}

pub fn apply_first_aid(incident: HealthIncident, response: ResponseProfile) -> RecoveryProjection {
    let time_penalty = (incident.elapsed_minutes / 120.0).clamp(0.0, 1.0) * 35.0;
    let responder_effect = response.responder_skill * 0.45 + response.resource_readiness * 0.35;

    let stabilization_score = (responder_effect - incident.severity * 0.3 - time_penalty).clamp(0.0, 100.0);

    let complication_risk = ((incident.severity * 0.6 + time_penalty - responder_effect * 0.5) / 100.0)
        .clamp(0.0, 1.0);

    RecoveryProjection {
        stabilization_score,
        complication_risk,
    }
}

pub fn recovery_projection(stabilization: RecoveryProjection) -> &'static str {
    if stabilization.stabilization_score >= 70.0 && stabilization.complication_risk < 0.3 {
        "stable"
    } else if stabilization.stabilization_score >= 40.0 {
        "watch"
    } else {
        "critical"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn higher_severity_means_higher_triage_priority() {
        assert_eq!(
            triage_level(HealthIncident {
                severity: 90.0,
                elapsed_minutes: 5.0,
            }),
            TriageLevel::Immediate
        );
        assert_eq!(
            triage_level(HealthIncident {
                severity: 40.0,
                elapsed_minutes: 10.0,
            }),
            TriageLevel::Delayed
        );
    }

    #[test]
    fn faster_better_response_improves_projection() {
        let poor = apply_first_aid(
            HealthIncident {
                severity: 70.0,
                elapsed_minutes: 90.0,
            },
            ResponseProfile {
                responder_skill: 30.0,
                resource_readiness: 25.0,
            },
        );

        let good = apply_first_aid(
            HealthIncident {
                severity: 70.0,
                elapsed_minutes: 15.0,
            },
            ResponseProfile {
                responder_skill: 80.0,
                resource_readiness: 75.0,
            },
        );

        assert!(good.stabilization_score > poor.stabilization_score);
        assert!(good.complication_risk < poor.complication_risk);
    }
}
