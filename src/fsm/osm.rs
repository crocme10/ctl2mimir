use snafu::ResultExt;
use std::path::PathBuf;
use std::process::Command;
use url::Url;

use super::download;
use super::error;

// Download the pbf associated with a region.
// This is a very rudimentary function, which:
// * does not handle correctly regions outside of france
// * has a hard coded timeout to 300s
// It will create a directory 'osm' inside the working directory (if not already present)
// It will download a file
pub fn download_osm_region(working_dir: PathBuf, region: &str) -> Result<PathBuf, error::Error> {
    let filename = format!("{}-latest.osm.pbf", region);
    let target = format!("https://download.geofabrik.de/europe/france/{}", filename);
    let mut filepath = working_dir;
    filepath.push("osm");
    if !filepath.is_dir() {
        std::fs::create_dir(filepath.as_path()).context(error::IOError {
            details: format!(
                "Expected to download OSM file in {}, which is not a directory",
                filepath.display()
            ),
        })?;
    }
    let res = download::download(&target, filepath)?;
    Ok(res.0)
}

pub fn index_osm_region(
    mimirs_dir: PathBuf,
    es: Url,
    filepath: PathBuf, // osm pbf
    admin: bool,
    way: bool,
    poi: bool,
    city_level: u32,
) -> Result<(), error::Error> {
    let mut execpath = mimirs_dir;
    execpath.push("target");
    execpath.push("release");
    execpath.push("osm2mimir");
    // FIXME Need test file exists
    let mut command = Command::new(&execpath);
    command
        .arg("--connection-string")
        .arg(es.as_str())
        .arg("--input")
        .arg(filepath.clone());
    if way {
        command.arg("--import-way");
    }
    if admin {
        command.arg("--import-admin");
    }
    if poi {
        command.arg("--import-poi");
    }
    command.arg("--city-level").arg(city_level.to_string());
    let output = command.output().context(error::IOError {
        details: format!(
            "Could not create osm2mimir command using {}",
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
