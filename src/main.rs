/*
# NOTES

- The Server has the following endpoints:
    - `/{task_id}`
    - `/annotation`
- The server listens under:
    - `/api/task`
- The server listens on:
    - Port 8080


*/
#[allow(non_camel_case_types)]
use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use rand::Rng;
use rusqlite::{self, Connection};
use serde_derive::{Deserialize, Serialize};
use serde_rusqlite::*;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const SQL_PATH: &str = "./db.sqlite";

#[derive(Debug)]
struct Database {
    connection: Connection,
}

#[derive(Debug, Serialize, Deserialize)]
struct Response {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    label: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Params {
    attachment_type: String,
    attachment: String,
    objects_to_annotate: Vec<String>,
    with_labels: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Task {
    id: u32,
    created_at: u64,
    completed_at: u64,
    instruction: String,
    status: String,
    urgency: Urgency,
    task_type: String,
    response: Vec<Response>,
    params: Vec<Params>,
}
impl Task {
    fn new(
        id: u32,
        created_at: u64,
        completed_at: u64,
        instruction: String,
        status: String,
        urgency: Urgency,
        task_type: String,
        response: Vec<Response>,
        params: Vec<Params>,
    ) -> Self {
        Self {
            id,
            created_at,
            completed_at,
            instruction,
            status,
            urgency,
            task_type,
            response,
            params,
        }
    }

    fn from_new_task(new_task: NewTask, id: u32) -> Self {
        Task::new(
            id,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs(),
            0,
            new_task.instruction,
            "pending".to_string(),
            new_task.urgency.unwrap_or(Urgency::week),
            "annotation".to_string(),
            Vec::new(),
            new_task.params,
        )
    }
}

impl Database {
    fn init(db_path: &Path) -> Result<Database> {
        Ok(Self {
            connection: Connection::open(db_path)?,
        })
    }

    fn close(self) -> std::result::Result<(), (Connection, rusqlite::Error)> {
        Ok(self.connection.close()?)
    }

    fn kill(self) -> () {
        drop(self.connection)
    }

    fn create_table(&self) -> std::result::Result<(), rusqlite::Error> {
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS 'tasks' (
                'id' INTEGER,
                'created_at' INTEGER,
                'completed_at' INTEGER,
                'instruction' TEXT,
                'status' TEXT,
                'urgency' TEXT,
                'task_type' TEXT,
                'response' BLOB,
                'params' BLOB,
            PRIMARY KEY('id'))",
            [],
        )?;
        Ok(())
    }

    fn get_task(&self, id: u32) -> std::result::Result<Option<Task>, rusqlite::Error> {
        let mut q = self
            .connection
            .prepare("SELECT * FROM 'tasks' WHERE id = (?)")?;
        let mut results = from_rows::<Task>(q.query([id])?);
        match results.next() {
            Some(val) => Ok(Some(val.unwrap())),
            None => Ok(None),
        }
    }

    fn write_task(&self, task: &Task) -> std::result::Result<usize, rusqlite::Error> {
        let mut stmt = self
            .connection
            .prepare("INSERT INTO 'tasks' (
                id, created_at, completed_at, instruction, status, urgency, task_type, response, params
            ) VALUES (
                :id, :created_at, :completed_at, :instruction, :status, :urgency, :task_type, :response, :params
            )")?;
        stmt.execute(to_params_named(&task).unwrap().to_slice().as_slice())
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum Urgency {
    immediate,
    week,
    day,
}
#[derive(Deserialize)]
struct NewTask {
    instruction: String,
    urgency: Option<Urgency>,
    params: Vec<Params>,
}

#[post("/annotation")]
async fn annotation(new_task: web::Json<NewTask>) -> actix_web::Result<HttpResponse> {
    let mut rng = rand::thread_rng();
    let db = Database::init(Path::new(SQL_PATH)).unwrap();
    let mut id = rng.gen::<u32>();
    // TODO implement a break after n tries of generating new ids
    while let Some(_) = &db.get_task(id).unwrap() {
        id = rng.gen::<u32>();
    }
    let task = Task::from_new_task(new_task.into_inner(), id);
    let num = db.write_task(&task).unwrap();
    if num == 1 {
        Ok(HttpResponse::Ok().json(task))
    } else {
        println!("{}", num);
        Ok(HttpResponse::InternalServerError().json(task))
    }
}

#[get("/{task_id}")]
async fn task_id(path: web::Path<u32>) -> actix_web::Result<HttpResponse> {
    let task_id = path.into_inner();
    let task = Database::init(Path::new(SQL_PATH))
        .unwrap()
        .get_task(task_id)
        .unwrap()
        .unwrap();

    Ok(HttpResponse::Ok().json(task))
}

#[actix_web::main]
async fn start_http_server() -> actix_web::Result<(), std::io::Error> {
    HttpServer::new(|| App::new().service(annotation).service(task_id))
        .bind(("0.0.0.0", 8080))?
        .run()
        .await
}

fn main() {
    // Connect to the database
    let database = Database::init(Path::new(SQL_PATH)).unwrap();
    database.create_table().unwrap();
    // Close the database connection
    if let Err(err) = database.close() {
        drop(err.0)
    };
    // Start the HTTP Server
    start_http_server().unwrap();
}
