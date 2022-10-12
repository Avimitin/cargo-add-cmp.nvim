// TODO: Remove this after finish the code
#![allow(unused)]

use nvim_oxi::api::opts::CreateCommandOpts;
use nvim_oxi::api::types::{CommandComplete, CommandArgs};
use nvim_oxi::{self, Function, print};
use serde::Deserialize;
use std::collections::HashMap;
use std::os::unix::thread;

#[derive(Deserialize)]
struct ApiResponse {
    versions: Vec<Version>,
}

#[derive(Deserialize)]
struct Version {
    features: HashMap<Box<str>, Vec<Box<str>>>,
    num: Box<str>,
}

struct CratesIoClient {
    c: reqwest::Client,
    host: &'static str,
}

impl CratesIoClient {
    fn new() -> Self {
        Self {
            c: reqwest::Client::new(),
            host: "https://crates.io",
        }
    }

    async fn get_crate_features(&self, binding: &str) -> anyhow::Result<Vec<Box<str>>> {
        if binding.is_empty() {
            anyhow::bail!("no crate info given");
        }

        let binding: Vec<_> = binding.split('@').collect();
        let mut name;
        let mut ver = None;
        name = binding[0];
        if binding.len() > 1 {
            ver = Some(binding[1]);
        }

        let features = if let Some(ver) = ver {
            self.get_features_from_specific_tag(name, ver).await?
        } else {
            self.get_features_from_latest_tag(name).await?
        };

        Ok(features)
    }

    async fn get_features_from_specific_tag(
        &self,
        name: &str,
        ver: &str,
    ) -> anyhow::Result<Vec<Box<str>>> {
        let endpoint = format!("{}/api/v1/crates/{name}/{ver}", self.host);
        let endpoint =
            reqwest::Url::parse(&endpoint).unwrap_or_else(|_| panic!("invalid url: {endpoint}"));
        let resp = self.c.get(endpoint).send().await?;
        let body = resp.bytes().await?;
        let mut response: HashMap<String, Version> = serde_json::from_slice(&body)?;

        let ver_info = response
            .remove("version")
            .ok_or_else(|| anyhow::anyhow!("expect keyword version, get nothing"))?;
        let features = ver_info.features.into_keys().collect::<Vec<_>>();
        Ok(features)
    }

    async fn get_features_from_latest_tag(&self, name: &str) -> anyhow::Result<Vec<Box<str>>> {
        let endpoint = format!("{}/api/v1/crates/{name}", self.host);
        let endpoint =
            reqwest::Url::parse(&endpoint).unwrap_or_else(|_| panic!("invalid url: {endpoint}"));
        let resp = self
            .c
            .get(endpoint)
            .header("User-Agent", "cargo_feature_cmp_nvim")
            .send()
            .await?;
        let body = resp.bytes().await?;
        let response: ApiResponse = serde_json::from_slice(&body)?;

        if response.versions.is_empty() {
            anyhow::bail!("This crate have empty version information")
        }

        let features = response
            .versions
            .into_iter()
            .next()
            .unwrap()
            .features
            .into_keys()
            .collect::<Vec<_>>();

        Ok(features)
    }
}

async fn list_features(crate_name: &str) -> anyhow::Result<Vec<Box<str>>> {
    let mut client = CratesIoClient::new();
    client.get_crate_features(crate_name).await
}

#[tokio::test]
async fn test_list_features() {
    let features = list_features("serde").await.unwrap();
    assert!(!features.is_empty());

    println!("{features:?}")
}

#[nvim_oxi::module]
fn cargo_add_nvim() -> nvim_oxi::Result<()> {
    let completion = Function::from_fn(generate_completion);
    let completion = CommandComplete::CustomList(completion);
    let opts = CreateCommandOpts::builder()
        .desc("Cargo add command but with completion menu")
        .nargs(nvim_oxi::api::types::CommandNArgs::OneOrMore)
        .complete(completion)
        .bang(false)
        .build();

    let cmd = |args: CommandArgs| {
        let arg = args.args.unwrap_or_else(|| "FUCK".to_string());
        print!("{}", arg);
        Ok(())
    };

    // TODO: Make it actually call `cargo add`
    nvim_oxi::api::create_user_command("CargoAdd", cmd, &opts).unwrap();
    Ok(())
}

fn generate_completion(arguments: (String, String, usize)) -> Result<Vec<String>, nvim_oxi::Error> {
    let (arg_lead, cmd_line, cursor_pos) = arguments;
    // TODO: Get version completion list
    if arg_lead.contains('@') {
        // ...
    }

    if cmd_line.contains(" -F ") {
        let rule =
            regex::Regex::new(r#"CargoAdd ([\w@\.]+) -F ?([\w, ]*)"#).expect("invalid regex rule");
        let capture = rule.captures(&cmd_line);
        if capture.is_none() {
            return Ok(Vec::new());
        }
        let capture = capture.unwrap();
        let crate_name = &capture[1];

        // TODO: Exclude writed features
        // let current_selection = &capture[2];

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let features = runtime.block_on(list_features(crate_name));

        // TODO: logging error to a file or...whatever, just don't mess up the :message panel
        if let Ok(result) = features {
            // FIXME: Maybe I should just deserialize the response into String
            let result = result
                .into_iter()
                .map(|elem| elem.to_string())
                .collect::<Vec<_>>();
            return Ok(result);
        }
    }

    Ok(Vec::new())
}
