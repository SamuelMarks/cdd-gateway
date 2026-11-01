#![deny(missing_docs, clippy::missing_docs_in_private_items)]
//! Database models and strong domain types for `PostgreSQL` persistence.
//!
//! Contains Diesel [`Queryable`], [`Selectable`], and [`Insertable`] structures
//! representing users, organizations, organization memberships, repositories, and releases,
//! as well as the strongly-typed [`OrgRole`], [`TopologyMode`], [`TargetLanguage`],
//! [`LanguageOptionConfig`], and [`DocsConfig`] types.

use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use utoipa::ToSchema;

use crate::db::schema::{organization_users, organizations, releases, repositories, users};
use crate::error::CddGatewayError;

/// Role assigned to a user within an organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum OrgRole {
    /// Full administrative ownership over the organization and its repositories.
    Owner,
    /// Administrative access to manage organization resources.
    Admin,
    /// Standard member access within the organization.
    Member,
}

impl OrgRole {
    /// Returns the canonical lowercase string representation of the role.
    ///
    /// # Arguments
    /// * `&self` - The role enum variant.
    ///
    /// # Returns
    /// * `&'static str` - The string slice `"owner"`, `"admin"`, or `"member"`.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }
}

impl fmt::Display for OrgRole {
    /// Formats the [`OrgRole`] as its canonical lowercase string.
    ///
    /// # Arguments
    /// * `&self` - The role being formatted.
    /// * `f` - The formatter buffer.
    ///
    /// # Returns
    /// * `fmt::Result` - Result of writing to the formatter.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for OrgRole {
    type Err = CddGatewayError;

    /// Parses an [`OrgRole`] from a case-insensitive string slice.
    ///
    /// # Arguments
    /// * `s` - The input role string (`"owner"`, `"admin"`, or `"member"`).
    ///
    /// # Returns
    /// * `Ok(OrgRole)` - The parsed organization role.
    ///
    /// # Errors
    /// * [`CddGatewayError::Validation`] - If the string does not match a known role.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "member" => Ok(Self::Member),
            other => Err(CddGatewayError::Validation(format!(
                "Invalid organization role: {other}"
            ))),
        }
    }
}

/// Repository provisioning topology mode.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TopologyMode {
    /// Provision a single monorepo containing all selected target language subdirectories.
    #[default]
    Monorepo,
    /// Provision an individual repository per selected target language.
    Polyrepo,
}

impl TopologyMode {
    /// Returns the canonical lowercase string representation of the topology mode.
    ///
    /// # Arguments
    /// * `&self` - The topology mode enum variant.
    ///
    /// # Returns
    /// * `&'static str` - `"monorepo"` or `"polyrepo"`.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Monorepo => "monorepo",
            Self::Polyrepo => "polyrepo",
        }
    }
}

impl fmt::Display for TopologyMode {
    /// Formats the [`TopologyMode`] as its canonical lowercase string.
    ///
    /// # Arguments
    /// * `&self` - The topology mode being formatted.
    /// * `f` - The formatter buffer.
    ///
    /// # Returns
    /// * `fmt::Result` - Result of writing to the formatter.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TopologyMode {
    type Err = CddGatewayError;

    /// Parses a [`TopologyMode`] from a case-insensitive string slice.
    ///
    /// # Arguments
    /// * `s` - The input topology mode string (`"monorepo"` or `"polyrepo"`).
    ///
    /// # Returns
    /// * `Ok(TopologyMode)` - The parsed topology mode.
    ///
    /// # Errors
    /// * [`CddGatewayError::Validation`] - If the string does not match a known topology mode.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "monorepo" => Ok(Self::Monorepo),
            "polyrepo" => Ok(Self::Polyrepo),
            other => Err(CddGatewayError::Validation(format!(
                "Invalid topology mode: {other}"
            ))),
        }
    }
}

