use module_plumbing_basics::{detect_leak, simulate_flow, PlumbingNetwork};

fn main() {
    let flow = simulate_flow(
        PlumbingNetwork {
            source_head: 18.0,
            pipe_efficiency: 0.78,
            leak_factor: 0.12,
        },
        12.0,
    );

    println!("delivered_lpm={:.2}", flow.delivered_lpm);
    println!("leak_lpm={:.2}", flow.leak_lpm);
    println!("contamination_risk={:.2}", flow.contamination_risk);
    println!("leak_detected={}", detect_leak(flow));
}
