use anyhow::Result;
use clap::Parser;
use serde::Deserialize;
use tracing::info;

/// Simple program helps you detect network quality.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[clap(short, long, default_value = "./config.toml")]
    pub conf: String,
}

#[derive(Deserialize)]
pub struct Conf {
    pub agent: Agent,
    pub controller: Controller,
    pub collector: Collector,
}

#[derive(Deserialize)]
pub struct Agent {
    pub id: u32,
}

#[derive(Deserialize)]
pub struct Controller {
    pub url: String,
}

#[derive(Deserialize)]
pub struct Collector {
    pub url: String,
}

pub async fn read_conf() -> Result<Conf> {
    use tokio::fs;

    let args = Args::parse();
    info!("read conf from {}", &args.conf);
    let conf = fs::read_to_string(&args.conf).await?;
    let conf = toml::from_str::<Conf>(&conf)?;

    Ok(conf)
}
