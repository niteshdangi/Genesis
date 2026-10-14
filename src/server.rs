use std::{io::Read, time::{Duration, Instant,SystemTime,UNIX_EPOCH},path::PathBuf,thread::JoinHandle};
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};
use crate::{law_system::SimulationError, persistence::{Checkpoint, MAX_DOCUMENT_BYTES, io_error}, universe::{Universe, UniverseSpec}};

const PAGE: &str = include_str!("../web/index.html");

fn respond(request: Request, status: u16, body: String, content_type: &str) {
    let mut response = Response::from_string(body).with_status_code(StatusCode(status));
    for (name, value) in [("Content-Type",content_type),("Cache-Control","no-store"),("X-Content-Type-Options","nosniff"),
        ("Content-Security-Policy","default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'")] {
        if let Ok(header) = Header::from_bytes(name, value) { response = response.with_header(header); }
    }
    if let Err(error) = request.respond(response) { eprintln!("response failed: {error}"); }
}
fn reject(request: Request, status: u16, message: &str) { respond(request,status,json!({"error":message}).to_string(),"application/json"); }

fn respond_binary(request:Request,bytes:Vec<u8>,content_type:&str) {
    let mut response=Response::from_data(bytes);
    for (name,value) in [("Content-Type",content_type),("Cache-Control","no-store"),("X-Content-Type-Options","nosniff")] {
        if let Ok(header)=Header::from_bytes(name,value) {response=response.with_header(header);}
    }
    if let Err(error)=request.respond(response){eprintln!("checkpoint response failed: {error}");}
}
struct Recording { path:PathBuf,handle:JoinHandle<Result<crate::experiment::RunOutcome,SimulationError>> }
fn status_snapshot(universe:&Universe,running:bool,error:Option<&str>,recording:&Option<Recording>,last:&Value)->Value {
    let mut snapshot=universe.snapshot(running,error);
    snapshot["recording"]=if let Some(recording)=recording {json!({"status":"running","path":recording.path.to_string_lossy()})} else {last.clone()};
    snapshot
}
fn experiment_list()->Result<Value,SimulationError> {
    let root=PathBuf::from("runs");if !root.exists(){return Ok(json!([]));}
    let mut entries=Vec::new();
    for entry in std::fs::read_dir(root).map_err(io_error)? {
        let entry=entry.map_err(io_error)?;if !entry.file_type().map_err(io_error)?.is_dir(){continue;}
        let path=entry.path();if !path.join("manifest.json").is_file(){continue;}
        let manifest:crate::experiment::RunManifest=crate::persistence::read_json(&path.join("manifest.json"))?;
        let outcome=if path.join("outcome.json").is_file(){Some(crate::persistence::read_json::<crate::experiment::RunOutcome>(&path.join("outcome.json"))?)}else{None};
        entries.push(json!({"path":path.to_string_lossy(),"label":manifest.options.label,"law":manifest.spec.id(),"outcome":outcome}));
    }
    entries.sort_by_key(|entry|entry["path"].as_str().unwrap_or_default().to_owned());Ok(json!(entries))
}

fn read_body(request: &mut Request) -> Result<Value, SimulationError> {
    let mut bytes = Vec::new();
    request.as_reader().take(MAX_DOCUMENT_BYTES + 1).read_to_end(&mut bytes).map_err(io_error)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES { return Err(SimulationError::new("request exceeds 64 MiB")); }
    if bytes.is_empty() { return Ok(Value::Null); }
    serde_json::from_slice(&bytes).map_err(crate::persistence::json_error)
}

