/// State machine for session verification flow.
///
/// Mirrors the Matrix SDK verification state transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerificationState {
    /// Verification request was just created.
    #[default]
    Created,
    /// We sent or received a verification request.
    Requested,
    /// Verification is ready to proceed (both sides agreed).
    Ready,
    /// Request was accepted, transitioning to SAS.
    Accepted,
    /// Waiting for user to confirm SAS emoji/decimal match.
    SasConfirm,
    /// Verification completed successfully.
    Done,
    /// Verification was cancelled.
    Cancelled,
    /// Verification was dismissed by user.
    Dismissed,
}

impl VerificationState {
    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Created => "Verification request created",
            Self::Requested => "Incoming verification request",
            Self::Ready => "Connecting...",
            Self::Accepted => "Accepted, starting verification...",
            Self::SasConfirm => "Compare the emoji below",
            Self::Done => "Verification complete!",
            Self::Cancelled => "Verification was cancelled",
            Self::Dismissed => "Verification was dismissed",
        }
    }
}
