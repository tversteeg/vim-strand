use anyhow::Result;
use async_std::fs;
use std::path::Path;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opts {
    /// Prints out the config file location
    #[structopt(long)]
    config_location: bool,

    #[structopt(subcommand)]
    cmd: Option<strand::Plugin>,
}

#[async_std::main]
async fn main() -> Result<()> {
    let opts = Opts::from_args();

    let config_dir = strand::get_config_dir();
    let config_path = config_dir.join("config.yaml");

    // We do this before loading the config file because loading it is not actually needed to
    // display the config file’s location.
    if opts.config_location {
        println!("{}", config_path.display());
        return Ok(());
    }

    let mut config = strand::get_config(&config_path).await?;

    if let Some(plugin) = opts.cmd {
        config.plugins.push(plugin);
        fs::write(&config_path, &yaml::to_string(&config)?).await?;
    }

    // Clean out the plugin directory before installing.
    ensure_empty_dir(&config.plugin_dir).await?;
    strand::install_plugins(config.plugins, config.plugin_dir).await?;

    Ok(())
}

async fn remove_path(path: &Path) -> Result<()> {
    if fs::metadata(path).await?.is_dir() {
        fs::remove_dir_all(path).await?;
    } else {
        fs::remove_file(path).await?;
    }

    Ok(())
}

async fn ensure_empty_dir(path: &Path) -> Result<()> {
    if path.exists() {
        remove_path(path).await?;
    }

    fs::create_dir_all(path).await?;

    Ok(())
}
