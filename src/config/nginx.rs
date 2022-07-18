use std::{
    fs::{File, OpenOptions},
    path::Path, io::{Read, Write},
};

use crate::utils::Error;
use super::RudraConfig;

pub fn configure_nginx(config: &RudraConfig) -> Result<(), Error> {
    replace_url_in_file(Path::new("/etc/nginx/nginx.conf"), config.app_base_url.as_str())
}

fn replace_url(base: &String, url: &str) -> String {
    base.replace("INSERT_URL_HERE", url)
}

fn open_config_file(path: &Path, for_writing: bool) -> Result<File, Error> {
    match OpenOptions::new().write(for_writing).read(true).truncate(for_writing).open(path) {
        Ok(file) => Ok(file),
        Err(why) => {
            return Err(Error::UnexpectedIOIssue(format!(
                "issue opening file {:?} due to: {}",
                path, why
            )))
        }
    }
}

fn replace_url_in_file(path: &Path, url: &str) -> Result<(), Error> {
    let mut file = open_config_file(path, false)?;

    let mut config_string = String::new();
    match file.read_to_string(&mut config_string) {
        Ok(_) => (),
        Err(why) => {
            return Err(Error::UnexpectedIOIssue(format!(
                "issue reading file {:?} due to: {}",
                path, why
            )))
        }
    }

    let config_string = replace_url(&config_string, url);
    let mut file = open_config_file(path, true)?;
    match file.write_all(config_string.as_bytes()) {
        Ok(_) => (),
        Err(why) => {
            return Err(Error::UnexpectedIOIssue(format!(
                "issue writing file {:?} due to: {}",
                path, why
            )))
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::{Read, Write},
        path::Path,
    };

    use crate::config::nginx::{replace_url, replace_url_in_file};

    use super::open_config_file;

    #[test]
    fn changes_marker_from_string() {
        let test_string = String::from("proxy_pass INSERT_URL_HERE");
        assert_eq!(
            replace_url(&test_string, "https://example.com"),
            "proxy_pass https://example.com"
        );
    }

    #[test]
    fn replaces_file_correctly() {
        write_default_config();

        let nginx_path = Path::new("./test/resource/nginx.conf");
        replace_url_in_file(&nginx_path, "https://example.com").unwrap();
        let mut conf_string = String::from("");
        File::open(&nginx_path)
            .unwrap()
            .read_to_string(&mut conf_string)
            .unwrap();
        assert_eq!(
            conf_string,
            "...some other conf\nproxy_pass https://example.com\n...some more conf\n"
        );

        write_default_config();
    }

    fn write_default_config() {
        let mut file = open_config_file(Path::new("./test/resource/nginx.conf"), true).unwrap();
        file.write_all(
            "...some other conf\nproxy_pass INSERT_URL_HERE\n...some more conf\n".as_bytes(),
        )
        .unwrap();
        file.flush().unwrap();
    }
}
