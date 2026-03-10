use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlumbingNetwork {
    pub source_head: f32,
    pub pipe_efficiency: f32,
    pub leak_factor: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FlowState {
    pub delivered_lpm: f32,
    pub leak_lpm: f32,
    pub contamination_risk: f32,
}

pub fn simulate_flow(network: PlumbingNetwork, demand_lpm: f32) -> FlowState {
    let available = (network.source_head * network.pipe_efficiency.clamp(0.0, 1.0)).max(0.0);
    let delivered_lpm = demand_lpm.min(available).max(0.0);
    let leak_lpm = delivered_lpm * network.leak_factor.clamp(0.0, 1.0);

    let contamination_risk = (network.leak_factor * 0.8 + (1.0 - network.pipe_efficiency).max(0.0) * 0.5)
        .clamp(0.0, 1.0);

    FlowState {
        delivered_lpm: (delivered_lpm - leak_lpm).max(0.0),
        leak_lpm,
        contamination_risk,
    }
}

pub fn detect_leak(flow: FlowState) -> bool {
    flow.leak_lpm > 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn higher_leak_factor_reduces_delivery() {
        let low = simulate_flow(
            PlumbingNetwork {
                source_head: 30.0,
                pipe_efficiency: 0.9,
                leak_factor: 0.05,
            },
            20.0,
        );

        let high = simulate_flow(
            PlumbingNetwork {
                source_head: 30.0,
                pipe_efficiency: 0.9,
                leak_factor: 0.3,
            },
            20.0,
        );

        assert!(high.delivered_lpm < low.delivered_lpm);
        assert!(detect_leak(high));
    }

    #[test]
    fn contamination_risk_increases_with_poor_network() {
        let good = simulate_flow(
            PlumbingNetwork {
                source_head: 25.0,
                pipe_efficiency: 0.95,
                leak_factor: 0.02,
            },
            10.0,
        );

        let poor = simulate_flow(
            PlumbingNetwork {
                source_head: 25.0,
                pipe_efficiency: 0.6,
                leak_factor: 0.2,
            },
            10.0,
        );

        assert!(poor.contamination_risk > good.contamination_risk);
    }
}
