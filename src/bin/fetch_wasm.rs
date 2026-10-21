#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Tool to fetch WebAssembly (`*.wasm`) compiler assets from GitHub releases for `cdd-gateway-wasm-sdk`.

use cdd_gateway::error::CddGatewayError;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use zip::ZipArchive;

/// Minimal representation of a GitHub Release returned by `/releases/latest`.
#[derive(Deserialize, Debug)]
struct Release {
    /// Release assets attached to the GitHub release.
    assets: Vec<Asset>,
}

/// Minimal representation of a single downloadable GitHub Release asset.
#[derive(Deserialize, Debug)]
struct Asset {
    /// Asset filename (e.g. `"cdd-rust.wasm"`).
    name: String,
    /// Direct download URL for the asset.
    browser_download_url: String,
}

/// Canonical list of `(owner/repo, execution_mode)` pairs for the CDD ecosystem.
const REPOS: &[(&str, &str)] = &[
    ("offscale/cdd-c", "wasi"),
    ("offscale/cdd-cpp", "wasi"),
    ("offscale/cdd-csharp", "js"),
    ("offscale/cdd-go", "wasi"),
    ("offscale/cdd-java", "none"),
    ("offscale/cdd-kotlin", "none"),
    ("offscale/cdd-php", "wasi"),
    ("offscale/cdd-python-all", "wasi"),
    ("offscale/cdd-ruby", "none"),
    ("offscale/cdd-rust", "wasi"),
    ("offscale/cdd-sh", "wasi"),
    ("offscale/cdd-swift", "none"),
    ("offscale/cdd-ts", "wasi"),
];

/// Entry point for the `fetch_wasm` binary.
///
/// # Returns
/// * `Ok(())` - When all WASM assets and `wasm-support.json` have been updated.
///
/// # Errors
/// * [`CddGatewayError`] - If creating directories or writing the support manifest fails.
#[cfg(not(tarpaulin_include))]
#[cfg_attr(coverage_nightly, coverage(off))]
#[tokio::main]
async fn main() -> Result<(), CddGatewayError> {
    let client = Client::builder().user_agent("cdd-wasm-fetcher").build()?;
    let dest_dir = Path::new("cdd-gateway-wasm-sdk/assets/wasm");
    let support_file = Path::new("cdd-gateway-wasm-sdk/assets/wasm-support.json");

    fetch_all_wasm(
        &client,
        "https://api.github.com",
        "https://github.com",
        dest_dir,
        support_file,
        REPOS,
    )
    .await
}

/// Fetches or extracts WASM assets for all `repos` and writes `support_file`.
///
/// # Arguments
/// * `client` - HTTP client used to query GitHub releases and download assets.
/// * `base_api_url` - Base URL for the GitHub REST API.
/// * `base_dl_url` - Base URL for direct GitHub release downloads.
/// * `dest_dir` - Directory where `*.wasm` files and extracted bundles are stored.
/// * `support_file` - Path to `wasm-support.json` to write.
/// * `repos` - Slice of `(owner/repo, mode)` tuples to process.
///
/// # Returns
/// * `Ok(())` - If all repositories were processed and `support_file` was written.
///
/// # Errors
/// * [`CddGatewayError`] - On filesystem or serialization failure.
async fn fetch_all_wasm(
    client: &Client,
    base_api_url: &str,
    base_dl_url: &str,
    dest_dir: &Path,
    support_file: &Path,
    repos: &[(&str, &str)],
) -> Result<(), CddGatewayError> {
    fs::create_dir_all(dest_dir)?;
    let mut support_map = HashMap::new();

    for &(repo, mode) in repos {
        let Some((_, tool)) = repo.split_once('/') else {
            return Err(CddGatewayError::Validation(format!(
                "Invalid repository identifier: {repo}"
            )));
        };

        support_map.insert(tool.to_string(), mode.to_string());

        if mode == "none" {
            println!("Skipping {tool} (WASM not supported yet)");
            continue;
        }

        println!("Fetching WASM for {tool} ({mode})...");

        let wasm_filename = format!("{tool}.wasm");
        let zip_filename = format!("{tool}-wasm.zip");

        let download_url = get_github_release_url(
            client,
            repo,
            &wasm_filename,
            &zip_filename,
            base_api_url,
            base_dl_url,
        )
        .await;

        if let Some((url, is_zip)) = download_url {
            println!("Downloading from {url}...");
            match download_file(client, &url).await {
                Ok(bytes) => {
                    if is_zip {
                        let zip_path = dest_dir.join(&zip_filename);
                        let mut file = File::create(&zip_path)?;
                        file.write_all(&bytes)?;

                        let extract_dir = dest_dir.join(tool);
                        fs::create_dir_all(&extract_dir)?;
                        extract_zip(&zip_path, &extract_dir)?;
                        fs::remove_file(zip_path)?;
                        println!("Extracted {tool} to {}", extract_dir.display());
                    } else {
                        let wasm_path = dest_dir.join(&wasm_filename);
                        let mut file = File::create(&wasm_path)?;
                        file.write_all(&bytes)?;
                        println!("Saved {wasm_filename} to {}", wasm_path.display());
                    }
                }
                Err(e) => {
                    eprintln!("Failed to download {url}: {e}");
                }
            }
        } else {
            eprintln!("Could not find WASM asset for {repo} in latest release.");
        }
    }

    let json = serde_json::to_string_pretty(&support_map)?;
    let _ = support_file.parent().map(fs::create_dir_all);
    fs::write(support_file, json)?;
    println!("Updated {}", support_file.display());

    Ok(())
}

