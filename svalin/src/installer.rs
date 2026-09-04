mod agent;

#[cfg(target_os = "windows")]
pub use agent::{WINDOWS_SERVICE_NAME, WINDOWS_UPDATE_SERVICE_NAME, update_from_update_service};
pub use agent::{
    cleanup_old_installations, install_agent, uninstall_agent, update::prepare_update_agent,
    update::update_agent,
};
