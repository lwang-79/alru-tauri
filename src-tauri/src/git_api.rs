use serde::{Deserialize, Serialize};
use crate::command::create_clean_shell_command;

/// Branch protection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchProtection {
    pub is_protected: bool,
    pub protection_info: Option<String>,
}

/// GitHub branch protection response
#[derive(Debug, Deserialize)]
struct GitHubBranchProtection {
    #[serde(default)]
    required_pull_request_reviews: Option<serde_json::Value>,
    #[serde(default)]
    lock_branch: Option<GitHubLockBranch>,
}

#[derive(Debug, Deserialize)]
struct GitHubLockBranch {
    enabled: bool,
}

/// Extract repository owner and name from a repository URL
/// Supports GitHub, GitLab, Bitbucket, CodeCommit URLs
fn parse_repository_url(url: &str) -> Option<(String, String, String)> {
    // Remove .git suffix if present
    let url = url.trim_end_matches(".git");

    // GitHub patterns
    if url.contains("github.com") {
        // HTTPS: https://github.com/owner/repo
        // SSH: git@github.com:owner/repo
        let parts: Vec<&str> = if url.contains("git@github.com:") {
            url.split("git@github.com:").collect()
        } else if url.contains("github.com/") {
            url.split("github.com/").collect()
        } else {
            return None;
        };

        if parts.len() == 2 {
            let repo_parts: Vec<&str> = parts[1].split('/').collect();
            if repo_parts.len() >= 2 {
                return Some((
                    "github".to_string(),
                    repo_parts[0].to_string(),
                    repo_parts[1].to_string(),
                ));
            }
        }
    }

    // GitLab patterns
    if url.contains("gitlab.com") {
        let parts: Vec<&str> = if url.contains("git@gitlab.com:") {
            url.split("git@gitlab.com:").collect()
        } else if url.contains("gitlab.com/") {
            url.split("gitlab.com/").collect()
        } else {
            return None;
        };

        if parts.len() == 2 {
            let repo_parts: Vec<&str> = parts[1].split('/').collect();
            if repo_parts.len() >= 2 {
                return Some((
                    "gitlab".to_string(),
                    repo_parts[0].to_string(),
                    repo_parts[1].to_string(),
                ));
            }
        }
    }

    // Bitbucket patterns
    if url.contains("bitbucket.org") {
        let parts: Vec<&str> = if url.contains("git@bitbucket.org:") {
            url.split("git@bitbucket.org:").collect()
        } else if url.contains("bitbucket.org/") {
            url.split("bitbucket.org/").collect()
        } else {
            return None;
        };

        if parts.len() == 2 {
            let repo_parts: Vec<&str> = parts[1].split('/').collect();
            if repo_parts.len() >= 2 {
                return Some((
                    "bitbucket".to_string(),
                    repo_parts[0].to_string(),
                    repo_parts[1].to_string(),
                ));
            }
        }
    }

    // CodeCommit - not supported for branch protection checks
    if url.contains("codecommit") {
        return Some(("codecommit".to_string(), "".to_string(), "".to_string()));
    }

    None
}

/// Get GitHub token from git credential helper or environment
fn get_github_token() -> Option<String> {
    // Try environment variable first
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        return Some(token);
    }

    // Try git credential helper
    let output = create_clean_shell_command("git")
        .args(["credential", "fill"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(b"protocol=https\nhost=github.com\n\n");
            }
            child.wait_with_output()
        });

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.starts_with("password=") {
                return Some(line.trim_start_matches("password=").to_string());
            }
        }
    }

    None
}

