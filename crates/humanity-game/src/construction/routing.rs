//! Auto-routing for pipes, wiring, and ventilation through structures.
//!
//! Routing rules loaded from `config/routing_rules.ron`.

/// Route type for infrastructure lines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RouteType {
    Pipe,
    Wire,
    Ventilation,
}

/// Auto-routes infrastructure through a structure.
pub struct AutoRouter;

impl AutoRouter {
    pub fn new() -> Self {
        Self
    }
}
