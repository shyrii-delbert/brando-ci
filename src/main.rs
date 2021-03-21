use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize};
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use chrono::prelude::*;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum ServerMode {
    Server,
    Client,
}

#[derive(Deserialize, Debug)]
struct BrandoPath {
    path: String,
}

#[derive(Deserialize, Debug)]
struct BranchInfo {
   host: String,
   token: String,
}

#[derive(Deserialize, Debug)]
struct Config {
    mode: ServerMode,
    token: String,
    port: i32,
    branches: Option<HashMap<String, BranchInfo>>,
    brando: Option<BrandoPath>,
}

#[derive(Deserialize, Debug)]
struct UpdateQuery {
    token: String,
    branch: String,
}

#[get("/config_mode")]
async fn config_mode(config: web::Data<Config>) -> impl Responder {
    HttpResponse::Ok().body(format!("{:?}", &config.mode))
}

#[get("/server/update")]
async fn server_update(config: web::Data<Config>, query: web::Query<UpdateQuery>) -> impl Responder {
    match &config.mode {
        ServerMode::Client => HttpResponse::Forbidden().body("Running in client mode now!"),
        _ => {
            if &query.token != &config.token {
                return HttpResponse::Forbidden().body("Token is not correct");
            }
            match config.branches.as_ref().unwrap().get(&query.branch) {
               Some(branch) => {
                   match reqwest::get(format!("{}{}?token={}", branch.host, "/client/update", "&branch.token")).await {
                        Err(_) => HttpResponse::ServiceUnavailable().body("Send Request Error"),
                        _ => HttpResponse::Ok().body("Ok")
                   }
               },
               None => HttpResponse::NotFound().body("No such branch registered"),
            }
        }
    }
}


#[get("/client/update")]
async fn client_update(config: web::Data<Config>, query: web::Query<UpdateQuery>) -> impl Responder {
    match &config.mode {
        ServerMode::Server => HttpResponse::Forbidden().body("Running in server mode now!"),
        _ => {
            if &query.token != &config.token {
                return HttpResponse::Forbidden().body("Token is not correct");
            }
            if let Some(brando_path) = &config.brando {
                let path = &brando_path.path;
                let mut current_dir = std::env::current_dir().unwrap();
                current_dir.push(path);
                let git_pull_command_output = Command::new("git")
                    .arg("pull")
                    .current_dir(&current_dir)
                    .output()
                    .expect("failed to git pull");
                let mut res: String = String::from_utf8(git_pull_command_output.stdout).unwrap();
                res.push_str(String::from_utf8(git_pull_command_output.stderr).unwrap().as_str());
                res.push_str("\n");
                let docker_pull_comamnd_output = Command::new("docker-compose")
                    .arg("pull")
                    .current_dir(&current_dir)
                    .output()
                    .expect("failed to docker pull");
                res.push_str(String::from_utf8(docker_pull_comamnd_output.stdout).unwrap().as_str());
                res.push_str(String::from_utf8(docker_pull_comamnd_output.stderr).unwrap().as_str());
                res.push_str("\n");
                let docker_up_comamnd_output = Command::new("docker-compose")
                    .arg("up")
                    .arg("-d")
                    .current_dir(&current_dir)
                    .output()
                    .expect("failed to docker up");
                res.push_str(String::from_utf8(docker_up_comamnd_output.stdout).unwrap().as_str());
                res.push_str(String::from_utf8(docker_up_comamnd_output.stderr).unwrap().as_str());
                res.push_str("\n");
                println!("======================");
                println!("{:?}\n\n{}", Local::now(), res);
                println!("======================");
                HttpResponse::Ok().body(res)
            } else {
                return HttpResponse::InternalServerError().body("No brando path defined");
            }
        }
    }
}

fn parse_config() -> Config {
    let config_info_str = fs::read_to_string("config.toml").unwrap();
    toml::from_str(config_info_str.as_str()).unwrap()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("starting brando ci...");
    let config = parse_config();
    let bind_address = format!("0.0.0.0:{}", config.port);
    println!("listening to {}", &bind_address);
    HttpServer::new(|| {
            App::new()
                .data(parse_config())
                .service(config_mode)
                .service(server_update)
                .service(client_update)
        })
    .bind(bind_address)?
    .run()
    .await
}
