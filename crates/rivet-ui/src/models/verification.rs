use crate::verification::VerificationState;
use gpui::*;
use matrix_sdk::encryption::verification::Emoji;
use matrix_sdk::encryption::verification::{SasVerification, VerificationRequest};

/// Event emitted by VerificationModel when the verification flow changes.
#[derive(Debug, Clone)]
pub enum VerificationEvent {
    /// Verification completed, cancelled, or dismissed — caller should clear the overlay.
    Finished,
}

/// Coordinates the Matrix SAS verification flow between the SDK and the UI.
///
/// Holds the `VerificationRequest` and/or `SasVerification` handles,
/// tracks state transitions, and exposes data the UI needs to render.
pub struct VerificationModel {
    request: Option<VerificationRequest>,
    sas: Option<SasVerification>,
    state: VerificationState,
    is_self_verification: bool,
    other_user_id: Option<String>,
    other_device_name: Option<String>,
    cancel_reason: Option<String>,
    /// Cached emoji from the SDK (symbol + description).
    emojis: Option<[Emoji; 7]>,
}

impl VerificationModel {
    pub fn new() -> Self {
        Self {
            request: None,
            sas: None,
            state: VerificationState::Created,
            is_self_verification: false,
            other_user_id: None,
            other_device_name: None,
            cancel_reason: None,
            emojis: None,
        }
    }

    // -- Accessors --

    pub fn state(&self) -> VerificationState {
        self.state
    }

    pub fn is_self_verification(&self) -> bool {
        self.is_self_verification
    }

    pub fn other_user_id(&self) -> Option<&str> {
        self.other_user_id.as_deref()
    }

    pub fn other_device_name(&self) -> Option<&str> {
        self.other_device_name.as_deref()
    }

    pub fn cancel_reason(&self) -> Option<&str> {
        self.cancel_reason.as_deref()
    }

    /// Returns the cached emoji array (symbol + description) if available.
    pub fn emojis(&self) -> Option<&[Emoji; 7]> {
        self.emojis.as_ref()
    }

    /// Get a clone of the verification request (for listeners in main.rs).
    pub fn request(&self) -> Option<&VerificationRequest> {
        self.request.as_ref()
    }

    /// Get a clone of the SAS verification (for listeners in main.rs).
    pub fn sas(&self) -> Option<&SasVerification> {
        self.sas.as_ref()
    }

    // -- Lifecycle: set request / sas --