/// Supported CDD ecosystem target languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum TargetLanguage {
    /// C (`c` / `cdd-c`).
    #[serde(rename = "c", alias = "cdd-c")]
    C,
    /// C++ (`cpp` / `cdd-cpp`).
    #[serde(rename = "cpp", alias = "cdd-cpp")]
    Cpp,
    /// C# (`csharp` / `cdd-csharp`).
    #[serde(rename = "csharp", alias = "cdd-csharp")]
    Csharp,
    /// Go (`go` / `cdd-go`).
    #[serde(rename = "go", alias = "cdd-go")]
    Go,
    /// Java (`java` / `cdd-java`).
    #[serde(rename = "java", alias = "cdd-java")]
    Java,
    /// Kotlin (`kotlin` / `cdd-kotlin`).
    #[serde(rename = "kotlin", alias = "cdd-kotlin")]
    Kotlin,
    /// PHP (`php` / `cdd-php`).
    #[serde(rename = "php", alias = "cdd-php")]
    Php,
    /// Python (`python-all` / `python` / `cdd-python-all`).
    #[serde(
        rename = "python-all",
        alias = "python",
        alias = "cdd-python-all",
        alias = "cdd-python"
    )]
    PythonAll,
    /// Ruby (`ruby` / `cdd-ruby`).
    #[serde(rename = "ruby", alias = "cdd-ruby")]
    Ruby,
    /// Rust (`rust` / `cdd-rust`).
    #[serde(rename = "rust", alias = "cdd-rust")]
    Rust,
    /// Shell (`sh` / `cdd-sh`).
    #[serde(rename = "sh", alias = "cdd-sh")]
    Sh,
    /// Swift (`swift` / `cdd-swift`).
    #[serde(rename = "swift", alias = "cdd-swift")]
    Swift,
    /// TypeScript (`ts` / `typescript` / `cdd-ts`).
    #[serde(rename = "ts", alias = "typescript", alias = "cdd-ts")]
    Ts,
}

impl TargetLanguage {
    /// Returns all 13 supported [`TargetLanguage`] variants.
    ///
    /// # Returns
    /// * `&'static [TargetLanguage]` - Slice containing all 13 target languages.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::C,
            Self::Cpp,
            Self::Csharp,
            Self::Go,
            Self::Java,
            Self::Kotlin,
            Self::Php,
            Self::PythonAll,
            Self::Ruby,
            Self::Rust,
            Self::Sh,
            Self::Swift,
            Self::Ts,
        ]
    }

    /// Returns the canonical short identifier of the target language (e.g. `"rust"`, `"python-all"`).
    ///
    /// # Arguments
    /// * `&self` - The target language variant.
    ///
    /// # Returns
    /// * `&'static str` - Canonical language slug.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Csharp => "csharp",
            Self::Go => "go",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::Php => "php",
            Self::PythonAll => "python-all",
            Self::Ruby => "ruby",
            Self::Rust => "rust",
            Self::Sh => "sh",
            Self::Swift => "swift",
            Self::Ts => "ts",
        }
    }

    /// Returns the canonical CDD compiler binary name (e.g. `"cdd-rust"`, `"cdd-python-all"`).
    ///
    /// # Arguments
    /// * `&self` - The target language variant.
    ///
    /// # Returns
    /// * `&'static str` - Canonical `cdd-<lang>` binary name.
    #[must_use]
    pub const fn binary_name(&self) -> &'static str {
        match self {
            Self::C => "cdd-c",
            Self::Cpp => "cdd-cpp",
            Self::Csharp => "cdd-csharp",
            Self::Go => "cdd-go",
            Self::Java => "cdd-java",
            Self::Kotlin => "cdd-kotlin",
            Self::Php => "cdd-php",
            Self::PythonAll => "cdd-python-all",
            Self::Ruby => "cdd-ruby",
            Self::Rust => "cdd-rust",
            Self::Sh => "cdd-sh",
            Self::Swift => "cdd-swift",
            Self::Ts => "cdd-ts",
        }
    }
}

impl fmt::Display for TargetLanguage {
    /// Formats the [`TargetLanguage`] as its canonical short identifier.
    ///
    /// # Arguments
    /// * `&self` - The language being formatted.
    /// * `f` - The formatter buffer.
    ///
    /// # Returns
    /// * `fmt::Result` - Result of writing to the formatter.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TargetLanguage {
    type Err = CddGatewayError;

