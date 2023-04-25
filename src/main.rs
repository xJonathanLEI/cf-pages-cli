use std::{collections::BTreeMap, io::Write, path::PathBuf, time::Duration};

use anyhow::Result;
use clap::{Parser, Subcommand};
use reqwest::blocking::ClientBuilder;
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Subcommands,
}

#[derive(Debug, Subcommand)]
enum Subcommands {
    #[clap(about = "Download environment variables into a local JSON file")]
    GetEnvVars(GetEnvVars),
    #[clap(about = "Upload environment variables from a local JSON file")]
    SetEnvVars(SetEnvVars),
}

#[derive(Debug, Parser)]
pub struct GetEnvVars {
    #[clap(flatten)]
    credentials: CredentialsArgs,
    #[clap(long, env = "CF_PAGES_PROJECT", help = "Name of the Pages project")]
    project: String,
    #[clap(
        long,
        env = "CF_PAGES_PATH",
        help = "Path to save the JSON file. Prints to stdout if not provided"
    )]
    path: Option<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct SetEnvVars {
    #[clap(flatten)]
    credentials: CredentialsArgs,
    #[clap(long, env = "CF_PAGES_PROJECT", help = "Name of the Pages project")]
    project: String,
    #[clap(
        long,
        env = "CF_PAGES_PATH",
        help = "Path to the file containing desired environment variables"
    )]
    path: PathBuf,
}

