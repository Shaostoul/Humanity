use core_offline_loop::WorldSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotHeader {
    pub tick: u64,
    pub state_hash_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryAction {
    InSync,
    SendDeltasFrom(u64),
    SendNearestSnapshotThenDeltas { snapshot_tick: u64 },
    FullResync,
}

pub fn snapshot_hash(world: &WorldSnapshot) -> String {
    let json = serde_json::to_vec(world).unwrap_or_default();
    blake3::hash(&json).to_hex().to_string()
}

pub fn header_for(world: &WorldSnapshot) -> SnapshotHeader {
    SnapshotHeader {
        tick: world.tick,
        state_hash_hex: snapshot_hash(world),
    }
}

pub fn pick_recovery_action(
    client_last: &SnapshotHeader,
    host_current: &SnapshotHeader,
    host_history_ticks_desc: &[u64],
) -> RecoveryAction {
    if client_last.tick == host_current.tick && client_last.state_hash_hex == host_current.state_hash_hex {
        return RecoveryAction::InSync;
    }

    if host_history_ticks_desc.contains(&client_last.tick) {
        return RecoveryAction::SendDeltasFrom(client_last.tick);
    }

    // choose nearest older snapshot retained by host
    if let Some(tick) = host_history_ticks_desc
        .iter()
        .copied()
        .find(|t| *t < client_last.tick)
    {
        return RecoveryAction::SendNearestSnapshotThenDeltas { snapshot_tick: tick };
    }

    RecoveryAction::FullResync
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_headers_are_in_sync() {
        let mut world = WorldSnapshot::new_default();
        world.tick = 10;
        let h = header_for(&world);

        let action = pick_recovery_action(&h, &h, &[10, 8, 6]);
        assert_eq!(action, RecoveryAction::InSync);
    }

    #[test]
    fn can_recover_with_deltas_when_tick_retained() {
        let mut a = WorldSnapshot::new_default();
        a.tick = 5;
        let client = header_for(&a);

        let mut b = WorldSnapshot::new_default();
        b.tick = 12;
        let host = header_for(&b);

        let action = pick_recovery_action(&client, &host, &[12, 10, 8, 5]);
        assert_eq!(action, RecoveryAction::SendDeltasFrom(5));
    }

    #[test]
    fn full_resync_when_no_usable_history() {
        let mut a = WorldSnapshot::new_default();
        a.tick = 2;
        let client = header_for(&a);

        let mut b = WorldSnapshot::new_default();
        b.tick = 20;
        let host = header_for(&b);

        let action = pick_recovery_action(&client, &host, &[20, 18, 17]);
        assert_eq!(action, RecoveryAction::FullResync);
    }
}
