use anyhow::anyhow;
use url::Url;
use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;
use std::process::{Command, Stdio};


#[derive(Parser, Debug)]
#[command(name="repo")]
#[command(bin_name="repo")]
enum Cli {
    Clone(CloneArgs),
    FetchAll,
}

#[derive(Args, Debug)]
struct CloneArgs {
    uri: String,
    #[clap(short = 'l')]
    link: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    match args {
        Cli::Clone(args) => do_clone(args),
        Cli::FetchAll => do_fetch_all(),
    }
}

fn do_clone(args: CloneArgs) -> anyhow::Result<()> {
    let repos_dir = repos_dir()?;

    let (domain, user, repo) = parse_uri(&args.uri)?;

    let domain_user_dir = repos_dir.join(domain).join(user);

    if !std::fs::metadata(&domain_user_dir).is_ok() {
        std::fs::create_dir_all(&domain_user_dir)?;
    }

    let target_dir = domain_user_dir.join(repo.clone());
    if !std::fs::metadata(&target_dir).is_err() {
        println!("{}", target_dir.display());
        return Err(anyhow!("Error: Repo already exists"));
    }

    std::env::set_current_dir(domain_user_dir)?;

    let mut child = Command::new("git")
        .arg("clone")
        .arg(args.uri)
        .arg(repo.clone())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let Some(stdout) = child.stdout.take() else {
        return Err(anyhow!("Error: Could not clone repo: Could not take stdout"));
    };

    let Some(stderr) = child.stderr.take() else {
        return Err(anyhow!("Error: Could not clone repo: Could not take stderr"));
    };

    std::thread::spawn(|| {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(stderr);

        for line in reader.lines() {
            println!("{}", line.unwrap());
        }
    });

    std::thread::spawn(|| {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(stdout);

        for line in reader.lines() {
            println!("{}", line.unwrap());
        }
    });


    let status = child.wait()?;

    if !status.success() {
        return Err(anyhow!("Error: Could not clone repo: Exist status {status}"));
    }

    let target_dir_path = String::from_utf8_lossy(target_dir.to_str().ok_or_else(|| anyhow!("non-utf8 path"))?.as_bytes());
    println!("{target_dir_path}");

    let projects_dir = projects_dir()?;
    let link = projects_dir.join(repo);
    let link_path = String::from_utf8_lossy(link.to_str().ok_or_else(|| anyhow!("non-utf8 path"))?.as_bytes());
    println!("{link_path}");

    let mut child = Command::new("ln")
        .arg("-s")
        .arg(&*target_dir_path)
        .arg(&*link_path)
        .spawn()?;

    let status = child.wait()?;

    if !status.success() {
        return Err(anyhow!("Error: Could not link: Exist status {status}"));
    }

    Ok(())
}

fn repos_dir() -> anyhow::Result<PathBuf> {
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find user home directory"))?;
    let repo_dir = home_dir.join("repos");
    if !std::fs::metadata(&repo_dir).is_ok() {
        std::fs::create_dir_all(&repo_dir)?;
    }

    Ok(repo_dir)
}

fn projects_dir() -> anyhow::Result<PathBuf> {
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find user home directory"))?;
    let projects_dir = home_dir.join("projects");
    if !std::fs::metadata(&projects_dir).is_ok() {
        std::fs::create_dir_all(&projects_dir)?;
    }

    Ok(projects_dir)
}

fn parse_uri(uri: &str) -> anyhow::Result<(String, String, String)> {
    if uri.starts_with("https://") {
        let uri_no_schema = &uri[8..];
        let Some(slash_idx) = uri_no_schema.find('/') else {
            return Err(anyhow!("Could not parse uri: {uri:?}"));
        };

        let domain = uri_no_schema[..slash_idx].to_string();
        let rest = &uri_no_schema[slash_idx + 1..];

        let Some(slash_idx2) = rest.find('/') else {
            return Err(anyhow!("Could not parse uri: {uri:?}"));
        };

        let user = rest[..slash_idx2].to_string();

        let mut repo = String::new();
        for ch in rest[slash_idx2+1..].chars() {
            if ch != '/' && ch != '.' {
                repo.push(ch);
            }
        }

        Ok((domain, user, repo))
    } else if uri.starts_with("git@") {
        let uri_no_schema = &uri[4..];
        let Some(slash_idx) = uri_no_schema.find(':') else {
            return Err(anyhow!("Could not parse uri: {uri:?}"));
        };

        let domain = uri_no_schema[..slash_idx].to_string();
        let rest = &uri_no_schema[slash_idx + 1..];

        let Some(slash_idx2) = rest.find('/') else {
            return Err(anyhow!("Could not parse uri: {uri:?}"));
        };

        let user = rest[..slash_idx2].to_string();

        let mut repo = String::new();
        for ch in rest[slash_idx2+1..].chars() {
            if ch != '/' && ch != '.' {
                repo.push(ch);
            } else {
                break;
            }
        }

        Ok((domain, user, repo))
    } else {
        Err(anyhow!("Could not parse uri: {uri:?}"))
    }
}

#[test]
fn test_parse_uri() {
    let (domain, group, repo) = parse_uri("https://github.com/rust-lang/rust/").unwrap();
    assert_eq!((domain.as_str(), group.as_str(), repo.as_str()), ("github.com", "rust-lang", "rust"));

    let (domain, group, repo) = parse_uri("git@github.com:rust-lang/rust").unwrap();
    assert_eq!((domain.as_str(), group.as_str(), repo.as_str()), ("github.com", "rust-lang", "rust"));

    let (domain, group, repo) = parse_uri("git@github.com:rust-lang/rust.git").unwrap();
    assert_eq!((domain.as_str(), group.as_str(), repo.as_str()), ("github.com", "rust-lang", "rust"));
}

fn do_fetch_all() -> anyhow::Result<()> {
    let repos_dir = repos_dir()?;
    for entry in walkdir::WalkDir::new(repos_dir).min_depth(2).max_depth(2) {
        println!("{entry:?}");
    }
    Ok(())
}
