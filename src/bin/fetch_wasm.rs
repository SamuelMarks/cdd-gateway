#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Tool to fetch WASM files from GitHub releases for the cdd-gateway-wasm-sdk.

use cdd_gateway::error::CddGatewayError;
use reqwest::{header, Client};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

/// Represents a GitHub release asset.
#[derive(Deserialize, Debug)]
struct GithubAsset {
    /// The name of the asset.
    name: String,
    /// The download URL for the asset.
    browser_download_url: String,
}

/// Represents a GitHub release.
#[derive(Deserialize, Debug)]
struct GithubRelease {
    /// List of assets in the release.
    assets: Vec<GithubAsset>,
}

/// The list of repositories to process and their languages.
const REPOS: &[(&str, &str)] = &[
    ("c", "SamuelMarks/cdd-c"),
    ("cpp", "SamuelMarks/cdd-cpp"),
    ("csharp", "SamuelMarks/cdd-csharp"),
    ("go", "SamuelMarks/cdd-go"),
    ("java", "SamuelMarks/cdd-java"),
    ("kotlin", "offscale/cdd-kotlin"),
    ("php", "SamuelMarks/cdd-php"),
    ("python", "offscale/cdd-python-all"),
    ("ruby", "SamuelMarks/cdd-ruby"),
    ("rust", "SamuelMarks/cdd-rust"),
    ("sh", "SamuelMarks/cdd-sh"),
    ("swift", "SamuelMarks/cdd-swift"),
    ("ts", "offscale/cdd-ts"),
];

/// Gets the expected release file name for a given tool.
fn get_release_file_name(tool: &str) -> String {
    if tool == "cdd-csharp" {
        "cdd-csharp-wasm.zip".to_string()
    } else if tool == "cdd-ts" {
        "cdd-ts-javy.wasm".to_string()
    } else {
        format!("{tool}.wasm")
    }
}

/// Downloads a file from a URL to a destination path.
async fn download_file(client: &Client, url: &str, dest: &Path) -> Result<(), CddGatewayError> {
    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = response.bytes().await?;
    fs::write(dest, bytes)?;
    Ok(())
}

/// Unzips a file using the `zip` crate.
fn unzip_file(zip_path: &Path, extract_to: &Path, is_csharp: bool) -> Result<(), CddGatewayError> {
    let file = fs::File::open(zip_path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| CddGatewayError::Internal(e.to_string()))?;

    if is_csharp {
        archive
            .extract(extract_to)
            .map_err(|e| CddGatewayError::Internal(e.to_string()))?;
    } else {
        // Find the first file in the archive (assuming it's the WASM file we want)
        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| CddGatewayError::Internal(e.to_string()))?;
            if !file.is_dir() {
                let mut out_file = fs::File::create(extract_to)?;
                io::copy(&mut file, &mut out_file)?;
                break;
            }
        }
    }
    Ok(())
}

