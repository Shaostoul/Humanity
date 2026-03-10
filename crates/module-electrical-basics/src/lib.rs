use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LoadProfile {
    pub demand_kw: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CircuitGraph {
    pub source_capacity_kw: f32,
    pub breaker_trip_kw: f32,
    pub line_loss_factor: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PowerStep {
    pub delivered_kw: f32,
    pub tripped: bool,
    pub overload_ratio: f32,
}

pub fn simulate_power_step(circuit: CircuitGraph, load: LoadProfile) -> PowerStep {
    let overload_ratio = if circuit.source_capacity_kw <= 0.0 {
        1.0
    } else {
        (load.demand_kw / circuit.source_capacity_kw).max(0.0)
    };

    let tripped = load.demand_kw > circuit.breaker_trip_kw;

    let raw_delivered = if tripped {
        0.0
    } else {
        load.demand_kw.min(circuit.source_capacity_kw)
    };

    let delivered_kw = (raw_delivered * (1.0 - circuit.line_loss_factor.clamp(0.0, 0.6))).max(0.0);

    PowerStep {
        delivered_kw,
        tripped,
        overload_ratio,
    }
}

pub fn fault_report(step: PowerStep) -> &'static str {
    if step.tripped {
        "breaker_trip"
    } else if step.overload_ratio > 0.95 {
        "high_load"
    } else {
        "normal"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overload_trips_breaker() {
        let step = simulate_power_step(
            CircuitGraph {
                source_capacity_kw: 8.0,
                breaker_trip_kw: 9.0,
                line_loss_factor: 0.1,
            },
            LoadProfile { demand_kw: 10.0 },
        );

        assert!(step.tripped);
        assert_eq!(step.delivered_kw, 0.0);
        assert_eq!(fault_report(step), "breaker_trip");
    }

    #[test]
    fn safe_load_delivers_power() {
        let step = simulate_power_step(
            CircuitGraph {
                source_capacity_kw: 8.0,
                breaker_trip_kw: 9.0,
                line_loss_factor: 0.1,
            },
            LoadProfile { demand_kw: 4.0 },
        );

        assert!(!step.tripped);
        assert!(step.delivered_kw > 0.0);
    }
}
