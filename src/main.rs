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
use actix_files;
use actix_web::{get, post, web, App, HttpResponse, HttpServer};
use chrono::{self, Local};
use env_logger::Builder;
use log::LevelFilter;
use rand::Rng;
use rusqlite::{self, Connection};
use serde_derive::{Deserialize, Serialize};
use serde_json;
use serde_rusqlite::*;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const SQL_PATH: &str = "./db.sqlite";

#[derive(Debug)]
struct Database {
    connection: Connection,
}

#[derive(Debug, Serialize, Deserialize)]
enum TaskStatus {
    pending,
    completed,
    broken,
}

#[derive(Debug, Serialize, Deserialize)]
struct Response {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}

impl Response {
    fn empty() -> Self {
        Self {
            left: 0,
            top: 0,
            width: 0,
            height: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Task {
    id: u32,
    created_at: u64,
    completed_at: u64,
    instruction: String,
    status: TaskStatus,
    urgency: Urgency,
    task_type: String,
    response: HashMap<String, Response>,
    attachment: String,
}
impl Task {
    fn new(
        id: u32,
        created_at: u64,
        completed_at: u64,
        instruction: String,
        status: TaskStatus,
        urgency: Urgency,
        task_type: String,
        response: HashMap<String, Response>,
        attachment: String,
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
            attachment,
        }
    }

    fn to_db_task(self) -> DatabaseTask {
        DatabaseTask {
            id: self.id,
            created_at: self.created_at,
            completed_at: self.completed_at,
            instruction: self.instruction,
            status: self.status,
            urgency: self.urgency,
            task_type: self.task_type,
            response: serde_json::to_string(&self.response).unwrap(),
            attachment: self.attachment,
        }
    }

    fn from_new_task(new_task: NewTask, id: u32) -> Self {
        let mut response = HashMap::new();
        new_task.objects.into_iter().for_each(|e| {
            response.insert(e, Response::empty());
        });
        Task::new(
            id,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs(),
            0,
            new_task.instruction,
            TaskStatus::pending,
            new_task.urgency.unwrap_or(Urgency::week),
            "annotation".to_string(),
            response,
            new_task.attachment,
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DatabaseTask {
    id: u32,
    created_at: u64,
    completed_at: u64,
    instruction: String,
    status: TaskStatus,
    urgency: Urgency,
    task_type: String,
    response: String,
    attachment: String,
}
impl DatabaseTask {
    fn to_task(self) -> Task {
        Task {
            id: self.id,
            created_at: self.created_at,
            completed_at: self.completed_at,
            instruction: self.instruction,
            status: self.status,
            urgency: self.urgency,
            task_type: self.task_type,
            response: serde_json::from_str(&self.response).unwrap(),
            attachment: self.attachment,
        }
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
                'attachment' TEXT,
            PRIMARY KEY('id'))",
            [],
        )?;
        Ok(())
    }

    fn get_task(&self, id: u32) -> std::result::Result<Option<Task>, rusqlite::Error> {
        let mut q = self
            .connection
            .prepare("SELECT * FROM 'tasks' WHERE id = (?)")?;
        let mut results = from_rows::<DatabaseTask>(q.query([id])?);
        match results.next() {
            Some(val) => Ok(Some(val.unwrap().to_task())),
            None => Ok(None),
        }
    }

    fn get_pending_tasks(&self) -> std::result::Result<Vec<Task>, rusqlite::Error> {
        let mut q = self
            .connection
            .prepare("SELECT * FROM 'tasks' WHERE status = 'pending'")?;
        let mut results = from_rows::<DatabaseTask>(q.query([])?);
        Ok(results.map(|t| t.unwrap().to_task()).collect::<Vec<Task>>())
    }

    fn write_task(&self, task: Task) -> std::result::Result<usize, rusqlite::Error> {
        let t = task.to_db_task();
        let mut stmt = self
            .connection
            .prepare("INSERT INTO 'tasks' (
                id, created_at, completed_at, instruction, status, urgency, task_type, response, attachment
            ) VALUES (
                :id, :created_at, :completed_at, :instruction, :status, :urgency, :task_type, :response, :attachment
            )")?;
        stmt.execute(to_params_named(&t).unwrap().to_slice().as_slice())
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
    objects: Vec<String>,
    attachment: String,
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
    let num = db.write_task(task).unwrap();
    if num == 1 {
        Ok(HttpResponse::Ok().json(id))
    } else {
        println!("{}", num);
        Ok(HttpResponse::InternalServerError().body("{}"))
    }
}

#[get("/pending")]
async fn get_pending(path: web::Path<u32>) -> actix_web::Result<HttpResponse> {
    let tasks = Database::init(Path::new(SQL_PATH))
        .unwrap()
        .get_pending_tasks()
        .unwrap();

    Ok(HttpResponse::Ok().json(tasks))
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
    HttpServer::new(|| {
        App::new()
            .service(actix_files::Files::new("/", "./public"))
            .service(web::scope("/api/task").service(annotation).service(task_id))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

fn main() {
    // Build the logger
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] {} - {}: {}",
                record.level(),
                Local::now().format("%d/%m/%y %H:%M:%S"),
                record.target(),
                record.args(),
            )
        })
        .filter(None, LevelFilter::Info)
        .init();
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
