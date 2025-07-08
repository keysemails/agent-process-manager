//! Agent context management for working directory-based access control

use std::path::Path;

/// Agent context that holds the working directory-based access group
#[derive(Debug, Clone)]
pub struct AgentContext {
    pub agent_id: String,
    pub access_groups: Vec<String>,
    pub is_superuser: bool,
}

impl AgentContext {
    /// Create a new agent context from a working directory
    pub fn from_working_dir(working_dir: &Path) -> Self {
        let access_group = crate::utils::access_group_from_dir(working_dir);
        Self {
            agent_id: access_group.clone(),
            access_groups: vec![access_group],
            is_superuser: true, // In trusted mode, everyone is superuser
        }
    }
    
    /// Create a superuser context that can access all processes
    pub fn superuser() -> Self {
        Self {
            agent_id: "superuser".to_string(),
            access_groups: vec![],
            is_superuser: true,
        }
    }
    
    /// Check if this context can access a process with the given access group
    pub fn can_access(&self, process_access_group: &Option<String>) -> bool {
        // Superusers can access everything
        if self.is_superuser && self.access_groups.is_empty() {
            return true;
        }
        
        // If process has no access group, it's globally accessible
        let Some(process_group) = process_access_group else {
            return true;
        };
        
        // Check if any of our access groups match
        self.access_groups.contains(process_group)
    }
}