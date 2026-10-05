//! What Cerno needs to know about the system it runs on: its name for a bug report, and on
//! Linux the distribution (`/etc/os-release`) – which command installs ExifTool there.

/// `/etc/os-release`, else `/usr/lib/os-release`; empty where there is none (Windows).
pub fn os_release() -> String {
    ["/etc/os-release", "/usr/lib/os-release"]
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default()
}

/// The system for a bug report: `Windows (x86_64)`, or the distribution's own name on Linux.
pub fn name() -> String {
    let arch = std::env::consts::ARCH;
    if cfg!(windows) {
        return format!("Windows ({arch})");
    }
    let release = os_release();
    let name = os_field(&release, "PRETTY_NAME").unwrap_or("Linux");
    format!("{name} ({arch})")
}

/// One `KEY=value` of an os-release text, without its quotes.
pub fn os_field<'a>(os_release: &'a str, key: &str) -> Option<&'a str> {
    os_release.lines().find_map(|line| {
        let value = line.trim().strip_prefix(key)?.strip_prefix('=')?;
        Some(value.trim().trim_matches(['"', '\'']))
    })
}

/// The command that installs ExifTool on this distribution, by its `ID` and `ID_LIKE`; `None`
/// where Cerno doesn't know the package manager.
pub fn exiftool_install_command(os_release: &str) -> Option<&'static str> {
    let names: Vec<String> = ["ID", "ID_LIKE"]
        .iter()
        .filter_map(|key| os_field(os_release, key))
        .flat_map(|value| value.split_whitespace().map(str::to_ascii_lowercase))
        .collect();
    let any = |known: &[&str]| names.iter().any(|name| known.contains(&name.as_str()));
    if any(&["debian", "ubuntu"]) {
        Some("sudo apt install libimage-exiftool-perl")
    } else if any(&["fedora", "rhel", "centos"]) {
        Some("sudo dnf install perl-Image-ExifTool")
    } else if any(&["arch"]) {
        Some("sudo pacman -S perl-image-exiftool")
    } else if any(&["suse", "opensuse"]) {
        Some("sudo zypper install exiftool")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_install_command_follows_the_distribution() {
        let ubuntu =
            "PRETTY_NAME=\"Ubuntu 24.04.1 LTS\"\nNAME=\"Ubuntu\"\nID=ubuntu\nID_LIKE=debian\n";
        let mint = "NAME=\"Linux Mint\"\nID=linuxmint\nID_LIKE=\"ubuntu debian\"\n";
        let fedora = "NAME=\"Fedora Linux\"\nID=fedora\n";
        let manjaro = "NAME=\"Manjaro Linux\"\nID=\"manjaro\"\nID_LIKE=\"arch\"\n";
        let tumbleweed = "ID=\"opensuse-tumbleweed\"\nID_LIKE=\"opensuse suse\"\n";
        let apt = Some("sudo apt install libimage-exiftool-perl");
        assert_eq!(exiftool_install_command(ubuntu), apt);
        assert_eq!(exiftool_install_command(mint), apt);
        assert_eq!(
            exiftool_install_command(fedora),
            Some("sudo dnf install perl-Image-ExifTool")
        );
        assert_eq!(
            exiftool_install_command(manjaro),
            Some("sudo pacman -S perl-image-exiftool")
        );
        assert_eq!(
            exiftool_install_command(tumbleweed),
            Some("sudo zypper install exiftool")
        );
        assert_eq!(exiftool_install_command("ID=nixos\n"), None);
        assert_eq!(exiftool_install_command(""), None);
        assert_eq!(os_field(ubuntu, "PRETTY_NAME"), Some("Ubuntu 24.04.1 LTS"));
        assert_eq!(os_field(ubuntu, "ID"), Some("ubuntu"));
        assert_eq!(os_field(fedora, "ID_LIKE"), None);
    }
}