/// Resolves the download URL for a tool's `.wasm` or `-wasm.zip` release asset.
///
/// Queries `/repos/{repo}/releases/latest` first and falls back to `releases/latest/download`
/// `HEAD` probes if the API call does not yield a matching asset.
///
/// # Arguments
/// * `client` - HTTP client instance.
/// * `repo` - `owner/repo` identifier.
/// * `wasm_filename` - Expected `.wasm` asset filename.
/// * `zip_filename` - Expected `-wasm.zip` asset filename.
/// * `base_api_url` - Base URL for the GitHub REST API.
/// * `base_dl_url` - Base URL for direct GitHub release downloads.
///
/// # Returns
/// * `Some((url, is_zip))` - Resolved download URL and whether it is a ZIP archive.
/// * `None` - If no matching asset was found.
async fn get_github_release_url(
    client: &Client,
    repo: &str,
    wasm_filename: &str,
    zip_filename: &str,
    base_api_url: &str,
    base_dl_url: &str,
) -> Option<(String, bool)> {
    let api_url = format!("{base_api_url}/repos/{repo}/releases/latest");
    if let Ok(resp) = client.get(&api_url).send().await {
        if resp.status().is_success() {
            if let Ok(release) = resp.json::<Release>().await {
                for asset in release.assets {
                    if asset.name == wasm_filename {
                        return Some((asset.browser_download_url, false));
                    }
                    if asset.name == zip_filename || asset.name == "cdd-csharp-wasm.zip" {
                        return Some((asset.browser_download_url, true));
                    }
                }
            }
        }
    }

    let direct_wasm = format!("{base_dl_url}/{repo}/releases/latest/download/{wasm_filename}");
    if let Ok(resp) = client.head(&direct_wasm).send().await {
        if resp.status().is_success() || resp.status().is_redirection() {
            return Some((direct_wasm, false));
        }
    }

    let direct_zip = format!("{base_dl_url}/{repo}/releases/latest/download/{zip_filename}");
    if let Ok(resp) = client.head(&direct_zip).send().await {
        if resp.status().is_success() || resp.status().is_redirection() {
            return Some((direct_zip, true));
        }
    }

    None
}

/// Downloads the raw bytes from `url`.
///
/// # Arguments
/// * `client` - HTTP client instance.
/// * `url` - Target URL to download.
///
/// # Returns
/// * `Ok(Vec<u8>)` - Downloaded payload bytes.
///
/// # Errors
/// * [`CddGatewayError::Http`] - If the request fails.
/// * [`CddGatewayError::Internal`] - If the server returns a non-success HTTP status.
async fn download_file(client: &Client, url: &str) -> Result<Vec<u8>, CddGatewayError> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(CddGatewayError::Internal(format!(
            "HTTP error: {}",
            resp.status()
        )));
    }
    let bytes = resp.bytes().await?;
    Ok(bytes.to_vec())
}