    /// Parses a [`TargetLanguage`] from a case-insensitive language or `cdd-*` binary identifier.
    ///
    /// # Arguments
    /// * `s` - Input language string (e.g. `"rust"`, `"cdd-rust"`, `"python"`, `"python-all"`).
    ///
    /// # Returns
    /// * `Ok(TargetLanguage)` - The parsed target language.
    ///
    /// # Errors
    /// * [`CddGatewayError::Validation`] - If `s` does not match any supported target language.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "c" | "cdd-c" => Ok(Self::C),
            "cpp" | "c++" | "cdd-cpp" => Ok(Self::Cpp),
            "csharp" | "c#" | "cdd-csharp" => Ok(Self::Csharp),
            "go" | "golang" | "cdd-go" => Ok(Self::Go),
            "java" | "cdd-java" => Ok(Self::Java),
            "kotlin" | "cdd-kotlin" => Ok(Self::Kotlin),
            "php" | "cdd-php" => Ok(Self::Php),
            "python" | "python-all" | "cdd-python" | "cdd-python-all" => Ok(Self::PythonAll),
            "ruby" | "cdd-ruby" => Ok(Self::Ruby),
            "rust" | "cdd-rust" => Ok(Self::Rust),
            "sh" | "shell" | "cdd-sh" => Ok(Self::Sh),
            "swift" | "cdd-swift" => Ok(Self::Swift),
            "ts" | "typescript" | "cdd-ts" => Ok(Self::Ts),
            other => Err(CddGatewayError::Validation(format!(
                "Unsupported target language: {other}"
            ))),
        }
    }
}

/// Per-language generator options configured for a repository.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct LanguageOptionConfig {
    /// Whether to generate an SDK client (`to_sdk`).
    #[serde(default)]
    pub sdk: bool,
    /// Whether to generate a CLI (`to_sdk_cli`).
    #[serde(default)]
    pub cli: bool,
    /// Whether to generate an MCP server (`--mcp`).
    #[serde(default)]
    pub mcp: bool,
    /// Whether to generate an Angular frontend (`cdd-ts` `--framework angular`).
    #[serde(default)]
    pub angular: bool,
    /// Optional server or client framework name (e.g. `"actix"`, `"fastapi"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    /// Optional ORM name (e.g. `"diesel"`, `"sqlalchemy"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orm: Option<String>,
    /// Optional custom GitHub repository name override when provisioning in polyrepo mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_repo_name: Option<String>,
}

/// Documentation hosting and theme configuration for a repository.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DocsConfig {
    /// Whether API documentation generation and hosting are enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Optional custom domain name for hosted API documentation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_domain: Option<String>,
    /// Optional UI theme identifier (e.g. `"dark"`, `"light"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
}

/// Configuration metadata describing repository topology, languages, registries, and docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RepositoryConfigMetadata {
    /// GitHub organization or user login handle (`${GITHUB_ORG_NAME}`).
    #[serde(default)]
    pub github_org_name: String,
    /// Repository topology mode (`monorepo` or `polyrepo`).
    #[serde(default)]
    pub topology_mode: TopologyMode,
    /// Selected target languages for code generation.
    #[serde(default)]
    pub target_languages: Vec<TargetLanguage>,
    /// Per-language generator options keyed by canonical language name.
    #[serde(default)]
    pub language_options: HashMap<String, LanguageOptionConfig>,
    /// Enabled package release registries (e.g. `"crates.io"`, `"pypi"`, `"npm"`).
    #[serde(default)]
    pub release_registries: Vec<String>,
    /// Whether PGP signing is enabled for releases.
    #[serde(default)]
    pub enable_pgp_signing: bool,
    /// Documentation generation and hosting configuration.
    #[serde(default)]
    pub docs_config: DocsConfig,
}

