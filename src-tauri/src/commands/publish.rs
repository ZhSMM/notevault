// Static site publishing (Quartz-style):
// - Shells out to the Node-based SSG script in scripts/static-build.mjs
// - The script lives next to the binary's source; in dev, we read it from the
//   `scripts/` dir; in production we embed it.
//
// The Tauri command `export_static(vault_path, output_path, base_url)` runs the
// script via `node` and returns a summary of the build.

use crate::error::{VaultError, VaultResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    pub output_path: String,
    pub pages: usize,
    pub tags: usize,
    pub log: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PublishOptions {
    pub output_path: String,
    #[serde(default = "default_base")]
    pub base_url: String,
}

fn default_base() -> String {
    "/".to_string()
}

fn script_path() -> std::path::PathBuf {
    // src-tauri/src/commands/publish.rs -> ../../../scripts/static-build.mjs
    let exe = std::env::current_exe().unwrap_or_default();
    let _ = exe; // unused
    // We always invoke via the project-relative path; in dev the binary lives
    // at src-tauri/target/debug/, so we walk up 4 levels to the project root.
    if let Ok(cwd) = std::env::current_dir() {
        for ancestor in cwd.ancestors() {
            let p = ancestor.join("scripts").join("static-build.mjs");
            if p.exists() {
                return p;
            }
        }
    }
    // Fallback (best-effort)
    std::path::PathBuf::from("scripts/static-build.mjs")
}

#[tauri::command]
pub fn export_static(vault_path: String, options: PublishOptions) -> VaultResult<PublishResult> {
    if !Path::new(&vault_path).is_dir() {
        return Err(VaultError::Other(format!("vault 路径不存在: {}", vault_path)));
    }
    let script = script_path();
    if !script.exists() {
        return Err(VaultError::Other(format!(
            "找不到 SSG 脚本: {}（应在项目根 scripts/ 下）",
            script.display()
        )));
    }
    // Ensure output dir exists / is empty
    let out = Path::new(&options.output_path);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    // Invoke node. Use `node` (resolved via PATH) and run the script.
    let mut cmd = Command::new("node");
    cmd.arg(script.to_string_lossy().to_string())
        .arg("--vault").arg(&vault_path)
        .arg("--out").arg(&options.output_path)
        .arg("--base").arg(&options.base_url);

    // Hide the console window on Windows so it doesn't pop up a flash.
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| VaultError::Other(format!("无法启动 node: {}", e)))?;

    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    if !output.status.success() {
        return Err(VaultError::Other(format!("SSG 失败 (exit {:?}):\n{}", output.status.code(), log)));
    }

    // Count pages
    let pages = std::fs::read_dir(&options.output_path)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    let p = e.path();
                    p.is_file() && p.extension().map(|s| s == "html").unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0);
    let tags = std::fs::read_dir(format!("{}/tags", options.output_path.trim_end_matches('/')))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|s| s == "html").unwrap_or(false))
                .count()
        })
        .unwrap_or(0);

    Ok(PublishResult {
        output_path: options.output_path,
        pages,
        tags,
        log,
    })
}
