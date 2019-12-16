use anyhow::Result;
use async_std::task;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};
use structopt::StructOpt;

fn get_home_dir() -> PathBuf {
    use std::process;

    match dirs::home_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("Error: could not locate home directory -- exiting.");
            process::exit(1);
        }
    }
}

pub fn get_config_dir() -> PathBuf {
    use std::{env, process};

    #[cfg(target_os = "macos")]
    let dir = match env::var_os("XDG_CONFIG_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => get_home_dir().join("config"),
    };

    #[cfg(not(target_os = "macos"))]
    let dir = match dirs::config_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("Error: could not locate config directory -- exiting.");
            process::exit(1);
        }
    };

    dir.join("strand")
}

fn expand_path(path: &Path) -> PathBuf {
    use std::path::Component;

    if path.starts_with("~") {
        let mut components: Vec<_> = path.components().collect();
        let home_dir = get_home_dir().into_os_string();

        // Remove the tilde and add in its place the home directory.
        components.remove(0);
        components.insert(0, Component::Normal(&home_dir));

        // Join the components back into a single unified PathBuf.
        let mut path = PathBuf::new();
        components.iter().for_each(|segment| path.push(segment));

        path
    } else {
        path.into()
    }
}

#[derive(Serialize, Deserialize, StructOpt)]
pub enum GitProvider {
    GitHub,
    Bitbucket,
}

impl FromStr for GitProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "github" => Ok(GitProvider::GitHub),
            "bitbucket" => Ok(GitProvider::Bitbucket),
            _ => Err(format!(
                "Git provider {} not recognised -- try ‘github’ or ‘bitbucket’ instead",
                s
            )),
        }
    }
}

#[derive(Serialize, Deserialize, StructOpt)]
pub struct GitRepo {
    /// The Git repo hosting provider; can be either ‘github’ or ‘bitbucket’
    #[structopt(short, long)]
    provider: GitProvider,

    /// The Git repo owner’s username
    #[structopt(short, long)]
    user: String,

    /// The Git repo’s name
    #[structopt(short, long)]
    repo: String,

    /// An optional branch name, tag name, or commit hash
    #[structopt(short, long)]
    git_ref: Option<String>,
}

impl fmt::Display for GitRepo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let git_ref = match &self.git_ref {
            Some(git_ref) => git_ref,
            None => "master",
        };

        match self.provider {
            GitProvider::GitHub => write!(
                f,
                "https://codeload.github.com/{}/{}/tar.gz/{}",
                self.user, self.repo, git_ref
            ),
            GitProvider::Bitbucket => write!(
                f,
                "https://bitbucket.org/{}/{}/get/{}.tar.gz",
                self.user, self.repo, git_ref
            ),
        }
    }
}

#[derive(Serialize, Deserialize, StructOpt)]
pub struct ArchivePlugin {
    url: String,
}

impl fmt::Display for ArchivePlugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.url)
    }
}

#[derive(Serialize, Deserialize, StructOpt)]
#[serde(untagged)]
pub enum Plugin {
    /// Install a Git plugin and append it to the config file
    #[structopt(name = "install-git")]
    Git(GitRepo),

    /// Install a tar.gz plugin and append it to the config file
    #[structopt(name = "install-tar")]
    Archive(ArchivePlugin),
}

impl fmt::Display for Plugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Plugin::Git(plugin) => write!(f, "{}", plugin),
            Plugin::Archive(plugin) => write!(f, "{}", plugin),
        }
    }
}

impl Plugin {
    async fn install_plugin(&self, path: PathBuf) -> Result<()> {
        use std::process;

        let url = format!("{}", self);
        let archive = match surf::get(url).recv_bytes().await {
            Ok(response) => response,
            Err(e) => {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        };
        decompress_tar_gz(&archive, &path)?;
        println!("Installed {}", self);

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub plugin_dir: PathBuf,
    pub plugins: Vec<Plugin>,
}

pub async fn get_config(config_file: &Path) -> Result<Config> {
    use async_std::fs;

    let config = fs::read_to_string(config_file).await?;
    let mut config: Config = yaml::from_str(&config)?;
    config.plugin_dir = expand_path(&config.plugin_dir);

    Ok(config)
}

fn decompress_tar_gz(bytes: &[u8], path: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let tar = GzDecoder::new(bytes);
    let mut archive = Archive::new(tar);
    archive.unpack(path)?;

    Ok(())
}

pub async fn install_plugins(plugins: Vec<Plugin>, dir: PathBuf) -> Result<()> {
    let mut tasks = Vec::with_capacity(plugins.len());

    plugins.into_iter().for_each(|p| {
        let dir = dir.clone();
        tasks.push(task::spawn(async move { p.install_plugin(dir).await }));
    });

    for task in tasks {
        task.await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_path() {
        let home_dir = get_home_dir();

        assert_eq!(
            expand_path(Path::new("~/foo.txt")),
            home_dir.join("foo.txt")
        );

        assert_eq!(
            expand_path(Path::new("/home/person/foo.txt")),
            PathBuf::from("/home/person/foo.txt")
        );

        assert_eq!(
            expand_path(Path::new("~/bar/baz/quux/foo.txt")),
            home_dir.join("bar/baz/quux/foo.txt")
        );
    }
}
