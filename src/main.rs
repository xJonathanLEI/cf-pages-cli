use std::{collections::BTreeMap, io::Write, path::PathBuf, str::FromStr, time::Duration};

use anyhow::Result;
use clap::{builder::PossibleValue, Parser, Subcommand, ValueEnum};
use reqwest::blocking::ClientBuilder;
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Subcommands,
}

#[derive(Debug, Clone, Copy)]
enum Environment {
    Production,
    Preview,
}

#[derive(Debug, Subcommand)]
enum Subcommands {
    #[clap(about = "Download environment variables into a local JSON file")]
    GetEnvVars(GetEnvVars),
    #[clap(about = "Upload environment variables from a local JSON file")]
    SetEnvVars(SetEnvVars),
    #[clap(about = "Generate .env file for front-end development")]
    ToEnvFile(ToEnvFile),
}

#[derive(Debug, Parser)]
pub struct GetEnvVars {
    #[clap(flatten)]
    credentials: CredentialsArgs,
    #[clap(long, env = "CF_PAGES_PROJECT", help = "Name of the Pages project")]
    project: String,
    #[clap(long, env = "CF_PAGES_DEPLOYMENT", help = "Deployment ID")]
    deployment: Option<String>,
    #[clap(
        long,
        env = "CF_PAGES_OUTPUT",
        help = "Path to save the JSON file. Prints to stdout if not provided"
    )]
    output: Option<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct SetEnvVars {
    #[clap(flatten)]
    credentials: CredentialsArgs,
    #[clap(long, env = "CF_PAGES_PROJECT", help = "Name of the Pages project")]
    project: String,
    #[clap(
        long,
        env = "CF_PAGES_FILE",
        help = "Path to the file containing desired environment variables"
    )]
    file: PathBuf,
}

#[derive(Debug, Parser)]
pub struct ToEnvFile {
    #[clap(
        long,
        env = "CF_PAGES_ENVIRONMENT",
        default_value = "production",
        help = "Environment to export"
    )]
    environment: Environment,
    #[clap(
        long,
        env = "CF_PAGES_EMPTY",
        help = "Emit the variable names only, with empty values"
    )]
    empty: bool,
    #[clap(
        long,
        env = "CF_PAGES_OUTPUT",
        help = "Path to save the .env file. Prints to stdout if not provided"
    )]
    output: Option<PathBuf>,
    #[clap(help = "Path to the JSON file containing environment variables")]
    file: String,
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
struct CloudflarePagesDeployment {
    id: String,
    environment: Environment,
    #[serde(flatten)]
    vars: CloudflarePagesEnvironment,
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
struct FullEnvVarsFile {
    production: BTreeMap<String, String>,
    preview: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnvVarsFile {
    production: Option<BTreeMap<String, String>>,
    preview: Option<BTreeMap<String, String>>,
}

impl FromStr for Environment {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "production" => Ok(Self::Production),
            "preview" => Ok(Self::Preview),
            _ => Err("unknown value"),
        }
    }
}

impl ValueEnum for Environment {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Production, Self::Preview]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            Environment::Production => Some(PossibleValue::new("production")),
            Environment::Preview => Some(PossibleValue::new("preview")),
        }
    }
}

impl Serialize for Environment {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            Environment::Production => "production",
            Environment::Preview => "preview",
        })
    }
}

impl<'de> Deserialize<'de> for Environment {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.parse() {
            Ok(value) => Ok(value),
            Err(err) => Err(serde::de::Error::custom(format!(
                "invalid environment string: {err}"
            ))),
        }
    }
}

impl GetEnvVars {
    fn run(self) -> Result<()> {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(10))
            .build()?;

        let existing_vars: EnvVarsFile = if let Some(deployment) = self.deployment {
            let deployment_response: CloudflareResponse<CloudflarePagesDeployment> = client
                .get(format!(
                    "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}/deployments/{}",
                    self.credentials.account, self.project, deployment
                ))
                .header(
                    "Authorization",
                    format!("Bearer {}", self.credentials.token),
                )
                .send()?
                .json()?;
            if !deployment_response.success {
                anyhow::bail!("unsuccessful Cloudflare request");
            }

            let deployment = deployment_response.result;
            let vars: BTreeMap<String, String> = deployment.vars.into();

            match deployment.environment {
                Environment::Production => EnvVarsFile {
                    production: Some(vars),
                    preview: None,
                },
                Environment::Preview => EnvVarsFile {
                    production: None,
                    preview: Some(vars),
                },
            }
        } else {
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

            project_response.result.deployment_configs.into()
        };

        if let Some(output) = self.output {
            let mut dump_file = std::fs::File::create(&output)?;
            serde_json::to_writer_pretty(&mut dump_file, &existing_vars)?;

            // EOF line for Unix platforms
            writeln!(&mut dump_file)?;

            println!(
                "Environment variables written to: {}",
                output.to_string_lossy()
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

        let existing_vars: FullEnvVarsFile = project_response.result.deployment_configs.into();

        let new_vars: EnvVarsFile = serde_json::from_reader(&mut std::fs::File::open(&self.file)?)?;

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

impl ToEnvFile {
    fn run(self) -> Result<()> {
        let all_vars: EnvVarsFile = serde_json::from_reader(&mut std::fs::File::open(self.file)?)?;
        let target_env_vars = match self.environment {
            Environment::Production => all_vars.production,
            Environment::Preview => all_vars.preview,
        };

        let target_env_vars = match target_env_vars {
            Some(value) => value,
            None => anyhow::bail!("empty environment"),
        };

        let mut buffer = String::new();

        for (key, value) in target_env_vars.iter() {
            if self.empty {
                buffer.push_str(&format!("{}=\"\"\n", key));
            } else {
                buffer.push_str(&format!("{}={}\n", key, serde_json::to_string(value)?));
            }
        }

        if let Some(output) = self.output {
            let mut dump_file = std::fs::File::create(&output)?;
            dump_file.write_all(buffer.as_bytes())?;

            println!(
                "Environment variables written to: {}",
                output.to_string_lossy()
            );
        } else {
            print!("{buffer}");
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

impl From<CloudflarePagesDeploymentConfigs> for FullEnvVarsFile {
    fn from(value: CloudflarePagesDeploymentConfigs) -> Self {
        Self {
            production: value.production.into(),
            preview: value.preview.into(),
        }
    }
}

impl From<CloudflarePagesDeploymentConfigs> for EnvVarsFile {
    fn from(value: CloudflarePagesDeploymentConfigs) -> Self {
        Self {
            production: Some(value.production.into()),
            preview: Some(value.preview.into()),
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
        Subcommands::ToEnvFile(cmd) => cmd.run()?,
    }

    Ok(())
}

fn generate_deployment_configs_patch(
    old_vars: &FullEnvVarsFile,
    new_vars: &EnvVarsFile,
) -> CloudflarePagesDeploymentConfigs {
    CloudflarePagesDeploymentConfigs {
        preview: generate_env_patch(&old_vars.preview, &new_vars.preview),
        production: generate_env_patch(&old_vars.production, &new_vars.production),
    }
}

fn generate_env_patch(
    old_env: &BTreeMap<String, String>,
    new_env: &Option<BTreeMap<String, String>>,
) -> CloudflarePagesEnvironment {
    let mut changes: BTreeMap<String, Option<CloudflarePagesEnvVarValue>> = Default::default();

    if let Some(new_env) = new_env.as_ref() {
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
    }

    CloudflarePagesEnvironment {
        env_vars: Some(changes),
    }
}