impl Default for RepositoryConfigMetadata {
    /// Creates a [`RepositoryConfigMetadata`] with empty defaults and [`TopologyMode::Monorepo`].
    ///
    /// # Returns
    /// * [`RepositoryConfigMetadata`] - Default metadata configuration.
    fn default() -> Self {
        Self {
            github_org_name: String::new(),
            topology_mode: TopologyMode::Monorepo,
            target_languages: Vec::new(),
            language_options: HashMap::new(),
            release_registries: Vec::new(),
            enable_pgp_signing: false,
            docs_config: DocsConfig::default(),
        }
    }
}

/// User model representing an authenticated local user, GitHub user, or Google user.
#[derive(
    Debug,
    Clone,
    Queryable,
    Selectable,
    Insertable,
    Identifiable,
    Serialize,
    Deserialize,
    ToSchema,
    PartialEq,
    Eq,
)]
#[diesel(table_name = users)]
pub struct User {
    /// Primary key identifier of the user.
    pub id: i32,
    /// Optional GitHub numeric user ID.
    pub github_id: Option<i64>,
    /// Unique login username.
    pub username: String,
    /// User's primary email address.
    pub email: String,
    /// Optional Argon2 password hash (never serialized in API responses).
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    /// Optional Google OAuth subject identifier (`sub`).
    #[serde(default)]
    pub google_id: Option<String>,
}

/// Insertable payload for creating a new [`User`] row.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    /// Optional GitHub numeric user ID.
    pub github_id: Option<i64>,
    /// Unique login username.
    pub username: &'a str,
    /// User's primary email address.
    pub email: &'a str,
    /// Optional Argon2 password hash.
    pub password_hash: Option<&'a str>,
    /// Optional Google OAuth subject identifier (`sub`).
    pub google_id: Option<&'a str>,
}

/// Organization model representing a logical grouping of users and repositories.
#[derive(
    Debug,
    Clone,
    Queryable,
    Selectable,
    Insertable,
    Identifiable,
    Serialize,
    Deserialize,
    ToSchema,
    PartialEq,
    Eq,
)]
#[diesel(table_name = organizations)]
pub struct Organization {
    /// Primary key identifier of the organization.
    pub id: i32,
    /// Optional GitHub numeric organization ID.
    pub github_id: Option<i64>,
    /// Unique organization login handle.
    pub login: String,
    /// Optional human-readable description of the organization.
    pub description: Option<String>,
}

/// Insertable payload for creating a new [`Organization`] row.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = organizations)]
pub struct NewOrganization<'a> {
    /// Optional GitHub numeric organization ID.
    pub github_id: Option<i64>,
    /// Unique organization login handle.
    pub login: &'a str,
    /// Optional human-readable description of the organization.
    pub description: Option<&'a str>,
}

/// Join record linking a [`User`] to an [`Organization`] with a specific role.
#[derive(
    Debug,
    Clone,
    Queryable,
    Selectable,
    Insertable,
    Identifiable,
    Serialize,
    Deserialize,
    ToSchema,
    PartialEq,
    Eq,
)]
#[diesel(table_name = organization_users)]
#[diesel(primary_key(organization_id, user_id))]
pub struct OrganizationUser {
    /// Foreign key referencing [`Organization::id`].
    pub organization_id: i32,
    /// Foreign key referencing [`User::id`].
    pub user_id: i32,
    /// Assigned role name (e.g. `"owner"`, `"admin"`, `"member"`).
    pub role: String,
}

/// Insertable payload for linking a user to an organization.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = organization_users)]
pub struct NewOrganizationUser<'a> {
    /// Foreign key referencing [`Organization::id`].
    pub organization_id: i32,
    /// Foreign key referencing [`User::id`].
    pub user_id: i32,
    /// Assigned role string slice.
    pub role: &'a str,
}

