// Git integration via shell `git` command (no native deps)
// - Detects if vault is a git repo
// - Initializes a new repo (with .gitignore)
// - Auto-commits on save
// - Provides log/status commands
// - (No push/pull yet — out of scope for MVP)

use crate::error::{VaultError, VaultResult};
use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Serialize, Clone)]
pub struct GitStatus {
    pub is_repo: bool,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub modified: Vec<String>,
    pub untracked: Vec<String>,
    pub staged: Vec<String>,
    pub ahead: u32,
    pub behind: u32,
    pub remote_url: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct GitLogEntry {
    pub id: String,
    pub full_id: String,
    pub summary: String,
    pub author: String,
    pub email: String,
    pub timestamp: i64,
    pub files_changed: usize,
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("git not found: {}", e))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn is_git_repo(vault_path: &Path) -> bool {
    run_git(vault_path, &["rev-parse", "--git-dir"]).is_ok()
}

/// Initialize a new git repo in the vault path, with a sensible .gitignore
pub fn git_init(vault_path: &Path) -> VaultResult<GitStatus> {
    let gi = vault_path.join(".gitignore");
    if !gi.exists() {
        std::fs::write(&gi, ".notevault/\n.config/\n*.tmp\n.DS_Store\nThumbs.db\n")?;
    }
    // Try to init; ignore error if already a repo
    let _ = run_git(vault_path, &["init", "-b", "main"]);
    // Stage and commit the gitignore
    let _ = run_git(vault_path, &["add", ".gitignore"]);
    let _ = run_git(vault_path, &["commit", "-m", "Initial commit"]);
    git_status(vault_path)
}

pub fn git_status(vault_path: &Path) -> VaultResult<GitStatus> {
    if !is_git_repo(vault_path) {
        return Ok(GitStatus {
            is_repo: false,
            head: None,
            branch: None,
            modified: vec![],
            untracked: vec![],
            staged: vec![],
            ahead: 0,
            behind: 0,
            remote_url: None,
        });
    }

    let head_out = run_git(vault_path, &["rev-parse", "--short", "HEAD"]).ok();
    let head = head_out.map(|s| s.trim().to_string());

    let branch_out = run_git(vault_path, &["branch", "--show-current"]).ok();
    let branch = branch_out.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

    // porcelain v1: XY format
    let status_out = run_git(vault_path, &["status", "--porcelain"]).unwrap_or_default();
    let mut modified = Vec::new();
    let mut untracked = Vec::new();
    let mut staged = Vec::new();
    for line in status_out.lines() {
        if line.len() < 3 { continue; }
        let code = &line[..2];
        let path = line[3..].to_string();
        // X = index, Y = worktree
        let x = code.as_bytes()[0];
        let y = code.as_bytes()[1];
        if x != b' ' && x != b'?' { staged.push(path.clone()); }
        if y == b'?' { untracked.push(path); }
        else if y == b'M' || y == b'D' { modified.push(path); }
    }

    let remote_url = run_git(vault_path, &["remote", "get-url", "origin"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // ahead/behind
    let mut ahead = 0u32;
    let mut behind = 0u32;
    if let Some(b) = &branch {
        if let Ok(ab) = run_git(vault_path, &["rev-list", "--left-right", "--count", &format!("{}...origin/{}", b, b)]) {
            for line in ab.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() == 2 {
                    ahead = parts[0].parse().unwrap_or(0);
                    behind = parts[1].parse().unwrap_or(0);
                }
            }
        }
    }

    Ok(GitStatus {
        is_repo: true,
        head,
        branch,
        modified,
        untracked,
        staged,
        ahead,
        behind,
        remote_url,
    })
}

pub fn git_commit(vault_path: &Path, message: &str) -> VaultResult<GitLogEntry> {
    if !is_git_repo(vault_path) {
        return Err(VaultError::Other("Not a git repo".into()));
    }
    run_git(vault_path, &["add", "-A"]).map_err(|e| VaultError::Other(e))?;
    run_git(vault_path, &["commit", "-m", message]).map_err(|e| VaultError::Other(e))?;
    let log_out = run_git(vault_path, &["log", "-1", "--format=%H%n%h%n%s%n%an%n%ae%n%ct"]).map_err(|e| VaultError::Other(e))?;
    let mut lines = log_out.lines();
    let full_id = lines.next().unwrap_or("").to_string();
    let id = lines.next().unwrap_or("").to_string();
    let summary = lines.next().unwrap_or("").to_string();
    let author = lines.next().unwrap_or("").to_string();
    let email = lines.next().unwrap_or("").to_string();
    let timestamp: i64 = lines.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    // file count via diff-tree
    let files_changed = if let Ok(s) = run_git(vault_path, &["diff-tree", "--no-commit-id", "--name-only", "-r", &full_id]) {
        s.lines().count()
    } else { 0 };
    Ok(GitLogEntry {
        id, full_id, summary, author, email, timestamp, files_changed,
    })
}

pub fn git_log(vault_path: &Path, limit: usize) -> VaultResult<Vec<GitLogEntry>> {
    if !is_git_repo(vault_path) {
        return Ok(Vec::new());
    }
    let fmt = "%H%n%h%n%s%n%an%n%ae%n%ct";
    let out = run_git(vault_path, &["log", &format!("-{}", limit), &format!("--format={}", fmt)])
        .map_err(|e| VaultError::Other(e))?;
    let mut entries = Vec::new();
    let chunks: Vec<&str> = out.split("\n\n").filter(|c| !c.is_empty()).collect();
    for chunk in chunks {
        let lines: Vec<&str> = chunk.lines().collect();
        if lines.len() < 6 { continue; }
        entries.push(GitLogEntry {
            id: lines[1].to_string(),
            full_id: lines[0].to_string(),
            summary: lines[2].to_string(),
            author: lines[3].to_string(),
            email: lines[4].to_string(),
            timestamp: lines[5].parse().unwrap_or(0),
            files_changed: 0,
        });
    }
    Ok(entries)
}

/// Auto-commit a single note change. Best-effort: silent if anything fails.
pub fn auto_commit_note(vault_path: &Path, note_rel_path: &str) -> Option<String> {
    if !is_git_repo(vault_path) { return None; }
    let _ = run_git(vault_path, &["add", note_rel_path]);
    // Check if there's anything to commit
    let staged = run_git(vault_path, &["diff", "--cached", "--name-only"]).unwrap_or_default();
    if staged.trim().is_empty() { return None; }
    let msg = format!("Update {}", note_rel_path);
    let _ = run_git(vault_path, &["commit", "-m", &msg]);
    run_git(vault_path, &["rev-parse", "--short", "HEAD"]).ok().map(|s| s.trim().to_string())
}