pub fn serve(mut universe: Universe, port: u16, tick_ms: u64) -> Result<(), SimulationError> {
    let address = format!("127.0.0.1:{port}");
    let origin = format!("http://{address}");
    let server = Server::http(address.as_str()).map_err(|e| SimulationError::new(e.to_string()))?;
    let interval = Duration::from_millis(tick_ms);
    let mut initial = universe.fork_current()?;
    let mut running = false;
    let mut simulation_error: Option<String> = None;
    let mut last_tick = Instant::now();
    let mut recording:Option<Recording>=None;
    let mut last_recording=Value::Null;
    println!("Genesis: {origin}. Starts paused. Ctrl+C stops the server.");
    loop {
        if recording.as_ref().is_some_and(|job|job.handle.is_finished()) {
            if let Some(job)=recording.take() {
                last_recording=match job.handle.join() {
                    Ok(Ok(outcome))=>json!({"status":if outcome.completed {"completed"} else {"failed"},"path":job.path.to_string_lossy(),"outcome":outcome}),
                    Ok(Err(error))=>json!({"status":"failed","path":job.path.to_string_lossy(),"error":error.to_string()}),
                    Err(_)=>json!({"status":"failed","path":job.path.to_string_lossy(),"error":"experiment worker panicked; inspect retained artifacts"}),
                };
            }
        }
        if running && last_tick.elapsed() >= interval {
            if let Err(error) = universe.step() { simulation_error = Some(error.to_string()); running = false; }
            last_tick = Instant::now();
        }
        let timeout = if running { interval.saturating_sub(last_tick.elapsed()) } else { Duration::from_millis(100) };
        let Some(mut request) = server.recv_timeout(timeout).map_err(io_error)? else { continue; };
        if request.headers().iter().find(|h| h.field.equiv("Host")).map(|h| h.value.as_str()) != Some(address.as_str()) {
            reject(request,403,"use the printed loopback address"); continue;
        }
        let requested=request.url().to_owned();
        let (route,query)=requested.split_once('?').unwrap_or((&requested,""));
        let path=route.to_owned();
        if request.method() == &Method::Get {
            match path.as_str() {
                "/" => respond(request,200,PAGE.into(),"text/html; charset=utf-8"),
                "/state" => {
                    let snapshot=if query.is_empty(){Ok(status_snapshot(&universe,running,simulation_error.as_deref(),&recording,&last_recording))}
                        else if let Some(value)=query.strip_prefix("z=") {match value.parse::<u32>() {
                            Ok(z)=>universe.snapshot_slice(z,running,simulation_error.as_deref()).map(|mut snapshot|{
                                snapshot["recording"]=if let Some(job)=&recording {json!({"status":"running","path":job.path.to_string_lossy()})} else {last_recording.clone()};snapshot}),
                            Err(_)=>Err(SimulationError::new("invalid slice index")),
                        }}else{Err(SimulationError::new("unsupported state query"))};
                    match snapshot {Ok(snapshot)=>respond(request,200,snapshot.to_string(),"application/json"),Err(error)=>reject(request,400,&error.to_string())}
                }
                "/spec" => match serde_json::to_string(universe.spec()) {
                    Ok(spec) => respond(request,200,spec,"application/json"), Err(e) => reject(request,500,&e.to_string()),
                },
                "/checkpoint" => match universe.checkpoint_download() {
                    Ok((bytes,content_type)) => respond_binary(request,bytes,content_type), Err(e) => reject(request,500,&e.to_string()),
                },
                "/experiments"=>match experiment_list(){Ok(list)=>respond(request,200,list.to_string(),"application/json"),Err(error)=>reject(request,500,&error.to_string())},
                _ => reject(request,404,"unknown route"),
            }
            continue;
        }
        if request.method() != &Method::Post {
            reject(request,405,"unsupported method"); continue;
        }
        let json_request = request.headers().iter().any(|h| h.field.equiv("Content-Type") && h.value.as_str() == "application/json");
        let same_origin = request.headers().iter().all(|h| !h.field.equiv("Origin") || h.value.as_str() == origin.as_str());
        if path=="/restore-binary" {
            let binary=request.headers().iter().any(|h|h.field.equiv("Content-Type") && h.value.as_str()=="application/octet-stream");
            if !binary || !same_origin {reject(request,403,"binary restore requires same-origin binary input");continue;}
            if running {reject(request,409,"pause before restoring");continue;}
            let mut bytes=Vec::new();
            let result=request.as_reader().take(crate::storage::MAX_BINARY_BYTES+1).read_to_end(&mut bytes).map_err(io_error)
                .and_then(|_|if bytes.len() as u64>crate::storage::MAX_BINARY_BYTES {Err(SimulationError::new("checkpoint bundle exceeds budget"))}else{Universe::restore_bundle(&bytes)});
            match result {
                Ok(replacement)=>match replacement.fork_current(){Ok(reset)=>{
                    universe=replacement;initial=reset;simulation_error=None;
                    respond(request,200,status_snapshot(&universe,false,None,&recording,&last_recording).to_string(),"application/json");
                },Err(error)=>reject(request,400,&error.to_string())},
                Err(error)=>reject(request,400,&error.to_string()),
            }
            continue;
        }
        if !json_request || !same_origin { reject(request,403,"controls require same-origin JSON requests"); continue; }
        if !["/play","/pause","/step","/batch","/reset","/rule","/configure","/fork","/restore","/record"].contains(&path.as_str()) {
            reject(request,404,"unknown route"); continue;
        }
        if running && ["/step","/batch","/restore","/configure","/fork","/record"].contains(&path.as_str()) {
            reject(request,409,"pause before stepping, restoring or configuring"); continue;
        }
        if simulation_error.is_some() && ["/play","/step","/batch"].contains(&path.as_str()) {
            reject(request,409,"restore, reset or configure after a simulation failure"); continue;
        }
        let body = match read_body(&mut request) { Ok(body) => body, Err(error) => { reject(request,400,&error.to_string()); continue; } };
        let action = (|| -> Result<(), SimulationError> {
            match path.as_str() {
                "/play" => { running = true; last_tick = Instant::now(); }
                "/pause" => running = false,
                "/step" | "/batch" => {
                    let steps = if path == "/step" { 1 } else { body["steps"].as_u64().filter(|&n| n > 0 && n <= 1_000)
                        .ok_or_else(|| SimulationError::new("batch needs 1 to 1000 steps"))? };
                    for _ in 0..steps {
                        if let Err(error) = universe.step() { simulation_error = Some(error.to_string()); return Err(error); }
                    }
                }
                "/reset" => { universe = initial.fork_current()?; simulation_error = None; running = false; }
                "/rule" => {
                    let id = body["rule"].as_str().ok_or_else(|| SimulationError::new("missing rule"))?;
                    let replacement = Universe::new(id)?;
                    let checkpoint = replacement.fork_current()?;
                    universe = replacement; initial = checkpoint; simulation_error = None; running = false;
                }
                "/configure" | "/fork" => {
                    let spec: UniverseSpec = serde_json::from_value(body).map_err(crate::persistence::json_error)?;
                    let replacement = if path == "/fork" { universe.fork_with(spec)? } else { Universe::from_spec(spec)? };
                    let checkpoint = replacement.fork_current()?;
                    universe = replacement; initial = checkpoint; simulation_error = None; running = false;
                }
                "/restore" => {
                    let checkpoint: Checkpoint = serde_json::from_value(body).map_err(crate::persistence::json_error)?;
                    let replacement = Universe::restore(checkpoint)?;
                    initial = replacement.fork_current()?;universe = replacement; simulation_error = None; running = false;
                }
                "/record"=>{
                    if recording.is_some(){return Err(SimulationError::new("an experiment is already recording"));}
                    let steps=body["steps"].as_u64().filter(|&n|n>0 && n<=1_000_000).ok_or_else(||SimulationError::new("recording needs 1..1000000 generations"))?;
                    if simulation_error.is_some(){return Err(SimulationError::new("restore or reset the failed world before recording"));}
                    let candidate=universe.fork_current()?;
                    let stamp=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|SimulationError::new(e.to_string()))?.as_nanos();
                    let path=PathBuf::from("runs").join(format!("run-{stamp}"));let destination=path.clone();
                    let options=crate::experiment::RunOptions {steps,label:format!("inspector-{stamp}"),..Default::default()};
                    let handle=std::thread::Builder::new().name("genesis-experiment".into()).spawn(move||crate::experiment::run(candidate,&destination,options)).map_err(io_error)?;
                    recording=Some(Recording {path,handle});last_recording=Value::Null;
                }
                _ => return Err(SimulationError::new("unknown action")),
            }
            Ok(())
        })();
        match action {
            Ok(()) => respond(request,200,status_snapshot(&universe,running,simulation_error.as_deref(),&recording,&last_recording).to_string(),"application/json"),
            Err(error) => reject(request,if simulation_error.is_some() {500} else {400},&error.to_string()),
        }
    }
}
