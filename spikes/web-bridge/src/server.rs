//! Serveur HTTP minimal : la même base SQLite que le desktop, servie en JSON.
//! Valide l'architecture « un seul cerveau » (PLAN.md §3) — le navigateur ne
//! parle jamais IMAP, il parle au moteur. `tiny_http` séquentiel suffit au
//! spike ; la production (Phase 4) sera asynchrone (axum) et authentifiée.

use std::time::Instant;

use serde_json::json;
use tiny_http::{Header, Method, Response, Server};

use crate::db::Store;
use crate::gmail::{self, ImapSession};
use crate::sync;

const PAGE: &str = include_str!("page.html");
const LIST_LIMIT: usize = 50;
const JSON_TYPE: &str = "application/json; charset=utf-8";

pub fn serve(mut store: Store, mut session: Option<ImapSession>, port: u16) -> anyhow::Result<()> {
    let server = Server::http(("127.0.0.1", port))
        .map_err(|err| anyhow::anyhow!("démarrage du serveur sur le port {port} : {err}"))?;
    println!("\nPont web prêt : http://127.0.0.1:{port} (Ctrl+C pour arrêter)");

    for request in server.incoming_requests() {
        let timer = Instant::now();
        let method = request.method().clone();
        let url = request.url().to_string();
        let (status, content_type, body) = match handle(&mut store, &mut session, &method, &url) {
            Ok(reply) => reply,
            Err(err) => (
                500,
                JSON_TYPE,
                json!({ "error": err.to_string() }).to_string(),
            ),
        };
        println!("  {method} {url} → {status} en {:?}", timer.elapsed());
        let Ok(header) = Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()) else {
            continue;
        };
        let response = Response::from_string(body)
            .with_status_code(status)
            .with_header(header);
        if let Err(err) = request.respond(response) {
            eprintln!("  réponse interrompue : {err}");
        }
    }
    Ok(())
}

fn handle(
    store: &mut Store,
    session: &mut Option<ImapSession>,
    method: &Method,
    url: &str,
) -> anyhow::Result<(u16, &'static str, String)> {
    match (method, url) {
        (Method::Get, "/") => Ok((200, "text/html; charset=utf-8", PAGE.to_string())),
        (Method::Get, "/api/messages") => list_messages(store),
        (Method::Post, "/api/sync") => run_sync(store, session),
        _ => Ok((
            404,
            JSON_TYPE,
            json!({ "error": "route inconnue" }).to_string(),
        )),
    }
}

fn list_messages(store: &mut Store) -> anyhow::Result<(u16, &'static str, String)> {
    let timer = Instant::now();
    let rows = store.recent(LIST_LIMIT)?;
    let total = store.count()?;
    let messages: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            json!({
                "uid": row.uid,
                "subject": row.subject,
                "sender": row.sender,
                "date": row.date,
            })
        })
        .collect();
    let body = json!({
        "messages": messages,
        "total": total,
        "server_elapsed_us": timer.elapsed().as_micros() as u64,
    });
    Ok((200, JSON_TYPE, body.to_string()))
}

fn run_sync(
    store: &mut Store,
    session: &mut Option<ImapSession>,
) -> anyhow::Result<(u16, &'static str, String)> {
    let Some(active) = session.as_mut() else {
        let body =
            json!({ "mode": "hors-ligne", "fetched": 0, "deleted": 0, "server_elapsed_ms": 0 });
        return Ok((200, JSON_TYPE, body.to_string()));
    };
    let timer = Instant::now();
    let report = match sync::sync_inbox(active, store) {
        Ok(report) => report,
        Err(err) => {
            // Les sessions IMAP expirent (~quelques minutes d'inactivité chez
            // Gmail) : une reconnexion silencieuse fait partie du contrat.
            println!("  session IMAP perdue ({err}) : reconnexion…");
            let (fresh, _) = gmail::connect()?;
            *active = fresh;
            sync::sync_inbox(active, store)?
        }
    };
    let body = json!({
        "mode": report.mode,
        "fetched": report.fetched,
        "deleted": report.deleted,
        "total_on_server": report.total_on_server,
        "server_elapsed_ms": timer.elapsed().as_millis() as u64,
    });
    Ok((200, JSON_TYPE, body.to_string()))
}
