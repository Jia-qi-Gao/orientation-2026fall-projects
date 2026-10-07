use clap::Parser;
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::io::{self, Write};
use std::time::Duration;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:7878")]
    url: String,
}
fn input(prompt: &str) -> io::Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    if io::stdin().read_line(&mut line)? == 0 {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    Ok(line.trim_end_matches(['\r', '\n']).to_owned())
}
fn multiline_input() -> io::Result<String> {
    println!("Enter text. Type <<<END>>> on a line by itself to finish.");
    println!("Type \\<<<END>>> if you want a literal <<<END>>> line.");
    let mut text = String::new();
    loop {
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line == "<<<END>>>" {
            break;
        }
        if line == r"\<<<END>>>" {
            text.push_str("<<<END>>>");
        } else {
            text.push_str(line);
        }
        text.push('\n');
    }
    Ok(text)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut token = String::new();
    loop {
        let command = match input(
            "ping / register / login / logout / list / echo / delete-user / put / get / delete / q > ",
        ) {
            Ok(command) => command,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        };
        let mut body = Value::Null;
        let (method, path): (&str, String) = match command.as_str() {
            "q" => break,
            "ping" => ("GET", "/ping".to_string()),
            "list" => ("GET", "/texts".to_string()),
            "logout" => ("DELETE", "/sessions/current".to_string()),
            "register" | "login" => {
                body = json!({"username": input("username: ")?, "password": rpassword::prompt_password("password: ")?});
                (
                    "POST",
                    if command == "register" {
                        "/users".to_string()
                    } else {
                        "/sessions".to_string()
                    },
                )
            }
            "echo" => {
                body = json!({"text": multiline_input()?});
                ("POST", "/echo".to_string())
            }
            "put" => {
                let name = input("text name: ")?;
                let text = multiline_input()?;
                body = json!({"text": text});
                ("PUT", format!("/texts/{name}"))
            }
            "get" => {
                let name = input("text name: ")?;
                ("GET", format!("/texts/{name}"))
            }
            "delete" => {
                let name = input("text name: ")?;
                ("DELETE", format!("/texts/{name}"))
            }
            "delete-user" => ("DELETE", "/users/me".to_string()),
            _ => {
                println!("Unknown command.");
                continue;
            }
        };
        let result = rm_client_sync::exchange(
            &client,
            &args.url,
            method.parse().unwrap(),
            &path,
            &token,
            if body.is_null() { None } else { Some(&body) },
        );
        match result {
            Ok((status, value)) => {
                println!("{status} {value}");
                if command == "login"
                    && status == 200
                    && let Some(next) = value["data"]["token"].as_str()
                {
                    token = next.into();
                }
                if status == 401 {
                    println!("Please log in again.");
                }
                if status == 401 || ((command == "logout" || command == "delete-user") && status == 200) {
                    token.clear();
                }
            }
            Err(error) => eprintln!("Request failed: {error}"),
        }
    }
    Ok(())
}
