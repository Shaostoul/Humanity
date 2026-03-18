//! Package tracking and route planning.

use serde::{Deserialize, Serialize};

/// Status of a shipment.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ShipmentStatus {
    Queued,
    InTransit,
    Delivered,
    Lost,
}

/// A shipment in the logistics network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shipment {
    pub id: u64,
    pub origin: String,
    pub destination: String,
    pub status: ShipmentStatus,
}
