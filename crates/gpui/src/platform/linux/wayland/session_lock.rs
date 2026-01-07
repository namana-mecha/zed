use thiserror::Error;

/// Options for creating a session-lock window.
///
/// Session lock is used to create lock screens that take exclusive access to all outputs,
/// preventing access to the rest of the desktop until the session is unlocked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionLockOptions {
    /// Optional namespace for the lock surface, can be used by compositors to apply special rules
    pub namespace: Option<String>,
}

/// An error indicating that an action failed because the compositor doesn't support the required
/// session-lock protocol.
#[derive(Debug, Error)]
#[error("Compositor doesn't support ext_session_lock_v1")]
pub struct SessionLockNotSupportedError;