/// Check GitHub rulesets for branch protection
fn check_github_rulesets(
    owner: &str,
    repo: &str,
    branch: &str,
    token: Option<&str>,
) -> Result<BranchProtection, String> {
    println!(
        "[check_github_rulesets] Checking rulesets for {}/{} branch: {}",
        owner, repo, branch
    );

    // Build the API URL for rulesets
    let url = format!("https://api.github.com/repos/{}/{}/rulesets", owner, repo);

    println!("[check_github_rulesets] API URL: {}", url);

    // Make HTTP request using curl
    let mut curl_args = vec![
        "-s",             // Silent mode
        "-w",             // Write out format
        "\n%{http_code}", // Append HTTP status code on new line
        "-H",
        "Accept: application/vnd.github+json",
        "-H",
        "X-GitHub-Api-Version: 2022-11-28",
    ];

    // Add authorization header if token is available
    let auth_header;
    if let Some(token) = token {
        auth_header = format!("Authorization: Bearer {}", token);
        curl_args.push("-H");
        curl_args.push(&auth_header);
        println!("[check_github_rulesets] Using authentication token");
    }

    curl_args.push(&url);

    let output = create_clean_shell_command("curl").args(&curl_args).output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Split response body and HTTP status code
            let parts: Vec<&str> = stdout.rsplitn(2, '\n').collect();
            let (response_body, http_code) = if parts.len() == 2 {
                (parts[1], parts[0])
            } else {
                (stdout.as_ref(), "")
            };

            println!(
                "[check_github_rulesets] HTTP Code: {}, Full Response: {}",
                http_code, response_body
            );

            // Check for 404 - no rulesets
            if http_code == "404" {
                println!("[check_github_rulesets] No rulesets found (404)");
                return Ok(BranchProtection {
                    is_protected: false,
                    protection_info: None,
                });
            }

            // Check for 403 - API not available
            if http_code == "403" {
                println!("[check_github_rulesets] API not available (403)");
                return Ok(BranchProtection {
                    is_protected: false,
                    protection_info: None,
                });
            }

            // Check for 200 - rulesets exist
            if http_code == "200" {
                // Try to parse the response as JSON array
                if let Ok(rulesets) = serde_json::from_str::<Vec<serde_json::Value>>(response_body) {
                    println!("[check_github_rulesets] Found {} ruleset(s)", rulesets.len());
                    
                    // Check each ruleset
                    for ruleset in &rulesets {
                        let ruleset_id = ruleset.get("id").and_then(|id| id.as_i64());
                        let ruleset_name = ruleset.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                        let target = ruleset.get("target").and_then(|t| t.as_str());
                        let enforcement = ruleset.get("enforcement").and_then(|e| e.as_str());
                        
                        println!("[check_github_rulesets] Ruleset: {} (ID: {:?}, target: {:?}, enforcement: {:?})", 
                                 ruleset_name, ruleset_id, target, enforcement);
                        
                        // Skip if not targeting branches or if disabled
                        if target != Some("branch") {
                            println!("[check_github_rulesets] Skipping - not targeting branches");
                            continue;
                        }
                        
                        if enforcement == Some("disabled") {
                            println!("[check_github_rulesets] Skipping - enforcement is disabled");
                            continue;
                        }
                        
                        // Get the detailed ruleset using the self link
                        if let Some(self_link) = ruleset.get("_links")
                            .and_then(|links| links.get("self"))
                            .and_then(|self_obj| self_obj.get("href"))
                            .and_then(|href| href.as_str()) {
                            
                            println!("[check_github_rulesets] Fetching ruleset details from: {}", self_link);
                            
                            // Fetch the detailed ruleset
                            let mut detail_curl_args = vec![
                                "-s",
                                "-H", "Accept: application/vnd.github+json",
                                "-H", "X-GitHub-Api-Version: 2022-11-28",
                            ];
                            
                            let detail_auth_header;
                            if let Some(token) = token {
                                detail_auth_header = format!("Authorization: Bearer {}", token);
                                detail_curl_args.push("-H");
                                detail_curl_args.push(&detail_auth_header);
                            }
                            
                            detail_curl_args.push(self_link);
                            
                            if let Ok(detail_output) = create_clean_shell_command("curl").args(&detail_curl_args).output() {
                                let detail_response = String::from_utf8_lossy(&detail_output.stdout);
                                println!("[check_github_rulesets] Ruleset details: {}", detail_response);
                                
                                if let Ok(detail) = serde_json::from_str::<serde_json::Value>(&detail_response) {
                                    // Check if this ruleset applies to our branch
                                    let applies_to_branch = detail
                                        .get("conditions")
                                        .and_then(|c| c.get("ref_name"))
                                        .and_then(|r| r.get("include"))
                                        .and_then(|i| i.as_array())
                                        .map(|includes| {
                                            includes.iter().any(|pattern| {
                                                if let Some(pattern_str) = pattern.as_str() {
                                                    println!("[check_github_rulesets] Checking pattern: {}", pattern_str);
                                                    // Check if pattern matches our branch
                                                    pattern_str == branch 
                                                        || pattern_str == format!("refs/heads/{}", branch)
                                                        || pattern_str == "~ALL"
                                                        || branch_matches_pattern(branch, pattern_str)
                                                } else {
                                                    false
                                                }
                                            })
                                        })
                                        .unwrap_or(false);
                                    
                                    if !applies_to_branch {
                                        println!("[check_github_rulesets] Ruleset does not apply to branch {}", branch);
                                        continue;
                                    }
                                    
                                    println!("[check_github_rulesets] Ruleset applies to branch {}", branch);
                                    
                                    // Check the rules
                                    if let Some(rules) = detail.get("rules").and_then(|r| r.as_array()) {
                                        for rule in rules {
                                            let rule_type = rule.get("type").and_then(|t| t.as_str());
                                            println!("[check_github_rulesets] Rule type: {:?}", rule_type);
                                            
                                            if rule_type == Some("pull_request") {
                                                println!("[check_github_rulesets] Found pull_request rule!");
                                                return Ok(BranchProtection {
                                                    is_protected: true,
                                                    protection_info: Some(
                                                        format!("Branch is protected: requires pull request before merging (ruleset: {})", ruleset_name)
                                                    ),
                                                });
                                            }
                                        }
                                        
                                        // Has rules but no PR requirement
                                        println!("[check_github_rulesets] Ruleset has rules but no pull_request requirement");
                                        return Ok(BranchProtection {
                                            is_protected: true,
                                            protection_info: Some(format!("Branch has protection rules (ruleset: {})", ruleset_name)),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // No rulesets found
            println!("[check_github_rulesets] No rulesets detected");
            Ok(BranchProtection {
                is_protected: false,
                protection_info: None,
            })
        }
        Err(e) => {
            println!("[check_github_rulesets] Failed to execute curl: {}", e);
            Ok(BranchProtection {
                is_protected: false,
                protection_info: None,
            })
        }
    }
}

/// Check if a branch name matches a pattern (simple wildcard matching)
fn branch_matches_pattern(branch: &str, pattern: &str) -> bool {
    // Remove refs/heads/ prefix if present
    let pattern = pattern.strip_prefix("refs/heads/").unwrap_or(pattern);
    
    // Simple wildcard matching
    if pattern == "*" || pattern == "**" {
        return true;
    }
    
    // For now, just do simple prefix/suffix matching
    if pattern.ends_with('*') {
        let prefix = pattern.trim_end_matches('*');
        return branch.starts_with(prefix);
    }
    
    if pattern.starts_with('*') {
        let suffix = pattern.trim_start_matches('*');
        return branch.ends_with(suffix);
    }
    
    branch == pattern
}

/// Check GitHub branch protection using HTTP API
fn check_github_branch_protection(
    owner: &str,
    repo: &str,
    branch: &str,
) -> Result<BranchProtection, String> {
    println!(
        "[check_github_branch_protection] Checking {}/{} branch: {}",
        owner, repo, branch
    );

    // Get GitHub token
    let token = get_github_token();

    // Build the API URL
    let url = format!(
        "https://api.github.com/repos/{}/{}/branches/{}/protection",
        owner, repo, branch
    );

    println!("[check_github_branch_protection] API URL: {}", url);

    // Make HTTP request using curl (available on all platforms)
    let mut curl_args = vec![
        "-s",             // Silent mode
        "-w",             // Write out format
        "\n%{http_code}", // Append HTTP status code on new line
        "-H",
        "Accept: application/vnd.github+json",
        "-H",
        "X-GitHub-Api-Version: 2022-11-28",
    ];

    // Add authorization header if token is available
    let auth_header;
    if let Some(ref token) = token {
        auth_header = format!("Authorization: Bearer {}", token);
        curl_args.push("-H");
        curl_args.push(&auth_header);
        println!("[check_github_branch_protection] Using authentication token");
    } else {
        println!("[check_github_branch_protection] No token found, making unauthenticated request");
    }

    curl_args.push(&url);

    let output = create_clean_shell_command("curl").args(&curl_args).output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Split response body and HTTP status code
            let parts: Vec<&str> = stdout.rsplitn(2, '\n').collect();
            let (response_body, http_code) = if parts.len() == 2 {
                (parts[1], parts[0])
            } else {
                (stdout.as_ref(), "")
            };

            println!(
                "[check_github_branch_protection] HTTP Code: {}, Response: {}",
                http_code,
                response_body.chars().take(200).collect::<String>()
            );

            // Check HTTP status code
            if http_code == "404" {
                println!("[check_github_branch_protection] Branch not protected (404), checking rulesets...");

                // Check rulesets as fallback (newer GitHub feature)
                return check_github_rulesets(owner, repo, branch, token.as_deref());
            }

            // Check for 403 - API not available (private repo without GitHub Pro)
            if http_code == "403" {
                println!("[check_github_branch_protection] API not available (403) - private repo or insufficient permissions");
                // For private repos without Pro, we can't check protection via API
                // Return as not protected since we can't determine
                return Ok(BranchProtection {
                    is_protected: false,
                    protection_info: None,
                });
            }

            // Check for 404 in response body (fallback)
            if response_body.contains("\"message\":\"Branch not protected\"")
                || response_body.contains("\"status\":\"404\"")
            {
                println!("[check_github_branch_protection] Branch not protected (404 in body)");
                return Ok(BranchProtection {
                    is_protected: false,
                    protection_info: None,
                });
            }

            // Check for authentication issues
            if response_body.contains("\"message\":\"Bad credentials\"") {
                println!("[check_github_branch_protection] Bad credentials");
                return Ok(BranchProtection {
                    is_protected: false,
                    protection_info: Some(
                        "Unable to check branch protection: GitHub authentication failed"
                            .to_string(),
                    ),
                });
            }

            // Only try to parse if we got a 200 response
            if http_code == "200" {
                // Try to parse the protection response
                if let Ok(protection) =
                    serde_json::from_str::<GitHubBranchProtection>(response_body)
                {
                    let mut protection_reasons = Vec::new();

                    if protection.required_pull_request_reviews.is_some() {
                        protection_reasons.push("requires pull request reviews");
                    }

                    if let Some(lock) = protection.lock_branch {
                        if lock.enabled {
                            protection_reasons.push("branch is locked");
                        }
                    }

                    if !protection_reasons.is_empty() {
                        let info =
                            format!("Branch is protected: {}", protection_reasons.join(", "));
                        println!("[check_github_branch_protection] {}", info);
                        return Ok(BranchProtection {
                            is_protected: true,
                            protection_info: Some(info),
                        });
                    } else {
                        // Successfully parsed but no protection rules found
                        println!("[check_github_branch_protection] No protection rules detected");
                        return Ok(BranchProtection {
                            is_protected: false,
                            protection_info: None,
                        });
                    }
                }
            }

            // Couldn't parse response or non-200 status
            println!("[check_github_branch_protection] Could not determine protection status");
            Ok(BranchProtection {
                is_protected: false,
                protection_info: Some("Unable to determine branch protection status".to_string()),
            })
        }
        Err(e) => {
            println!(
                "[check_github_branch_protection] Failed to execute curl: {}",
                e
            );
            Ok(BranchProtection {
                is_protected: false,
                protection_info: Some(format!("Failed to check branch protection: {}", e)),
            })
        }
    }
}

/// Tauri command to check branch protection for a repository
#[tauri::command]
pub async fn check_branch_protection(
    repository_url: &str,
    branch_name: &str,
) -> Result<BranchProtection, String> {
    println!(
        "[check_branch_protection] Checking protection for repo: {}, branch: {}",
        repository_url, branch_name
    );

    let parsed = parse_repository_url(repository_url);

    match parsed {
        Some((provider, owner, repo)) => {
            println!(
                "[check_branch_protection] Parsed - provider: {}, owner: {}, repo: {}",
                provider, owner, repo
            );

            match provider.as_str() {
                "github" => check_github_branch_protection(&owner, &repo, branch_name),
                "gitlab" => {
                    // GitLab support could be added here
                    Ok(BranchProtection {
                        is_protected: false,
                        protection_info: Some(
                            "GitLab branch protection check not yet implemented".to_string(),
                        ),
                    })
                }
                "bitbucket" => {
                    // Bitbucket support could be added here
                    Ok(BranchProtection {
                        is_protected: false,
                        protection_info: Some(
                            "Bitbucket branch protection check not yet implemented".to_string(),
                        ),
                    })
                }
                "codecommit" => Ok(BranchProtection {
                    is_protected: false,
                    protection_info: Some(
                        "CodeCommit branch protection check not supported".to_string(),
                    ),
                }),
                _ => Ok(BranchProtection {
                    is_protected: false,
                    protection_info: Some("Unknown repository provider".to_string()),
                }),
            }
        }
        None => {
            println!("[check_branch_protection] Failed to parse repository URL");
            Ok(BranchProtection {
                is_protected: false,
                protection_info: Some("Could not parse repository URL".to_string()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repository_url() {
        // GitHub HTTPS
        assert_eq!(
            parse_repository_url("https://github.com/owner/repo"),
            Some((
                "github".to_string(),
                "owner".to_string(),
                "repo".to_string()
            ))
        );

        // GitHub SSH
        assert_eq!(
            parse_repository_url("git@github.com:owner/repo.git"),
            Some((
                "github".to_string(),
                "owner".to_string(),
                "repo".to_string()
            ))
        );

        // GitLab HTTPS
        assert_eq!(
            parse_repository_url("https://gitlab.com/owner/repo"),
            Some((
                "gitlab".to_string(),
                "owner".to_string(),
                "repo".to_string()
            ))
        );

        // Bitbucket HTTPS
        assert_eq!(
            parse_repository_url("https://bitbucket.org/owner/repo"),
            Some((
                "bitbucket".to_string(),
                "owner".to_string(),
                "repo".to_string()
            ))
        );

        // CodeCommit
        assert!(parse_repository_url(
            "https://git-codecommit.us-east-1.amazonaws.com/v1/repos/my-repo"
        )
        .is_some());
    }
}
