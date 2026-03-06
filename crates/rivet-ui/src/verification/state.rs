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
    /// Showing QR code for the other device to scan.
    QrScan,
    /// Our QR code was successfully scanned.
    QrScanned,
    /// Waiting for user to confirm scanned QR code.
    QrConfirm,
    /// Verification completed successfully.
    Done,
    /// Verification was cancelled.
    Cancelled,
    /// Verification was dismissed by user.
    Dismissed,
    /// The verification DM room was left.
    RoomLeft,
    /// An error occurred during verification.
    Error,
}

impl VerificationState {
    /// Check if verification is in a final state.
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            Self::Done | Self::Cancelled | Self::Dismissed | Self::RoomLeft | Self::Error
        )
    }

    /// Check if we're waiting for user action.
    pub fn needs_user_action(&self) -> bool {
        matches!(self, Self::Requested | Self::SasConfirm | Self::QrConfirm)
    }

    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Created => "Verification request created",
            Self::Requested => "Incoming verification request",
            Self::Ready => "Connecting...",
            Self::Accepted => "Accepted, starting verification...",
            Self::SasConfirm => "Compare the emoji below",
            Self::QrScan => "Scan this QR code with your other device",
            Self::QrScanned => "QR code scanned",
            Self::QrConfirm => "Confirm the QR code scan",
            Self::Done => "Verification complete!",
            Self::Cancelled => "Verification was cancelled",
            Self::Dismissed => "Verification was dismissed",
            Self::RoomLeft => "The verification room was left",
            Self::Error => "Verification failed",
        }
    }
}
