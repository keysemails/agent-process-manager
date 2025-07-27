//! Process tree utilities for finding child processes

use sysinfo::{System, Pid, Process};
use std::collections::HashMap;
use std::ffi::OsString;
use tracing::{debug, trace};

/// Information about a process in the tree
#[derive(Debug, Clone)]
pub struct ProcessNode {
    pub pid: u32,
    pub name: String,
    pub cmd: Vec<String>,
    pub parent_pid: Option<u32>,
    pub children: Vec<u32>,
}

/// Build a process tree starting from a given PID
pub fn build_process_tree(system: &System, root_pid: u32) -> Option<ProcessNode> {
    let pid = Pid::from(root_pid as usize);
    let process = system.process(pid)?;
    
    let mut node = ProcessNode {
        pid: root_pid,
        name: process.name().to_str().unwrap_or("unknown").to_string(),
        cmd: process.cmd().iter().map(|s| s.to_string_lossy().to_string()).collect(),
        parent_pid: process.parent().map(|p| p.as_u32()),
        children: Vec::new(),
    };
    
    // Find all children
    for (_pid, proc) in system.processes() {
        if let Some(parent) = proc.parent() {
            if parent.as_u32() == root_pid {
                node.children.push(proc.pid().as_u32());
            }
        }
    }
    
    Some(node)
}

/// Find all descendant PIDs of a given process
pub fn find_all_descendants(system: &System, root_pid: u32) -> Vec<u32> {
    let mut descendants = Vec::new();
    let mut to_check = vec![root_pid];
    
    while let Some(current_pid) = to_check.pop() {
        // Find children of current PID
        for (_pid, proc) in system.processes() {
            if let Some(parent) = proc.parent() {
                if parent.as_u32() == current_pid {
                    let child_pid = proc.pid().as_u32();
                    descendants.push(child_pid);
                    to_check.push(child_pid);
                }
            }
        }
    }
    
    descendants
}

/// Find the actual command process (deepest non-shell child)
pub fn find_actual_command_pid(system: &System, shell_pid: u32) -> Option<(u32, String)> {
    debug!("Finding actual command for shell PID {}", shell_pid);
    
    // Build a map of parent->children relationships
    let mut children_map: HashMap<u32, Vec<&Process>> = HashMap::new();
    
    for (_pid, proc) in system.processes() {
        if let Some(parent) = proc.parent() {
            children_map.entry(parent.as_u32())
                .or_insert_with(Vec::new)
                .push(proc);
        }
    }
    
    // Start from the shell and find the deepest non-shell process
    let mut current_pid = shell_pid;
    let mut current_name = "bash".to_string();
    let mut depth = 0;
    
    loop {
        // Get children of current process
        let children = children_map.get(&current_pid);
        
        if let Some(children) = children {
            // Filter out shell processes and find the most interesting child
            let mut best_child: Option<(&Process, i32)> = None;
            
            for child in children {
                let name = child.name().to_str().unwrap_or("unknown");
                let cmd = child.cmd();
                
                // Skip common shell processes
                if is_shell_process(name) {
                    continue;
                }
                
                // Prioritize processes based on various factors
                let priority = calculate_process_priority(name, cmd);
                
                if best_child.is_none() || priority > best_child.unwrap().1 {
                    best_child = Some((child, priority));
                }
            }
            
            if let Some((child, _)) = best_child {
                current_pid = child.pid().as_u32();
                current_name = child.name().to_str().unwrap_or("unknown").to_string();
                depth += 1;
                trace!("Found child at depth {}: {} (PID {})", depth, current_name, current_pid);
            } else {
                // No non-shell children found
                break;
            }
        } else {
            // No children
            break;
        }
        
        // Prevent infinite loops
        if depth > 10 {
            debug!("Max depth reached while searching for actual command");
            break;
        }
    }
    
    // If we found a different process than the shell, return it
    if current_pid != shell_pid {
        debug!("Found actual command: {} (PID {})", current_name, current_pid);
        Some((current_pid, current_name))
    } else {
        debug!("No actual command found, only shell process");
        None
    }
}

/// Check if a process name is a shell
fn is_shell_process(name: &str) -> bool {
    matches!(name, "bash" | "sh" | "zsh" | "fish" | "ksh" | "tcsh" | "csh" | "dash")
}

/// Calculate priority for a process (higher is better)
fn calculate_process_priority(name: &str, cmd: &[OsString]) -> i32 {
    let mut priority = 0;
    
    // Prefer processes that are clearly the main command
    if !name.starts_with("apm-script-") && !name.contains("temp") {
        priority += 10;
    }
    
    // Prefer processes with meaningful command lines
    if !cmd.is_empty() && !cmd[0].to_string_lossy().contains("/bin/sh") {
        priority += 5;
    }
    
    // Deprioritize certain helper processes
    if name == "sleep" || name == "read" || name == "cat" {
        priority -= 5;
    }
    
    priority
}

/// Get a summary of the process tree for debugging
pub fn get_process_tree_summary(system: &System, root_pid: u32) -> String {
    let mut summary = String::new();
    let mut to_process = vec![(root_pid, 0)]; // (pid, depth)
    let mut processed = std::collections::HashSet::new();
    
    while let Some((pid, depth)) = to_process.pop() {
        if processed.contains(&pid) {
            continue;
        }
        processed.insert(pid);
        
        let pid_obj = Pid::from(pid as usize);
        if let Some(proc) = system.process(pid_obj) {
            let indent = "  ".repeat(depth);
            let name = proc.name().to_str().unwrap_or("unknown");
            let cmd_str = proc.cmd().iter()
                .map(|s| s.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ");
            summary.push_str(&format!("{}[{}] {} ({})\n", indent, pid, name, cmd_str));
            
            // Add children
            for (_child_pid, child_proc) in system.processes() {
                if let Some(parent) = child_proc.parent() {
                    if parent.as_u32() == pid {
                        to_process.push((child_proc.pid().as_u32(), depth + 1));
                    }
                }
            }
        }
    }
    
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_shell_process() {
        assert!(is_shell_process("bash"));
        assert!(is_shell_process("zsh"));
        assert!(!is_shell_process("node"));
        assert!(!is_shell_process("python"));
    }
    
    #[test]
    fn test_calculate_process_priority() {
        // Regular command should have higher priority than temp script
        assert!(calculate_process_priority("node", &["node".into(), "app.js".into()]) > 
                calculate_process_priority("apm-script-123", &["/tmp/apm-script-123.sh".into()]));
        
        // Helper processes should have lower priority
        assert!(calculate_process_priority("python", &["python".into()]) > 
                calculate_process_priority("sleep", &["sleep".into(), "10".into()]));
    }
}