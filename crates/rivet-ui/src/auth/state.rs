use rivet_core::client::RivetClient;

pub enum LoginEvent {
    Success(RivetClient),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoginFlowStep {
    #[default]
    Welcome,
    SsoInProgress,
    PasswordForm,
}