    /// Set an incoming or outgoing VerificationRequest and begin tracking it.
    pub fn set_request(&mut self, request: VerificationRequest, cx: &mut Context<Self>) {
        self.is_self_verification = request.is_self_verification();
        self.other_user_id = Some(request.other_user_id().to_string());
        self.request = Some(request.clone());
        self.state = VerificationState::Requested;
        cx.notify();

        // Background task
        let request_clone = request.clone();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            use futures::StreamExt;
            use matrix_sdk::encryption::verification::VerificationRequestState;

            let mut stream = request_clone.changes();
            while let Some(state) = stream.next().await {
                let state = state.clone();
                let res = this.update(cx, |model, cx| match state {
                    VerificationRequestState::Ready { .. } => {
                        tracing::info!("Verification request is READY");
                        model.state = VerificationState::Ready;
                        cx.notify();
                    }
                    VerificationRequestState::Transitioned { verification } => {
                        tracing::info!("Verification request TRANSITIONED");
                        if let matrix_sdk::encryption::verification::Verification::SasV1(sas) =
                            verification
                        {
                            model.set_sas(sas, cx);
                        }
                    }
                    VerificationRequestState::Done => {
                        tracing::info!("Verification request DONE");
                        model.set_done(cx);
                    }
                    VerificationRequestState::Cancelled(info) => {
                        tracing::info!("Verification request CANCELLED: {:?}", info.cancel_code());
                        model.set_cancelled(format!("Cancelled: {:?}", info.cancel_code()), cx);
                    }
                    VerificationRequestState::Created { .. } => {
                        tracing::info!("Verification request CREATED");
                        model.state = VerificationState::Created;
                        cx.notify();
                    }
                    VerificationRequestState::Requested { .. } => {
                        tracing::info!("Verification request REQUESTED");
                        model.state = VerificationState::Requested;
                        cx.notify();
                    }
                });

                if res.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Set a SasVerification directly (e.g. when transitioning from a request).
    pub fn set_sas(&mut self, sas: SasVerification, cx: &mut Context<Self>) {
        self.is_self_verification = sas.is_self_verification();
        self.other_user_id = Some(sas.other_user_id().to_string());
        self.other_device_name = Some(sas.other_device().device_id().to_string());

        // Grab emoji if already available
        if let Some(emojis) = sas.emoji() {
            self.emojis = Some(emojis);
            self.state = VerificationState::SasConfirm;
        } else {
            self.state = VerificationState::Ready;
        }

        self.sas = Some(sas.clone());
        cx.notify();

        // Background task
        let sas_clone = sas.clone();
        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            use futures::StreamExt;
            use matrix_sdk::encryption::verification::SasState;

            let mut stream = sas_clone.changes();
            while let Some(state) = stream.next().await {
                let state = state.clone();
                let res = this.update(cx, |model: &mut VerificationModel, cx| match state {
                    SasState::KeysExchanged { emojis, .. } => {
                        if let Some(e) = emojis {
                            model.set_emojis(e.emojis, cx);
                        }
                    }
                    SasState::Done { .. } => model.set_done(cx),
                    SasState::Cancelled(info) => {
                        model.set_cancelled(format!("Cancelled: {:?}", info.cancel_code()), cx)
                    }
                    _ => {}
                });

                if res.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Update state to show emoji grid.
    pub fn set_emojis(&mut self, emojis: [Emoji; 7], cx: &mut Context<Self>) {
        self.emojis = Some(emojis);
        self.state = VerificationState::SasConfirm;
        cx.notify();
    }

    /// Mark as done.
    pub fn set_done(&mut self, cx: &mut Context<Self>) {
        self.state = VerificationState::Done;
        cx.notify();
    }

    /// Mark as cancelled with a reason.
    pub fn set_cancelled(&mut self, reason: String, cx: &mut Context<Self>) {
        self.state = VerificationState::Cancelled;
        self.cancel_reason = Some(reason);
        cx.notify();
    }

    /// Mark as error.
    pub fn set_error(&mut self, reason: String, cx: &mut Context<Self>) {
        self.state = VerificationState::Error;
        self.cancel_reason = Some(reason);
        cx.notify();
    }

    /// Transition to accepted/ready state.
    pub fn set_ready(&mut self, cx: &mut Context<Self>) {
        self.state = VerificationState::Ready;
        cx.notify();
    }

    // -- Actions (fire-and-forget SDK calls via tokio::spawn) --

    /// Accept the request and start SAS.
    pub fn accept_and_start_sas(&mut self, cx: &mut Context<Self>) {
        if let Some(request) = self.request.clone() {
            tracing::info!("Accepting verification request");
            self.state = VerificationState::Accepted;
            cx.notify();

            tokio::spawn(async move {
                tracing::info!("Calling request.accept()");
                if let Err(e) = request.accept().await {
                    tracing::error!("Failed to accept verification request: {:?}", e);
                }
            });
        }
    }

    /// Start SAS verification (transition from Ready to SAS).
    pub fn start_sas(&mut self, cx: &mut Context<Self>) {
        if let Some(request) = self.request.clone() {
            tracing::info!("Starting SAS verification from Ready state");
            self.state = VerificationState::Accepted;
            cx.notify();

            tokio::spawn(async move {
                tracing::info!("Calling request.start_sas()");
                match request.start_sas().await {
                    Ok(Some(_sas)) => {
                        tracing::info!("SAS verification started successfully");
                        // The model will be updated via the request.changes() stream
                        // but we can also set it directly if we want immediate feedback
                    }
                    Ok(None) => {
                        tracing::warn!("request.start_sas() returned None");
                    }
                    Err(e) => {
                        tracing::error!("Failed to start SAS verification: {:?}", e);
                    }
                }
            });
        }
    }

    /// Confirm that the emoji match (fire-and-forget).
    pub fn confirm(&mut self, _cx: &mut Context<Self>) {
        if let Some(sas) = &self.sas {
            let sas = sas.clone();
            tokio::spawn(async move {
                if let Err(e) = sas.confirm().await {
                    tracing::error!("Failed to confirm SAS: {:?}", e);
                }
            });
        }
    }

    /// Cancel because emoji did not match (fire-and-forget).
    pub fn mismatch(&mut self, cx: &mut Context<Self>) {
        if let Some(sas) = &self.sas {
            let sas = sas.clone();
            self.state = VerificationState::Cancelled;
            self.cancel_reason = Some("Emoji did not match".to_string());
            cx.notify();
            tokio::spawn(async move {
                if let Err(e) = sas.mismatch().await {
                    tracing::error!("Failed to send mismatch cancel: {:?}", e);
                }
            });
        }
    }

    /// Cancel the verification flow entirely (fire-and-forget).
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        tracing::info!("Cancelling verification");
        self.state = VerificationState::Cancelled;
        self.cancel_reason = Some("Cancelled by user".to_string());
        cx.notify();

        if let Some(sas) = self.sas.clone() {
            tokio::spawn(async move {
                tracing::info!("Calling sas.cancel()");
                if let Err(e) = sas.cancel().await {
                    tracing::error!("Failed to cancel SAS: {:?}", e);
                }
            });
        } else if let Some(request) = self.request.clone() {
            tokio::spawn(async move {
                tracing::info!("Calling request.cancel()");
                if let Err(e) = request.cancel().await {
                    tracing::error!("Failed to cancel verification request: {:?}", e);
                }
            });
        }
    }

    /// Dismiss the verification overlay (after Done or Cancelled).
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.state = VerificationState::Dismissed;
        cx.notify();
        cx.emit(VerificationEvent::Finished);
    }
}

impl EventEmitter<VerificationEvent> for VerificationModel {}
