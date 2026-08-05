/// MCP-equivalent session data used by `GameConfiguration.UserInformation`.
/// Authentication transport is added with the multiplayer login stage; the
/// field names and accessors already mirror `net.minecraft.util.Session`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    username: String,
    playerID: String,
    token: String,
    sessionType: String,
}

impl Session {
    pub fn new(
        username: impl Into<String>,
        playerID: impl Into<String>,
        token: impl Into<String>,
        sessionType: impl Into<String>,
    ) -> Self {
        Self {
            username: username.into(),
            playerID: playerID.into(),
            token: token.into(),
            sessionType: sessionType.into(),
        }
    }

    pub fn getUsername(&self) -> &str {
        &self.username
    }

    pub fn getPlayerID(&self) -> &str {
        &self.playerID
    }

    pub fn getToken(&self) -> &str {
        &self.token
    }

    pub fn getSessionType(&self) -> &str {
        &self.sessionType
    }

    pub fn getSessionID(&self) -> String {
        format!("token:{}:{}", self.token, self.playerID)
    }

    pub fn getProfile(&self) -> crate::com::mojang::authlib::GameProfile::GameProfile {
        let id = uuid::Uuid::parse_str(&self.playerID).ok();
        crate::com::mojang::authlib::GameProfile::GameProfile::new(id, self.username.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_matches_mcp_format() {
        let session = Session::new("Player", "uuid", "access-token", "legacy");
        assert_eq!(session.getSessionID(), "token:access-token:uuid");
    }
}