#[derive(Debug, Clone, Parser)]
struct CredentialsArgs {
    #[clap(long, env = "CLOUDFLARE_ACCOUNT", help = "Cloudflare account ID")]
    account: String,
    #[clap(long, env = "CLOUDFLARE_TOKEN", help = "Cloudflare access token")]
    token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CloudflareResponse<T> {
    result: T,
    success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CloudflarePagesProject {
    id: String,
    name: String,
    deployment_configs: CloudflarePagesDeploymentConfigs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CloudflarePagesPatchRequest {
    deployment_configs: CloudflarePagesDeploymentConfigs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CloudflarePagesDeploymentConfigs {
    preview: CloudflarePagesEnvironment,
    production: CloudflarePagesEnvironment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CloudflarePagesEnvironment {
    env_vars: Option<BTreeMap<String, Option<CloudflarePagesEnvVarValue>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CloudflarePagesEnvVarValue {
    r#type: CloudflarePagesEnvVarValueType,
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CloudflarePagesEnvVarValueType {
    PlainText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnvVarsFile {
    production: BTreeMap<String, String>,
    preview: BTreeMap<String, String>,
}

impl GetEnvVars {
    fn run(self) -> Result<()> {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(10))
            .build()?;

        let project_response: CloudflareResponse<CloudflarePagesProject> = client
            .get(format!(
                "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}",
                self.credentials.account, self.project
            ))
            .header(
                "Authorization",
                format!("Bearer {}", self.credentials.token),
            )
            .send()?
            .json()?;
        if !project_response.success {
            anyhow::bail!("unsuccessful Cloudflare request");
        }

        let existing_vars: EnvVarsFile = project_response.result.deployment_configs.into();

        if let Some(path) = self.path {
            let mut dump_file = std::fs::File::create(&path)?;
            serde_json::to_writer_pretty(&mut dump_file, &existing_vars)?;

            // EOF line for Unix platforms
            writeln!(&mut dump_file)?;

            println!(
                "Environment variables written to: {}",
                path.to_string_lossy()
            );
        } else {
            let json = serde_json::to_string_pretty(&existing_vars)?;
            println!("{json}");
        }

        Ok(())
    }
}

impl SetEnvVars {
    fn run(self) -> Result<()> {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(10))
            .build()?;

        let project_response: CloudflareResponse<CloudflarePagesProject> = client
            .get(format!(
                "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}",
                self.credentials.account, self.project
            ))
            .header(
                "Authorization",
                format!("Bearer {}", self.credentials.token),
            )
            .send()?
            .json()?;
        if !project_response.success {
            anyhow::bail!("unsuccessful Cloudflare request");
        }

        let existing_vars: EnvVarsFile = project_response.result.deployment_configs.into();

        let new_vars: EnvVarsFile = serde_json::from_reader(&mut std::fs::File::open(&self.path)?)?;

        let deployment_configs_patch = generate_deployment_configs_patch(&existing_vars, &new_vars);
        if deployment_configs_patch.is_empty() {
            println!("No changes detected. Not submitting patch.");
        } else {
            let patch_response: CloudflareResponse<CloudflarePagesProject> = client
                .patch(format!(
                    "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}",
                    self.credentials.account, self.project
                ))
                .header(
                    "Authorization",
                    format!("Bearer {}", self.credentials.token),
                )
                .json(&CloudflarePagesPatchRequest {
                    deployment_configs: deployment_configs_patch,
                })
                .send()?
                .json()?;
            if !patch_response.success {
                anyhow::bail!("unsuccessful Cloudflare request");
            }

            println!("Environment variables successfully updated");
        }

        Ok(())
    }
}

impl CloudflarePagesDeploymentConfigs {
    pub fn is_empty(&self) -> bool {
        let is_preview_empty = match &self.preview.env_vars {
            Some(preview) => preview.is_empty(),
            None => true,
        };
        let is_production_empty = match &self.production.env_vars {
            Some(production) => production.is_empty(),
            None => true,
        };

        is_preview_empty && is_production_empty
    }
}

impl From<CloudflarePagesDeploymentConfigs> for EnvVarsFile {
    fn from(value: CloudflarePagesDeploymentConfigs) -> Self {
        Self {
            production: value.production.into(),
            preview: value.preview.into(),
        }
    }
}

impl From<CloudflarePagesEnvironment> for BTreeMap<String, String> {
    fn from(value: CloudflarePagesEnvironment) -> Self {
        match value.env_vars {
            Some(env_vars) => env_vars
                .into_iter()
                .map(|(key, value)| {
                    (
                        key,
                        value.map(|var_value| var_value.value).unwrap_or_default(),
                    )
                })
                .collect(),
            None => Self::default(),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Subcommands::GetEnvVars(cmd) => cmd.run()?,
        Subcommands::SetEnvVars(cmd) => cmd.run()?,
    }

    Ok(())
}

fn generate_deployment_configs_patch(
    old_vars: &EnvVarsFile,
    new_vars: &EnvVarsFile,
) -> CloudflarePagesDeploymentConfigs {
    CloudflarePagesDeploymentConfigs {
        preview: generate_env_patch(&old_vars.preview, &new_vars.preview),
        production: generate_env_patch(&old_vars.production, &new_vars.production),
    }
}

fn generate_env_patch(
    old_env: &BTreeMap<String, String>,
    new_env: &BTreeMap<String, String>,
) -> CloudflarePagesEnvironment {
    let mut changes: BTreeMap<String, Option<CloudflarePagesEnvVarValue>> = Default::default();

    // Finds new and changed variables
    new_env
        .iter()
        .filter(|(key, value)| match old_env.get(*key) {
            Some(old_value) => {
                // Keep the patch minimal: do not generate entry if not necessary
                *value != old_value
            }
            None => {
                // This is a new env var
                true
            }
        })
        .for_each(|(key, value)| {
            changes.insert(
                key.to_owned(),
                Some(CloudflarePagesEnvVarValue {
                    r#type: CloudflarePagesEnvVarValueType::PlainText,
                    value: value.to_owned(),
                }),
            );
        });

    // Finds removed variables and generates null entries
    old_env
        .iter()
        .filter(|(key, _)| !new_env.contains_key(*key))
        .for_each(|(key, _)| {
            changes.insert(key.to_owned(), None);
        });

    CloudflarePagesEnvironment {
        env_vars: Some(changes),
    }
}
