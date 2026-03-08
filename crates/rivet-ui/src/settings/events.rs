use crate::timeline::ChatStyle;

#[derive(Debug, Clone)]
pub enum SettingsEvent {
    Close,
    Logout,
    VerifySession,
    RecoverWithKey(String),
    SetChatStyle(ChatStyle),
    SetShowRoomsInHome(bool),
}
