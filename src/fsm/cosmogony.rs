use snafu::ResultExt;
use std::path::PathBuf;
use std::process::Command;
use url::Url;

use super::error;

pub fn index_cosmogony_region(
    mimirs_dir: PathBuf,
    es: Url,
    filepath: PathBuf,
) -> Result<(), error::Error> {
    let mut execpath = mimirs_dir;
    execpath.push("cosmogony2mimir");
    // FIXME Need test file exists
    let mut command = Command::new(&execpath);
    command
        .arg("--connection-string")
        .arg(es.as_str())
        .arg("--input")
        .arg(filepath.clone());
    let output = command.output().context(error::IOError {
        details: format!(
            "Could not create cosmogony2mimir command using {}",
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

pub fn generate_cosmogony(
    cosmogony_dir: PathBuf,
    working_dir: PathBuf,
    inputpath: PathBuf,
    region: &str,
) -> Result<PathBuf, error::Error> {
    let filename = format!("{}.json.gz", region);
    let mut outputpath = working_dir;
    outputpath.push("cosmogony");
    if !outputpath.is_dir() {
        std::fs::create_dir(outputpath.as_path()).context(error::IOError {
            details: format!(
                "Could not create output directory for cosmogony {}",
                outputpath.display()
            ),
        })?;
    }
    outputpath.push(&filename);
    let mut execpath = cosmogony_dir;
    execpath.push("cosmogony");
    // FIXME Need to test exec exists
    let mut command = Command::new(&execpath);
    command
        .arg("--country-code")
        .arg("FR")
        .arg("--input")
        .arg(inputpath.clone())
        .arg("--output")
        .arg(outputpath.clone());
    let output = command.output().context(error::IOError {
        details: format!(
            "Could not create cosmogony command using {}",
            execpath.display()
        ),
    })?;
    if !output.status.success() {
        Err(error::Error::MiscError {
            details: format!("=> {}", String::from_utf8(output.stderr).unwrap()),
        })
    } else {
        Ok(outputpath)
    }
}
