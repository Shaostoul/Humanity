use module_electrical_basics::{fault_report, simulate_power_step, CircuitGraph, LoadProfile};

fn main() {
    let step = simulate_power_step(
        CircuitGraph {
            source_capacity_kw: 12.0,
            breaker_trip_kw: 14.0,
            line_loss_factor: 0.08,
        },
        LoadProfile { demand_kw: 10.5 },
    );

    println!("delivered_kw={:.2}", step.delivered_kw);
    println!("tripped={}", step.tripped);
    println!("fault={}", fault_report(step));
}
