use crate::timeline::ChatStyle;

#[derive(Debug, Clone)]
pub enum SettingsEvent {
    Close,
    Logout,
    DeleteAllData,
    VerifySession,
    RecoverWithKey(String),
    SetChatStyle(ChatStyle),
    SetShowRoomsInHome(bool),
    SetShowSidecart(bool),
    SetShowOtherRooms(bool),
    SetRememberLastRoom(bool),
}
