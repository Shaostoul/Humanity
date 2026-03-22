use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JoinType {
    Butt,
    Lap,
    MortiseTenon,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CarpentryTask {
    pub complexity: f32,
    pub target_tolerance_mm: f32,
    pub join: JoinType,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorkerProfile {
    /// 0..100
    pub skill: f32,
    /// 0..100 (higher is more fatigued)
    pub fatigue: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ToolCondition {
    /// 0..100
    pub sharpness: f32,
    /// 0..100
    pub calibration: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QualityReport {
    pub achieved_tolerance_mm: f32,
    pub quality_score: f32,
    pub defect_risk: f32,
}

pub fn execute_step(task: CarpentryTask, worker: WorkerProfile, tools: ToolCondition) -> QualityReport {
    let join_penalty = match task.join {
        JoinType::Butt => 0.9,
        JoinType::Lap => 1.0,
        JoinType::MortiseTenon => 1.2,
    };

    let capability = (worker.skill * 0.5 + tools.sharpness * 0.25 + tools.calibration * 0.25)
        - worker.fatigue * 0.35;

    let capability = capability.clamp(0.0, 100.0);
    let variance = (100.0 - capability) * 0.03 * join_penalty * (task.complexity / 100.0).max(0.2);
    let achieved_tolerance_mm = (task.target_tolerance_mm + variance).max(0.1);

    let quality_score = (capability - task.complexity * 0.3).clamp(0.0, 100.0);
    let defect_risk = ((task.complexity * 0.6 + worker.fatigue * 0.5 - capability * 0.4) / 100.0)
        .clamp(0.0, 1.0);

    QualityReport {
        achieved_tolerance_mm,
        quality_score,
        defect_risk,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn better_worker_has_better_quality() {
        let task = CarpentryTask {
            complexity: 55.0,
            target_tolerance_mm: 1.5,
            join: JoinType::Lap,
        };

        let novice = execute_step(
            task,
            WorkerProfile {
                skill: 30.0,
                fatigue: 40.0,
            },
            ToolCondition {
                sharpness: 45.0,
                calibration: 50.0,
            },
        );

        let expert = execute_step(
            task,
            WorkerProfile {
                skill: 85.0,
                fatigue: 15.0,
            },
            ToolCondition {
                sharpness: 80.0,
                calibration: 85.0,
            },
        );

        assert!(expert.quality_score > novice.quality_score);
        assert!(expert.defect_risk < novice.defect_risk);
    }
}
