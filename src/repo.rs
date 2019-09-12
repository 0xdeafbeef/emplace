use git2::Repository;
use std::{
    fs::{self, File},
    path::Path,
    error::Error,
    io::Read,
};

use crate::config::Config;
use crate::catch::Packages;

pub struct Repo {
    config: Config,
    repo: Repository
}

impl Repo {
    pub fn new(config: Config) -> Result<Self, Box<dyn Error>> {
        let mirror_directory = config.mirror_directory.clone();
        let repo_url = config.repo.url.clone();
        let repo_branch = config.repo.branch.clone();
        let username = config.repo.username.clone();

        let path = Path::new(&mirror_directory);
        let path_str = path.to_str().expect("Could not get directory").to_string();

        let is_new = !path.join(".git").exists();
        let repo = match is_new {
            false => {
                println!("Opening existing repo: \"{}\"", path_str);
                let repo = Repository::open(path)?;

                pull(&repo, &*repo_branch)?;

                repo
            },
            true => {
                println!("Cloning repo \"{}\" to \"{}\"", repo_url, path_str);
                let mut callbacks = git2::RemoteCallbacks::new();
                callbacks.credentials(|_, _, _| {
                    git2::Cred::ssh_key_from_agent(&*username)
                });

                let mut fetch_options = git2::FetchOptions::new();
                fetch_options.remote_callbacks(callbacks);

                git2::build::RepoBuilder::new()
                    .branch(&*repo_branch)
                    .fetch_options(fetch_options)
                    .clone(&*repo_url, path)?
            }
        };

        Ok(Repo {
            config,
            repo,
        })
    }

    pub fn mirror(&self, mut commands: Packages) -> Result<(), Box<dyn Error>> {
        // Get the message first before the old stuff is added
        let commit_msg = commands.commit_message();

        let full_path = self.config.full_file_path();
        if full_path.exists() {
            // A file already exists, merge the existing one with the current one
            self.merge_file(&mut commands)?;
        }

        // There's no file yet, just serialize everything and write it to a new file
        let toml_string = serde_json::to_string(&commands)?;
        fs::write(&full_path, toml_string)?;

        println!("Commiting with message \"{}\"..", commit_msg);
        commit(&self.repo, &self.config.repo.path(), &*commit_msg)?;

        println!("Pushing..");
        push(&self.repo, &*self.config.repo.branch, &*self.config.repo.url)?;

        Ok(())
    }

    pub fn merge_file(&self, commands: &mut Packages) -> Result<(), Box<dyn Error>> {
        // Open the file
        let mut file = File::open(&self.config.full_file_path())?;

        // Read the contents
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        // Deserialize the file into the struct
        let mut old: Packages = serde_json::from_str(&*contents)?;
        // Merge it with the new one
        commands.merge(&mut old);

        Ok(())
    }
}

fn pull(repo: &Repository, branch: &str) -> Result<(), git2::Error> {
    println!("Pulling origin/{}..", branch);

    // Do a fetch
    let mut remote = repo.find_remote("origin")?;
    remote.fetch(&[branch], None, None)?;

    // Get the FETCH_HEAD commit
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;

    // Do a merge
    let analysis = repo.merge_analysis(&[&fetch_commit])?;
    if analysis.0.is_fast_forward() {
        let refname = format!("refs/heads/{}", branch);
        let mut reference = repo.find_reference(&refname)?;

        fast_forward(repo, &mut reference, &fetch_commit)?;
    } else if analysis.0.is_normal() {
        unimplemented!("Unhandled normal merge situation");
    }

    Ok(())
}

fn fast_forward(repo: &Repository, local_ref: &mut git2::Reference, remote_commit: &git2::AnnotatedCommit) -> Result<(), git2::Error> {
    let name = match local_ref.name() {
        Some(s) => s.to_string(),
        None => String::from_utf8_lossy(local_ref.name_bytes()).to_string(),
    };
    let msg = format!("Fast-Forward: Setting {} to id: {}", name, remote_commit.id());
    local_ref.set_target(remote_commit.id(), &msg)?;
    repo.set_head(&name)?;
    repo.checkout_head(None)?;

    Ok(())
}

fn find_last_commit(repo: &Repository) -> Result<git2::Commit, git2::Error> {
    let obj = repo.head()?.resolve()?.peel(git2::ObjectType::Commit)?;
    obj.into_commit().map_err(|_| git2::Error::from_str("Couldn't find commit"))
}

fn commit(repo: &Repository, file: &Path, msg: &str) -> Result<git2::Oid, git2::Error> {
    // Add the file to git
    let parent_commit = find_last_commit(repo)?;

    let mut index = repo.index()?;
    index.add_path(file)?;
    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;

    let signature = git2::Signature::now("Emplace", "emplace@emplace")?;

    repo.commit(Some("HEAD"), &signature, &signature, &*msg, &tree, &[&parent_commit])
}

fn push(repo: &Repository, branch: &str, url: &str) -> Result<(), git2::Error> {
    let mut remote = match repo.find_remote("origin") {
        Ok(r) => r,
        Err(_) => repo.remote("origin", url)?,
    };
    match remote.connect(git2::Direction::Push) {
        Err(error) => {
            println!("Error when connecting to repo: {}", error);
            return Ok(())
        },
        _ => ()
    }
    match remote.push(&[&*format!("refs/heads/{b}:refs/heads/{b}", b=branch)], None) {
        Ok(_) => Ok(()),
        Err(error) => {
            println!("Error when pushing repo: {}", error);
            Ok(())
        }
    }
}