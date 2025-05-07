use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use figment::{
    Figment,
    providers::{Format, Yaml},
};
use serde::Deserialize;

#[derive(Parser)]
pub struct Args {
    #[clap(short, long)]
    config: Vec<PathBuf>,
}

#[derive(Deserialize, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub data_dir: PathBuf,
    pub utxorpc: UtxorpcConfig,
}

#[derive(Deserialize, Clone)]
pub struct UtxorpcConfig {
    pub endpoint_url: String,
    pub headers: Option<HashMap<String, String>>,
}

impl AppConfig {
    pub fn load(args: Args) -> Result<Self> {
        let mut figment = Figment::new().merge(Yaml::string(include_str!("../config.base.yaml")));
        for config_file in args.config {
            figment = figment.merge(Yaml::file(config_file))
        }

        Ok(figment.extract()?)
    }
}
