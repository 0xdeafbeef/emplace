#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;

use clap::{App, AppSettings, Arg, SubCommand};

mod init;
mod catch;

fn main() {
    let matches = App::new("emplace")
        .version(crate_version!())
        .author(crate_authors!())
        .after_help("https://github.com/tversteeg/emplace")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("init")
            .about("Prints the shell function used to execute emplace")
            .arg(
                Arg::with_name("shell")
                .value_name("SHELL")
                .help(
                    "The name of the currently running shell\nCurrently supported options: bash",
                )
                .required(true)
            )
        )
        .subcommand(
            SubCommand::with_name("catch")
            .about("Capture a command entired in a terminal")
            .arg(
                Arg::with_name("line")
                .value_name("LINE")
                .help(
                    "The command as entired in the terminal",
                )
                .required(true)
            )
        )
        .get_matches();

    match matches.subcommand() {
        ("init", Some(sub_m)) => {
            let shell_name = sub_m.value_of("shell").expect("Shell name is missing.");
            init::init_main(shell_name).expect("Could not initialize terminal script");
        },
        ("catch", Some(sub_m)) => {
            let line = sub_m.value_of("line").expect("Line is missing");
            let catches = catch::catch(line).expect("Could not parse line");
            if catches.len() > 0 {
                if dialoguer::Confirmation::new()
                    .with_text("Do you want mirror this command?")
                        .interact()
                        .unwrap()
                {
                    println!("Y");
                } else {
                    println!("N");
                }
            }
        },
        (&_, _) => { }
    }
}