/// Gets the GitHub release URL for the given repository and tool.
async fn get_github_release_url(
    client: &Client,
    repo: &str,
    tool: &str,
    base_api_url: &str,
    base_dl_url: &str,
) -> String {
    let url = format!("{base_api_url}/repos/{repo}/releases/latest");
    let default_url = format!(
        "{base_dl_url}/{repo}/releases/latest/download/{}",
        get_release_file_name(tool)
    );

    if let Ok(resp) = client.get(&url).send().await {
        if let Ok(release) = resp.json::<GithubRelease>().await {
            let target_names = if tool == "cdd-csharp" {
                vec![format!("{tool}-wasm.zip"), format!("{tool}.wasm")]
            } else if tool == "cdd-ts" {
                vec![
                    "cdd-ts-javy.wasm".to_string(),
                    format!("{tool}.wasm"),
                    format!("{tool}-wasm.zip"),
                ]
            } else {
                vec![
                    format!("{tool}.wasm"),
                    format!("{tool}-wasm.zip"),
                    format!("{tool}.js.wasm"),
                ]
            };

            for target in target_names {
                if let Some(asset) = release.assets.iter().find(|a| a.name == target) {
                    return asset.browser_download_url.clone();
                }
            }
        }
    }
    default_url
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(not(tarpaulin))]
#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<(), CddGatewayError> {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("cdd-gateway-fetch-wasm"),
    );
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if let Ok(val) = header::HeaderValue::from_str(&format!("Bearer {token}")) {
            headers.insert(header::AUTHORIZATION, val);
        }
    }

    let client = Client::builder().default_headers(headers).build()?;
    let base_api_url = "https://api.github.com";
    let base_dl_url = "https://github.com";

    let cdd_ctl_dir = std::env::current_dir()?;
    let dest_dir = cdd_ctl_dir
        .join("cdd-gateway-wasm-sdk")
        .join("assets")
        .join("wasm");
    let support_file = cdd_ctl_dir
        .join("cdd-gateway-wasm-sdk")
        .join("assets")
        .join("wasm-support.json");

    if !dest_dir.exists() {
        fs::create_dir_all(&dest_dir)?;
    }

    let mut support_map: HashMap<String, bool> = HashMap::new();

    println!("Gathering WASM files...");

    for (lang, repo) in REPOS {
        let parts: Vec<&str> = repo.split('/').collect();
        let tool = parts[1];

        let wasm_dest_name = if tool == "cdd-ts" {
            "cdd-ts-javy.wasm".to_string()
        } else {
            format!("{tool}.wasm")
        };

        let wasm_dest = dest_dir.join(&wasm_dest_name);
        println!("Processing {tool} ({lang})...");

        let mut supported = false;

        let url = get_github_release_url(&client, repo, tool, base_api_url, base_dl_url).await;
        let is_zip = std::path::Path::new(&url)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));
        let dl_dest = if is_zip {
            dest_dir.join(format!("{wasm_dest_name}.zip"))
        } else {
            wasm_dest.clone()
        };

        if download_file(&client, &url, &dl_dest).await.is_ok() {
            if is_zip {
                let extract_target = if tool == "cdd-csharp" {
                    let dest_csharp = dest_dir.join("cdd-csharp");
                    let _ = fs::remove_dir_all(&dest_csharp);
                    dest_dir.as_path()
                } else {
                    wasm_dest.as_path()
                };

                if let Err(e) = unzip_file(&dl_dest, extract_target, tool == "cdd-csharp") {
                    println!("  ❌ Failed to unzip {}: {e}", dl_dest.display());
                }
                let _ = fs::remove_file(&dl_dest);
            }
            supported = true;
            println!("  ✅ Successfully downloaded {wasm_dest_name}");
        } else {
            println!("  ❌ Failed to download from GitHub releases. Attempting fallback to v0.0.1 tag...");
            let fallback_filename = get_release_file_name(tool);
            let is_fallback_zip = std::path::Path::new(&fallback_filename)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));
            let fallback_dl_dest = if is_fallback_zip {
                dest_dir.join(format!("{wasm_dest_name}.zip"))
            } else {
                wasm_dest.clone()
            };

            let mut fallback_success = false;
            let fallbacks = [
                format!("{base_dl_url}/{repo}/releases/download/v0.0.1/{fallback_filename}"),
                format!("{base_dl_url}/{repo}/releases/download/0.0.1/{fallback_filename}"),
                format!("{base_dl_url}/{repo}/releases/download/v0.0.1/{tool}-wasm.zip"),
            ];

            for fb_url in fallbacks {
                if download_file(&client, &fb_url, &fallback_dl_dest)
                    .await
                    .is_ok()
                {
                    let process_zip = std::path::Path::new(&fb_url)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));
                    if process_zip {
                        let extract_target = if tool == "cdd-csharp" {
                            let dest_csharp = dest_dir.join("cdd-csharp");
                            let _ = fs::remove_dir_all(&dest_csharp);
                            dest_dir.as_path()
                        } else {
                            wasm_dest.as_path()
                        };

                        if let Err(e) =
                            unzip_file(&fallback_dl_dest, extract_target, tool == "cdd-csharp")
                        {
                            println!("  ❌ Failed to unzip {}: {e}", fallback_dl_dest.display());
                        }
                        let _ = fs::remove_file(&fallback_dl_dest);
                    }
                    supported = true;
                    fallback_success = true;
                    println!("  ✅ Successfully downloaded fallback {fallback_filename}");
                    break;
                }
            }

            if !fallback_success {
                println!("  ⚠️ WASM not found on GitHub for {tool}.");
            }
        }

        support_map.insert(lang.to_string(), supported);
    }

    let support_json = serde_json::to_string_pretty(&support_map).unwrap_or_default();
    fs::write(support_file, support_json)?;
    println!("✅ Copied wasm-support.json to assets/");
    println!("\nWASM Gather Complete.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[test]
    fn test_get_release_file_name() {
        assert_eq!(get_release_file_name("cdd-csharp"), "cdd-csharp-wasm.zip");
        assert_eq!(get_release_file_name("cdd-ts"), "cdd-ts-javy.wasm");
        assert_eq!(get_release_file_name("cdd-rust"), "cdd-rust.wasm");
    }

    #[tokio::test]
    async fn test_download_file() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/download");
            then.status(200).body("file_content");
        });

        let client = Client::new();
        let url = server.url("/download");
        let dest = std::env::temp_dir().join("test_file.txt");

        let res = download_file(&client, &url, &dest).await;
        assert!(res.is_ok());
        if let Ok(content) = fs::read_to_string(&dest) {
            assert_eq!(content, "file_content");
        } else {
            panic!("Could not read temp file");
        }
        let _ = fs::remove_file(dest);
    }

    #[tokio::test]
    async fn test_download_file_error() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/download_err");
            then.status(404);
        });

        let client = Client::new();
        let url = server.url("/download_err");
        let dest = std::env::temp_dir().join("test_file_err.txt");

        let res = download_file(&client, &url, &dest).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_get_github_release_url() {
        let server = MockServer::start();
        let repo = "test/test-repo";
        let path = format!("/repos/{repo}/releases/latest");

        let _m = server.mock(|when, then| {
            when.method(GET).path(&path);
            then.status(200).json_body(serde_json::json!({
                "assets": [
                    {
                        "name": "cdd-rust.wasm",
                        "browser_download_url": "http://found-url.com"
                    }
                ]
            }));
        });

        let client = Client::new();
        let url = get_github_release_url(
            &client,
            repo,
            "cdd-rust",
            &server.url(""),
            "http://default-dl.com",
        )
        .await;
        assert_eq!(url, "http://found-url.com");
    }

    #[tokio::test]
    async fn test_get_github_release_url_csharp() {
        let server = MockServer::start();
        let repo = "test/test-repo-csharp";
        let path = format!("/repos/{repo}/releases/latest");

        let _m = server.mock(|when, then| {
            when.method(GET).path(&path);
            then.status(200).json_body(serde_json::json!({
                "assets": [
                    {
                        "name": "cdd-csharp-wasm.zip",
                        "browser_download_url": "http://found-url-csharp.com"
                    }
                ]
            }));
        });

        let client = Client::new();
        let url = get_github_release_url(
            &client,
            repo,
            "cdd-csharp",
            &server.url(""),
            "http://default-dl.com",
        )
        .await;
        assert_eq!(url, "http://found-url-csharp.com");
    }

    #[tokio::test]
    async fn test_get_github_release_url_ts() {
        let server = MockServer::start();
        let repo = "test/test-repo-ts";
        let path = format!("/repos/{repo}/releases/latest");

        let _m = server.mock(|when, then| {
            when.method(GET).path(&path);
            then.status(200).json_body(serde_json::json!({
                "assets": [
                    {
                        "name": "cdd-ts-javy.wasm",
                        "browser_download_url": "http://found-url-ts.com"
                    }
                ]
            }));
        });

        let client = Client::new();
        let url = get_github_release_url(
            &client,
            repo,
            "cdd-ts",
            &server.url(""),
            "http://default-dl.com",
        )
        .await;
        assert_eq!(url, "http://found-url-ts.com");
    }

    #[tokio::test]
    async fn test_get_github_release_url_not_found() {
        let server = MockServer::start();
        let repo = "test/test-repo-not-found";
        let path = format!("/repos/{repo}/releases/latest");

        let _m = server.mock(|when, then| {
            when.method(GET).path(&path);
            then.status(404);
        });

        let client = Client::new();
        let url = get_github_release_url(
            &client,
            repo,
            "cdd-rust",
            &server.url(""),
            "http://default-dl.com",
        )
        .await;
        assert_eq!(
            url,
            format!("http://default-dl.com/{repo}/releases/latest/download/cdd-rust.wasm")
        );
    }

    #[test]
    fn test_unzip_file() -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;

        let temp_dir = std::env::temp_dir();
        let zip_path = temp_dir.join("test_file.zip");
        let file = fs::File::create(&zip_path)?;
        let mut zip = zip::ZipWriter::new(file);

        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("test.wasm", options)?;
        zip.write_all(b"wasm content")?;
        zip.finish()?;

        // Test standard extraction
        let extract_path = temp_dir.join("test_extracted.wasm");
        let res = unzip_file(&zip_path, &extract_path, false);
        assert!(res.is_ok());
        let content = fs::read_to_string(&extract_path)?;
        assert_eq!(content, "wasm content");

        // Test csharp (directory) extraction
        let extract_dir = temp_dir.join("test_extract_dir");
        fs::create_dir_all(&extract_dir)?;
        let res = unzip_file(&zip_path, &extract_dir, true);
        assert!(res.is_ok());
        let csharp_content = fs::read_to_string(extract_dir.join("test.wasm"))?;
        assert_eq!(csharp_content, "wasm content");

        // Cleanup
        let _ = fs::remove_file(zip_path);
        let _ = fs::remove_file(extract_path);
        let _ = fs::remove_dir_all(extract_dir);
        Ok(())
    }
}