/// Repository model representing an SDK or code repository owned by an organization.
#[derive(
    Debug,
    Clone,
    Queryable,
    Selectable,
    Insertable,
    Identifiable,
    Serialize,
    Deserialize,
    ToSchema,
    PartialEq,
    Eq,
)]
#[diesel(table_name = repositories)]
pub struct Repository {
    /// Primary key identifier of the repository.
    pub id: i32,
    /// Foreign key referencing the owning [`Organization::id`].
    pub organization_id: i32,
    /// Optional GitHub numeric repository ID.
    pub github_id: Option<i64>,
    /// Repository name within the organization.
    pub name: String,
    /// Optional human-readable description of the repository.
    pub description: Option<String>,
    /// GitHub organization or user namespace (`${GITHUB_ORG_NAME}`).
    #[serde(default)]
    pub github_org_name: String,
    /// Topology mode string (`"monorepo"` or `"polyrepo"`).
    #[serde(default = "default_topology_mode_str")]
    pub topology_mode: String,
    /// JSON-serialized list of target languages (`Vec<TargetLanguage>`).
    #[serde(default = "default_empty_json_array")]
    pub target_languages_json: String,
    /// JSON-serialized map of per-language generator options (`HashMap<String, LanguageOptionConfig>`).
    #[serde(default = "default_empty_json_object")]
    pub language_options_json: String,
    /// JSON-serialized list of enabled release registries (`Vec<String>`).
    #[serde(default = "default_empty_json_array")]
    pub release_registries_json: String,
    /// Whether PGP signing is enabled for releases.
    #[serde(default)]
    pub enable_pgp_signing: bool,
    /// JSON-serialized documentation configuration (`DocsConfig`).
    #[serde(default = "default_empty_json_object")]
    pub docs_config_json: String,
}

/// Returns `"monorepo"` as the default topology mode string for serde deserialization.
fn default_topology_mode_str() -> String {
    "monorepo".to_string()
}

/// Returns `"[]"` as the default empty JSON array string for serde deserialization.
fn default_empty_json_array() -> String {
    "[]".to_string()
}

/// Returns `"{}"` as the default empty JSON object string for serde deserialization.
fn default_empty_json_object() -> String {
    "{}".to_string()
}

/// Insertable payload for creating a new [`Repository`] row.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = repositories)]
pub struct NewRepository<'a> {
    /// Foreign key referencing the owning [`Organization::id`].
    pub organization_id: i32,
    /// Optional GitHub numeric repository ID.
    pub github_id: Option<i64>,
    /// Repository name within the organization.
    pub name: &'a str,
    /// Optional human-readable description of the repository.
    pub description: Option<&'a str>,
    /// GitHub organization or user namespace (`${GITHUB_ORG_NAME}`).
    pub github_org_name: &'a str,
    /// Topology mode string (`"monorepo"` or `"polyrepo"`).
    pub topology_mode: &'a str,
    /// JSON-serialized list of target languages.
    pub target_languages_json: &'a str,
    /// JSON-serialized map of per-language generator options.
    pub language_options_json: &'a str,
    /// JSON-serialized list of release registries.
    pub release_registries_json: &'a str,
    /// Whether PGP signing is enabled for releases.
    pub enable_pgp_signing: bool,
    /// JSON-serialized documentation configuration.
    pub docs_config_json: &'a str,
}

/// Release model representing a versioned tag/release of a [`Repository`].
#[derive(
    Debug,
    Clone,
    Queryable,
    Selectable,
    Insertable,
    Identifiable,
    Serialize,
    Deserialize,
    ToSchema,
    PartialEq,
    Eq,
)]
#[diesel(table_name = releases)]
pub struct Release {
    /// Primary key identifier of the release.
    pub id: i32,
    /// Foreign key referencing [`Repository::id`].
    pub repository_id: i32,
    /// Optional GitHub numeric release ID.
    pub github_id: Option<i64>,
    /// Git tag name associated with the release (e.g. `"v1.0.0"`).
    pub tag_name: String,
    /// Optional release title.
    pub name: Option<String>,
    /// Optional release notes / changelog body.
    pub body: Option<String>,
}