/// Extracts all entries of a ZIP archive at `zip_path` into `extract_to`.
///
/// # Arguments
/// * `zip_path` - Path to the `.zip` file on disk.
/// * `extract_to` - Destination directory where archive entries will be unpacked.
///
/// # Returns
/// * `Ok(())` - If extraction succeeded.
///
/// # Errors
/// * [`CddGatewayError`] - On file I/O or ZIP format error.
fn extract_zip(zip_path: &Path, extract_to: &Path) -> Result<(), CddGatewayError> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let Some(outpath) = file.enclosed_name() else {
            continue;
        };
        let outpath = extract_to.join(outpath);

        if file.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent().filter(|p| !p.exists()) {
                fs::create_dir_all(p)?;
            }
            let mut outfile = File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use httpmock::Method::HEAD;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn test_get_github_release_url_branches() {
        let server = MockServer::start();
        let repo = "offscale/cdd-c";
        let wasm_file = "cdd-c.wasm";
        let zip_file = "cdd-c-wasm.zip";
        let client = Client::new();

        // 1. Found WASM via API
        let mut m1 = server.mock(|when, then| {
            when.method(GET)
                .path("/repos/offscale/cdd-c/releases/latest");
            then.status(200).json_body(serde_json::json!({
                "assets": [
                    { "name": "other.txt", "browser_download_url": "http://other" },
                    { "name": wasm_file, "browser_download_url": "http://found-wasm" }
                ]
            }));
        });
        let res1 = get_github_release_url(
            &client,
            repo,
            wasm_file,
            zip_file,
            &server.base_url(),
            &server.base_url(),
        )
        .await;
        assert_eq!(res1, Some(("http://found-wasm".to_string(), false)));
        m1.delete();

        // 2. Found ZIP via API
        let mut m2 = server.mock(|when, then| {
            when.method(GET)
                .path("/repos/offscale/cdd-c/releases/latest");
            then.status(200).json_body(serde_json::json!({
                "assets": [
                    { "name": zip_file, "browser_download_url": "http://found-zip" }
                ]
            }));
        });
        let res2 = get_github_release_url(
            &client,
            repo,
            wasm_file,
            zip_file,
            &server.base_url(),
            &server.base_url(),
        )
        .await;
        assert_eq!(res2, Some(("http://found-zip".to_string(), true)));
        m2.delete();

        // 3. Fallback to direct WASM HEAD
        let _m_api_404 = server.mock(|when, then| {
            when.method(GET)
                .path("/repos/offscale/cdd-c/releases/latest");
            then.status(404);
        });
        let mut m_head_wasm = server.mock(|when, then| {
            when.method(HEAD)
                .path("/offscale/cdd-c/releases/latest/download/cdd-c.wasm");
            then.status(200);
        });
        let res3 = get_github_release_url(
            &client,
            repo,
            wasm_file,
            zip_file,
            &server.base_url(),
            &server.base_url(),
        )
        .await;
        assert_eq!(
            res3,
            Some((
                format!(
                    "{}/offscale/cdd-c/releases/latest/download/cdd-c.wasm",
                    server.base_url()
                ),
                false
            ))
        );
        m_head_wasm.delete();

        // 4. Fallback to direct ZIP HEAD
        let mut m_head_wasm_404 = server.mock(|when, then| {
            when.method(HEAD)
                .path("/offscale/cdd-c/releases/latest/download/cdd-c.wasm");
            then.status(404);
        });
        let mut m_head_zip = server.mock(|when, then| {
            when.method(HEAD)
                .path("/offscale/cdd-c/releases/latest/download/cdd-c-wasm.zip");
            then.status(302);
        });
        let res4 = get_github_release_url(
            &client,
            repo,
            wasm_file,
            zip_file,
            &server.base_url(),
            &server.base_url(),
        )
        .await;
        assert_eq!(
            res4,
            Some((
                format!(
                    "{}/offscale/cdd-c/releases/latest/download/cdd-c-wasm.zip",
                    server.base_url()
                ),
                true
            ))
        );
        m_head_zip.delete();

        // 5. None when all fail
        let _m_head_zip_404 = server.mock(|when, then| {
            when.method(HEAD)
                .path("/offscale/cdd-c/releases/latest/download/cdd-c-wasm.zip");
            then.status(404);
        });
        let res5 = get_github_release_url(
            &client,
            repo,
            wasm_file,
            zip_file,
            &server.base_url(),
            &server.base_url(),
        )
        .await;
        assert!(res5.is_none());
        m_head_wasm_404.delete();

        // 6. Invalid JSON on 200 OK API response + unreachable fallback URLs
        let mut m_bad_json = server.mock(|when, then| {
            when.method(GET).path("/repos/offscale/bad/releases/latest");
            then.status(200).body("not-json");
        });
        let res6 = get_github_release_url(
            &client,
            "offscale/bad",
            wasm_file,
            zip_file,
            &server.base_url(),
            "http://127.0.0.1:1",
        )
        .await;
        assert!(res6.is_none());
        m_bad_json.delete();

        // 7. Unreachable API URL
        let res7 = get_github_release_url(
            &client,
            repo,
            wasm_file,
            zip_file,
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
        )
        .await;
        assert!(res7.is_none());
    }

    #[tokio::test]
    async fn test_fetch_all_wasm_and_extract_zip() -> Result<(), Box<dyn std::error::Error>> {
        let server = MockServer::start();
        let client = Client::new();

        // Build an in-memory zip archive containing a directory, nested file, and slip entry
        let mut zip_buf = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut zip_buf);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = SimpleFileOptions::default();

            zip.add_directory("subdir/", options)?;
            zip.start_file("subdir/hello.txt", options)?;
            zip.write_all(b"hello world")?;
            zip.start_file("nested_no_dir/file.txt", options)?;
            zip.write_all(b"nested content")?;
            zip.start_file("../evil.txt", options)?;
            zip.write_all(b"evil")?;
            zip.finish()?;
        }

        // Mock cdd-rust (.wasm), cdd-csharp (.zip), cdd-go (download failure), cdd-c (missing)
        server.mock(|when, then| {
            when.method(GET)
                .path("/repos/offscale/cdd-rust/releases/latest");
            then.status(200).json_body(serde_json::json!({
                "assets": [{
                    "name": "cdd-rust.wasm",
                    "browser_download_url": format!("{}/dl/cdd-rust.wasm", server.base_url())
                }]
            }));
        });
        server.mock(|when, then| {
            when.method(GET).path("/dl/cdd-rust.wasm");
            then.status(200).body("wasm-bytes");
        });

        server.mock(|when, then| {
            when.method(GET)
                .path("/repos/offscale/cdd-csharp/releases/latest");
            then.status(200).json_body(serde_json::json!({
                "assets": [{
                    "name": "cdd-csharp-wasm.zip",
                    "browser_download_url": format!("{}/dl/cdd-csharp-wasm.zip", server.base_url())
                }]
            }));
        });
        server.mock(|when, then| {
            when.method(GET).path("/dl/cdd-csharp-wasm.zip");
            then.status(200).body(zip_buf);
        });

        server.mock(|when, then| {
            when.method(GET)
                .path("/repos/offscale/cdd-go/releases/latest");
            then.status(200).json_body(serde_json::json!({
                "assets": [{
                    "name": "cdd-go.wasm",
                    "browser_download_url": format!("{}/dl/cdd-go.wasm", server.base_url())
                }]
            }));
        });
        server.mock(|when, then| {
            when.method(GET).path("/dl/cdd-go.wasm");
            then.status(500);
        });

        let temp_root = std::env::temp_dir().join(format!("fetch_wasm_{}", uuid::Uuid::new_v4()));
        let dest_dir = temp_root.join("wasm");
        let support_file = temp_root.join("wasm-support.json");

        let test_repos = &[
            ("offscale/cdd-rust", "wasi"),
            ("offscale/cdd-csharp", "js"),
            ("offscale/cdd-java", "none"),
            ("offscale/cdd-go", "wasi"),
            ("offscale/cdd-c", "wasi"),
        ];

        fetch_all_wasm(
            &client,
            &server.base_url(),
            &server.base_url(),
            &dest_dir,
            &support_file,
            test_repos,
        )
        .await?;

        assert!(dest_dir.join("cdd-rust.wasm").exists());
        assert!(dest_dir.join("cdd-csharp/subdir/hello.txt").exists());
        assert!(dest_dir.join("cdd-csharp/nested_no_dir/file.txt").exists());
        assert!(support_file.exists());

        // Invalid repo format test
        let bad_res = fetch_all_wasm(
            &client,
            &server.base_url(),
            &server.base_url(),
            &dest_dir,
            &support_file,
            &[("invalid_repo_no_slash", "wasi")],
        )
        .await;
        assert!(bad_res.is_err());

        let _ = fs::remove_dir_all(&temp_root);
        Ok(())
    }
}
