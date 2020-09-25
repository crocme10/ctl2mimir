use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::path::PathBuf;
use std::process::Command;
use url::Url;

use super::download;
use super::error;

#[derive(Debug, Serialize, Deserialize)]
struct NTFSDownload {
    format: String,
    filename: String,
    width: u32,
    id: String,
    height: u32,
    thumbnail: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct NTFSFields {
    license_link: String,
    update_date: String,
    description: String,
    license: String,
    format: String,
    validity_end_date: String,
    validity_start_date: String,
    download: NTFSDownload,
    id: String,
    size: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct NTFSDataset {
    datasetid: String,
    recordid: String,
    fields: NTFSFields,
    record_timestamp: String,
}

// Download the pbf associated with a region.
// This is a very rudimentary function, which:
// * does not handle correctly regions outside of france
// * has a hard coded timeout to 300s
// It will create a directory 'osm' inside the working directory (if not already present)
// It will download a file
pub fn download_ntfs_region(working_dir: PathBuf, region: &str) -> Result<PathBuf, error::Error> {
    // For NTFS, the download is a bit more involved.
    // We need to download a first file, which describe the available datasets.
    // So we download the file in json format, and use serde to get a list of datasets.
    // We filter that list to get the 'NTFS' dataset, and extract the id which is used to generate
    // the URL from which we can download the data.
    // Finally we download the dataset, which is a zip, so we call unzip to extract the data.
    let target = format!(
        "https://navitia.opendatasoft.com/explore/dataset/{}/download/?format=json",
        region
    );
    let mut filepath = working_dir;
    filepath.push("ntfs");
    filepath.push(region);
    if !filepath.is_dir() {
        std::fs::create_dir_all(filepath.as_path()).context(error::IOError {
            details: format!(
                "Expected to download NTFS file in {}, which is not a directory",
                filepath.display()
            ),
        })?;
    }
    let res = download::download(&target, filepath.clone())?;
    let datasets = std::fs::read_to_string(&res.0).context(error::IOError {
        details: format!(
            "Could not read content of NTFS first download {}",
            res.0.display()
        ),
    })?;
    let datasets: Vec<NTFSDataset> =
        serde_json::from_str(&datasets).context(error::SerdeJSONError {
            details: "Could not deserialize NTFS datasets",
        })?;
    let url = datasets
        .iter()
        .find_map(|dataset| {
            if dataset.fields.format == "NTFS" {
                Some(format!(
                    "https://navitia.opendatasoft.com/api/v2/catalog/datasets/fr-ne/files/{}",
                    dataset.fields.download.id
                ))
            } else {
                None
            }
        })
        .ok_or(error::Error::MiscError {
            details: String::from("Could not find NTFS dataset"),
        })?;
    // Note, that since we have the URL, we don't need the file returned by the previous
    // download... so bye bye
    std::fs::remove_file(res.0.as_path()).context(error::IOError {
        details: format!("Could not remove {}", res.0.display()),
    })?;
    let res = download::download(&url, filepath.clone())?;
    let mut command = Command::new("unzip");
    // We want to unzip in the director 'filepath'
    command.arg("-d").arg(filepath.clone());
    // We want to overwrite files without prompting
    command.arg("-o");
    command.arg(res.0.as_path());
    let output = command.output().context(error::IOError {
        details: format!("Could not unzip {}", filepath.display()),
    })?;
    // Same thing, we don't need the zip file, so remove it.
    std::fs::remove_file(res.0.as_path()).context(error::IOError {
        details: format!("Could not remove {}", res.0.display()),
    })?;
    if !output.status.success() {
        Err(error::Error::MiscError {
            details: format!("=> {}", String::from_utf8(output.stderr).unwrap()),
        })
    } else {
        Ok(filepath)
    }
}

pub fn index_ntfs_region(
    mimirs_dir: PathBuf,
    es: Url,
    filepath: PathBuf,
) -> Result<(), error::Error> {
    let mut execpath = mimirs_dir;
    execpath.push("ntfs2mimir");
    // FIXME Need test file exists
    let mut command = Command::new(&execpath);
    command
        .arg("--connection-string")
        .arg(es.as_str())
        .arg("--input")
        .arg(filepath.clone());
    let output = command.output().context(error::IOError {
        details: format!(
            "Could not create ntfs2mimir command using {}",
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
