use std::path::Path;
use rusqlite::{Connection,params};
use serde_json::{Value,json};
use crate::law_system::SimulationError;

pub struct Catalog { connection:Connection }
fn sql(error:rusqlite::Error)->SimulationError { SimulationError::new(error.to_string()) }
impl Catalog {
    pub fn open(path:&Path)->Result<Self,SimulationError> {
        let connection=Connection::open(path).map_err(sql)?;
        connection.busy_timeout(std::time::Duration::from_secs(5)).map_err(sql)?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS metadata (key TEXT PRIMARY KEY,value TEXT NOT NULL);
            INSERT OR IGNORE INTO metadata VALUES ('schema','1');
            CREATE TABLE IF NOT EXISTS runs (id INTEGER PRIMARY KEY,label TEXT NOT NULL,law TEXT NOT NULL,manifest TEXT NOT NULL,status TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS checkpoints (run INTEGER NOT NULL,generation TEXT NOT NULL,digest TEXT NOT NULL,path TEXT NOT NULL,bytes INTEGER NOT NULL,PRIMARY KEY(run,generation));
            CREATE TABLE IF NOT EXISTS samples (run INTEGER NOT NULL,generation TEXT NOT NULL,metrics TEXT NOT NULL,PRIMARY KEY(run,generation));
            CREATE TABLE IF NOT EXISTS event_batches (run INTEGER NOT NULL,generation TEXT NOT NULL,path TEXT NOT NULL,digest TEXT NOT NULL,retained INTEGER NOT NULL,truncated INTEGER NOT NULL,PRIMARY KEY(run,generation));
        ").map_err(sql)?;
        let schema:String=connection.query_row("SELECT value FROM metadata WHERE key='schema'",[],|row|row.get(0)).map_err(sql)?;
        if schema!="1" { return Err(SimulationError::new("unsupported experiment catalog schema")); }
        Ok(Self {connection})
    }
    pub fn begin(&self,label:&str,law:&str,manifest:&Value)->Result<i64,SimulationError> {
        self.connection.execute("INSERT INTO runs(label,law,manifest,status) VALUES (?1,?2,?3,'running')",params![label,law,manifest.to_string()]).map_err(sql)?;
        Ok(self.connection.last_insert_rowid())
    }
    pub fn checkpoint(&self,run:i64,generation:u64,digest:&str,path:&str,bytes:u64)->Result<(),SimulationError> {
        let bytes=i64::try_from(bytes).map_err(|_|SimulationError::new("catalog byte count out of range"))?;
        self.connection.execute("INSERT OR IGNORE INTO checkpoints VALUES (?1,?2,?3,?4,?5)",params![run,generation.to_string(),digest,path,bytes]).map_err(sql)?; Ok(())
    }
    pub fn sample(&self,run:i64,generation:u64,metrics:&Value)->Result<(),SimulationError> {
        self.connection.execute("INSERT OR REPLACE INTO samples VALUES (?1,?2,?3)",params![run,generation.to_string(),metrics.to_string()]).map_err(sql)?; Ok(())
    }
    pub fn events(&self,run:i64,generation:u64,path:&str,digest:&str,retained:usize,truncated:bool)->Result<(),SimulationError> {
        self.connection.execute("INSERT INTO event_batches VALUES (?1,?2,?3,?4,?5,?6)",params![run,generation.to_string(),path,digest,retained as i64,i64::from(truncated)]).map_err(sql)?; Ok(())
    }
    pub fn finish(&self,run:i64,completed:bool)->Result<(),SimulationError> {
        self.connection.execute("UPDATE runs SET status=?1 WHERE id=?2",params![if completed {"completed"} else {"failed"},run]).map_err(sql)?; Ok(())
    }
    pub fn list(&self)->Result<Value,SimulationError> {
        let mut statement=self.connection.prepare("SELECT id,label,law,status,manifest FROM runs ORDER BY id").map_err(sql)?;
        let rows=statement.query_map([],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?))).map_err(sql)?;
        let mut result=Vec::new();
        for row in rows { let (id,label,law,status,manifest)=row.map_err(sql)?;
            result.push(json!({"id":id,"label":label,"law":law,"status":status,"manifest":serde_json::from_str::<Value>(&manifest).map_err(crate::persistence::json_error)?})); }
        Ok(json!(result))
    }
}
