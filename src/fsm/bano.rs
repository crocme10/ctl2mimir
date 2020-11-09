use snafu::ResultExt;
use std::path::PathBuf;
use std::process::Command;
use url::Url;

use super::download;
use super::error;

pub fn index_bano_region(
    mimirs_dir: PathBuf,
    es: Url,
    filepath: PathBuf,
) -> Result<(), error::Error> {
    let mut execpath = mimirs_dir;
    // execpath.push("target");
    // execpath.push("release");
    execpath.push("bano2mimir");
    // FIXME Need test file exists
    let mut command = Command::new(&execpath);
    command
        .arg("--connection-string")
        .arg(es.as_str())
        .arg("--input")
        .arg(filepath);
    let output = command.output().context(error::IOError {
        details: format!(
            "Could not create bano2mimir command using {}",
            execpath.display()
        ),
    })?;
    if !output.status.success() {
        Err(error::Error::MiscError {
            details: format!("=> {}", String::from_utf8(output.stderr).unwrap()),
        })
    } else {
        Ok(())
    }
}

pub fn download_bano_region(working_dir: PathBuf, region: &str) -> Result<PathBuf, error::Error> {
    let filename = match region.len() {
        1 => format!("bano-0{}.csv", region),
        _ => format!("bano-{}.csv", region),
    };
    let target = format!("http://bano.openstreetmap.fr/data/{}", filename);
    let mut filepath = working_dir;
    filepath.push("bano");
    if !filepath.is_dir() {
        std::fs::create_dir(filepath.as_path()).context(error::IOError {
            details: format!(
                "Expected to download in BANO file in {}, which is not a directory",
                filepath.display()
            ),
        })?;
    }
    let res = download::download(&target, filepath)?;
    Ok(res.0)
}