/// Insertable payload for creating a new [`Release`] row.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = releases)]
pub struct NewRelease<'a> {
    /// Foreign key referencing [`Repository::id`].
    pub repository_id: i32,
    /// Optional GitHub numeric release ID.
    pub github_id: Option<i64>,
    /// Git tag name associated with the release.
    pub tag_name: &'a str,
    /// Optional release title.
    pub name: Option<&'a str>,
    /// Optional release notes / changelog body.
    pub body: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_org_role_display_and_from_str() -> Result<(), CddGatewayError> {
        assert_eq!(OrgRole::Owner.as_str(), "owner");
        assert_eq!(OrgRole::Admin.as_str(), "admin");
        assert_eq!(OrgRole::Member.as_str(), "member");

        assert_eq!(OrgRole::Owner.to_string(), "owner");
        assert_eq!(OrgRole::Admin.to_string(), "admin");
        assert_eq!(OrgRole::Member.to_string(), "member");

        assert_eq!("owner".parse::<OrgRole>()?, OrgRole::Owner);
        assert_eq!("ADMIN".parse::<OrgRole>()?, OrgRole::Admin);
        assert_eq!("Member".parse::<OrgRole>()?, OrgRole::Member);
        assert!("guest".parse::<OrgRole>().is_err());
        Ok(())
    }

    #[test]
    fn test_topology_mode_display_and_from_str() -> Result<(), CddGatewayError> {
        assert_eq!(TopologyMode::default(), TopologyMode::Monorepo);
        assert_eq!(TopologyMode::Monorepo.as_str(), "monorepo");
        assert_eq!(TopologyMode::Polyrepo.as_str(), "polyrepo");
        assert_eq!(TopologyMode::Monorepo.to_string(), "monorepo");
        assert_eq!(TopologyMode::Polyrepo.to_string(), "polyrepo");

        assert_eq!("monorepo".parse::<TopologyMode>()?, TopologyMode::Monorepo);
        assert_eq!("POLYREPO".parse::<TopologyMode>()?, TopologyMode::Polyrepo);
        assert!("invalid".parse::<TopologyMode>().is_err());
        Ok(())
    }

    #[test]
    fn test_target_language_all_display_and_from_str() -> Result<(), CddGatewayError> {
        let all = TargetLanguage::all();
        assert_eq!(all.len(), 13);
        for lang in all {
            let slug = lang.as_str();
            let bin = lang.binary_name();
            assert_eq!(lang.to_string(), slug);
            assert_eq!(slug.parse::<TargetLanguage>()?, *lang);
            assert_eq!(bin.parse::<TargetLanguage>()?, *lang);
        }
        assert_eq!("c++".parse::<TargetLanguage>()?, TargetLanguage::Cpp);
        assert_eq!("c#".parse::<TargetLanguage>()?, TargetLanguage::Csharp);
        assert_eq!("golang".parse::<TargetLanguage>()?, TargetLanguage::Go);
        assert_eq!(
            "python".parse::<TargetLanguage>()?,
            TargetLanguage::PythonAll
        );
        assert_eq!(
            "cdd-python".parse::<TargetLanguage>()?,
            TargetLanguage::PythonAll
        );
        assert_eq!("shell".parse::<TargetLanguage>()?, TargetLanguage::Sh);
        assert_eq!("typescript".parse::<TargetLanguage>()?, TargetLanguage::Ts);
        assert!("brainfuck".parse::<TargetLanguage>().is_err());
        Ok(())
    }

    #[test]
    fn test_repository_serde_defaults_and_metadata() -> Result<(), CddGatewayError> {
        let meta = RepositoryConfigMetadata::default();
        assert_eq!(meta.topology_mode, TopologyMode::Monorepo);
        assert_eq!(meta.github_org_name, "");
        assert_eq!(meta.target_languages, [] as [TargetLanguage; 0]);

        let json_str =
            r#"{"id":1,"organization_id":2,"github_id":null,"name":"r","description":null}"#;
        let repo: Repository = serde_json::from_str(json_str)?;
        assert_eq!(repo.topology_mode, "monorepo");
        assert_eq!(repo.target_languages_json, "[]");
        assert_eq!(repo.language_options_json, "{}");
        assert_eq!(repo.release_registries_json, "[]");
        assert!(!repo.enable_pgp_signing);
        assert_eq!(repo.docs_config_json, "{}");
        Ok(())
    }
}
