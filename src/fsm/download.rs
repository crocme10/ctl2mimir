use super::error;
use snafu::ResultExt;
use std::fs;
use std::io::prelude::*;
use std::path::PathBuf;
use url::Url;

pub fn download(link: &str, download_path: PathBuf) -> Result<(PathBuf, usize), error::Error> {
    let mut download_path = download_path;
    // checks if the download path exists, and tries to create the folders if it doesn't
    if !download_path.exists() {
        fs::create_dir_all(&download_path).context(error::IOError {
            details: format!("Could not create {}", download_path.display()),
        })?;
    }

    let file = get_filename_from_url(link)?;

    download_path.push(format!("{}", file));

    if download_path.exists() {
        return Ok((download_path, 0));
    }

    let client = reqwest::blocking::Client::new();
    let mut resp = client.get(link).send().context(error::ReqwestError {
        details: format!("Could not get {}", link),
    })?;

    if resp.status().is_success() {
        let chunk_size = 1024usize;
        let mut buffer: Vec<u8> = Vec::new();

        loop {
            let mut small_buffer = vec![0; chunk_size];
            let small_buffer_read = resp.read(&mut small_buffer[..]).context(error::IOError {
                details: "Could not read buffer",
            })?;
            small_buffer.truncate(small_buffer_read);

            match small_buffer.is_empty() {
                true => break,
                false => {
                    buffer.extend(small_buffer);
                }
            }
        }

        let mut disk_file = fs::File::create(&download_path).context(error::IOError {
            details: format!("Could not create file {}", download_path.display()),
        })?;
        let size_disk = disk_file.write(&buffer).context(error::IOError {
            details: format!("Could not write to {}", download_path.display()),
        })?;

        Ok((download_path, size_disk))
    } else {
        Err(error::Error::MiscError {
            details: format!("No response while trying to download {}", link),
        })
    }
}

pub fn get_filename_from_url(link: &str) -> Result<String, error::Error> {
    let url = Url::parse(link).context(error::URLError {
        details: format!("Could not parse URL {}", link),
    })?;
    let last = url
        .path_segments()
        .ok_or(error::Error::MiscError {
            details: format!("There is such thing as cannot-be-a-base URL... {}", link),
        })?
        .last()
        .ok_or(error::Error::MiscError {
            details: format!("There is such thing as cannot-be-a-base URL... {}", link),
        })?;
    if last.len() == 0 {
        Ok(String::from("foo"))
    } else {
        Ok(String::from(last))
    }
}
