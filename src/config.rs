use serde::{Deserialize, Serialize};
use serde_yaml;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub search_count: u32,
    pub search_terms: Vec<String>,
    pub black_list: Vec<String>,
}

pub fn load() -> Result<Config, Box<dyn std::error::Error>> {
    let mut path = PathBuf::new();
    path.push(env::current_dir()?);
    path.push("config.yaml");

    let content = fs::read_to_string(path)?.parse::<String>()?;
    let config = serde_yaml::from_str::<Config>(content.as_str())?;

    Ok(config)
}
