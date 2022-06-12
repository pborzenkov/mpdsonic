use clap::{crate_version, Arg, Command};

pub fn build_app() -> Command<'static> {
    Command::new("mpdsonic")
        .version(crate_version!())
        .arg(
            Arg::new("address")
                .long("address")
                .short('a')
                .help("The network address to listen to")
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .short('p')
                .help("The network port to listen to")
                .default_value("3000"),
        )
        .arg(
            Arg::new("username")
                .long("username")
                .short('u')
                .help("Username")
                .env("MPDSONIC_USERNAME")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::new("password")
                .long("password")
                .short('P')
                .help("Password")
                .env("MPDSONIC_PASSWORD")
                .takes_value(true)
                .required(true),
        )
}

#[cfg(test)]
mod tests {
    #[test]
    fn verify_app() {
        super::build_app().debug_assert()
    }
}
