use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionMode {
    Offline,
    HostP2p,
    JoinP2p,
    Dedicated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressionPolicy {
    OpenProfile,
    ClosedProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkScope {
    Offline,
    Lan,
    DirectInternet,
    Tailnet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FidelityPreset {
    BabyCreative,
    Easy,
    Medium,
    Hard,
    Realistic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionConfig {
    pub mode: SessionMode,
    pub policy: ProgressionPolicy,
    pub network: NetworkScope,
    pub fidelity: FidelityPreset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionReason {
    UserRequested,
    HostMigration,
    ServerHandoff,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransitionError {
    #[error("offline mode must use offline network scope")]
    OfflineNetworkMismatch,
    #[error("join mode cannot use offline network scope")]
    JoinOfflineMismatch,
    #[error("closed profile requires host or dedicated authority")]
    ClosedProfileAuthorityMismatch,
    #[error("invalid transition from {from:?} to {to:?}")]
    InvalidTransition { from: SessionMode, to: SessionMode },
}

pub fn validate_config(cfg: &SessionConfig) -> Result<(), TransitionError> {
    if cfg.mode == SessionMode::Offline && cfg.network != NetworkScope::Offline {
        return Err(TransitionError::OfflineNetworkMismatch);
    }

    if cfg.mode == SessionMode::JoinP2p && cfg.network == NetworkScope::Offline {
        return Err(TransitionError::JoinOfflineMismatch);
    }

    if cfg.policy == ProgressionPolicy::ClosedProfile && cfg.mode == SessionMode::Offline {
        return Err(TransitionError::ClosedProfileAuthorityMismatch);
    }

    Ok(())
}

pub fn can_transition(from: SessionMode, to: SessionMode, _reason: TransitionReason) -> Result<(), TransitionError> {
    match (from, to) {
        (a, b) if a == b => Ok(()),
        (SessionMode::Offline, SessionMode::HostP2p) => Ok(()),
        (SessionMode::HostP2p, SessionMode::Dedicated) => Ok(()),
        (SessionMode::JoinP2p, SessionMode::Offline) => Ok(()),
        (SessionMode::Dedicated, SessionMode::Offline) => Ok(()),
        (SessionMode::Dedicated, SessionMode::HostP2p) => Ok(()),
        _ => Err(TransitionError::InvalidTransition { from, to }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_requires_offline_network() {
        let cfg = SessionConfig {
            mode: SessionMode::Offline,
            policy: ProgressionPolicy::OpenProfile,
            network: NetworkScope::Lan,
            fidelity: FidelityPreset::Easy,
        };

        assert_eq!(
            validate_config(&cfg).expect_err("expected mismatch"),
            TransitionError::OfflineNetworkMismatch
        );
    }

    #[test]
    fn closed_profile_rejects_offline_authority() {
        let cfg = SessionConfig {
            mode: SessionMode::Offline,
            policy: ProgressionPolicy::ClosedProfile,
            network: NetworkScope::Offline,
            fidelity: FidelityPreset::Hard,
        };

        assert_eq!(
            validate_config(&cfg).expect_err("expected mismatch"),
            TransitionError::ClosedProfileAuthorityMismatch
        );
    }

    #[test]
    fn allows_offline_to_host_transition() {
        can_transition(
            SessionMode::Offline,
            SessionMode::HostP2p,
            TransitionReason::UserRequested,
        )
        .expect("should allow");
    }
}
